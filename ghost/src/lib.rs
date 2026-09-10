//! Hedged posting-reality analyzer.
//!
//! Emits one claim per criterion that *appears* to fire, plus defeaters that
//! take them back. The kernel decides the level; this crate never names one.
//!
//! Two tiers:
//!
//! - **Structural** — `evergreen_posting`, `source_absent`, `repost_reset`.
//!   Shape of the posting.
//! - **Outcome** — `silent_applications`, `user_reports`. What happened to
//!   people. The only tier that observes reality.
//!
//! Witness identity:
//!
//! - If ingest supplied `outcome_witness_ids`, the distinct count is exact.
//! - Otherwise the kernel uses an inclusion interval. `likely` reads the
//!   **lower** bound (`max` of the two channel counters), so one applicant
//!   plus one reporter who might be the same person cannot mark `likely`.

use jm_common::{GhostCriterion, ListingRecord};
use jm_kernel::{
    AnalyzerId, Bundle, Claim, ClaimKind, DefeatReason, Defeater, EvidenceTier, FactId, FactKind,
    FactSet,
};
use jm_params::Params;

pub fn analyze(facts: &FactSet, params: &Params) -> Bundle {
    let mut bundle = Bundle::default();

    maybe_evergreen(facts, params, &mut bundle);
    maybe_absent(facts, params, &mut bundle);
    maybe_repost(facts, &mut bundle);
    maybe_outcome(
        GhostCriterion::SilentApplications,
        facts.pick(|k| match k {
            FactKind::SilentApplicants { n } => Some(*n),
            _ => None,
        }),
        &mut bundle,
    );
    maybe_outcome(
        GhostCriterion::UserReports,
        facts.pick(|k| match k {
            FactKind::ReportContributors { n } => Some(*n),
            _ => None,
        }),
        &mut bundle,
    );

    // A closed requisition opposes the conclusion: there is nobody left to
    // warn, so the head is rebutted rather than undercut.
    if let Some(closed) = facts.cite(|k| matches!(k, FactKind::Closed)) {
        let targets: Vec<_> = bundle.claims.iter().map(|c| c.id.clone()).collect();
        for target in targets {
            bundle.defeaters.push(Defeater::rebut(
                target,
                DefeatReason::ClosedRequisition,
                format!("a closed job carries no ghost signal ({})", closed.0),
            ));
        }
    }

    bundle
}

/// Two triggers, two gates: an ingest hint stands alone only at
/// `ghost_evergreen_age_days`, always-hiring language at that age or on a
/// date reset. Whichever appears, the claim is filed and then undercut
/// unless at least one trigger cleared its own gate.
fn maybe_evergreen(facts: &FactSet, params: &Params, bundle: &mut Bundle) {
    let (age, age_fact) = age_days(facts);
    let hint = facts.cite(|k| matches!(k, FactKind::EvergreenHint));
    let language = facts.cite(|k| matches!(k, FactKind::AlwaysHiringLanguage));
    let reset = facts.cite(|k| matches!(k, FactKind::DateResetHint));
    if hint.is_none() && language.is_none() {
        return;
    }

    let old_enough = age >= params.ghost_evergreen_age_days;
    let hint_stands = hint.is_some() && old_enough;
    let language_stands = language.is_some() && (old_enough || reset.is_some());

    let premises = [hint.clone(), language.clone(), reset.clone(), age_fact]
        .into_iter()
        .flatten()
        .collect();
    let claim = structural(
        GhostCriterion::EvergreenPosting,
        "evergreen shape",
        premises,
    );
    if !(hint_stands || language_stands) {
        bundle.defeaters.push(Defeater::undercut(
            claim.id.clone(),
            DefeatReason::YoungForEvergreen,
            format!(
                "age {age}d < evergreen floor {}d and no date reset",
                params.ghost_evergreen_age_days
            ),
        ));
    }
    bundle.claims.push(claim);
}

fn maybe_absent(facts: &FactSet, params: &Params, bundle: &mut Bundle) {
    let Some(hint) = facts.cite(|k| matches!(k, FactKind::SourceAbsentHint)) else {
        return;
    };
    let coverage = facts.cite(|k| matches!(k, FactKind::NoCoverage));
    let stamp = facts.pick(|k| match k {
        FactKind::AbsenceStampAgeDays { n } => Some(*n),
        _ => None,
    });

    let premises = [
        Some(hint),
        coverage.clone(),
        stamp.clone().map(|(id, _)| id),
    ]
    .into_iter()
    .flatten()
    .collect();
    let claim = structural(
        GhostCriterion::SourceAbsent,
        "source absent stamp",
        premises,
    );

    if let Some(no_coverage) = coverage {
        bundle.defeaters.push(Defeater::undercut(
            claim.id.clone(),
            DefeatReason::NoCoverage,
            format!(
                "empty crawl is no coverage, not absence ({})",
                no_coverage.0
            ),
        ));
    }
    if let Some((_, age)) = stamp {
        if age > params.ghost_absence_stamp_max_age_days {
            bundle.defeaters.push(Defeater::undercut(
                claim.id.clone(),
                DefeatReason::StaleAbsenceStamp,
                format!("stamp age {age}d"),
            ));
        }
    }
    bundle.claims.push(claim);
}

fn maybe_repost(facts: &FactSet, bundle: &mut Bundle) {
    if let Some(reset) = facts.cite(|k| matches!(k, FactKind::DateResetHint)) {
        bundle.claims.push(structural(
            GhostCriterion::RepostReset,
            "date reset",
            vec![reset],
        ));
    }
}

fn maybe_outcome(criterion: GhostCriterion, channel: Option<(FactId, u32)>, bundle: &mut Bundle) {
    debug_assert!(
        !criterion.is_structural(),
        "{criterion:?} is structural and cannot be filed at the outcome tier"
    );
    let Some((fact, n)) = channel else { return };
    if n == 0 {
        return;
    }
    bundle.claims.push(Claim::new(
        AnalyzerId::Ghost,
        criterion.as_str(),
        EvidenceTier::Outcome,
        0.8,
        ClaimKind::Ghost { criterion },
        vec![fact],
        format!("{criterion:?} n={n}"),
    ));
}

fn structural(criterion: GhostCriterion, note: &str, premises: Vec<FactId>) -> Claim {
    debug_assert!(criterion.is_structural(), "{criterion:?} is not structural");
    Claim::new(
        AnalyzerId::Ghost,
        criterion.as_str(),
        EvidenceTier::Structural,
        0.55,
        ClaimKind::Ghost { criterion },
        premises,
        note,
    )
}

/// Prefer the age derived from stamps, then the ingest figure. No age fact
/// at all reads as young.
fn age_days(facts: &FactSet) -> (u32, Option<FactId>) {
    facts
        .pick(|k| match k {
            FactKind::DerivedAgeDays { n } => Some(*n),
            _ => None,
        })
        .or_else(|| {
            facts.pick(|k| match k {
                FactKind::PostingAgeDays { n } => Some(*n),
                _ => None,
            })
        })
        .map_or((0, None), |(id, n)| (n, Some(id)))
}

/// Serving projection: withhold the witness count below the gate so a
/// single applicant is not deanonymized to the employer.
pub fn serve_witness_count(listing: &ListingRecord, params: &Params) -> Option<u32> {
    let n = if listing.outcome_witness_ids.is_empty() {
        listing
            .silent_application_contributors
            .max(listing.report_contributors)
    } else {
        let mut ids = listing.outcome_witness_ids.clone();
        ids.sort();
        ids.dedup();
        ids.len() as u32
    };
    if n >= params.ghost_contributor_gate {
        Some(n)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use jm_common::{GhostLevel, ListingRecord};
    use jm_extract::{canonicalize, extract};
    use jm_kernel::judge;

    fn as_of() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 8, 26, 0, 0, 0).unwrap()
    }

    fn run(rec: ListingRecord) -> jm_kernel::Judgement {
        let facts = extract(&canonicalize(&rec, as_of()));
        judge(
            &facts,
            analyze(&facts, &Params::default()),
            &Params::default(),
        )
    }

    /// Old enough to clear the evergreen gate on age alone.
    fn old() -> ListingRecord {
        ListingRecord {
            posting_age_days: 400,
            first_seen: "2025-01-01".into(),
            ..ListingRecord::default()
        }
    }

    #[test]
    fn closed_job_is_quiet() {
        let rec = ListingRecord {
            closed: true,
            evergreen: true,
            source_absent: true,
            ..old()
        };
        assert_eq!(run(rec).ghost.level, GhostLevel::None);
    }

    #[test]
    fn structure_alone_never_likely() {
        let rec = ListingRecord {
            evergreen: true,
            source_absent: true,
            has_source_coverage: true,
            source_absent_stamp_age_days: Some(1),
            ..old()
        };
        let j = run(rec);
        assert_eq!(j.ghost.level, GhostLevel::Possible);
        assert!(j.ghost.fired.iter().all(|c| c.is_structural()));
    }

    #[test]
    fn two_outcome_witnesses_and_convergence_is_likely() {
        let rec = ListingRecord {
            silent_application_contributors: 2,
            report_contributors: 2,
            evergreen: true,
            ..old()
        };
        assert_eq!(run(rec).ghost.level, GhostLevel::Likely);
    }

    #[test]
    fn one_witness_cannot_mark_likely() {
        let rec = ListingRecord {
            silent_application_contributors: 1,
            evergreen: true,
            source_absent: true,
            has_source_coverage: true,
            source_absent_stamp_age_days: Some(1),
            ..old()
        };
        let j = run(rec);
        assert_eq!(j.ghost.level, GhostLevel::Possible);
        assert!(serve_witness_count(
            &ListingRecord {
                silent_application_contributors: 1,
                ..ListingRecord::default()
            },
            &Params::default()
        )
        .is_none());
        assert_eq!(j.ghost.witness_lower_bound, 1);
    }

    #[test]
    fn empty_coverage_is_not_absent() {
        let rec = ListingRecord {
            source_absent: true,
            has_source_coverage: false,
            evergreen: true,
            ..old()
        };
        let j = run(rec);
        assert!(!j.ghost.fired.contains(&GhostCriterion::SourceAbsent));
    }

    #[test]
    fn stale_absence_stamp_is_ignored() {
        let rec = ListingRecord {
            source_absent: true,
            has_source_coverage: true,
            source_absent_stamp_age_days: Some(30),
            evergreen: true,
            ..old()
        };
        let j = run(rec);
        assert!(!j.ghost.fired.contains(&GhostCriterion::SourceAbsent));
    }

    #[test]
    fn outcome_alone_with_gate_is_possible() {
        let rec = ListingRecord {
            silent_application_contributors: 2,
            ..ListingRecord::default()
        };
        let j = run(rec.clone());
        assert_eq!(j.ghost.level, GhostLevel::Possible);
        assert_eq!(serve_witness_count(&rec, &Params::default()), Some(2));
    }

    #[test]
    fn witness_ids_are_exact() {
        let rec = ListingRecord {
            silent_application_contributors: 4,
            report_contributors: 3,
            outcome_witness_ids: vec!["a".into(), "b".into(), "a".into()],
            evergreen: true,
            ..old()
        };
        let j = run(rec);
        assert_eq!(j.ghost.witness_lower_bound, 2);
        assert_eq!(j.ghost.witness_upper_bound, 2);
        assert_eq!(j.ghost.level, GhostLevel::Likely);
    }

    fn evergreen_fires(hint: bool, language: bool, reset: bool, old_enough: bool) -> bool {
        let rec = ListingRecord {
            evergreen: hint,
            description: if language {
                "We are always hiring engineers.".into()
            } else {
                "Build the product.".into()
            },
            date_reset_detected: reset,
            first_seen: if old_enough {
                "2025-01-01".into()
            } else {
                "2026-08-20".into()
            },
            posting_age_days: if old_enough { 400 } else { 6 },
            ..ListingRecord::default()
        };
        run(rec)
            .ghost
            .fired
            .contains(&GhostCriterion::EvergreenPosting)
    }

    #[test]
    fn evergreen_gate_truth_table() {
        // hint alone needs age
        assert!(!evergreen_fires(true, false, false, false));
        assert!(evergreen_fires(true, false, false, true));
        // language alone needs age or a date reset
        assert!(!evergreen_fires(false, true, false, false));
        assert!(evergreen_fires(false, true, true, false));
        assert!(evergreen_fires(false, true, false, true));
        // language that cleared nothing must not carry a young hint
        assert!(!evergreen_fires(true, true, false, false));
        // but either one clearing its own gate is enough
        assert!(evergreen_fires(true, true, true, false));
        assert!(evergreen_fires(true, true, false, true));
        // neither trigger present
        assert!(!evergreen_fires(false, false, true, true));
    }

    #[test]
    fn a_young_hint_is_recorded_and_then_defeated() {
        let rec = ListingRecord {
            evergreen: true,
            first_seen: "2026-08-20".into(),
            posting_age_days: 6,
            ..ListingRecord::default()
        };
        let j = run(rec);
        assert_eq!(j.issued, 1);
        assert_eq!(j.surviving, 0);
        assert!(j
            .defeated_notes
            .iter()
            .any(|n| n.contains("undercut") && n.contains("young_for_evergreen")));
    }

    #[test]
    fn every_claim_cites_a_fact() {
        let rec = ListingRecord {
            evergreen: true,
            source_absent: true,
            has_source_coverage: true,
            source_absent_stamp_age_days: Some(1),
            date_reset_detected: true,
            silent_application_contributors: 2,
            report_contributors: 2,
            ..old()
        };
        let j = run(rec);
        assert_eq!(j.surviving_claims.len(), 5);
        for claim in &j.surviving_claims {
            assert!(!claim.premises.is_empty(), "{} cites no fact", claim.id.0);
        }
    }

    #[test]
    fn tiers_follow_the_published_vocabulary() {
        // The judge's structural cap reads `Claim.tier`.
        for criterion in GhostCriterion::ALL {
            let expected = if criterion.is_structural() {
                EvidenceTier::Structural
            } else {
                EvidenceTier::Outcome
            };
            let rec = ListingRecord {
                evergreen: criterion == GhostCriterion::EvergreenPosting,
                source_absent: criterion == GhostCriterion::SourceAbsent,
                has_source_coverage: true,
                source_absent_stamp_age_days: Some(1),
                date_reset_detected: criterion == GhostCriterion::RepostReset,
                silent_application_contributors: u32::from(
                    criterion == GhostCriterion::SilentApplications,
                ),
                report_contributors: u32::from(criterion == GhostCriterion::UserReports),
                ..old()
            };
            let j = run(rec);
            let filed = j
                .surviving_claims
                .iter()
                .find(|c| c.id.0.ends_with(criterion.as_str()))
                .unwrap_or_else(|| panic!("{criterion:?} did not fire"));
            assert_eq!(filed.tier, expected, "{criterion:?}");
        }
    }
}

use super::claim::{Claim, ClaimHead, ClaimId, ClaimKind};
use super::defeat::{DefeatKind, Defeater};
use super::fact::{FactKind, FactSet};
use super::lattice::Obligation;
use jm_common::{
    Disposition, FraudHead, FraudScores, GhostCriterion, GhostLevel, GhostVerdict, SourceKind,
    SourceLiveness, SourceResolution,
};
use jm_params::Params;
use std::collections::{BTreeSet, HashSet};

/// Float-equality slack for "the same confidence", not a policy band.
const SOURCE_TIE_TOLERANCE: f64 = 1e-9;

/// Analyzer output. Order of `claims` is the analyzer's reporting order;
/// the judge never depends on it.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    pub claims: Vec<Claim>,
    pub defeaters: Vec<Defeater>,
}

impl Bundle {
    pub fn merge(mut self, other: Bundle) -> Self {
        self.claims.extend(other.claims);
        self.defeaters.extend(other.defeaters);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Judgement {
    pub source: SourceResolution,
    pub ghost: GhostVerdict,
    pub fraud: FraudScores,
    pub liveness: SourceLiveness,
    pub obligation: Obligation,
    pub issued: u32,
    pub surviving: u32,
    pub surviving_claims: Vec<Claim>,
    /// Heads whose conclusion was rebutted, and so may not be re-established.
    pub rebutted: BTreeSet<ClaimHead>,
    pub defeated_notes: Vec<String>,
}

/// Least-commitment aggregation. Deterministic and order-independent: every
/// fold reads the multiset of surviving claims, not a rule list.
pub fn judge(facts: &FactSet, bundle: Bundle, params: &Params) -> Judgement {
    let Bundle { claims, defeaters } = bundle;
    let issued = claims.len() as u32;
    let Defeated {
        surviving,
        rebutted,
        defeated_notes,
    } = apply_defeaters(claims, &defeaters);

    let source = fold_source(&surviving, &rebutted, params);
    let ghost = fold_ghost(&surviving, &rebutted, facts, params);
    let fraud = fold_fraud(&surviving, &rebutted);
    let liveness = fold_liveness(&surviving, &rebutted);
    let obligation = fold_obligation(&surviving, &rebutted, &ghost, &fraud, liveness, params);

    Judgement {
        source,
        ghost,
        fraud,
        liveness,
        obligation,
        issued,
        surviving: surviving.len() as u32,
        surviving_claims: surviving,
        rebutted,
        defeated_notes,
    }
}

struct Defeated {
    surviving: Vec<Claim>,
    rebutted: BTreeSet<ClaimHead>,
    defeated_notes: Vec<String>,
}

/// Both kinds remove the claim; only a rebuttal bars the head.
fn apply_defeaters(claims: Vec<Claim>, defeaters: &[Defeater]) -> Defeated {
    let mut dead: HashSet<&ClaimId> = HashSet::new();
    let mut rebutted_ids: HashSet<&ClaimId> = HashSet::new();
    let mut defeated_notes = Vec::new();
    for d in defeaters {
        dead.insert(&d.target);
        if d.kind == DefeatKind::Rebut {
            rebutted_ids.insert(&d.target);
        }
        defeated_notes.push(format!(
            "{} {} ({})",
            d.kind_label(),
            d.target.0,
            d.reason.as_str()
        ));
    }
    defeated_notes.sort();
    defeated_notes.dedup();

    // A rebuttal only bars a head that was actually asserted.
    let rebutted: BTreeSet<ClaimHead> = claims
        .iter()
        .filter(|c| rebutted_ids.contains(&c.id))
        .map(|c| c.kind.head())
        .collect();

    let mut surviving: Vec<Claim> = claims
        .into_iter()
        .filter(|c| !dead.contains(&c.id))
        .collect();
    surviving.sort_by(|a, b| a.id.cmp(&b.id));

    Defeated {
        surviving,
        rebutted,
        defeated_notes,
    }
}

fn barred(rebutted: &BTreeSet<ClaimHead>, head: ClaimHead) -> bool {
    rebutted.contains(&head)
}

/// Source resolution is agreement, not election: the highest confidence that
/// cleared the floor, and `unknown` if that band disagrees on kind. An
/// `unknown` from a conflict still publishes the signals that conflicted.
fn fold_source(
    claims: &[Claim],
    rebutted: &BTreeSet<ClaimHead>,
    params: &Params,
) -> SourceResolution {
    if barred(rebutted, ClaimHead::Source) {
        return unresolved_source(Vec::new());
    }

    let eligible: Vec<(f64, SourceKind, &str)> = claims
        .iter()
        .filter_map(|c| match c.kind {
            ClaimKind::Source {
                source_kind,
                confidence,
            } if source_kind != SourceKind::Unknown
                && confidence >= params.source_confidence_floor =>
            {
                Some((confidence, source_kind, c.note.as_str()))
            }
            _ => None,
        })
        .collect();

    let Some(top) = eligible.iter().map(|(c, ..)| *c).reduce(f64::max) else {
        return unresolved_source(Vec::new());
    };
    let band: Vec<&(f64, SourceKind, &str)> = eligible
        .iter()
        .filter(|(c, ..)| (top - c).abs() <= SOURCE_TIE_TOLERANCE)
        .collect();

    let mut signals: Vec<String> = band.iter().map(|(_, _, n)| (*n).to_string()).collect();
    signals.sort();
    signals.dedup();

    let kind = band[0].1;
    if band.iter().any(|(_, k, _)| *k != kind) {
        return unresolved_source(signals);
    }

    SourceResolution {
        kind,
        origin_name: String::new(),
        confidence: top,
        // Post-condition, not policy: a projected kind must be backed by a
        // surviving claim.
        invented: signals.is_empty(),
        signals,
    }
}

fn unresolved_source(signals: Vec<String>) -> SourceResolution {
    SourceResolution {
        kind: SourceKind::Unknown,
        origin_name: String::new(),
        confidence: 0.0,
        invented: false,
        signals,
    }
}

fn fold_ghost(
    claims: &[Claim],
    rebutted: &BTreeSet<ClaimHead>,
    facts: &FactSet,
    params: &Params,
) -> GhostVerdict {
    let (lower, upper) = witness_interval(facts);
    let quiet = GhostVerdict {
        level: GhostLevel::None,
        fired: Vec::new(),
        criteria_total: GhostCriterion::ALL.len(),
        witness_lower_bound: lower,
        witness_upper_bound: upper,
    };

    if barred(rebutted, ClaimHead::Ghost) {
        return quiet;
    }

    let mut fired = Vec::new();
    let mut outcome_fired = false;
    for c in claims {
        if let ClaimKind::Ghost { criterion } = c.kind {
            if !fired.contains(&criterion) {
                fired.push(criterion);
            }
            if c.tier.observes_outcome() {
                outcome_fired = true;
            }
        }
    }
    fired.sort();

    let n = fired.len() as u32;
    let gate = params.ghost_contributor_gate;
    let convergence = params.ghost_convergence;

    let mut level = if n == 0 {
        GhostLevel::None
    } else if outcome_fired && lower >= gate && n >= convergence {
        GhostLevel::Likely
    } else if (outcome_fired && lower >= gate) || n >= convergence {
        // Outcome-at-gate without convergence, or structure at convergence:
        // both are Possible, for different reasons. Combined so the branches
        // cannot drift apart.
        GhostLevel::Possible
    } else {
        GhostLevel::None
    };

    // Type-level cap: if nothing outcome-tier survived, Likely is unspeakable.
    if !outcome_fired {
        level = level.cap_structural();
    }

    GhostVerdict {
        level,
        fired,
        ..quiet
    }
}

/// Inclusion interval over the two channel counters, or an exact id count.
///
/// - With witness ids: lower = upper = unique(ids).
/// - Without: lower = max(silent, reports) (every overlapping person counted
///   once), upper = silent + reports (disjoint channels).
///
/// `likely` reads the **lower** bound. A single applicant plus a single
/// distinct reporter therefore cannot mark `likely` unless ingest supplied
/// ids. That is the conservative reading of a contract that demands distinct
/// people we can actually name.
fn witness_interval(facts: &FactSet) -> (u32, u32) {
    if let Some((_, n)) = facts.pick(|k| match k {
        FactKind::WitnessIds { n } => Some(*n),
        _ => None,
    }) {
        return (n, n);
    }
    let silent = facts
        .pick(|k| match k {
            FactKind::SilentApplicants { n } => Some(*n),
            _ => None,
        })
        .map_or(0, |(_, n)| n);
    let reports = facts
        .pick(|k| match k {
            FactKind::ReportContributors { n } => Some(*n),
            _ => None,
        })
        .map_or(0, |(_, n)| n);
    (silent.max(reports), silent.saturating_add(reports))
}

fn fold_fraud(claims: &[Claim], rebutted: &BTreeSet<ClaimHead>) -> FraudScores {
    let mut scores = FraudScores::quiet();
    if barred(rebutted, ClaimHead::Fraud) {
        return scores;
    }
    for c in claims {
        if let ClaimKind::Fraud { head, score } = c.kind {
            match head {
                FraudHead::UpfrontFee => scores.upfront_fee = scores.upfront_fee.max(score),
                FraudHead::Impersonation => scores.impersonation = scores.impersonation.max(score),
                FraudHead::IdentityHarvest => {
                    scores.identity_harvest = scores.identity_harvest.max(score)
                }
                FraudHead::PersonalEmailRecruiter => {
                    scores.personal_email_recruiter = scores.personal_email_recruiter.max(score)
                }
            }
        }
    }
    scores.recompute_combined();
    scores
}

fn fold_liveness(claims: &[Claim], rebutted: &BTreeSet<ClaimHead>) -> SourceLiveness {
    // Most conservative surviving liveness. Closed beats uncertain beats
    // not-found beats live. A 200 and a 404 in the same set must not become
    // live. An absent or rebutted head is uncertain, never live.
    if barred(rebutted, ClaimHead::Liveness) {
        return SourceLiveness::StatusUncertain;
    }
    claims
        .iter()
        .filter_map(|c| match c.kind {
            ClaimKind::Liveness { state } => Some(state),
            _ => None,
        })
        .reduce(worse_liveness)
        .unwrap_or(SourceLiveness::StatusUncertain)
}

fn worse_liveness(a: SourceLiveness, b: SourceLiveness) -> SourceLiveness {
    use SourceLiveness::*;
    let rank = |s| match s {
        LiveAtSource => 0,
        SourceNotFound => 1,
        StatusUncertain => 2,
        ClosedAtSource => 3,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// The one implementation of serving policy. Obligations join on
/// `Show ⊑ Disclose ⊑ Restrict ⊑ Drop`, so rule order cannot change the
/// disposition; `by` names the rule that raised the floor.
fn fold_obligation(
    claims: &[Claim],
    rebutted: &BTreeSet<ClaimHead>,
    ghost: &GhostVerdict,
    fraud: &FraudScores,
    liveness: SourceLiveness,
    params: &Params,
) -> Obligation {
    let mut acc = Obligation::show();

    let human_drop = !barred(rebutted, ClaimHead::Drop)
        && claims
            .iter()
            .any(|c| matches!(c.kind, ClaimKind::HumanDrop));
    if human_drop {
        return Obligation {
            disposition: Disposition::Drop,
            by: "HumanConfirmedFraudRule".into(),
        };
    }

    if let Some(threshold) = params.fraud_restrict_threshold() {
        if fraud.max() >= threshold || fraud.combined >= threshold {
            acc = acc.join(Obligation {
                disposition: Disposition::Restrict,
                by: "AutoRestrictFraudRule".into(),
            });
        }
    }

    if matches!(ghost.level, GhostLevel::Possible | GhostLevel::Likely) {
        acc = acc.join(Obligation {
            disposition: Disposition::Disclose,
            by: "GhostDiscloseRule".into(),
        });
    }

    if liveness.is_disclosable_miss() {
        acc = acc.join(Obligation {
            disposition: Disposition::Disclose,
            by: "StaleDiscloseRule".into(),
        });
    }

    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim::{AnalyzerId, Claim, EvidenceTier};
    use crate::defeat::DefeatReason;
    use crate::fact::{Fact, FactId, Provenance};

    fn ghost_claim(crit: GhostCriterion) -> Claim {
        Claim::new(
            AnalyzerId::Ghost,
            crit.as_str(),
            if crit.is_structural() {
                EvidenceTier::Structural
            } else {
                EvidenceTier::Outcome
            },
            0.7,
            ClaimKind::Ghost { criterion: crit },
            vec![FactId::new("test", crit.as_str())],
            crit.as_str(),
        )
    }

    fn source_claim(key: &str, kind: SourceKind, confidence: f64) -> Claim {
        Claim::new(
            AnalyzerId::Source,
            key,
            EvidenceTier::Provenance,
            confidence,
            ClaimKind::Source {
                source_kind: kind,
                confidence,
            },
            vec![FactId::new("test", key)],
            key,
        )
    }

    fn human_drop() -> Claim {
        Claim::new(
            AnalyzerId::Reviewer,
            "human_drop",
            EvidenceTier::Reviewer,
            1.0,
            ClaimKind::HumanDrop,
            vec![FactId::new("flag", "human_fraud")],
            "reviewer",
        )
    }

    fn liveness_claim(state: SourceLiveness) -> Claim {
        Claim::new(
            AnalyzerId::Stale,
            state.as_str(),
            EvidenceTier::Provenance,
            0.9,
            ClaimKind::Liveness { state },
            vec![FactId::new("test", "http")],
            state.as_str(),
        )
    }

    fn fraud_claim(head: FraudHead, score: f64) -> Claim {
        Claim::new(
            AnalyzerId::Fraud,
            head.as_str(),
            EvidenceTier::Linguistic,
            score,
            ClaimKind::Fraud { head, score },
            vec![FactId::new("test", head.as_str())],
            head.as_str(),
        )
    }

    fn witnesses(silent: u32, reports: u32) -> FactSet {
        let mut facts = FactSet::new();
        facts.insert(Fact {
            id: FactId::new("witness", "silent"),
            kind: FactKind::SilentApplicants { n: silent },
            provenance: Provenance::Witness,
        });
        facts.insert(Fact {
            id: FactId::new("witness", "reports"),
            kind: FactKind::ReportContributors { n: reports },
            provenance: Provenance::Witness,
        });
        facts
    }

    fn run(facts: &FactSet, bundle: Bundle) -> Judgement {
        judge(facts, bundle, &Params::default())
    }

    #[test]
    fn structure_alone_never_likely() {
        let j = run(
            &witnesses(9, 9),
            Bundle {
                claims: vec![
                    ghost_claim(GhostCriterion::EvergreenPosting),
                    ghost_claim(GhostCriterion::SourceAbsent),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.ghost.level, GhostLevel::Possible);
    }

    #[test]
    fn outcome_tier_is_what_lifts_likely() {
        let j = run(
            &witnesses(2, 2),
            Bundle {
                claims: vec![
                    ghost_claim(GhostCriterion::EvergreenPosting),
                    ghost_claim(GhostCriterion::UserReports),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.ghost.level, GhostLevel::Likely);
    }

    #[test]
    fn undercut_removes_the_claim_but_leaves_the_head_open() {
        let dead = ghost_claim(GhostCriterion::SourceAbsent);
        let j = run(
            &FactSet::new(),
            Bundle {
                defeaters: vec![Defeater::undercut(
                    dead.id.clone(),
                    DefeatReason::NoCoverage,
                    "empty crawl",
                )],
                claims: vec![
                    dead,
                    ghost_claim(GhostCriterion::EvergreenPosting),
                    ghost_claim(GhostCriterion::RepostReset),
                ],
            },
        );
        assert!(!j.ghost.fired.contains(&GhostCriterion::SourceAbsent));
        assert_eq!(j.ghost.level, GhostLevel::Possible);
        assert!(j.rebutted.is_empty());
    }

    #[test]
    fn rebut_bars_the_whole_head() {
        let opposed = ghost_claim(GhostCriterion::SourceAbsent);
        let j = run(
            &FactSet::new(),
            Bundle {
                defeaters: vec![Defeater::rebut(
                    opposed.id.clone(),
                    DefeatReason::ClosedRequisition,
                    "closed",
                )],
                claims: vec![
                    opposed,
                    ghost_claim(GhostCriterion::EvergreenPosting),
                    ghost_claim(GhostCriterion::RepostReset),
                ],
            },
        );
        // Two criteria survived and would have converged.
        assert_eq!(j.ghost.level, GhostLevel::None);
        assert!(j.ghost.fired.is_empty());
        assert!(j.rebutted.contains(&ClaimHead::Ghost));
    }

    #[test]
    fn a_defeater_against_nothing_bars_nothing() {
        let j = run(
            &witnesses(2, 2),
            Bundle {
                claims: vec![
                    ghost_claim(GhostCriterion::UserReports),
                    ghost_claim(GhostCriterion::EvergreenPosting),
                ],
                defeaters: vec![Defeater::rebut(
                    ClaimId::new(AnalyzerId::Ghost, "never_filed"),
                    DefeatReason::ClosedRequisition,
                    "stray",
                )],
            },
        );
        assert!(j.rebutted.is_empty());
        assert_eq!(j.ghost.level, GhostLevel::Likely);
    }

    #[test]
    fn a_disagreeing_band_collapses_to_unknown() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    source_claim("a", SourceKind::Live, 0.90),
                    source_claim("b", SourceKind::Indexed, 0.90),
                    source_claim("c", SourceKind::Live, 0.90),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.source.kind, SourceKind::Unknown);
        assert!(!j.source.invented);
        assert_eq!(
            j.source.signals,
            vec!["a".to_string(), "b".into(), "c".into()]
        );
    }

    #[test]
    fn fold_source_is_order_independent() {
        // Straight at the fold, without the id sort in front of it.
        let claims = vec![
            source_claim("a", SourceKind::Live, 0.90),
            source_claim("b", SourceKind::Indexed, 0.90),
            source_claim("c", SourceKind::Live, 0.90),
        ];
        let barred = BTreeSet::new();
        for permutation in permutations(&claims) {
            let resolved = fold_source(&permutation, &barred, &Params::default());
            assert_eq!(resolved.kind, SourceKind::Unknown);
            assert!(!resolved.invented);
        }
    }

    #[test]
    fn equal_confidence_agreement_keeps_both_signals() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    source_claim("ats", SourceKind::Indexed, 0.92),
                    source_claim("ingest", SourceKind::Indexed, 0.92),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.source.kind, SourceKind::Indexed);
        assert_eq!(j.source.signals, vec!["ats".to_string(), "ingest".into()]);
    }

    #[test]
    fn a_clearer_signal_beats_a_tie_below_it() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    source_claim("a", SourceKind::Live, 0.85),
                    source_claim("b", SourceKind::Indexed, 0.85),
                    source_claim("c", SourceKind::Marketplace, 0.95),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.source.kind, SourceKind::Marketplace);
        assert_eq!(j.source.signals, vec!["c".to_string()]);
    }

    #[test]
    fn below_floor_is_never_resolved() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![source_claim("ingest", SourceKind::Live, 0.4)],
                defeaters: vec![],
            },
        );
        assert_eq!(j.source.kind, SourceKind::Unknown);
        assert_eq!(j.source.confidence, 0.0);
        assert!(j.source.signals.is_empty());
    }

    #[test]
    fn conflicting_liveness_takes_the_worse_reading() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    liveness_claim(SourceLiveness::LiveAtSource),
                    liveness_claim(SourceLiveness::ClosedAtSource),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.liveness, SourceLiveness::ClosedAtSource);
    }

    #[test]
    fn absent_liveness_is_uncertain_not_live() {
        let j = run(&FactSet::new(), Bundle::default());
        assert_eq!(j.liveness, SourceLiveness::StatusUncertain);
    }

    // --- serving policy (ported from the removed `jm-disposition` crate) --

    #[test]
    fn quiet_listing_is_shown() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![liveness_claim(SourceLiveness::LiveAtSource)],
                defeaters: vec![],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Show);
        assert_eq!(j.obligation.by, "LeastCommitment");
    }

    #[test]
    fn human_fraud_drops() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![liveness_claim(SourceLiveness::LiveAtSource), human_drop()],
                defeaters: vec![],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Drop);
        assert_eq!(j.obligation.by, "HumanConfirmedFraudRule");
    }

    #[test]
    fn a_rebutted_reviewer_drop_does_not_drop() {
        let reviewer = human_drop();
        let j = run(
            &FactSet::new(),
            Bundle {
                defeaters: vec![Defeater::rebut(
                    reviewer.id.clone(),
                    DefeatReason::ReviewerOverride,
                    "overturned on appeal",
                )],
                claims: vec![liveness_claim(SourceLiveness::LiveAtSource), reviewer],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Show);
    }

    #[test]
    fn public_threshold_does_not_restrict() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    liveness_claim(SourceLiveness::LiveAtSource),
                    fraud_claim(FraudHead::UpfrontFee, 1.0),
                ],
                defeaters: vec![],
            },
        );
        assert_ne!(j.obligation.disposition, Disposition::Restrict);
    }

    #[test]
    fn local_threshold_restricts() {
        let params = Params {
            fraud_auto_restrict_threshold: Some(0.8),
            ..Params::default()
        };
        let j = judge(
            &FactSet::new(),
            Bundle {
                claims: vec![
                    liveness_claim(SourceLiveness::LiveAtSource),
                    fraud_claim(FraudHead::UpfrontFee, 1.0),
                ],
                defeaters: vec![],
            },
            &params,
        );
        assert_eq!(j.obligation.disposition, Disposition::Restrict);
        assert_eq!(j.obligation.by, "AutoRestrictFraudRule");
    }

    #[test]
    fn ghost_discloses() {
        let j = run(
            &witnesses(2, 2),
            Bundle {
                claims: vec![
                    liveness_claim(SourceLiveness::LiveAtSource),
                    ghost_claim(GhostCriterion::UserReports),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Disclose);
        assert_eq!(j.obligation.by, "GhostDiscloseRule");
    }

    #[test]
    fn source_not_found_discloses() {
        let j = run(
            &FactSet::new(),
            Bundle {
                claims: vec![liveness_claim(SourceLiveness::SourceNotFound)],
                defeaters: vec![],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Disclose);
        assert_eq!(j.obligation.by, "StaleDiscloseRule");
    }

    #[test]
    fn drop_absorbs_disclose() {
        let j = run(
            &witnesses(2, 2),
            Bundle {
                claims: vec![
                    ghost_claim(GhostCriterion::UserReports),
                    liveness_claim(SourceLiveness::ClosedAtSource),
                    human_drop(),
                ],
                defeaters: vec![],
            },
        );
        assert_eq!(j.obligation.disposition, Disposition::Drop);
    }

    #[test]
    fn restrict_beats_disclose_regardless_of_analyzer_order() {
        let params = Params {
            fraud_auto_restrict_threshold: Some(0.5),
            ..Params::default()
        };
        // Permuting the bundle is permuting analyzer registration order.
        let claims = vec![
            fraud_claim(FraudHead::UpfrontFee, 1.0),
            ghost_claim(GhostCriterion::UserReports),
            liveness_claim(SourceLiveness::ClosedAtSource),
        ];
        for permutation in permutations(&claims) {
            let j = judge(
                &witnesses(2, 2),
                Bundle {
                    claims: permutation,
                    defeaters: vec![],
                },
                &params,
            );
            assert_eq!(j.obligation.disposition, Disposition::Restrict);
            assert_eq!(j.obligation.by, "AutoRestrictFraudRule");
        }
    }

    fn permutations(claims: &[Claim]) -> Vec<Vec<Claim>> {
        assert_eq!(claims.len(), 3, "hand-rolled for three");
        [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ]
        .iter()
        .map(|order| order.iter().map(|&i| claims[i].clone()).collect())
        .collect()
    }
}

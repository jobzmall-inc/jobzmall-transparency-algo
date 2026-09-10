//! Phase 0–1 of the compiler: canonicalize the wire record, then extract
//! ground facts.
//!
//! Analyzers never parse URLs, dates, or email addresses themselves. If a
//! fact is not in the [`FactSet`], it does not exist.

mod timeparse;
mod urlclass;

use chrono::{DateTime, Utc};
use jm_common::ListingRecord;
use jm_kernel::{Fact, FactId, FactKind, FactSet, Provenance};
use urlclass::{ApplyKind, OriginClass};

pub use timeparse::{parse_time, resolve_as_of};
pub use urlclass::{classify_apply, classify_origin, AtsProvider, OriginClass as OriginHint};

/// Typed listing after parse. The compiler's only view of a posting.
#[derive(Debug, Clone)]
pub struct CanonicalListing {
    pub raw: ListingRecord,
    pub as_of: DateTime<Utc>,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_synced: Option<DateTime<Utc>>,
    pub last_checked: Option<DateTime<Utc>>,
    pub derived_age_days: Option<u32>,
    /// Hours between the newest source contact and `as_of`. `None` is no
    /// measurement, not zero lag.
    pub sync_lag_hours: Option<f64>,
    pub origin: OriginClass,
    pub apply: ApplyKind,
    pub text: String,
}

impl CanonicalListing {
    pub fn last_verification(&self) -> Option<DateTime<Utc>> {
        self.last_checked.or(self.last_synced)
    }
}

pub fn canonicalize(raw: &ListingRecord, as_of: DateTime<Utc>) -> CanonicalListing {
    let first_seen = parse_time(&raw.first_seen);
    let last_synced = parse_time(&raw.last_synced);
    let last_checked = parse_time(&raw.last_checked);
    let derived_age_days =
        first_seen.map(|t| as_of.signed_duration_since(t).num_days().max(0) as u32);
    let sync_lag_hours = last_checked
        .or(last_synced)
        .map(|t| as_of.signed_duration_since(t).num_minutes() as f64 / 60.0);
    let origin = classify_origin(&raw.origin_url, &raw.origin_name, &raw.jobzmall_url);
    let apply = classify_apply(&raw.apply_destination);
    let text = format!(
        "{} {} {} {}",
        raw.title, raw.company, raw.description, raw.apply_destination
    )
    .to_ascii_lowercase();

    CanonicalListing {
        raw: raw.clone(),
        as_of,
        first_seen,
        last_synced,
        last_checked,
        derived_age_days,
        sync_lag_hours,
        origin,
        apply,
        text,
    }
}

pub fn extract(canon: &CanonicalListing) -> FactSet {
    let mut facts = FactSet::new();
    let r = &canon.raw;

    flag(
        &mut facts,
        "closed",
        r.closed,
        FactKind::Closed,
        Provenance::Ingest,
    );
    flag(
        &mut facts,
        "claimed",
        r.claimed,
        FactKind::Claimed,
        Provenance::Ingest,
    );
    flag(
        &mut facts,
        "human_fraud",
        r.human_confirmed_fraud,
        FactKind::HumanConfirmedFraud,
        Provenance::Reviewer,
    );
    flag(
        &mut facts,
        "evergreen_hint",
        r.evergreen,
        FactKind::EvergreenHint,
        Provenance::Ingest,
    );
    flag(
        &mut facts,
        "date_reset",
        r.date_reset_detected,
        FactKind::DateResetHint,
        Provenance::Ingest,
    );
    flag(
        &mut facts,
        "source_absent_hint",
        r.source_absent,
        FactKind::SourceAbsentHint,
        Provenance::Ingest,
    );

    if r.has_source_coverage {
        insert(
            &mut facts,
            "coverage",
            FactKind::HasCoverage,
            Provenance::Ingest,
        );
    } else {
        insert(
            &mut facts,
            "coverage",
            FactKind::NoCoverage,
            Provenance::Ingest,
        );
    }

    if let Some(kind) = r.suggested_source_kind {
        insert(
            &mut facts,
            "suggested",
            FactKind::SuggestedSource {
                source_kind: format!("{kind:?}").to_ascii_lowercase(),
                confidence: r.source_confidence,
            },
            Provenance::Ingest,
        );
    }

    if let Some(code) = r.source_http_status {
        insert(
            &mut facts,
            "http",
            FactKind::HttpStatus { code },
            Provenance::Ingest,
        );
    }
    insert(
        &mut facts,
        "missed",
        FactKind::MissedSyncs { n: r.missed_syncs },
        Provenance::Ingest,
    );
    insert(
        &mut facts,
        "age_ingest",
        FactKind::PostingAgeDays {
            n: r.posting_age_days,
        },
        Provenance::Ingest,
    );
    if let Some(n) = r.source_absent_stamp_age_days {
        insert(
            &mut facts,
            "absent_age",
            FactKind::AbsenceStampAgeDays { n },
            Provenance::Extract,
        );
    }
    if let Some(n) = canon.derived_age_days {
        insert(
            &mut facts,
            "age_derived",
            FactKind::DerivedAgeDays { n },
            Provenance::Extract,
        );
    }
    if let Some(n) = canon.sync_lag_hours {
        insert(
            &mut facts,
            "lag",
            FactKind::SyncLagHours { n },
            Provenance::Extract,
        );
    }

    insert(
        &mut facts,
        "silent",
        FactKind::SilentApplicants {
            n: r.silent_application_contributors,
        },
        Provenance::Witness,
    );
    insert(
        &mut facts,
        "reports",
        FactKind::ReportContributors {
            n: r.report_contributors,
        },
        Provenance::Witness,
    );
    if !r.outcome_witness_ids.is_empty() {
        let mut ids = r.outcome_witness_ids.clone();
        ids.sort();
        ids.dedup();
        insert(
            &mut facts,
            "witness_ids",
            FactKind::WitnessIds {
                n: ids.len() as u32,
            },
            Provenance::Witness,
        );
    }

    match &canon.origin {
        OriginClass::Ats { provider, .. } => insert(
            &mut facts,
            "origin",
            FactKind::OriginAts {
                provider: provider.as_str().into(),
            },
            Provenance::Extract,
        ),
        OriginClass::Career { host } => insert(
            &mut facts,
            "origin",
            FactKind::OriginCareerHost { host: host.clone() },
            Provenance::Extract,
        ),
        OriginClass::Jobzmall => insert(
            &mut facts,
            "origin",
            FactKind::OriginJobzmall,
            Provenance::Extract,
        ),
        OriginClass::Unknown => insert(
            &mut facts,
            "origin",
            FactKind::OriginUnparseable,
            Provenance::Extract,
        ),
    }

    match &canon.apply {
        ApplyKind::PersonalEmail { domain } => insert(
            &mut facts,
            "apply",
            FactKind::ApplyPersonalEmail {
                domain: domain.clone(),
            },
            Provenance::Extract,
        ),
        ApplyKind::CorporateEmail { domain } => insert(
            &mut facts,
            "apply",
            FactKind::ApplyCorporateEmail {
                domain: domain.clone(),
            },
            Provenance::Extract,
        ),
        ApplyKind::Url { host } => insert(
            &mut facts,
            "apply",
            FactKind::ApplyUrl { host: host.clone() },
            Provenance::Extract,
        ),
        ApplyKind::Text(_) => {}
    }

    if let Some(domain) = urlclass::personal_mail_in_text(&canon.text) {
        insert(
            &mut facts,
            "personal_mail_in_text",
            FactKind::PersonalEmailInText { domain },
            Provenance::Extract,
        );
    }

    if ALWAYS_HIRING.is_match(&canon.text) {
        insert(
            &mut facts,
            "always_hiring",
            FactKind::AlwaysHiringLanguage,
            Provenance::Extract,
        );
    }

    facts
}

fn flag(facts: &mut FactSet, key: &str, on: bool, kind: FactKind, provenance: Provenance) {
    if on {
        insert(facts, key, kind, provenance);
    }
}

fn insert(facts: &mut FactSet, key: &str, kind: FactKind, provenance: Provenance) {
    let tag = match &kind {
        FactKind::Closed => "flag",
        FactKind::Claimed => "flag",
        FactKind::HumanConfirmedFraud => "flag",
        FactKind::SuggestedSource { .. } => "source",
        FactKind::HttpStatus { .. } => "http",
        FactKind::MissedSyncs { .. } => "sync",
        FactKind::HasCoverage | FactKind::NoCoverage => "coverage",
        FactKind::SourceAbsentHint => "ghost",
        FactKind::AbsenceStampAgeDays { .. } => "ghost",
        FactKind::EvergreenHint => "ghost",
        FactKind::DateResetHint => "ghost",
        FactKind::PostingAgeDays { .. } => "time",
        FactKind::SyncLagHours { .. } => "time",
        FactKind::DerivedAgeDays { .. } => "time",
        FactKind::SilentApplicants { .. } => "witness",
        FactKind::ReportContributors { .. } => "witness",
        FactKind::WitnessIds { .. } => "witness",
        FactKind::ApplyPersonalEmail { .. }
        | FactKind::ApplyCorporateEmail { .. }
        | FactKind::ApplyUrl { .. } => "apply",
        FactKind::OriginAts { .. }
        | FactKind::OriginCareerHost { .. }
        | FactKind::OriginJobzmall
        | FactKind::OriginUnparseable => "origin",
        FactKind::PhraseHit { .. } | FactKind::NegatedPhrase { .. } => "lex",
        FactKind::BrandImpersonation { .. } => "lex",
        FactKind::AlwaysHiringLanguage => "lex",
        FactKind::PersonalEmailInText { .. } => "lex",
    };
    facts.insert(Fact {
        id: FactId::new(tag, key),
        kind,
        provenance,
    });
}

static ALWAYS_HIRING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\b(always hiring|evergreen|talent bench|rolling applications)\b")
        .expect("always-hiring pattern")
});

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn derives_age_from_iso_dates() {
        let rec = ListingRecord {
            first_seen: "2026-01-01".into(),
            last_synced: "2026-08-01T00:00:00Z".into(),
            ..ListingRecord::default()
        };
        let canon = canonicalize(&rec, at(2026, 8, 26));
        assert_eq!(canon.derived_age_days, Some(237));
    }

    #[test]
    fn empty_coverage_is_a_fact() {
        let rec = ListingRecord {
            has_source_coverage: false,
            ..ListingRecord::default()
        };
        let facts = extract(&canonicalize(&rec, at(2026, 8, 26)));
        assert!(facts.has(|k| matches!(k, FactKind::NoCoverage)));
    }

    #[test]
    fn unparseable_stamps_leave_lag_unmeasured() {
        let rec = ListingRecord {
            last_synced: "sometime last spring".into(),
            last_checked: String::new(),
            ..ListingRecord::default()
        };
        let canon = canonicalize(&rec, at(2026, 8, 26));
        assert_eq!(canon.sync_lag_hours, None);
        assert!(!extract(&canon).has(|k| matches!(k, FactKind::SyncLagHours { .. })));
    }

    #[test]
    fn a_personal_mailbox_in_the_body_is_a_fact() {
        let rec = ListingRecord {
            description: "Send your CV to Talent.Team@Gmail.com".into(),
            apply_destination: "Career page".into(),
            ..ListingRecord::default()
        };
        let facts = extract(&canonicalize(&rec, at(2026, 8, 26)));
        assert!(facts.has(
            |k| matches!(k, FactKind::PersonalEmailInText { domain } if domain == "gmail.com")
        ));
    }

    #[test]
    fn non_ascii_body_extracts_without_panicking() {
        let rec = ListingRecord {
            title: "Ingénieur — Données".into(),
            company: "Société Générale".into(),
            description: "Envoyez — ne payez pas — votre dossier à rh@gmail.com".into(),
            apply_destination: "Écrire à rh@gmail.com".into(),
            ..ListingRecord::default()
        };
        let facts = extract(&canonicalize(&rec, at(2026, 8, 26)));
        assert!(facts.has(|k| matches!(k, FactKind::ApplyPersonalEmail { .. })));
    }
}

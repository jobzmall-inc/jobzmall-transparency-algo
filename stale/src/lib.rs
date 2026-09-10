//! Source liveness. A miss is `status_uncertain` or `source_not_found` —
//! never silently "open".
//!
//! HTTP is classified by semantic family, then gated on sync freshness and
//! the missed-sync budget. A 200 older than `live_job_max_sync_lag_hours` is
//! not live; it is uncertain. Freshness has to be *measured* — a listing
//! whose stamps do not parse has no lag reading, and no reading is not zero.

use jm_common::SourceLiveness;
use jm_kernel::{AnalyzerId, Bundle, Claim, ClaimKind, EvidenceTier, FactId, FactKind, FactSet};
use jm_params::Params;

pub fn analyze(facts: &FactSet, params: &Params) -> Bundle {
    let mut bundle = Bundle::default();
    let assessment = assess(facts, params);
    let state = assessment.state;
    bundle.claims.push(Claim::new(
        AnalyzerId::Stale,
        state.as_str(),
        EvidenceTier::Provenance,
        match state {
            SourceLiveness::LiveAtSource | SourceLiveness::ClosedAtSource => 0.9,
            _ => 0.45,
        },
        ClaimKind::Liveness { state },
        assessment.premises,
        format!("{state:?}"),
    ));
    bundle
}

pub fn classify(facts: &FactSet, params: &Params) -> SourceLiveness {
    assess(facts, params).state
}

struct Assessment {
    state: SourceLiveness,
    premises: Vec<FactId>,
}

fn assess(facts: &FactSet, params: &Params) -> Assessment {
    if let Some(closed) = facts.cite(|k| matches!(k, FactKind::Closed)) {
        return Assessment {
            state: SourceLiveness::ClosedAtSource,
            premises: vec![closed],
        };
    }

    let status = facts.pick(|k| match k {
        FactKind::HttpStatus { code } => Some(*code),
        _ => None,
    });
    let missed = facts.pick(|k| match k {
        FactKind::MissedSyncs { n } => Some(*n),
        _ => None,
    });
    let lag = facts.pick(|k| match k {
        FactKind::SyncLagHours { n } => Some(*n),
        _ => None,
    });
    let no_coverage = facts.cite(|k| matches!(k, FactKind::NoCoverage));

    let premises: Vec<FactId> = [
        status.as_ref().map(|(id, _)| id.clone()),
        missed.as_ref().map(|(id, _)| id.clone()),
        lag.as_ref().map(|(id, _)| id.clone()),
        no_coverage.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    // No reading is not fresh, and neither is a stamp from the future.
    let fresh = matches!(lag, Some((_, hours))
        if (0.0..=f64::from(params.live_job_max_sync_lag_hours)).contains(&hours));
    let over_budget = missed.is_some_and(|(_, n)| n >= params.stale_after_missed_syncs);

    let state = match status.map(|(_, code)| code) {
        Some(code) if is_gone(code) => SourceLiveness::ClosedAtSource,
        Some(code) if is_success(code) && fresh && !over_budget => SourceLiveness::LiveAtSource,
        Some(_) => SourceLiveness::StatusUncertain,
        None if no_coverage.is_some() => SourceLiveness::SourceNotFound,
        None => SourceLiveness::StatusUncertain,
    };

    Assessment { state, premises }
}

fn is_success(code: u16) -> bool {
    (200..300).contains(&code)
}

fn is_gone(code: u16) -> bool {
    matches!(code, 404 | 410)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use jm_common::ListingRecord;
    use jm_extract::{canonicalize, extract};

    fn facts_for(rec: &ListingRecord) -> jm_kernel::FactSet {
        let as_of = chrono::Utc.with_ymd_and_hms(2026, 8, 26, 12, 0, 0).unwrap();
        extract(&canonicalize(rec, as_of))
    }

    fn run(rec: ListingRecord) -> SourceLiveness {
        classify(&facts_for(&rec), &Params::default())
    }

    /// Stamps one hour behind `as_of`.
    fn synced() -> ListingRecord {
        ListingRecord {
            last_synced: "2026-08-26T11:00:00Z".into(),
            last_checked: "2026-08-26T11:00:00Z".into(),
            missed_syncs: 0,
            ..ListingRecord::default()
        }
    }

    #[test]
    fn live_200_is_live_when_fresh() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            ..synced()
        };
        assert_eq!(run(rec), SourceLiveness::LiveAtSource);
    }

    #[test]
    fn gone_410_is_closed() {
        let rec = ListingRecord {
            source_http_status: Some(410),
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::ClosedAtSource);
    }

    #[test]
    fn no_coverage_is_not_found() {
        let rec = ListingRecord {
            source_http_status: None,
            has_source_coverage: false,
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::SourceNotFound);
    }

    #[test]
    fn missed_syncs_are_uncertain() {
        let rec = ListingRecord {
            source_http_status: Some(503),
            missed_syncs: 4,
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::StatusUncertain);
    }

    #[test]
    fn stale_lag_on_200_is_uncertain() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            last_synced: "2026-08-01T00:00:00Z".into(),
            last_checked: "2026-08-01T00:00:00Z".into(),
            missed_syncs: 0,
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::StatusUncertain);
    }

    #[test]
    fn a_200_over_the_missed_sync_budget_is_uncertain() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            missed_syncs: 3,
            ..synced()
        };
        assert_eq!(run(rec), SourceLiveness::StatusUncertain);
    }

    #[test]
    fn an_unmeasured_lag_is_never_fresh() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            last_synced: "sometime last spring".into(),
            last_checked: String::new(),
            missed_syncs: 0,
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::StatusUncertain);
    }

    #[test]
    fn a_stamp_from_the_future_is_not_fresh() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            last_synced: "2027-01-01T00:00:00Z".into(),
            last_checked: "2027-01-01T00:00:00Z".into(),
            missed_syncs: 0,
            ..ListingRecord::default()
        };
        assert_eq!(run(rec), SourceLiveness::StatusUncertain);
    }

    #[test]
    fn the_claim_cites_what_it_read() {
        let rec = ListingRecord {
            source_http_status: Some(200),
            ..synced()
        };
        let bundle = analyze(&facts_for(&rec), &Params::default());
        let premises = &bundle.claims[0].premises;
        assert!(premises.iter().any(|p| p.0.starts_with("http:")));
        assert!(premises.iter().any(|p| p.0.starts_with("sync:")));
        assert!(premises.iter().any(|p| p.0.starts_with("time:lag")));
    }
}

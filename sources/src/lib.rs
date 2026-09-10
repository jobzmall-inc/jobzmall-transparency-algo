//! Source resolution by *agreement*, not ingest suggestion alone.
//!
//! Each independent signal (ingest hint, ATS host, career host, JobzMall
//! URL) is a claim. The kernel keeps the highest-confidence kind that
//! clears the floor. Two equally confident *disagreeing* kinds collapse
//! to `unknown` — we do not invent a winner.
//!
//! A suggestion below the floor is emitted and then undercut, so the
//! warrant shows why it did not count.

use jm_common::SourceKind;
use jm_kernel::{
    AnalyzerId, Bundle, Claim, ClaimKind, DefeatReason, Defeater, EvidenceTier, FactId, FactKind,
    FactSet,
};
use jm_params::Params;

pub fn analyze(facts: &FactSet, params: &Params) -> Bundle {
    let mut bundle = Bundle::default();

    if let Some((fact, (raw, confidence))) = facts.pick(|k| match k {
        FactKind::SuggestedSource {
            source_kind,
            confidence,
        } => Some((source_kind.clone(), *confidence)),
        _ => None,
    }) {
        let kind = parse_kind(&raw);
        let claim = source_claim(
            "ingest",
            kind,
            confidence,
            vec![fact],
            format!("ingest {kind:?}"),
        );
        if kind == SourceKind::Unknown || confidence < params.source_confidence_floor {
            bundle.defeaters.push(Defeater::undercut(
                claim.id.clone(),
                DefeatReason::BelowConfidenceFloor,
                format!(
                    "confidence {confidence:.2} < floor {:.2}",
                    params.source_confidence_floor
                ),
            ));
        }
        bundle.claims.push(claim);
    }

    if let Some(fact) = facts.cite(|k| matches!(k, FactKind::OriginAts { .. })) {
        bundle.claims.push(source_claim(
            "ats",
            SourceKind::Indexed,
            0.92,
            vec![fact],
            "ATS host class",
        ));
    }
    if let Some(fact) = facts.cite(|k| matches!(k, FactKind::OriginCareerHost { .. })) {
        bundle.claims.push(source_claim(
            "career",
            SourceKind::Live,
            0.88,
            vec![fact],
            "career-page host class",
        ));
    }
    if let Some(fact) = facts.cite(|k| matches!(k, FactKind::OriginJobzmall)) {
        bundle.claims.push(source_claim(
            "jobzmall",
            SourceKind::Marketplace,
            0.90,
            vec![fact],
            "JobzMall origin",
        ));
    }

    bundle
}

fn source_claim(
    key: &str,
    kind: SourceKind,
    confidence: f64,
    premises: Vec<FactId>,
    note: impl Into<String>,
) -> Claim {
    Claim::new(
        AnalyzerId::Source,
        key,
        EvidenceTier::Provenance,
        confidence,
        ClaimKind::Source {
            source_kind: kind,
            confidence,
        },
        premises,
        note,
    )
}

fn parse_kind(raw: &str) -> SourceKind {
    match raw {
        "live" => SourceKind::Live,
        "marketplace" => SourceKind::Marketplace,
        "indexed" => SourceKind::Indexed,
        _ => SourceKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use jm_common::ListingRecord;
    use jm_extract::{canonicalize, extract};
    use jm_kernel::judge;

    fn resolve(rec: ListingRecord) -> jm_common::SourceResolution {
        let as_of = chrono::Utc.with_ymd_and_hms(2026, 8, 26, 0, 0, 0).unwrap();
        let facts = extract(&canonicalize(&rec, as_of));
        judge(
            &facts,
            analyze(&facts, &Params::default()),
            &Params::default(),
        )
        .source
    }

    #[test]
    fn accepts_confident_live() {
        let r = resolve(ListingRecord {
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.95,
            origin_url: "https://careers.acme.com/eng".into(),
            origin_name: "careers.acme.com".into(),
            ..ListingRecord::default()
        });
        assert_eq!(r.kind, SourceKind::Live);
        assert!(!r.invented);
    }

    #[test]
    fn refuses_to_invent_below_floor() {
        let r = resolve(ListingRecord {
            suggested_source_kind: Some(SourceKind::Indexed),
            source_confidence: 0.4,
            origin_url: "https://example.invalid/apply".into(),
            origin_name: String::new(),
            jobzmall_url: "https://www.jobzmall.com/jobs/x".into(),
            ..ListingRecord::default()
        });
        assert_eq!(r.kind, SourceKind::Unknown);
        assert!(!r.invented);
        assert!(r.signals.is_empty());
    }

    #[test]
    fn greenhouse_url_is_indexed_even_without_ingest() {
        let r = resolve(ListingRecord {
            suggested_source_kind: None,
            source_confidence: 0.0,
            origin_url: "https://job-boards.greenhouse.io/figma/jobs/1".into(),
            origin_name: "Greenhouse · Figma".into(),
            ..ListingRecord::default()
        });
        assert_eq!(r.kind, SourceKind::Indexed);
        assert!(r.confidence >= 0.80);
    }

    #[test]
    fn a_disagreement_at_the_same_confidence_resolves_to_unknown() {
        let r = resolve(ListingRecord {
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.90,
            origin_url: "https://www.jobzmall.com/jobs/x".into(),
            origin_name: String::new(),
            jobzmall_url: "https://www.jobzmall.com/jobs/x".into(),
            ..ListingRecord::default()
        });
        assert_eq!(r.kind, SourceKind::Unknown);
    }

    #[test]
    fn every_source_claim_cites_a_fact() {
        let as_of = chrono::Utc.with_ymd_and_hms(2026, 8, 26, 0, 0, 0).unwrap();
        let rec = ListingRecord {
            suggested_source_kind: Some(SourceKind::Indexed),
            source_confidence: 0.91,
            origin_url: "https://job-boards.greenhouse.io/figma/jobs/1".into(),
            origin_name: "Greenhouse · Figma".into(),
            ..ListingRecord::default()
        };
        let facts = extract(&canonicalize(&rec, as_of));
        let bundle = analyze(&facts, &Params::default());
        assert_eq!(bundle.claims.len(), 2);
        for claim in &bundle.claims {
            assert!(!claim.premises.is_empty(), "{} cites no fact", claim.id.0);
            for premise in &claim.premises {
                assert!(facts.get(premise).is_some(), "{} is not a fact", premise.0);
            }
        }
    }
}

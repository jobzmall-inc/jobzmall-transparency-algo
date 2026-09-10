//! The warranted-judgment compiler.
//!
//! A listing is compiled, not mixed. Phases:
//!
//! ```text
//! 0  as_of          resolve the evaluation instant (no wall clock by default)
//! 1  canonicalize   parse dates, URLs, apply targets
//! 2  extract        ground facts with provenance
//! 3  analyze        source · ghost · fraud · stale · reviewer  (no cross-talk)
//! 4  defeat+judge   undercut / rebut / least-commitment / lattice join
//! 5  project        Nutrition Label
//! 6  warrant        public proof tree
//! 7  aftermath      serve record + correction hint
//! ```
//!
//! Analyzers cannot see each other's claims. The shared input is the
//! [`jm_kernel::FactSet`], and for three of the four it is the only input;
//! `jm_fraud` also reads the canonical text. Two identical listings produce
//! identical dossiers.

use chrono::{DateTime, Utc};
use jm_common::{Aftermath, Dossier, ListingRecord};
use jm_extract::{canonicalize, extract, resolve_as_of};
use jm_kernel::{
    build_warrant, judge, AnalyzerId, Bundle, Claim, ClaimKind, EvidenceTier, FactKind,
};
use jm_params::Params;

/// Compile one listing. `now` overrides the evaluation instant; `None` is
/// derived from the listing so fixtures stay deterministic.
pub fn compile(listing: ListingRecord, params: &Params, now: Option<DateTime<Utc>>) -> Dossier {
    let as_of = resolve_as_of(&listing, now);
    let canon = canonicalize(&listing, as_of);
    let mut facts = extract(&canon);

    let mut bundle = Bundle::default();
    bundle = bundle.merge(jm_sources::analyze(&facts, params));
    bundle = bundle.merge(jm_ghost::analyze(&facts, params));
    bundle = bundle.merge(jm_fraud::analyze(&canon, &mut facts, params));
    bundle = bundle.merge(jm_stale::analyze(&facts, params));
    if let Some(reviewed) = facts.cite(|k| matches!(k, FactKind::HumanConfirmedFraud)) {
        bundle.claims.push(Claim::new(
            AnalyzerId::Reviewer,
            "human_drop",
            EvidenceTier::Reviewer,
            1.0,
            ClaimKind::HumanDrop,
            vec![reviewed],
            "human-confirmed fraud",
        ));
    }

    let facts_extracted = facts.len() as u32;
    let judgement = judge(&facts, bundle, params);

    let mut source = judgement.source.clone();
    if source.kind != jm_common::SourceKind::Unknown {
        source.origin_name = listing.origin_name.clone();
    }

    let label = jm_nutrition::project(&listing, &source, &judgement.ghost, judgement.liveness);
    let warrant = build_warrant(&judgement);
    let aftermath =
        Aftermath::from_verdict(judgement.liveness, &source, listing.human_confirmed_fraud);

    let mut dossier = Dossier::from_listing_shell(&listing);
    dossier.source = source;
    dossier.ghost = judgement.ghost;
    dossier.fraud = judgement.fraud;
    dossier.liveness = judgement.liveness;
    dossier.label = label;
    dossier.disposition = judgement.obligation.disposition;
    dossier.decided_by = judgement.obligation.by;
    dossier.as_of = as_of.to_rfc3339();
    dossier.facts_extracted = facts_extracted;
    dossier.claims_issued = judgement.issued;
    dossier.claims_surviving = judgement.surviving;
    dossier.warrant = warrant;
    dossier.aftermath = aftermath;
    dossier
}

#[cfg(test)]
mod tests {
    use super::*;
    use jm_common::{Disposition, GhostLevel, ListingRecord, OpeningStatus, SourceKind};

    #[test]
    fn compile_is_deterministic() {
        let rec = ListingRecord::default();
        let a = compile(rec.clone(), &Params::default(), None);
        let b = compile(rec, &Params::default(), None);
        assert_eq!(a, b);
    }

    #[test]
    fn default_listing_is_a_live_show() {
        let rec = ListingRecord {
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.95,
            last_synced: "2026-08-26T11:00:00Z".into(),
            last_checked: "2026-08-26T11:00:00Z".into(),
            first_seen: "2026-08-20".into(),
            as_of: Some("2026-08-26T12:00:00Z".into()),
            ..ListingRecord::default()
        };
        let d = compile(rec, &Params::default(), None);
        assert_eq!(d.source.kind, SourceKind::Live);
        assert_eq!(d.disposition, Disposition::Show);
        assert_eq!(d.label.current_opening_status, OpeningStatus::Open);
        assert!(!d.warrant.premises.is_empty());
        assert_eq!(d.schema, jm_common::DOSSIER_SCHEMA);
    }

    #[test]
    fn closed_plus_ghost_inputs_stay_quiet_and_stale() {
        let rec = ListingRecord {
            closed: true,
            evergreen: true,
            silent_application_contributors: 4,
            source_http_status: Some(410),
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.9,
            ..ListingRecord::default()
        };
        let d = compile(rec, &Params::default(), None);
        assert_eq!(d.ghost.level, GhostLevel::None);
        assert_eq!(d.label.current_opening_status, OpeningStatus::Stale);
        assert_eq!(d.disposition, Disposition::Disclose);
    }

    #[test]
    fn a_listing_with_no_parseable_stamps_is_not_open() {
        let rec = ListingRecord {
            first_seen: "last spring".into(),
            last_synced: "recently".into(),
            last_checked: String::new(),
            source_http_status: Some(200),
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.95,
            ..ListingRecord::default()
        };
        let d = compile(rec, &Params::default(), None);
        assert_eq!(d.label.current_opening_status, OpeningStatus::Unknown);
        assert_eq!(d.disposition, Disposition::Disclose);
    }

    #[test]
    fn the_warrant_walks_down_to_facts() {
        let rec = ListingRecord {
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.95,
            as_of: Some("2026-08-26T12:00:00Z".into()),
            last_checked: "2026-08-26T11:00:00Z".into(),
            ..ListingRecord::default()
        };
        let d = compile(rec, &Params::default(), None);
        let source_node = d
            .warrant
            .premises
            .iter()
            .find(|n| n.rule == "SourceAgreement")
            .expect("a source line");
        let claim = source_node.premises.first().expect("a claim line");
        let fact = claim.premises.first().expect("a fact line");
        assert_eq!(fact.rule, "fact");
        assert!(!fact.conclusion.is_empty());
    }

    #[test]
    fn a_non_ascii_listing_compiles() {
        let rec = ListingRecord {
            title: "Développeur — Rust".into(),
            company: "Société Générale".into(),
            location: "Paris, Île-de-France".into(),
            description: "Ne payez pas — jamais — pour une formation. Écrivez à rh@gmail.com"
                .into(),
            apply_destination: "Écrire à rh@gmail.com".into(),
            ..ListingRecord::default()
        };
        let d = compile(rec, &Params::default(), None);
        assert!(d.fraud.personal_email_recruiter > 0.0);
    }
}

//! End-to-end contract over the shipped fixtures. Every label field is
//! pinned; fraud probabilities are asserted as bounds, not digits.

use jm_common::{
    CorrectionKind, Disposition, EmployerRelationship, GhostCriterion, GhostLevel, NutritionLabel,
    OpeningStatus, SourceKind, SourceLiveness,
};
use jm_enforcement::{recommend, RecommendedAction};
use jm_params::Params;

const NAMES: [&str; 5] = [
    "stripe-staff-engineer",
    "figma-engineering-manager",
    "notion-product-designer",
    "ghost-evergreen",
    "fee-scam",
];

fn compile(name: &str) -> jm_common::Dossier {
    let listing = label_mixer::load_listing(format!("fixtures/{name}.json"))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    jm_engine::compile(listing, &Params::default(), None)
}

#[test]
fn stripe_is_a_live_show() {
    let d = compile("stripe-staff-engineer");
    assert_eq!(d.source.kind, SourceKind::Live);
    assert_eq!(d.source.signals, vec!["ingest Live".to_string()]);
    assert!(!d.source.invented);
    assert_eq!(d.liveness, SourceLiveness::LiveAtSource);
    assert_eq!(d.ghost.level, GhostLevel::None);
    assert!(d.ghost.fired.is_empty());
    assert_eq!(d.fraud.max(), 0.0);
    assert_eq!(d.disposition, Disposition::Show);
    assert_eq!(d.decided_by, "LeastCommitment");
    assert_eq!(recommend(&d), RecommendedAction::None);
    assert_eq!(d.aftermath.correction, None);
    assert_eq!(
        d.label,
        NutritionLabel {
            source: "careers.stripe.com".into(),
            employer_relationship: EmployerRelationship::Official,
            original_publication_date: "Mar 12, 2026".into(),
            last_verification_date: "Aug 26, 2026 · 14:08 UTC".into(),
            application_destination: "Official career page".into(),
            work_arrangement: "Remote".into(),
            compensation_status: "Range published".into(),
            ai_involvement: "none stated".into(),
            current_opening_status: OpeningStatus::Open,
            reporting_mechanism: "On the posting".into(),
        }
    );
}

#[test]
fn figma_resolves_by_ats_host_over_the_ingest_hint() {
    let d = compile("figma-engineering-manager");
    assert_eq!(d.source.kind, SourceKind::Indexed);
    // The ATS host class (0.92) outranks the ingest suggestion (0.91).
    assert_eq!(d.source.signals, vec!["ATS host class".to_string()]);
    assert_eq!(d.liveness, SourceLiveness::LiveAtSource);
    assert_eq!(d.disposition, Disposition::Show);
    assert_eq!(
        d.label,
        NutritionLabel {
            source: "Greenhouse · Figma".into(),
            employer_relationship: EmployerRelationship::Indexed,
            original_publication_date: "Apr 28, 2026".into(),
            last_verification_date: "Aug 26, 2026 · 11:22 UTC".into(),
            application_destination: "Employer ATS".into(),
            work_arrangement: "On-site".into(),
            compensation_status: "Range published".into(),
            ai_involvement: "none stated".into(),
            current_opening_status: OpeningStatus::Open,
            reporting_mechanism: "On the posting".into(),
        }
    );
}

#[test]
fn notion_is_a_claimed_marketplace_listing() {
    let d = compile("notion-product-designer");
    assert_eq!(d.source.kind, SourceKind::Marketplace);
    assert_eq!(d.liveness, SourceLiveness::LiveAtSource);
    assert_eq!(d.disposition, Disposition::Show);
    assert_eq!(
        d.label,
        NutritionLabel {
            source: "Posted on JobzMall".into(),
            employer_relationship: EmployerRelationship::Claimed,
            original_publication_date: "Jun 2, 2026".into(),
            last_verification_date: "Aug 25, 2026 · 09:41 UTC".into(),
            application_destination: "JobzMall marketplace listing".into(),
            work_arrangement: "Hybrid".into(),
            compensation_status: "not disclosed".into(),
            ai_involvement: "none stated".into(),
            current_opening_status: OpeningStatus::Open,
            reporting_mechanism: "On the posting".into(),
        }
    );
}

#[test]
fn evergreen_is_a_likely_ghost_that_is_disclosed_not_removed() {
    let d = compile("ghost-evergreen");
    assert_eq!(d.ghost.level, GhostLevel::Likely);
    assert_eq!(
        d.ghost.fired,
        vec![
            GhostCriterion::EvergreenPosting,
            GhostCriterion::RepostReset,
            GhostCriterion::SilentApplications,
            GhostCriterion::UserReports,
        ]
    );
    assert_eq!(d.ghost.witness_lower_bound, 2);
    assert_eq!(d.ghost.witness_upper_bound, 4);
    // Live at source and still a likely ghost.
    assert_eq!(d.liveness, SourceLiveness::LiveAtSource);
    assert_eq!(d.disposition, Disposition::Disclose);
    assert_eq!(d.decided_by, "GhostDiscloseRule");
    assert_eq!(recommend(&d), RecommendedAction::Review);
    assert_eq!(
        d.label,
        NutritionLabel {
            source: "careers.pipeline.example".into(),
            employer_relationship: EmployerRelationship::Official,
            original_publication_date: "Jan 4, 2025".into(),
            last_verification_date: "Aug 20, 2026".into(),
            application_destination: "Official career page".into(),
            work_arrangement: "Hybrid".into(),
            compensation_status: "not disclosed".into(),
            ai_involvement: "none stated".into(),
            current_opening_status: OpeningStatus::Unknown,
            reporting_mechanism: "On the posting".into(),
        }
    );
}

#[test]
fn the_fee_scam_scores_high_and_still_does_not_auto_drop() {
    let d = compile("fee-scam");
    assert!(d.fraud.upfront_fee > 0.5);
    assert!(d.fraud.identity_harvest > 0.5);
    assert!(d.fraud.personal_email_recruiter > 0.5);
    // A head that never fired reports zero, not its prior.
    assert_eq!(d.fraud.impersonation, 0.0);
    assert!(d.fraud.combined > d.fraud.max());

    // Machines never drop, and the public defaults leave the restrict
    // threshold unset.
    assert_eq!(d.disposition, Disposition::Disclose);
    assert_eq!(d.decided_by, "StaleDiscloseRule");
    assert_eq!(recommend(&d), RecommendedAction::Review);

    assert_eq!(d.source.kind, SourceKind::Unknown);
    assert!(d.source.signals.is_empty());
    assert!(!d.source.invented);
    assert_eq!(d.liveness, SourceLiveness::SourceNotFound);
    assert!(d.aftermath.source_moved);
    assert_eq!(d.aftermath.correction, Some(CorrectionKind::ResyncSource));
    assert_eq!(
        d.label,
        NutritionLabel {
            source: "unknown".into(),
            employer_relationship: EmployerRelationship::Unknown,
            original_publication_date: "Aug 1, 2026".into(),
            last_verification_date: "Aug 2, 2026".into(),
            application_destination: "hr.recruiting@gmail.com".into(),
            work_arrangement: "not disclosed".into(),
            compensation_status: "not disclosed".into(),
            ai_involvement: "none stated".into(),
            current_opening_status: OpeningStatus::Unknown,
            reporting_mechanism: "On the posting".into(),
        }
    );
}

#[test]
fn a_local_threshold_is_what_restricts_the_fee_scam() {
    let listing = label_mixer::load_listing("fixtures/fee-scam.json").unwrap();
    let params = Params {
        fraud_auto_restrict_threshold: Some(0.9),
        ..Params::default()
    };
    let d = jm_engine::compile(listing, &params, None);
    assert_eq!(d.disposition, Disposition::Restrict);
    assert_eq!(d.decided_by, "AutoRestrictFraudRule");
    assert_eq!(recommend(&d), RecommendedAction::Restrict);
}

#[test]
fn every_fixture_publishes_a_walkable_warrant() {
    for name in NAMES {
        let d = compile(name);
        assert_eq!(d.warrant.rule, "LatticeJoin", "{name}");
        assert!(d.facts_extracted > 0, "{name}");
        assert!(
            d.claims_surviving <= d.claims_issued,
            "{name}: more claims survived than were issued"
        );

        let reached_a_fact = d
            .warrant
            .premises
            .iter()
            .flat_map(|head| &head.premises)
            .flat_map(|claim| &claim.premises)
            .any(|fact| fact.rule == "fact" && !fact.conclusion.is_empty());
        assert!(reached_a_fact, "{name}: warrant does not reach a fact");
    }
}

#[test]
fn compiling_a_fixture_twice_gives_the_same_dossier() {
    for name in NAMES {
        assert_eq!(compile(name), compile(name), "{name}");
    }
}

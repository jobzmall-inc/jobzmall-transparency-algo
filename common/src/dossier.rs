use crate::listing::ListingRecord;
use crate::types::{
    CompensationStatus, Disposition, EmployerRelationship, FraudScores, GhostVerdict,
    OpeningStatus, SourceKind, SourceLiveness, SourceResolution, WorkArrangement,
};
use serde::{Deserialize, Serialize};

/// Public schema id. Bump when a field is added, removed, or changes meaning.
pub const DOSSIER_SCHEMA: &str = "https://transparency.jobzmall.com/schema/dossier/v2";

/// Structured reason the compiler reached a conclusion.
///
/// Premises are themselves warrants, so a reader can walk the proof. Defeated
/// lines are *shown*, not dropped — an undercut fact that did not count is
/// part of the public record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarrantNode {
    pub conclusion: String,
    pub rule: String,
    pub force: f64,
    #[serde(default)]
    pub premises: Vec<WarrantNode>,
    #[serde(default)]
    pub defeated: Vec<String>,
}

impl WarrantNode {
    pub fn leaf(conclusion: impl Into<String>, rule: impl Into<String>, force: f64) -> Self {
        Self {
            conclusion: conclusion.into(),
            rule: rule.into(),
            force,
            premises: Vec::new(),
            defeated: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionKind {
    ResyncSource,
    ReclassifySource,
    ReviewFraud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aftermath {
    pub served: bool,
    pub source_moved: bool,
    pub correction: Option<CorrectionKind>,
}

impl Aftermath {
    pub fn from_verdict(
        liveness: SourceLiveness,
        source: &SourceResolution,
        human_fraud: bool,
    ) -> Self {
        let source_moved = matches!(
            liveness,
            SourceLiveness::ClosedAtSource | SourceLiveness::SourceNotFound
        );
        let correction = if human_fraud {
            Some(CorrectionKind::ReviewFraud)
        } else if source_moved {
            Some(CorrectionKind::ResyncSource)
        } else if source.kind == SourceKind::Unknown && !source.signals.is_empty() {
            Some(CorrectionKind::ReclassifySource)
        } else {
            None
        };
        Self {
            served: true,
            source_moved,
            correction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NutritionLabel {
    pub source: String,
    pub employer_relationship: EmployerRelationship,
    pub original_publication_date: String,
    pub last_verification_date: String,
    pub application_destination: String,
    pub work_arrangement: String,
    pub compensation_status: String,
    pub ai_involvement: String,
    pub current_opening_status: OpeningStatus,
    pub reporting_mechanism: String,
}

impl NutritionLabel {
    pub fn codes(&self) -> (WorkArrangement, CompensationStatus) {
        (
            WorkArrangement::parse(Some(&self.work_arrangement)),
            CompensationStatus::parse(Some(&self.compensation_status)),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dossier {
    pub schema: String,
    pub listing_id: String,
    pub slug: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub source: SourceResolution,
    pub ghost: GhostVerdict,
    pub fraud: FraudScores,
    pub liveness: SourceLiveness,
    pub label: NutritionLabel,
    pub disposition: Disposition,
    pub decided_by: String,
    pub report_url: String,
    pub origin_url: String,
    pub jobzmall_url: String,
    pub as_of: String,
    pub facts_extracted: u32,
    pub claims_issued: u32,
    pub claims_surviving: u32,
    pub warrant: WarrantNode,
    pub aftermath: Aftermath,
}

impl Dossier {
    pub fn from_listing_shell(listing: &ListingRecord) -> Self {
        Self {
            schema: DOSSIER_SCHEMA.into(),
            listing_id: listing.id.clone(),
            slug: listing.slug.clone(),
            title: listing.title.clone(),
            company: listing.company.clone(),
            location: listing.location.clone(),
            source: SourceResolution {
                kind: SourceKind::Unknown,
                origin_name: String::new(),
                confidence: 0.0,
                invented: false,
                signals: Vec::new(),
            },
            ghost: GhostVerdict::quiet(),
            fraud: FraudScores::quiet(),
            liveness: SourceLiveness::StatusUncertain,
            label: NutritionLabel {
                source: "unknown".into(),
                employer_relationship: EmployerRelationship::Unknown,
                original_publication_date: "unknown".into(),
                last_verification_date: "unknown".into(),
                application_destination: "unknown".into(),
                work_arrangement: WorkArrangement::NotDisclosed.as_label().into(),
                compensation_status: CompensationStatus::NotDisclosed.as_label().into(),
                ai_involvement: "none stated".into(),
                current_opening_status: OpeningStatus::Unknown,
                reporting_mechanism: "On the posting".into(),
            },
            disposition: Disposition::Show,
            decided_by: "LeastCommitment".into(),
            report_url: listing.report_url(),
            origin_url: listing.origin_url.clone(),
            jobzmall_url: listing.jobzmall_url.clone(),
            as_of: String::new(),
            facts_extracted: 0,
            claims_issued: 0,
            claims_surviving: 0,
            warrant: WarrantNode::leaf("unevaluated", "identity", 0.0),
            aftermath: Aftermath {
                served: false,
                source_moved: false,
                correction: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn keys(value: &Value) -> Vec<&str> {
        let mut keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn wire_shape_is_pinned_to_the_schema() {
        assert_eq!(
            DOSSIER_SCHEMA,
            "https://transparency.jobzmall.com/schema/dossier/v2"
        );

        let dossier = Dossier::from_listing_shell(&ListingRecord::default());
        let wire = serde_json::to_value(&dossier).unwrap();

        assert_eq!(
            keys(&wire),
            [
                "aftermath",
                "as_of",
                "claims_issued",
                "claims_surviving",
                "company",
                "decided_by",
                "disposition",
                "facts_extracted",
                "fraud",
                "ghost",
                "jobzmall_url",
                "label",
                "listing_id",
                "liveness",
                "location",
                "origin_url",
                "report_url",
                "schema",
                "slug",
                "source",
                "title",
                "warrant",
            ]
        );
        assert_eq!(
            keys(&wire["label"]),
            [
                "ai_involvement",
                "application_destination",
                "compensation_status",
                "current_opening_status",
                "employer_relationship",
                "last_verification_date",
                "original_publication_date",
                "reporting_mechanism",
                "source",
                "work_arrangement",
            ]
        );
        assert_eq!(
            keys(&wire["source"]),
            ["confidence", "invented", "kind", "origin_name", "signals"]
        );
        assert_eq!(
            keys(&wire["ghost"]),
            [
                "criteria_total",
                "fired",
                "level",
                "witness_lower_bound",
                "witness_upper_bound",
            ]
        );
        assert_eq!(
            keys(&wire["fraud"]),
            [
                "combined",
                "identity_harvest",
                "impersonation",
                "personal_email_recruiter",
                "upfront_fee",
            ]
        );
        assert_eq!(
            keys(&wire["warrant"]),
            ["conclusion", "defeated", "force", "premises", "rule"]
        );
        assert_eq!(
            keys(&wire["aftermath"]),
            ["correction", "served", "source_moved"]
        );
    }

    #[test]
    fn wire_vocabulary_is_snake_case() {
        let mut dossier = Dossier::from_listing_shell(&ListingRecord::default());
        dossier.disposition = Disposition::Disclose;
        dossier.liveness = SourceLiveness::SourceNotFound;
        dossier.label.employer_relationship = EmployerRelationship::Official;
        dossier.label.current_opening_status = OpeningStatus::Stale;
        dossier.aftermath.correction = Some(CorrectionKind::ResyncSource);

        let wire = serde_json::to_value(&dossier).unwrap();
        assert_eq!(wire["disposition"], "disclose");
        assert_eq!(wire["liveness"], "source_not_found");
        assert_eq!(wire["label"]["employer_relationship"], "official");
        assert_eq!(wire["label"]["current_opening_status"], "stale");
        assert_eq!(wire["aftermath"]["correction"], "resync_source");
    }

    #[test]
    fn a_dossier_round_trips() {
        let dossier = Dossier::from_listing_shell(&ListingRecord::default());
        let json = serde_json::to_string(&dossier).unwrap();
        let back: Dossier = serde_json::from_str(&json).unwrap();
        assert_eq!(dossier, back);
    }

    #[test]
    fn aftermath_asks_for_a_reclassify_only_when_signals_conflicted() {
        let conflicted = SourceResolution {
            kind: SourceKind::Unknown,
            origin_name: String::new(),
            confidence: 0.0,
            invented: false,
            signals: vec!["ingest Live".into(), "JobzMall origin".into()],
        };
        let a = Aftermath::from_verdict(SourceLiveness::LiveAtSource, &conflicted, false);
        assert_eq!(a.correction, Some(CorrectionKind::ReclassifySource));

        let no_provenance_at_all = SourceResolution {
            signals: Vec::new(),
            ..conflicted
        };
        let b = Aftermath::from_verdict(SourceLiveness::LiveAtSource, &no_provenance_at_all, false);
        assert_eq!(b.correction, None);
    }

    #[test]
    fn human_fraud_outranks_a_moved_source() {
        let a = Aftermath::from_verdict(
            SourceLiveness::SourceNotFound,
            &SourceResolution {
                kind: SourceKind::Live,
                origin_name: String::new(),
                confidence: 0.9,
                invented: false,
                signals: vec!["ingest".into()],
            },
            true,
        );
        assert_eq!(a.correction, Some(CorrectionKind::ReviewFraud));
        assert!(a.source_moved);
    }
}

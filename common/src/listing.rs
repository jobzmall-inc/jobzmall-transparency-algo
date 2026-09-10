use crate::types::SourceKind;
use serde::{Deserialize, Serialize};

/// Wire record the compiler hydrates. Production stores this; fixtures are JSON.
///
/// Feature flags such as `evergreen` and `source_absent` are *hints from
/// ingest*, not conclusions. The extract phase re-derives what it can
/// (temporal age, ATS class, apply-target kind) and the analyzers treat ingest
/// hints as facts with provenance `ingest`, which defeaters may undercut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingRecord {
    pub id: String,
    pub slug: String,
    pub org_slug: String,
    pub job_slug: String,
    pub title: String,
    pub company: String,
    pub location: String,
    #[serde(default)]
    pub description: String,
    pub origin_name: String,
    pub origin_url: String,
    pub first_seen: String,
    pub last_synced: String,
    #[serde(default)]
    pub last_checked: String,
    pub apply_destination: String,
    #[serde(default)]
    pub work_arrangement: Option<String>,
    #[serde(default)]
    pub compensation_status: Option<String>,
    #[serde(default)]
    pub ai_involvement_stated: Option<String>,
    pub jobzmall_url: String,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub suggested_source_kind: Option<SourceKind>,
    #[serde(default)]
    pub source_confidence: f64,
    #[serde(default)]
    pub source_http_status: Option<u16>,
    #[serde(default)]
    pub missed_syncs: u32,
    #[serde(default)]
    pub posting_age_days: u32,
    #[serde(default)]
    pub date_reset_detected: bool,
    #[serde(default)]
    pub evergreen: bool,
    #[serde(default)]
    pub source_absent: bool,
    #[serde(default)]
    pub source_absent_stamp_age_days: Option<u32>,
    #[serde(default)]
    pub has_source_coverage: bool,
    #[serde(default)]
    pub silent_application_contributors: u32,
    #[serde(default)]
    pub report_contributors: u32,
    /// Optional stable witness identifiers (already hashed upstream). When
    /// present, distinct-person count is `unique(ids)`. When absent, the
    /// compiler uses an inclusion interval over the two channel counters.
    #[serde(default)]
    pub outcome_witness_ids: Vec<String>,
    #[serde(default)]
    pub human_confirmed_fraud: bool,
    /// Evaluation instant. Empty means "derive from the listing's newest stamp"
    /// so fixture runs stay deterministic without a wall clock.
    #[serde(default)]
    pub as_of: Option<String>,
}

impl ListingRecord {
    pub fn report_url(&self) -> String {
        format!(
            "https://app.jobzmall.com/{}/job/{}/report",
            self.org_slug, self.job_slug
        )
    }

    /// The stamp to show on the label, verbatim from the wire.
    pub fn last_verification_display(&self) -> &str {
        if !self.last_checked.is_empty() {
            &self.last_checked
        } else {
            &self.last_synced
        }
    }
}

impl Default for ListingRecord {
    fn default() -> Self {
        Self {
            id: "1".into(),
            slug: "acme-eng".into(),
            org_slug: "acme".into(),
            job_slug: "eng".into(),
            title: "Engineer".into(),
            company: "Acme".into(),
            location: "Remote".into(),
            description: String::new(),
            origin_name: "careers.acme.com".into(),
            origin_url: "https://careers.acme.com/eng".into(),
            first_seen: "2026-01-01".into(),
            last_synced: "2026-08-01".into(),
            last_checked: String::new(),
            apply_destination: "Career page".into(),
            work_arrangement: None,
            compensation_status: None,
            ai_involvement_stated: None,
            jobzmall_url: "https://www.jobzmall.com/jobs/acme-eng".into(),
            closed: false,
            claimed: false,
            suggested_source_kind: None,
            source_confidence: 0.9,
            source_http_status: Some(200),
            missed_syncs: 0,
            posting_age_days: 10,
            date_reset_detected: false,
            evergreen: false,
            source_absent: false,
            source_absent_stamp_age_days: None,
            has_source_coverage: true,
            silent_application_contributors: 0,
            report_contributors: 0,
            outcome_witness_ids: Vec::new(),
            human_confirmed_fraud: false,
            as_of: None,
        }
    }
}

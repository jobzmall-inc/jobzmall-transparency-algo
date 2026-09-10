use super::claim::ClaimId;
use serde::{Deserialize, Serialize};

/// Pollock's distinction. Both kinds remove the targeted claim; they differ
/// in what may happen next.
///
/// - **Undercut** — the inference is not licensed, but the conclusion is not
///   opposed, so another claim may still establish it.
/// - **Rebut** — the conclusion is opposed, and the whole head is barred for
///   this listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeatKind {
    Undercut,
    Rebut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeatReason {
    NoCoverage,
    StaleAbsenceStamp,
    ClosedRequisition,
    BelowConfidenceFloor,
    NegatedLanguage,
    YoungForEvergreen,
    ReviewerOverride,
}

impl DefeatReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoCoverage => "no_coverage",
            Self::StaleAbsenceStamp => "stale_absence_stamp",
            Self::ClosedRequisition => "closed_requisition",
            Self::BelowConfidenceFloor => "below_confidence_floor",
            Self::NegatedLanguage => "negated_language",
            Self::YoungForEvergreen => "young_for_evergreen",
            Self::ReviewerOverride => "reviewer_override",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defeater {
    pub target: ClaimId,
    pub kind: DefeatKind,
    pub reason: DefeatReason,
    pub note: String,
}

impl Defeater {
    pub fn undercut(target: ClaimId, reason: DefeatReason, note: impl Into<String>) -> Self {
        Self {
            target,
            kind: DefeatKind::Undercut,
            reason,
            note: note.into(),
        }
    }

    pub fn rebut(target: ClaimId, reason: DefeatReason, note: impl Into<String>) -> Self {
        Self {
            target,
            kind: DefeatKind::Rebut,
            reason,
            note: note.into(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            DefeatKind::Undercut => "undercut",
            DefeatKind::Rebut => "rebut",
        }
    }
}

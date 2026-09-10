use super::fact::FactId;
use jm_common::{FraudHead, GhostCriterion, SourceKind, SourceLiveness};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClaimId(pub String);

impl ClaimId {
    pub fn new(analyzer: AnalyzerId, key: &str) -> Self {
        Self(format!("{analyzer}:{key}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzerId {
    Source,
    Ghost,
    Fraud,
    Stale,
    Reviewer,
}

impl std::fmt::Display for AnalyzerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Source => "source",
            Self::Ghost => "ghost",
            Self::Fraud => "fraud",
            Self::Stale => "stale",
            Self::Reviewer => "reviewer",
        };
        f.write_str(s)
    }
}

/// Epistemic tier. The judge uses this as a type constraint, not a weight:
/// only [`EvidenceTier::Outcome`] observes what happened to a person, and
/// the judge cannot reach `likely` without an outcome-tier claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    Structural,
    Outcome,
    Linguistic,
    Provenance,
    Reviewer,
}

impl EvidenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Outcome => "outcome",
            Self::Linguistic => "linguistic",
            Self::Provenance => "provenance",
            Self::Reviewer => "reviewer",
        }
    }

    pub fn observes_outcome(self) -> bool {
        matches!(self, Self::Outcome)
    }
}

/// The conclusion a claim speaks to. A rebuttal opposes a head, not one
/// inference, so the judge has to know which head a defeated claim was on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimHead {
    Source,
    Ghost,
    Fraud,
    Liveness,
    Drop,
}

/// What a claim asserts.
///
/// There is no negative claim. An analyzer that wants to deny a conclusion
/// files a [`super::Defeater`] against it. A ghost claim carries the
/// criterion, never a level — the level is the judge's under least
/// commitment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaimKind {
    Source {
        source_kind: SourceKind,
        confidence: f64,
    },
    Ghost {
        criterion: GhostCriterion,
    },
    Fraud {
        head: FraudHead,
        score: f64,
    },
    Liveness {
        state: SourceLiveness,
    },
    HumanDrop,
}

impl ClaimKind {
    pub fn head(&self) -> ClaimHead {
        match self {
            Self::Source { .. } => ClaimHead::Source,
            Self::Ghost { .. } => ClaimHead::Ghost,
            Self::Fraud { .. } => ClaimHead::Fraud,
            Self::Liveness { .. } => ClaimHead::Liveness,
            Self::HumanDrop => ClaimHead::Drop,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub analyzer: AnalyzerId,
    pub tier: EvidenceTier,
    /// How hard the analyzer leans on this inference. Published on the
    /// warrant; the judge's gates are counts and tiers, never this number.
    pub force: f64,
    pub kind: ClaimKind,
    /// The facts this claim was grounded in, cited on the warrant.
    pub premises: Vec<FactId>,
    pub note: String,
}

impl Claim {
    pub fn new(
        analyzer: AnalyzerId,
        key: &str,
        tier: EvidenceTier,
        force: f64,
        kind: ClaimKind,
        premises: Vec<FactId>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            id: ClaimId::new(analyzer, key),
            analyzer,
            tier,
            force: force.clamp(0.0, 1.0),
            kind,
            premises,
            note: note.into(),
        }
    }
}

use serde::{Deserialize, Serialize};

/// How JobzMall obtained the posting. Provenance, not a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Live,
    Marketplace,
    Indexed,
    Unknown,
}

impl SourceKind {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Live => "Live Job",
            Self::Marketplace => "Marketplace",
            Self::Indexed => "Indexed channel",
            Self::Unknown => "Unknown",
        }
    }
}

/// Who the employer is to JobzMall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmployerRelationship {
    Official,
    Claimed,
    Indexed,
    Unknown,
}

impl EmployerRelationship {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Official => "Official",
            Self::Claimed => "Claimed",
            Self::Indexed => "Indexed",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningStatus {
    Open,
    Stale,
    Unknown,
}

impl OpeningStatus {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Stale => "Stale",
            Self::Unknown => "Unknown",
        }
    }
}

/// Bounded lattice of serving obligations.
///
/// ```text
/// Show ⊑ Disclose ⊑ Restrict ⊑ Drop
/// ```
///
/// `join` is least-upper-bound: the stronger obligation wins. A drop is
/// absorbing. This is the compiler's replacement for first-match-wins rule
/// order — obligations commute, so analyzer registration order cannot change
/// the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Show,
    Disclose,
    Restrict,
    Drop,
}

impl Disposition {
    pub fn rank(self) -> u8 {
        match self {
            Self::Show => 0,
            Self::Disclose => 1,
            Self::Restrict => 2,
            Self::Drop => 3,
        }
    }

    pub fn join(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub fn join_all<I: IntoIterator<Item = Self>>(items: I) -> Self {
        items.into_iter().fold(Self::Show, Self::join)
    }
}

/// Hedged posting-reality verdict. Computed on read, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GhostLevel {
    None,
    Possible,
    Likely,
}

impl GhostLevel {
    /// Structural claims are type-capped here: they may raise the running
    /// level to `Possible` and no further.
    pub fn cap_structural(self) -> Self {
        match self {
            Self::Likely => Self::Possible,
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GhostCriterion {
    EvergreenPosting,
    SourceAbsent,
    RepostReset,
    SilentApplications,
    UserReports,
}

impl GhostCriterion {
    pub const ALL: [Self; 5] = [
        Self::EvergreenPosting,
        Self::SourceAbsent,
        Self::RepostReset,
        Self::SilentApplications,
        Self::UserReports,
    ];

    pub fn is_structural(self) -> bool {
        matches!(
            self,
            Self::EvergreenPosting | Self::SourceAbsent | Self::RepostReset
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::EvergreenPosting => "evergreen_posting",
            Self::SourceAbsent => "source_absent",
            Self::RepostReset => "repost_reset",
            Self::SilentApplications => "silent_applications",
            Self::UserReports => "user_reports",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GhostVerdict {
    pub level: GhostLevel,
    pub fired: Vec<GhostCriterion>,
    pub criteria_total: usize,
    /// Distinct-person lower bound used at the gate. Withheld from serving
    /// below `ghost_contributor_gate` — see `jm_ghost::serve_witness_count`.
    #[serde(default)]
    pub witness_lower_bound: u32,
    /// Distinct-person upper bound (`sum` of channel counts when ids are
    /// absent). Never used to reach `likely`. Published so a reader can see
    /// how much identity we did not have.
    #[serde(default)]
    pub witness_upper_bound: u32,
}

impl GhostVerdict {
    pub fn quiet() -> Self {
        Self {
            level: GhostLevel::None,
            fired: Vec::new(),
            criteria_total: GhostCriterion::ALL.len(),
            witness_lower_bound: 0,
            witness_upper_bound: 0,
        }
    }

    pub fn fired_of_total(&self) -> String {
        format!("{} of {}", self.fired.len(), self.criteria_total)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceLiveness {
    LiveAtSource,
    ClosedAtSource,
    SourceNotFound,
    StatusUncertain,
}

impl SourceLiveness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveAtSource => "live_at_source",
            Self::ClosedAtSource => "closed_at_source",
            Self::SourceNotFound => "source_not_found",
            Self::StatusUncertain => "status_uncertain",
        }
    }

    pub fn is_disclosable_miss(self) -> bool {
        matches!(
            self,
            Self::ClosedAtSource | Self::StatusUncertain | Self::SourceNotFound
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FraudHead {
    UpfrontFee,
    Impersonation,
    IdentityHarvest,
    PersonalEmailRecruiter,
}

impl FraudHead {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpfrontFee => "upfront_fee",
            Self::Impersonation => "impersonation",
            Self::IdentityHarvest => "identity_harvest",
            Self::PersonalEmailRecruiter => "personal_email_recruiter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FraudScores {
    pub upfront_fee: f64,
    pub impersonation: f64,
    pub identity_harvest: f64,
    pub personal_email_recruiter: f64,
    /// Noisy-OR of the four heads. Used by disposition, never as a blended
    /// "quality" number — ghost and stale stay out of this value.
    #[serde(default)]
    pub combined: f64,
}

impl FraudScores {
    pub fn max(&self) -> f64 {
        self.upfront_fee
            .max(self.impersonation)
            .max(self.identity_harvest)
            .max(self.personal_email_recruiter)
    }

    pub fn quiet() -> Self {
        Self {
            upfront_fee: 0.0,
            impersonation: 0.0,
            identity_harvest: 0.0,
            personal_email_recruiter: 0.0,
            combined: 0.0,
        }
    }

    pub fn recompute_combined(&mut self) {
        // Noisy-OR: 1 - Π(1 - p_i). Independent heads; a listing that trips
        // two heads is worse than one, but never worse than certainty.
        self.combined = 1.0
            - (1.0 - self.upfront_fee)
                * (1.0 - self.impersonation)
                * (1.0 - self.identity_harvest)
                * (1.0 - self.personal_email_recruiter);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceResolution {
    pub kind: SourceKind,
    pub origin_name: String,
    pub confidence: f64,
    /// True only if the compiler *constructed* a kind the evidence did not
    /// support. The resolver is required to keep this false; the field exists
    /// so a future regression is visible on the dossier, not silent.
    pub invented: bool,
    /// How the kind was earned. Empty when `Unknown`.
    #[serde(default)]
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkArrangement {
    #[serde(
        alias = "on_site",
        alias = "onsite",
        alias = "On-site",
        alias = "On site"
    )]
    OnSite,
    #[serde(alias = "Hybrid")]
    Hybrid,
    #[serde(alias = "Remote")]
    Remote,
    #[serde(alias = "not_disclosed", alias = "Not disclosed")]
    NotDisclosed,
}

impl WorkArrangement {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|s| s.trim().to_ascii_lowercase()) {
            Some(s) if s == "on-site" || s == "onsite" || s == "on site" || s == "on_site" => {
                Self::OnSite
            }
            Some(s) if s == "hybrid" => Self::Hybrid,
            Some(s) if s == "remote" => Self::Remote,
            _ => Self::NotDisclosed,
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::OnSite => "On-site",
            Self::Hybrid => "Hybrid",
            Self::Remote => "Remote",
            Self::NotDisclosed => "not disclosed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationStatus {
    Published,
    RangePublished,
    NotDisclosed,
}

impl CompensationStatus {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|s| s.trim().to_ascii_lowercase()) {
            Some(s) if s == "published" || s == "disclosed" => Self::Published,
            Some(s) if s.contains("range") => Self::RangePublished,
            _ => Self::NotDisclosed,
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::Published => "Published",
            Self::RangePublished => "Range published",
            Self::NotDisclosed => "not disclosed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_join_is_commutative_and_absorbing() {
        assert_eq!(
            Disposition::Disclose.join(Disposition::Restrict),
            Disposition::Restrict
        );
        assert_eq!(
            Disposition::Restrict.join(Disposition::Disclose),
            Disposition::Restrict
        );
        assert_eq!(
            Disposition::Drop.join(Disposition::Restrict),
            Disposition::Drop
        );
        assert_eq!(
            Disposition::join_all([
                Disposition::Show,
                Disposition::Disclose,
                Disposition::Restrict
            ]),
            Disposition::Restrict
        );
    }

    #[test]
    fn structural_ghost_cannot_name_likely() {
        assert_eq!(GhostLevel::Likely.cap_structural(), GhostLevel::Possible);
        assert_eq!(GhostLevel::Possible.cap_structural(), GhostLevel::Possible);
        assert_eq!(GhostLevel::None.cap_structural(), GhostLevel::None);
    }
}

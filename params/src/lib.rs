use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Published defaults. Fraud restrict thresholds are omitted here on purpose.
///
/// Every analyzer reads from this one struct. There is not a second copy of
/// any gate in `ghost/`, `stale/`, `fraud/`, or `engine/`.
///
/// A local policy file may set only the fields it overrides. An unknown
/// field is an error, not a silent default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Below this, source resolution emits `unknown` and refuses to invent.
    pub source_confidence_floor: f64,
    /// Hours of sync lag before a 2xx stops meaning "live".
    pub live_job_max_sync_lag_hours: u32,
    /// Missed source syncs before stale scoring may say uncertain.
    pub stale_after_missed_syncs: u32,
    /// Structural criteria that must fire before structure alone says anything.
    pub ghost_convergence: u32,
    /// Distinct people required for `likely`. Also the serving withhold gate.
    pub ghost_contributor_gate: u32,
    /// `source_absent` stamps older than this are ignored.
    pub ghost_absence_stamp_max_age_days: u32,
    /// Age (days) at which an ingest evergreen hint, or "always hiring"
    /// language plus a date reset, may fire the structural criterion.
    pub ghost_evergreen_age_days: u32,
    /// Absent in the public tree. Set via a local policy file.
    pub fraud_auto_restrict_threshold: Option<f64>,
    /// Machines never drop. Reviewers do. Sealed `false`.
    pub fraud_auto_drop: bool,
    /// Log-odds prior for each public fraud head (probability, not logit).
    pub fraud_head_prior: f64,
    /// Temperature applied to summed phrase weights before the sigmoid.
    pub fraud_temperature: f64,
    /// How far back of a lexicon hit a negation still suppresses it.
    pub fraud_negation_window_bytes: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            source_confidence_floor: 0.80,
            live_job_max_sync_lag_hours: 2,
            stale_after_missed_syncs: 3,
            ghost_convergence: 2,
            ghost_contributor_gate: 2,
            ghost_absence_stamp_max_age_days: 14,
            ghost_evergreen_age_days: 270,
            fraud_auto_restrict_threshold: None,
            fraud_auto_drop: false,
            fraud_head_prior: 0.02,
            fraud_temperature: 1.15,
            fraud_negation_window_bytes: 28,
        }
    }
}

impl Params {
    pub fn from_policy_file(path: impl AsRef<Path>) -> Result<Self, PolicyError> {
        let raw = std::fs::read_to_string(path.as_ref())?;
        let parsed: Params = serde_json::from_str(&raw)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<(), PolicyError> {
        if !(0.0..=1.0).contains(&self.source_confidence_floor) {
            return Err(PolicyError::Invalid(
                "source_confidence_floor must be in [0, 1]".into(),
            ));
        }
        if self.ghost_contributor_gate < 2 {
            return Err(PolicyError::Invalid(
                "ghost_contributor_gate must be >= 2 (privacy and abuse)".into(),
            ));
        }
        if self.ghost_convergence < 1 {
            return Err(PolicyError::Invalid(
                "ghost_convergence must be >= 1".into(),
            ));
        }
        if self.ghost_evergreen_age_days < 30 {
            return Err(PolicyError::Invalid(
                "ghost_evergreen_age_days must be >= 30".into(),
            ));
        }
        if self.fraud_auto_drop {
            return Err(PolicyError::Invalid(
                "fraud_auto_drop must stay false; humans drop fraud".into(),
            ));
        }
        if let Some(threshold) = self.fraud_auto_restrict_threshold {
            if !(0.0..=1.0).contains(&threshold) {
                return Err(PolicyError::Invalid(
                    "fraud_auto_restrict_threshold must be in [0, 1] when set".into(),
                ));
            }
        }
        if !(0.0..=0.5).contains(&self.fraud_head_prior) {
            return Err(PolicyError::Invalid(
                "fraud_head_prior must be in [0, 0.5]".into(),
            ));
        }
        if self.fraud_temperature <= 0.0 {
            return Err(PolicyError::Invalid("fraud_temperature must be > 0".into()));
        }
        if self.fraud_negation_window_bytes == 0 {
            return Err(PolicyError::Invalid(
                "fraud_negation_window_bytes must be > 0".into(),
            ));
        }
        Ok(())
    }

    pub fn fraud_restrict_threshold(&self) -> Option<f64> {
        self.fraud_auto_restrict_threshold
    }
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("read policy: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse policy: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        Params::default().validate().unwrap();
    }

    #[test]
    fn refuses_to_lower_contributor_gate() {
        let params = Params {
            ghost_contributor_gate: 1,
            ..Params::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn refuses_auto_drop() {
        let params = Params {
            fraud_auto_drop: true,
            ..Params::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn public_threshold_is_unset() {
        assert_eq!(Params::default().fraud_restrict_threshold(), None);
    }

    #[test]
    fn policy_file_may_override_one_field() {
        let parsed: Params = serde_json::from_str(r#"{"fraud_auto_restrict_threshold": 0.85}"#)
            .expect("partial policy parses");
        parsed.validate().unwrap();
        assert_eq!(parsed.fraud_restrict_threshold(), Some(0.85));
        assert_eq!(
            parsed.ghost_contributor_gate,
            Params::default().ghost_contributor_gate
        );
    }

    #[test]
    fn policy_file_rejects_a_misspelled_gate() {
        let err = serde_json::from_str::<Params>(r#"{"ghost_contributer_gate": 9}"#);
        assert!(err.is_err());
    }

    #[test]
    fn policy_file_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("jm-params-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("policy.json");
        std::fs::write(&path, r#"{"fraud_auto_restrict_threshold": 0.75}"#).unwrap();
        let loaded = Params::from_policy_file(&path).unwrap();
        assert_eq!(loaded.fraud_restrict_threshold(), Some(0.75));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn refuses_a_zero_negation_window() {
        let params = Params {
            fraud_negation_window_bytes: 0,
            ..Params::default()
        };
        assert!(params.validate().is_err());
    }
}

use jm_common::{Disposition, Dossier};
use serde::{Deserialize, Serialize};

/// What a reviewer should do. Machines recommend. Humans close.
///
/// `ConfirmedDrop` is the post-review state — the listing was already dropped
/// by a human. It is not a request to drop again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    None,
    Review,
    Restrict,
    ConfirmedDrop,
}

pub fn recommend(dossier: &Dossier) -> RecommendedAction {
    match dossier.disposition {
        Disposition::Drop => RecommendedAction::ConfirmedDrop,
        Disposition::Restrict => RecommendedAction::Restrict,
        Disposition::Disclose => RecommendedAction::Review,
        Disposition::Show if dossier.fraud.max() > 0.15 => RecommendedAction::Review,
        Disposition::Show => RecommendedAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jm_common::{FraudScores, ListingRecord};

    fn dossier(disposition: Disposition, fraud: FraudScores) -> Dossier {
        let mut d = Dossier::from_listing_shell(&ListingRecord::default());
        d.disposition = disposition;
        d.fraud = fraud;
        d
    }

    #[test]
    fn show_is_none() {
        assert_eq!(
            recommend(&dossier(Disposition::Show, FraudScores::quiet())),
            RecommendedAction::None
        );
    }

    #[test]
    fn drop_is_confirmed() {
        assert_eq!(
            recommend(&dossier(Disposition::Drop, FraudScores::quiet())),
            RecommendedAction::ConfirmedDrop
        );
    }

    #[test]
    fn quiet_fraud_on_show_does_not_page_reviewers() {
        let mut fraud = FraudScores::quiet();
        fraud.upfront_fee = 0.05;
        fraud.recompute_combined();
        assert_eq!(
            recommend(&dossier(Disposition::Show, fraud)),
            RecommendedAction::None
        );
    }
}

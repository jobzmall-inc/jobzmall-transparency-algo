//! CLI support. The compiler itself is [`jm_engine::compile`].

use jm_common::ListingRecord;

pub fn load_listing(path: impl AsRef<std::path::Path>) -> Result<ListingRecord, String> {
    let raw = std::fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_fixture() {
        let listing = load_listing("fixtures/stripe-staff-engineer.json").unwrap();
        assert_eq!(listing.company, "Stripe");
    }

    #[test]
    fn a_bad_path_is_an_error_not_a_panic() {
        assert!(load_listing("fixtures/does-not-exist.json").is_err());
    }
}

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use jm_common::ListingRecord;

/// Parse the informal stamps fixtures use *and* RFC3339 / ISO dates.
/// A miss is `None`, never a silent epoch.
pub fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|ndt| Utc.from_utc_datetime(&ndt));
    }
    // "Mar 12, 2026"
    if let Ok(d) = NaiveDate::parse_from_str(s, "%b %e, %Y") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|ndt| Utc.from_utc_datetime(&ndt));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%B %e, %Y") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|ndt| Utc.from_utc_datetime(&ndt));
    }
    // "Aug 26, 2026 · 14:08 UTC"
    let cleaned = s.replace(" · ", " ").replace("·", " ");
    for fmt in [
        "%b %e, %Y %H:%M UTC",
        "%B %e, %Y %H:%M UTC",
        "%b %e, %Y %H:%M",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&cleaned, fmt) {
            return Some(Utc.from_utc_datetime(&ndt));
        }
    }
    None
}

/// Evaluation instant. Prefer an explicit `as_of`, then the newest stamp on
/// the listing. Falling back to a fixed epoch would make every age zero;
/// falling back to `Utc::now()` would make fixtures non-deterministic.
/// The last resort is 2026-09-01T00:00:00Z — the published freeze date of
/// this tree — so a listing with no parseable stamps still compiles.
pub fn resolve_as_of(
    listing: &ListingRecord,
    override_now: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    if let Some(now) = override_now {
        return now;
    }
    if let Some(raw) = listing.as_of.as_deref() {
        if let Some(t) = parse_time(raw) {
            return t;
        }
    }
    let candidates = [
        parse_time(&listing.last_checked),
        parse_time(&listing.last_synced),
        parse_time(&listing.first_seen),
    ];
    candidates.into_iter().flatten().max().unwrap_or_else(|| {
        Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0)
            .single()
            .expect("freeze date")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_display_stamps() {
        let t = parse_time("Aug 26, 2026 · 14:08 UTC").unwrap();
        assert_eq!(t.format("%Y-%m-%d %H:%M").to_string(), "2026-08-26 14:08");
        assert!(parse_time("Mar 12, 2026").is_some());
        assert!(parse_time("2026-01-01").is_some());
        assert!(parse_time("").is_none());
    }
}

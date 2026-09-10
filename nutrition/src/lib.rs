use jm_common::{
    CompensationStatus, EmployerRelationship, GhostLevel, GhostVerdict, ListingRecord,
    NutritionLabel, OpeningStatus, SourceKind, SourceLiveness, SourceResolution, WorkArrangement,
};

/// Deterministic projection. Always emits ten fields. A miss is an explicit
/// value, never a blank that looks like certainty. Display strings are
/// normalized to the published vocabulary.
pub fn project(
    listing: &ListingRecord,
    source: &SourceResolution,
    ghost: &GhostVerdict,
    liveness: SourceLiveness,
) -> NutritionLabel {
    NutritionLabel {
        source: source_field(listing, source),
        employer_relationship: relationship(source.kind),
        original_publication_date: display_or_unknown(&listing.first_seen),
        last_verification_date: display_or_unknown(listing.last_verification_display()),
        application_destination: display_or_unknown(&listing.apply_destination),
        work_arrangement: WorkArrangement::parse(listing.work_arrangement.as_deref())
            .as_label()
            .into(),
        compensation_status: CompensationStatus::parse(listing.compensation_status.as_deref())
            .as_label()
            .into(),
        ai_involvement: ai_involvement(listing.ai_involvement_stated.as_deref()),
        current_opening_status: opening_status(ghost, liveness),
        reporting_mechanism: "On the posting".into(),
    }
}

fn source_field(listing: &ListingRecord, source: &SourceResolution) -> String {
    if source.kind == SourceKind::Unknown {
        return "unknown".into();
    }
    if !listing.origin_name.trim().is_empty() {
        return listing.origin_name.clone();
    }
    if !source.origin_name.is_empty() {
        return source.origin_name.clone();
    }
    source.kind.as_label().into()
}

/// Follows the source kind and nothing else. `ListingRecord::claimed` does
/// not appear: marketplace is "posted or claimed" either way, and splitting
/// the two would need a relationship value the vocabulary does not have.
fn relationship(kind: SourceKind) -> EmployerRelationship {
    match kind {
        SourceKind::Live => EmployerRelationship::Official,
        SourceKind::Marketplace => EmployerRelationship::Claimed,
        SourceKind::Indexed => EmployerRelationship::Indexed,
        SourceKind::Unknown => EmployerRelationship::Unknown,
    }
}

fn opening_status(ghost: &GhostVerdict, liveness: SourceLiveness) -> OpeningStatus {
    if matches!(liveness, SourceLiveness::ClosedAtSource) {
        return OpeningStatus::Stale;
    }
    match ghost.level {
        GhostLevel::Possible | GhostLevel::Likely => OpeningStatus::Unknown,
        GhostLevel::None if liveness == SourceLiveness::LiveAtSource => OpeningStatus::Open,
        GhostLevel::None => OpeningStatus::Unknown,
    }
}

fn display_or_unknown(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "unknown".into()
    } else {
        trimmed.to_string()
    }
}

fn ai_involvement(raw: Option<&str>) -> String {
    match raw.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s.is_empty() || s == "none stated" || s == "none" => "none stated".into(),
        Some(s) if s.contains("generat") => "generated".into(),
        Some(s) if s.contains("modif") || s.contains("rewrit") => "modified".into(),
        Some(s) => s,
        None => "none stated".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jm_common::{GhostVerdict, ListingRecord};

    fn live_source() -> SourceResolution {
        SourceResolution {
            kind: SourceKind::Live,
            origin_name: "Workday".into(),
            confidence: 0.96,
            invented: false,
            signals: vec!["ingest".into()],
        }
    }

    fn ghost_at(level: GhostLevel) -> GhostVerdict {
        GhostVerdict {
            level,
            ..GhostVerdict::quiet()
        }
    }

    #[test]
    fn live_quiet_listing_is_open() {
        let listing = ListingRecord {
            origin_name: "Workday".into(),
            first_seen: "Mar 12, 2026".into(),
            last_synced: "Aug 26, 2026 · 14:08 UTC".into(),
            apply_destination: "Official career page".into(),
            work_arrangement: Some("Remote".into()),
            compensation_status: Some("Range published".into()),
            suggested_source_kind: Some(SourceKind::Live),
            source_confidence: 0.96,
            posting_age_days: 160,
            ..ListingRecord::default()
        };
        let label = project(
            &listing,
            &live_source(),
            &GhostVerdict::quiet(),
            SourceLiveness::LiveAtSource,
        );
        assert_eq!(label.source, "Workday");
        assert_eq!(label.employer_relationship, EmployerRelationship::Official);
        assert_eq!(label.current_opening_status, OpeningStatus::Open);
        assert_eq!(label.ai_involvement, "none stated");
        assert_eq!(label.work_arrangement, "Remote");
        assert_eq!(label.compensation_status, "Range published");
        assert_eq!(label.last_verification_date, "Aug 26, 2026 · 14:08 UTC");
    }

    #[test]
    fn ghost_forces_unknown_opening() {
        let label = project(
            &ListingRecord::default(),
            &live_source(),
            &ghost_at(GhostLevel::Possible),
            SourceLiveness::LiveAtSource,
        );
        assert_eq!(label.current_opening_status, OpeningStatus::Unknown);
    }

    #[test]
    fn closed_source_is_stale() {
        let label = project(
            &ListingRecord::default(),
            &live_source(),
            &GhostVerdict::quiet(),
            SourceLiveness::ClosedAtSource,
        );
        assert_eq!(label.current_opening_status, OpeningStatus::Stale);
    }

    #[test]
    fn a_miss_is_never_open() {
        for liveness in [
            SourceLiveness::SourceNotFound,
            SourceLiveness::StatusUncertain,
        ] {
            let label = project(
                &ListingRecord::default(),
                &live_source(),
                &GhostVerdict::quiet(),
                liveness,
            );
            assert_eq!(
                label.current_opening_status,
                OpeningStatus::Unknown,
                "{liveness:?}"
            );
        }
    }

    #[test]
    fn an_unresolved_source_says_unknown_not_a_host_name() {
        let listing = ListingRecord {
            origin_name: "careers.acme.com".into(),
            ..ListingRecord::default()
        };
        let unresolved = SourceResolution {
            kind: SourceKind::Unknown,
            origin_name: String::new(),
            confidence: 0.0,
            invented: false,
            signals: Vec::new(),
        };
        let label = project(
            &listing,
            &unresolved,
            &GhostVerdict::quiet(),
            SourceLiveness::LiveAtSource,
        );
        assert_eq!(label.source, "unknown");
        assert_eq!(label.employer_relationship, EmployerRelationship::Unknown);
    }

    #[test]
    fn last_verification_prefers_the_later_check() {
        let listing = ListingRecord {
            last_synced: "Aug 20, 2026".into(),
            last_checked: "Aug 26, 2026 · 14:08 UTC".into(),
            ..ListingRecord::default()
        };
        let label = project(
            &listing,
            &live_source(),
            &GhostVerdict::quiet(),
            SourceLiveness::LiveAtSource,
        );
        assert_eq!(label.last_verification_date, "Aug 26, 2026 · 14:08 UTC");
    }

    #[test]
    fn every_field_is_filled_for_an_empty_record() {
        let bare = ListingRecord {
            origin_name: String::new(),
            first_seen: String::new(),
            last_synced: String::new(),
            last_checked: String::new(),
            apply_destination: String::new(),
            ..ListingRecord::default()
        };
        let label = project(
            &bare,
            &live_source(),
            &GhostVerdict::quiet(),
            SourceLiveness::StatusUncertain,
        );
        assert_eq!(label.source, "Workday");
        assert_eq!(label.original_publication_date, "unknown");
        assert_eq!(label.last_verification_date, "unknown");
        assert_eq!(label.application_destination, "unknown");
        assert_eq!(label.work_arrangement, "not disclosed");
        assert_eq!(label.compensation_status, "not disclosed");
        assert_eq!(label.ai_involvement, "none stated");
        assert_eq!(label.current_opening_status, OpeningStatus::Unknown);
        assert_eq!(label.reporting_mechanism, "On the posting");
    }
}

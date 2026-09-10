use jm_common::{Disposition, Dossier, GhostLevel, SourceKind};
use serde::{Deserialize, Serialize};

/// Aggregate statistics for the Ads Transparency library.
/// Per-signal gaming data is not published here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub listings: usize,
    pub by_source: SourceCounts,
    pub by_disposition: DispositionCounts,
    pub ghost_possible: usize,
    pub ghost_likely: usize,
    pub facts_extracted: u32,
    pub claims_surviving: u32,
    pub corrections_queued: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceCounts {
    pub live: usize,
    pub marketplace: usize,
    pub indexed: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DispositionCounts {
    pub show: usize,
    pub disclose: usize,
    pub restrict: usize,
    pub drop: usize,
}

pub fn aggregate(dossiers: &[Dossier]) -> Report {
    let mut report = Report {
        listings: dossiers.len(),
        by_source: SourceCounts::default(),
        by_disposition: DispositionCounts::default(),
        ghost_possible: 0,
        ghost_likely: 0,
        facts_extracted: 0,
        claims_surviving: 0,
        corrections_queued: 0,
    };

    for dossier in dossiers {
        match dossier.source.kind {
            SourceKind::Live => report.by_source.live += 1,
            SourceKind::Marketplace => report.by_source.marketplace += 1,
            SourceKind::Indexed => report.by_source.indexed += 1,
            SourceKind::Unknown => report.by_source.unknown += 1,
        }
        match dossier.disposition {
            Disposition::Show => report.by_disposition.show += 1,
            Disposition::Disclose => report.by_disposition.disclose += 1,
            Disposition::Restrict => report.by_disposition.restrict += 1,
            Disposition::Drop => report.by_disposition.drop += 1,
        }
        match dossier.ghost.level {
            GhostLevel::Possible => report.ghost_possible += 1,
            GhostLevel::Likely => report.ghost_likely += 1,
            GhostLevel::None => {}
        }
        report.facts_extracted += dossier.facts_extracted;
        report.claims_surviving += dossier.claims_surviving;
        if dossier.aftermath.correction.is_some() {
            report.corrections_queued += 1;
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use jm_common::ListingRecord;

    fn dossier(kind: SourceKind, disposition: Disposition) -> Dossier {
        let mut d = Dossier::from_listing_shell(&ListingRecord::default());
        d.source.kind = kind;
        d.disposition = disposition;
        d.facts_extracted = 4;
        d.claims_surviving = 2;
        d
    }

    #[test]
    fn counts_kinds() {
        let report = aggregate(&[
            dossier(SourceKind::Live, Disposition::Show),
            dossier(SourceKind::Marketplace, Disposition::Disclose),
        ]);
        assert_eq!(report.listings, 2);
        assert_eq!(report.by_source.live, 1);
        assert_eq!(report.by_source.marketplace, 1);
        assert_eq!(report.by_disposition.disclose, 1);
        assert_eq!(report.facts_extracted, 8);
    }
}

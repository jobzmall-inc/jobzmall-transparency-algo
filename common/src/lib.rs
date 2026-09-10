//! Shared vocabulary for the warranted-judgment compiler.
//!
//! Types here are the *public* ones: wire records, verdicts, the Nutrition
//! Label, the dossier. The compiler IR (`Fact`, `Claim`, `Defeater`) lives in
//! `jm-kernel`. Analyzers must not invent a parallel vocabulary.

mod dossier;
mod listing;
mod types;

pub use dossier::{
    Aftermath, CorrectionKind, Dossier, NutritionLabel, WarrantNode, DOSSIER_SCHEMA,
};
pub use listing::ListingRecord;
pub use types::{
    CompensationStatus, Disposition, EmployerRelationship, FraudHead, FraudScores, GhostCriterion,
    GhostLevel, GhostVerdict, OpeningStatus, SourceKind, SourceLiveness, SourceResolution,
    WorkArrangement,
};

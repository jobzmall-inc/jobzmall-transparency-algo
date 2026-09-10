//! The warranted-judgment calculus.
//!
//! This crate is the IR and the evaluator. Analyzers do not call each other.
//! They emit [`Claim`]s and [`Defeater`]s against a shared [`FactSet`]. The
//! kernel then:
//!
//! 1. **Undercuts** — a defeater that attacks a claim's warrant removes that
//!    claim from the surviving set. Another claim may still establish the
//!    same conclusion.
//! 2. **Rebuts** — a defeater that attacks a conclusion bars the whole head,
//!    so no surviving claim may re-establish it.
//! 3. **Judges** — remaining claims are aggregated under least commitment.
//!    Unknown is the default. Certainty is earned, never inferred from a
//!    missing fact.
//! 4. **Joins** — serving obligations form a bounded lattice
//!    `Show ⊑ Disclose ⊑ Restrict ⊑ Drop`. Join is commutative, so the
//!    order analyzers were registered cannot change the disposition.
//!
//! The kernel has no I/O, no clock, no listing-vs-listing coupling, and no
//! analyzer's domain knowledge — it is told that a head was rebutted, never
//! that a requisition was closed. Two identical `FactSet`s produce identical
//! `Judgement`s.

mod claim;
mod defeat;
mod fact;
mod judge;
mod lattice;
mod warrant;

pub use claim::{AnalyzerId, Claim, ClaimHead, ClaimId, ClaimKind, EvidenceTier};
pub use defeat::{DefeatKind, DefeatReason, Defeater};
pub use fact::{Fact, FactId, FactKind, FactSet, Provenance};
pub use judge::{judge, Bundle, Judgement};
pub use lattice::Obligation;
pub use warrant::build_warrant;

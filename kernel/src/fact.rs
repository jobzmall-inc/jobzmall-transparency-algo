use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable identifier. Facts are addressed by id so claims and defeaters can
/// point at them without cloning payloads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FactId(pub String);

impl FactId {
    pub fn new(kind: &str, key: &str) -> Self {
        Self(format!("{kind}:{key}"))
    }
}

/// Where a fact came from. Ingest hints are first-class and therefore
/// defeasible — an `ingest` evergreen flag can be undercut by a young
/// posting, an `extract` ATS class cannot be undercut by a missing ingest
/// suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Ingest,
    Extract,
    Witness,
    Reviewer,
}

/// A ground observation. The [`FactSet`] is the complete record of what was
/// observed, not a rule input list — `Claimed`, `HasCoverage` and
/// `PostingAgeDays` are on the record whether or not a gate reads them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactKind {
    Closed,
    Claimed,
    HumanConfirmedFraud,
    SuggestedSource {
        source_kind: String,
        confidence: f64,
    },
    HttpStatus {
        code: u16,
    },
    MissedSyncs {
        n: u32,
    },
    HasCoverage,
    NoCoverage,
    SourceAbsentHint,
    AbsenceStampAgeDays {
        n: u32,
    },
    EvergreenHint,
    DateResetHint,
    PostingAgeDays {
        n: u32,
    },
    SyncLagHours {
        n: f64,
    },
    DerivedAgeDays {
        n: u32,
    },
    SilentApplicants {
        n: u32,
    },
    ReportContributors {
        n: u32,
    },
    WitnessIds {
        n: u32,
    },
    ApplyPersonalEmail {
        domain: String,
    },
    ApplyCorporateEmail {
        domain: String,
    },
    ApplyUrl {
        host: String,
    },
    /// A personal mailbox named in the body rather than in the apply target.
    PersonalEmailInText {
        domain: String,
    },
    OriginAts {
        provider: String,
    },
    OriginCareerHost {
        host: String,
    },
    OriginJobzmall,
    OriginUnparseable,
    PhraseHit {
        head: String,
        phrase: String,
        weight: f64,
    },
    NegatedPhrase {
        head: String,
        phrase: String,
    },
    BrandImpersonation {
        brand: String,
    },
    AlwaysHiringLanguage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub kind: FactKind,
    pub provenance: Provenance,
}

/// Ordered fact store. `BTreeMap` so iteration — and therefore claim ids
/// derived from walk order — is deterministic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FactSet {
    facts: BTreeMap<FactId, Fact>,
}

impl FactSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, fact: Fact) {
        self.facts.insert(fact.id.clone(), fact);
    }

    pub fn get(&self, id: &FactId) -> Option<&Fact> {
        self.facts.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Fact> {
        self.facts.values()
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub fn has(&self, pred: impl Fn(&FactKind) -> bool) -> bool {
        self.facts.values().any(|f| pred(&f.kind))
    }

    pub fn find(&self, pred: impl Fn(&FactKind) -> bool) -> Option<&Fact> {
        self.facts.values().find(|f| pred(&f.kind))
    }

    pub fn all(&self, pred: impl Fn(&FactKind) -> bool) -> Vec<&Fact> {
        self.facts.values().filter(|f| pred(&f.kind)).collect()
    }

    /// First fact whose payload `pick` accepts, with its id for `premises`.
    pub fn pick<T>(&self, pick: impl Fn(&FactKind) -> Option<T>) -> Option<(FactId, T)> {
        self.facts
            .values()
            .find_map(|f| pick(&f.kind).map(|v| (f.id.clone(), v)))
    }

    /// Id of the first fact matching `pred`.
    pub fn cite(&self, pred: impl Fn(&FactKind) -> bool) -> Option<FactId> {
        self.find(pred).map(|f| f.id.clone())
    }
}

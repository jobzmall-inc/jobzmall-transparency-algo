//! Public fraud floor: four independent heads, log-odds combination.
//!
//! Production classifiers and prompts are not in this tree. What *is* here is
//! inspectable: a weighted lexicon, a negation window, a personal-mailbox
//! destination check, and a brand-impersonation detector.
//!
//! Scoring, per head:
//!
//! 1. Start from `fraud_head_prior` (published, default 0.02).
//! 2. Each non-negated phrase is a detector: `p = σ(weight / temperature)`.
//! 3. Detectors inside a head combine by noisy-OR. One high-precision
//!    phrase is enough; a second phrase raises the head, never past 1.
//! 4. Destination / brand facts are additional detectors on their heads.
//! 5. The kernel noisy-ORs the four heads into `FraudScores.combined`.
//!
//! A phrase counts if **any** occurrence of it is un-negated. Mailbox
//! domains arrive as facts; `extract/` owns the vocabulary.

use jm_common::FraudHead;
use jm_extract::CanonicalListing;
use jm_kernel::{
    AnalyzerId, Bundle, Claim, ClaimKind, EvidenceTier, Fact, FactId, FactKind, FactSet, Provenance,
};
use jm_params::Params;

struct Phrase {
    head: FraudHead,
    needle: &'static str,
    weight: f64,
}

const LEXICON: &[Phrase] = &[
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "pay for training",
        weight: 2.4,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "pay for equipment",
        weight: 2.2,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "pay for a background check",
        weight: 2.3,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "starter kit fee",
        weight: 2.6,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "processing fee",
        weight: 1.8,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "wire transfer",
        weight: 1.6,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "gift card",
        weight: 1.7,
    },
    Phrase {
        head: FraudHead::UpfrontFee,
        needle: "training fee",
        weight: 2.1,
    },
    Phrase {
        head: FraudHead::Impersonation,
        needle: "we are hiring on behalf of",
        weight: 2.5,
    },
    Phrase {
        head: FraudHead::Impersonation,
        needle: "official microsoft recruiter",
        weight: 2.8,
    },
    Phrase {
        head: FraudHead::Impersonation,
        needle: "from the ceo office",
        weight: 2.2,
    },
    Phrase {
        head: FraudHead::Impersonation,
        needle: "do not contact the company directly",
        weight: 2.6,
    },
    Phrase {
        head: FraudHead::Impersonation,
        needle: "hiring on behalf of google",
        weight: 2.8,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "send your social security",
        weight: 3.0,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "send your ssn",
        weight: 3.0,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "copy of your passport before interview",
        weight: 2.7,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "bank account number to onboard",
        weight: 2.8,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "routing number to get started",
        weight: 2.8,
    },
    Phrase {
        head: FraudHead::IdentityHarvest,
        needle: "social security number",
        weight: 2.4,
    },
];

/// Detectors that read a fact rather than a phrase, on the [`LEXICON`] scale.
const APPLY_PERSONAL_MAIL_WEIGHT: f64 = 2.4;
const TEXT_PERSONAL_MAIL_WEIGHT: f64 = 1.4;
const BRAND_IMPERSONATION_WEIGHT: f64 = 2.1;

const BRANDS: &[&str] = &[
    "google",
    "microsoft",
    "apple",
    "amazon",
    "meta",
    "facebook",
    "tesla",
    "netflix",
    "stripe",
    "openai",
    "nvidia",
];

const NEGATION: &[&str] = &[
    "do not",
    "don't",
    "never",
    "no need to",
    "will not",
    "won't",
    "not required to",
];

struct Head {
    p: f64,
    premises: Vec<FactId>,
}

impl Head {
    fn detect(&mut self, weight: f64, temperature: f64, premise: FactId) {
        self.p = noisy_or(self.p, sigmoid(weight / temperature));
        self.premises.push(premise);
    }
}

pub fn analyze(canon: &CanonicalListing, facts: &mut FactSet, params: &Params) -> Bundle {
    let mut bundle = Bundle::default();
    let text = &canon.text;
    let mut heads: [Head; 4] = std::array::from_fn(|_| Head {
        p: params.fraud_head_prior,
        premises: Vec::new(),
    });

    for phrase in LEXICON {
        let key = phrase.needle.replace(' ', "_");
        match scan(text, phrase.needle, params.fraud_negation_window_bytes) {
            Scan::Absent => {}
            Scan::Negated => {
                facts.insert(Fact {
                    id: FactId::new("lex", &format!("neg-{key}")),
                    kind: FactKind::NegatedPhrase {
                        head: phrase.head.as_str().into(),
                        phrase: phrase.needle.into(),
                    },
                    provenance: Provenance::Extract,
                });
            }
            Scan::Hit => {
                let id = FactId::new("lex", &key);
                facts.insert(Fact {
                    id: id.clone(),
                    kind: FactKind::PhraseHit {
                        head: phrase.head.as_str().into(),
                        phrase: phrase.needle.into(),
                        weight: phrase.weight,
                    },
                    provenance: Provenance::Extract,
                });
                heads[head_ix(phrase.head)].detect(phrase.weight, params.fraud_temperature, id);
            }
        }
    }

    // The apply target is the stronger reading, and does not double up.
    let mailbox = facts
        .cite(|k| matches!(k, FactKind::ApplyPersonalEmail { .. }))
        .map(|id| (id, APPLY_PERSONAL_MAIL_WEIGHT))
        .or_else(|| {
            facts
                .cite(|k| matches!(k, FactKind::PersonalEmailInText { .. }))
                .map(|id| (id, TEXT_PERSONAL_MAIL_WEIGHT))
        });
    if let Some((id, weight)) = mailbox {
        heads[head_ix(FraudHead::PersonalEmailRecruiter)].detect(
            weight,
            params.fraud_temperature,
            id,
        );
    }

    if let Some(brand) = impersonated_brand(canon) {
        let id = FactId::new("lex", "brand");
        facts.insert(Fact {
            id: id.clone(),
            kind: FactKind::BrandImpersonation {
                brand: brand.into(),
            },
            provenance: Provenance::Extract,
        });
        heads[head_ix(FraudHead::Impersonation)].detect(
            BRAND_IMPERSONATION_WEIGHT,
            params.fraud_temperature,
            id,
        );
    }

    for (head, state) in [
        FraudHead::UpfrontFee,
        FraudHead::Impersonation,
        FraudHead::IdentityHarvest,
        FraudHead::PersonalEmailRecruiter,
    ]
    .into_iter()
    .zip(heads)
    {
        // A head that stayed at the prior is silence, not a claim.
        if state.premises.is_empty() {
            continue;
        }
        let score = state.p;
        bundle.claims.push(Claim::new(
            AnalyzerId::Fraud,
            head.as_str(),
            EvidenceTier::Linguistic,
            score,
            ClaimKind::Fraud { head, score },
            state.premises,
            format!("{head:?}={score:.3}"),
        ));
    }

    bundle
}

enum Scan {
    Absent,
    Negated,
    Hit,
}

/// A phrase fires if any occurrence of it escapes a negation.
fn scan(text: &str, needle: &str, window: usize) -> Scan {
    let mut seen = false;
    for (idx, _) in text.match_indices(needle) {
        seen = true;
        if !negated_at(text, idx, window) {
            return Scan::Hit;
        }
    }
    if seen {
        Scan::Negated
    } else {
        Scan::Absent
    }
}

/// Is the occurrence at `idx` inside a negation? `window` is a byte count
/// floored forward to a char boundary — the text is not ASCII-only, and
/// slicing inside a codepoint panics.
fn negated_at(text: &str, idx: usize, window: usize) -> bool {
    let mut start = idx.saturating_sub(window);
    while start < idx && !text.is_char_boundary(start) {
        start += 1;
    }
    let before = &text[start..idx];
    NEGATION.iter().any(|n| before.contains(n))
}

/// Which well-known brand this posting speaks for without being it. A
/// company named after the brand is exempt only when provenance agrees, and
/// the name is matched on whole tokens.
fn impersonated_brand(canon: &CanonicalListing) -> Option<&'static str> {
    let text = &canon.text;
    let name_tokens = tokens(&canon.raw.company);
    let origin_host = canon.origin.host().unwrap_or_default().to_ascii_lowercase();
    for brand in BRANDS {
        let named_as_brand = name_tokens.iter().any(|t| t == brand);
        if named_as_brand && origin_host.contains(brand) {
            continue;
        }
        if text.contains(&format!("on behalf of {brand}"))
            || text.contains(&format!("official {brand}"))
        {
            return Some(brand);
        }
    }
    None
}

fn tokens(raw: &str) -> Vec<String> {
    raw.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn head_ix(head: FraudHead) -> usize {
    match head {
        FraudHead::UpfrontFee => 0,
        FraudHead::Impersonation => 1,
        FraudHead::IdentityHarvest => 2,
        FraudHead::PersonalEmailRecruiter => 3,
    }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn noisy_or(a: f64, b: f64) -> f64 {
    1.0 - (1.0 - a) * (1.0 - b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jm_common::{FraudScores, ListingRecord};
    use jm_extract::{canonicalize, extract, resolve_as_of};
    use jm_kernel::judge;

    fn compile(listing: &ListingRecord) -> jm_kernel::Judgement {
        let canon = canonicalize(listing, resolve_as_of(listing, None));
        let mut facts = extract(&canon);
        let bundle = analyze(&canon, &mut facts, &Params::default());
        judge(&facts, bundle, &Params::default())
    }

    fn score(listing: &ListingRecord) -> FraudScores {
        compile(listing).fraud
    }

    fn posting(description: &str, apply_destination: &str) -> ListingRecord {
        ListingRecord {
            description: description.into(),
            apply_destination: apply_destination.into(),
            ..ListingRecord::default()
        }
    }

    #[test]
    fn fee_language_fires() {
        let s = score(&posting(
            "Please pay for training before day one.",
            "Career page",
        ));
        assert!(s.upfront_fee > 0.5);
        assert!(s.impersonation < 0.2);
    }

    #[test]
    fn negation_does_not_fire() {
        let s = score(&posting(
            "Do not pay for training. We never ask for a starter kit fee.",
            "Career page",
        ));
        assert!(s.upfront_fee < 0.15, "got {}", s.upfront_fee);
    }

    #[test]
    fn a_negated_phrase_is_recorded() {
        let listing = posting("We never ask for a starter kit fee.", "Career page");
        let canon = canonicalize(&listing, resolve_as_of(&listing, None));
        let mut facts = extract(&canon);
        analyze(&canon, &mut facts, &Params::default());
        assert!(facts.has(|k| matches!(k, FactKind::NegatedPhrase { .. })));
        assert!(!facts.has(|k| matches!(k, FactKind::PhraseHit { .. })));
    }

    #[test]
    fn a_later_unnegated_occurrence_still_fires() {
        let s = score(&posting(
            "Do not pay for training up front. You pay for training in week two.",
            "Career page",
        ));
        assert!(s.upfront_fee > 0.5, "got {}", s.upfront_fee);
    }

    #[test]
    fn non_ascii_text_around_a_hit_does_not_panic() {
        for prefix in [
            "Rejoignez-nous —",
            "Zapłać —",
            "Требуется — ",
            "日本語のテキスト、",
            "é",
        ] {
            let s = score(&posting(
                &format!("{prefix} please pay for training now."),
                "Career page",
            ));
            assert!(s.upfront_fee > 0.5, "{prefix}");
        }
    }

    #[test]
    fn non_ascii_negation_window_still_negates() {
        let s = score(&posting(
            "Attention — do not pay for training.",
            "Career page",
        ));
        assert!(s.upfront_fee < 0.15, "got {}", s.upfront_fee);
    }

    #[test]
    fn personal_email_destination_fires() {
        let s = score(&posting("Apply today", "hr.recruiting@gmail.com"));
        assert!(s.personal_email_recruiter > 0.5);
    }

    #[test]
    fn personal_email_in_the_body_fires_more_weakly() {
        let body = score(&posting("Email hr.recruiting@gmail.com", "Career page"));
        let destination = score(&posting("Apply today", "hr.recruiting@gmail.com"));
        assert!(body.personal_email_recruiter > 0.0);
        assert!(body.personal_email_recruiter < destination.personal_email_recruiter);
    }

    #[test]
    fn clean_listing_is_quiet() {
        let s = score(&posting(
            "Staff engineer on the risk team.",
            "Official career page",
        ));
        assert!(s.max() < 0.1);
        assert!(s.combined < 0.15);
    }

    #[test]
    fn two_heads_raise_combined() {
        let s = score(&posting(
            "Please pay for training and send your social security number to onboard.",
            "Career page",
        ));
        assert!(s.combined > s.upfront_fee);
        assert!(s.combined > s.identity_harvest);
    }

    #[test]
    fn every_fraud_claim_cites_a_fact() {
        let j = compile(&posting(
            "Pay for training and send your ssn. We are hiring on behalf of google.",
            "hr@gmail.com",
        ));
        let fraud: Vec<_> = j
            .surviving_claims
            .iter()
            .filter(|c| c.analyzer == AnalyzerId::Fraud)
            .collect();
        assert_eq!(fraud.len(), 4, "all four heads should have fired");
        for claim in fraud {
            assert!(!claim.premises.is_empty(), "{} cites no fact", claim.id.0);
        }
    }

    #[test]
    fn a_lookalike_company_name_does_not_exempt_itself() {
        // Wording chosen to miss the lexicon, so this measures the brand
        // detector alone.
        let listing = ListingRecord {
            company: "Google Careers LLC".into(),
            description: "An official google recruiter will contact you.".into(),
            origin_url: "https://hiring-portal.example/apply".into(),
            origin_name: String::new(),
            ..ListingRecord::default()
        };
        assert!(score(&listing).impersonation > 0.5);
    }

    #[test]
    fn the_real_brand_is_exempt_when_provenance_agrees() {
        let listing = ListingRecord {
            company: "Google".into(),
            description: "An official google recruiter will contact you.".into(),
            origin_url: "https://careers.google.com/jobs/1".into(),
            origin_name: "careers.google.com".into(),
            ..ListingRecord::default()
        };
        assert_eq!(score(&listing).impersonation, 0.0);
    }

    #[test]
    fn a_brand_that_is_only_a_substring_is_not_the_brand() {
        let listing = ListingRecord {
            company: "Applebee's".into(),
            description: "We are recruiting on behalf of apple.".into(),
            origin_url: "https://careers.applebees.example/1".into(),
            origin_name: "careers.applebees.example".into(),
            ..ListingRecord::default()
        };
        assert!(score(&listing).impersonation > 0.5);
    }
}

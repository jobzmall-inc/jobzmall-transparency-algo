# Warranted-judgment calculus

This repository is a compiler, not a mixer. A listing is lowered to facts,
analyzers emit claims, defeaters remove warrants, and a least-commitment
judge joins serving obligations on a lattice. The public dossier carries
the proof.

## Phases

```
listing JSON
    │
    ▼
as_of           evaluation instant (derived; no wall clock unless --now)
    │
    ▼
canonicalize    dates · URLs · apply targets · ATS class
    │
    ▼
extract         FactSet — every ground atom, with provenance
    │
    ▼
analyze         source · ghost · fraud · stale · reviewer
                (no analyzer reads another analyzer's claims)
    │
    ▼
defeat          undercut (warrant dies) · rebut (conclusion dies)
    │
    ▼
judge           least commitment · lattice join
    │
    ▼
project         ten-field Nutrition Label
    │
    ▼
warrant         proof tree — disposition → head → claim → fact id,
                defeated lines included
    │
    ▼
dossier v2
```

## Least commitment

Unknown is the default. A missing fact is not evidence of openness, of a
source kind, or of innocence. Two equally confident disagreeing source
signals collapse to `unknown` rather than electing a winner — and that
`unknown` still publishes the signals that conflicted, because "unknown"
with nothing behind it is indistinguishable from "nobody looked".

The same rule applies to measurements. An unparseable timestamp yields no
freshness reading, and no reading is not fresh. An absent age is not old
enough to clear a gate.

Every fold reads the multiset of surviving claims rather than walking them
as a rule list, so re-ordering analyzers cannot change a verdict.

## Defeat

After Pollock. Both kinds remove the targeted claim; they differ in what may
happen next.

| Kind | Meaning | Effect | Example |
| --- | --- | --- | --- |
| Undercut | The inference is no longer licensed | The claim is dropped; another claim may still establish the same conclusion | Empty crawl undercuts `source_absent` |
| Rebut | The conclusion is opposed | The whole head is barred; no surviving claim re-establishes it | A closed requisition rebuts every ghost claim |

There is no negative claim. An analyzer that wants to deny a conclusion files
a defeater against it, so the record shows what was taken back and why.

The kernel is told only *that* a head was rebutted — never that a requisition
was closed. Domain knowledge stays in the analyzer.

An undercut fact is still printed on the warrant. Hiding it would look like certainty.

## Lattice

```
Show ⊑ Disclose ⊑ Restrict ⊑ Drop
```

`join` is least-upper-bound and commutative. Analyzer registration order
cannot change the disposition. Drop is absorbing. Auto may restrict;
only a reviewer claim may drop.

## Ghost tiers

Structural claims are type-capped at `possible`. They describe the shape
of a posting. Outcome claims describe what happened to people. `likely`
requires the outcome tier and a distinct-person lower bound at
`ghost_contributor_gate` (published, ≥ 2).

When ingest does not supply witness ids the compiler uses an inclusion
interval: lower = `max(silent, reports)`, upper = `silent + reports`.
`likely` reads the lower bound.

## Fraud heads

Four independent log-odds heads. A published prior (0.02) and temperature
(1.15). A negation inside `fraud_negation_window_bytes` suppresses an
occurrence, and a phrase counts if *any* occurrence escapes negation.
Combined score is noisy-OR of the heads — never mixed with ghost or stale.

Public defaults leave the restrict threshold unset.

## Determinism

No `Utc::now()` on the compile path. `as_of` is the newest parseable stamp
on the listing, or `--now`. Fact and claim stores are `BTreeMap` / sorted
`Vec`. Two identical inputs produce identical dossiers.

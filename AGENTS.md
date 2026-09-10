# AGENTS.md

Guidance for AI agents working in this repository.

## What this is

`jobzmall-transparency-algorithm` is a warranted-judgment compiler for
JobzMall listing provenance: source kinds, the Job Posting Nutrition Label,
ghost / fraud / stale heads, and show / disclose / restrict / drop.

It does **not** rank jobs or people. It does **not** invent a source when
confidence is below the floor, or when two signals disagree at the same
confidence. It is **not** a ranking mixer.

The public renderer is [jobzmall-transparency](https://transparency.jobzmall.com).
This repo produces the record. Read [`docs/CALCULUS.md`](docs/CALCULUS.md)
before changing judge, defeat, or lattice code.

## Layout

| Area | Path |
| --- | --- |
| Shared types / dossier v2 | `common/` |
| Published defaults | `params/` |
| IR + judge + warrant | `kernel/` |
| Canonicalize + extract | `extract/` |
| Compiler driver | `engine/` |
| CLI | `label-mixer/` |
| Ghost analyzer | `ghost/` |
| Fraud language heads | `fraud/` |
| Source liveness | `stale/` |
| Nutrition projector | `nutrition/` |
| Source agreement | `sources/` |
| Reviewer actioning | `enforcement/` |
| Aggregate labels | `public-report/` |
| Synthetic listings | `reference/` |

## Commands

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p label-mixer -- label-mixer/fixtures/stripe-staff-engineer.json
```

Before committing any `*.rs` file: `cargo fmt --all`, then `cargo test --workspace`. Do not commit if they fail.

## Conventions

- English only.
- Surgical changes. Do not invent review SLAs, extra source kinds, or certification language.
- Fraud restrict thresholds stay unset in public defaults unless the user is adding a local policy file.
- Ghost verdicts are compiled when the dossier is built, never stored as a standing score.
- Structural ghost claims are type-capped at `possible`. They can never produce `likely`.
- `contributor_gate = 2` is a privacy *and* abuse guarantee — do not lower it.
- `likely` reads the witness **lower** bound.
- An empty career-page crawl undercuts `source_absent`; it is no coverage.
- Closed jobs rebut every ghost claim. Undercut removes one claim; rebut bars
  the head, and the kernel is never told *which* domain fact did it.
- Humans drop fraud. Machines may only restrict.
- Do not blend fraud / ghost / stale into one score.
- Analyzers emit claims, cite the facts they read as `premises`, and do not
  call each other. They also do not parse URLs, dates, or mailboxes —
  `extract/` owns every vocabulary, including which domains are personal.
- A miss is `unknown`. Absent freshness is not fresh; absent age is not old.
- No `Utc::now()` on the compile path. `as_of` is derived or `--now`.
- Deterministic stores only (`BTreeMap`, sorted `Vec`).
- Folds read the multiset of surviving claims, not a walk order. Re-ordering
  analyzers must not change a verdict.
- Disposition is a lattice join. Do not restore first-match-wins rule order.
- Public wire shapes are pinned by tests. Adding, removing, or renaming a
  dossier field means bumping `DOSSIER_SCHEMA`.

#!/usr/bin/env python3
"""Generate a deterministic synthetic listing world for local mixer runs.

No production data. The Rust mixer is the source of truth; this only writes JSON.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


KINDS = ("live", "marketplace", "indexed", "unknown")


def listing(i: int) -> dict:
    kind = KINDS[i % len(KINDS)]
    confident = kind != "unknown"
    return {
        "id": f"synth-{i:04d}",
        "slug": f"synth-co-{i}-engineer",
        "org_slug": f"synth-co-{i}",
        "job_slug": "engineer",
        "title": f"Engineer {i}",
        "company": f"Synth Co {i}",
        "location": "Remote",
        "description": (
            "Pay for training before day one."
            if i % 11 == 0
            else "Build the product."
        ),
        "origin_name": "careers.synth.example" if confident else "",
        "origin_url": f"https://careers.synth.example/{i}",
        "first_seen": "2026-03-01",
        "last_synced": "2026-08-26",
        "apply_destination": "Official career page" if i % 11 else "hr@gmail.com",
        "work_arrangement": "Remote",
        "compensation_status": "Range published" if i % 2 == 0 else None,
        "jobzmall_url": f"https://www.jobzmall.com/jobs/synth-co-{i}-engineer",
        "closed": False,
        "claimed": kind == "marketplace",
        "suggested_source_kind": kind if confident else None,
        "source_confidence": 0.94 if confident else 0.31,
        "source_http_status": 200 if confident else None,
        "missed_syncs": 0,
        "posting_age_days": 20 + i,
        "date_reset_detected": i % 7 == 0,
        "evergreen": i % 7 == 0,
        "source_absent": False,
        "has_source_coverage": confident,
        "silent_application_contributors": 2 if i % 7 == 0 else 0,
        "report_contributors": 2 if i % 7 == 0 else 0,
        "human_confirmed_fraud": False,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--n", type=int, default=12)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for i in range(args.n):
        path = args.out / f"synth-{i:04d}.json"
        path.write_text(json.dumps(listing(i), indent=2) + "\n")
    print(f"wrote {args.n} listings to {args.out}")


if __name__ == "__main__":
    main()

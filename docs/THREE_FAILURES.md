# Three failures, three responses

Fraud, ghost jobs, and stale jobs get lumped together. Separating them is the point of this repository.

## Fraud — enforcement

A listing designed to steal money, data, or identity, often by impersonating a real employer. That is a crime.

- Heads: upfront fee, impersonation, identity harvest, personal-email recruiter
- Auto may **RESTRICT**
- **DROP** only after a human confirms
- Public defaults leave the restrict threshold unset — supply a local policy to fire

## Ghost — disclosure

A real employer, no active near-term vacancy. Not a felony. Still a waste of time.

- Hedged verdict: `none` / `possible` / `likely`
- Two evidence tiers (structural vs outcome)
- Response is **DISCLOSE**, never silent removal
- A Live Job can still be a ghost between syncs

## Stale — synchronization

A real opportunity that stayed online after the original role closed.

- States: `live_at_source` / `closed_at_source` / `source_not_found` / `status_uncertain`
- First response is sync harder
- If we still serve it — including `source_not_found` — **DISCLOSE**
- A 200 older than `live_job_max_sync_lag_hours` is uncertain, not live
- A 200 with no parseable stamp has no freshness reading at all, and is also uncertain

Do not blend these three heads into one “bad listing” score. That is how a ghost is hidden as low quality and a scam is excused as stale.

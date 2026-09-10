# Job Posting Nutrition Label

Ten fields. On the listing. Before anyone applies.

This is the public contract `nutrition/` projects. The mixer always emits every field. A missing fact becomes an explicit value (`unknown`, `not disclosed`, `none stated`) — never a blank that looks like certainty.

| Field | Meaning | Allowed values (OSS) |
| --- | --- | --- |
| Source | Where we obtained the posting | Career page, ATS name, JobzMall, `unknown` |
| Employer relationship | Who the employer is to us | `official` · `claimed` · `indexed` · `unknown` |
| Original publication date | When the role first appeared | Display date, or `unknown` |
| Last verification date | When we last heard from the source | Display datetime, or `unknown` |
| Application destination | Where the information actually goes | Career page, marketplace, ATS, `unknown` |
| Work arrangement | As stated | On-site / hybrid / remote / `not disclosed` |
| Compensation status | Pay transparency | Published / range published / `not disclosed` |
| AI involvement | Whether AI wrote or rewrote the description | `none stated` / generated / modified |
| Current-opening status | Is it a live opening | `open` / `stale` / `unknown` |
| Reporting mechanism | How to flag it and get a reply | On the posting |

Current-opening status is derived, not stored:

- ghost `none` and source live → `open`
- ghost `possible` or `likely` → `unknown` (disclosed separately)
- stale `closed_at_source` → `stale`
- `source_not_found` or `status_uncertain` → `unknown` (disposition discloses)
- anything else → `unknown`

A logo and a job title are not proof. The fields are what you can inspect.

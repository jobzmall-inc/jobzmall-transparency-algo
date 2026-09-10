# Example parameter change: ghost contributor gate

When a published gate changes, the diff should make the reason obvious.

`ghost_contributor_gate` is the number of distinct people who must have contributed outcome evidence before the classifier may say `likely`. It is also the count below which the serving layer withholds the witness number, so a single applicant cannot be deanonymized to the employer.

Lowering it breaks a privacy guarantee and an abuse guarantee at once.

The compiler and the serving projection both read this one number. There is not a second copy in `ghost/` or `engine/`. `likely` reads the witness **lower** bound (exact id count, or `max` of the two channel counters).

## Illustrative change

After coordinated false reports we considered raising the gate from 2 to 3. Most traffic stayed at 2.

```diff
--- a/params/src/lib.rs
+++ b/params/src/lib.rs
@@
     pub ghost_contributor_gate: u32,
@@
         Self {
             ghost_convergence: 2,
-            ghost_contributor_gate: 2,
+            ghost_contributor_gate: 3,
             ghost_absence_stamp_max_age_days: 14,
             ghost_evergreen_age_days: 270,
```

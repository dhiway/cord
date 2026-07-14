# Orbis evidence v5 delta approval — Architect

Approved baseline: `317a3a5b3dda0964958e08b94c683b1cc1b8e387`

The Architect approves a manifest-version-5 transition limited to the following delta:

1. Correct PAL-097/PAL-099 and BENCH-041/BENCH-043 to name the local Orbis packages and separately
   retain the Individuality upstream package/target.
2. Substitute the legacy Score and Honour migration IDs with local Orbis migration IDs, recording
   local package, upstream package, storage version 1, and `present-initial-version`.
3. Add present evidence rows `S2-SURFACES-01`, `S2-FIXTURES-01`, `S2-PAYOUT-01`,
   `S2-BENCHMARK-01`, and `S2-MIGRATIONS-01`.
4. Add `GATE-6-SLICE2-EVIDENCE` as pending, without implementation or review evidence.

No other existing row body, state, or status may change; no existing row may be removed. All v4
artifacts and Gates 1–5 remain inherited. The source/test marker commit may contain only the v5
inventory, marker registry, and aggregate test emitters. The evidence-review phase remains pending
after the transition. Exact pending counts: 651 total = 443 present + 140 planned + 68 excluded.


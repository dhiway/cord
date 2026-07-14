# P1 canonical materialization and upgrade candidates

These tools prepare prerequisites; they do not authorize a network launch or manufacture an
acceptance result.

## Higher-spec candidates

Production remains Origin spec `9901` and Orbis spec `29`/transaction `8`. The
`p1-upgrade-candidate` Cargo feature is opt-in and changes only an isolated candidate build to
Origin `9902` or Orbis `30`. Orbis must additionally use `fast-runtime`.

From a clean, reviewed commit, with absent/empty output and target directories:

```sh
python3 scripts/test-build-p1-upgrade-candidates.py
python3 scripts/build-p1-upgrade-candidates.py --execute \
  --output /durable/p1/upgrade-candidates \
  --target-root /dedicated/p1-upgrade-target
```

The builder fails unless `subwasm` decodes the expected higher runtime version. It emits two
hash-bound Wasm files, build logs, decoded runtime information, and `manifest.json`. A dirty tree,
missing tool, non-higher version, production-version edit, ambiguous artifact, or build failure
leaves the manifest non-authoritative and non-pass.

## Materializer input ledger

`materialize-p1-verification.py` accepts one JSON ledger with schema
`cord.p1-materializer-inputs.v1`. Paths may be absolute or relative to the ledger. Every path has a
lowercase SHA-256 binding.

Required top-level keys are:

- `source`: the exact Git `commit` and exactly one file binding for each role
  `workspace_lock`, `origin_runtime`, `orbis_runtime`, `control_runner`, `control_scenarios`,
  `smoke_runner`, `proof_campaign_runner`, `proof_case_driver`, and `proof_verifier`;
- `ac7_ac8`: control/broker raw result, scenario and authoritative candidate manifests, generated
  topology, exact Origin/Orbis/driver/candidate-Wasm binaries, and fixed current/candidate runtime
  identities;
- `ac10`: raw smoke JSON, checked-in topology, exact Origin/Orbis binaries, current runtime
  identities, and `unincluded_segment_capacity: 12`;
- `ac13`: production campaign verdict, proof-verifier verdict, evidence root, topology, case driver,
  every binary from the campaign ledger, and the exact Orbis metadata hash.

The validator re-hashes source, binaries, candidates, topology, raw ledgers and every referenced
raw artifact. It rejects prepared/blocked/failed results, incomplete scenario sets, non-finalized or
unbound control cases, stale runtime identities, fewer than ten storage cases, cleanup failure, and
any false or mismatched storage/weight/length/proof/DB/finality operand. P1 AC13 is recomputed from
the capacity sample files rather than copied from the campaign aggregate.

Run without a review first:

```sh
python3 scripts/test-materialize-p1-verification.py
python3 scripts/materialize-p1-verification.py \
  --inputs /durable/p1/materializer-inputs.json \
  --output-dir docs/evidence/verification/p1
```

A valid run atomically writes the four canonical verdicts but returns `2` and does **not** create
`index.json`. An independent reviewer then supplies a JSON document with schema
`cord.p1-independent-review.v1`, `status: pass`, `independent: true`, reviewer identity, exact source
commit, and hashes of all four canonical files. Re-run with its hash-bound path:

```sh
python3 scripts/materialize-p1-verification.py \
  --inputs /durable/p1/materializer-inputs.json \
  --output-dir docs/evidence/verification/p1 \
  --review /durable/p1/independent-review.json
```

Only that second successful validation atomically emits `docs/evidence/verification/p1/index.json`.
Missing, PREPARED, BLOCKED, INVALID, FAIL, tampered, stale, or partially reviewed evidence never
becomes PASS and never creates the index.

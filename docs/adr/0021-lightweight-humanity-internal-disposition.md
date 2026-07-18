# ADR 0021: Lightweight humanity runtime disposition

## Status

Accepted.

## Context

Commons has a lightweight recognition state machine at runtime index 94. The unified Identity
product facade must not expose that implementation as a second permission, package, route or
credential taxonomy. Deleting the state machine is nevertheless unsafe: its governed verifier
quota, atomic account-and-ring-key possession check, unique lightweight membership, consented
consumer registration and alias-origin execution are not jointly reproduced by Identity, People,
Members or the full recognition pallet.

### Decision: invariant-proven internal retention

Retain the pallet and its signed extension as runtime internals until every distinct invariant in
`docs/specs/web3-storage-disposition-v1.toml` has a tested replacement. The public developer
contract is only the seven scoped `identity.*` operations and separately consented
`transaction.sign`. In particular, lightweight enrolment, verifier allowance and category-specific
status are not public Product SDK operations.

Raw FRAME metadata and the signed-extension transaction layout may contain internal implementation
names. This is accepted operator visibility, not SDK admission. The disposition ledger enumerates
each accepted internal surface. Product descriptors may carry the exact signed-extension layout for
transaction construction, but no public route, permission, package or example may be derived from
it. A metadata filter, string split, alias route or compatibility shim is prohibited.

Shared mechanics do not justify deletion. Members already owns ring construction and proof
verification, and the full recognition pallet already implements contextual alias bookkeeping, but
the lightweight pallet binds those mechanics to a different governed onboarding and origin model.
Those shared rows are recorded as reproduced dependencies while the distinct rows keep the
component internal.

## Consequences

- Applications use `identity.humanity.status` and `identity.humanity.prove`; they cannot invoke the
  lightweight enrolment call or read verifier allowance through the product facade.
- Runtime operators can inspect and govern the retained state through normal FRAME tooling.
- Removing the pallet requires a new accepted ADR proving every `distinct` row reproduced and must
  delete the runtime index, extension, configuration, storage, weights and tests together.
- Origin and Commons are new networks; no compatibility route or data migration is authorized.

## Verification

Run:

```text
python3 scripts/validate-product-projection.py --mode people-lite-disposition \
  --ledger docs/specs/web3-storage-disposition-v1.toml --out /tmp/ac9-assertions.json
python3 -m unittest scripts.tests.test_product_projection
```

The first command must report zero public old taxonomy, zero internal projection and zero metadata
filter shims while proving the accepted internal-retain branch and every invariant/source marker.

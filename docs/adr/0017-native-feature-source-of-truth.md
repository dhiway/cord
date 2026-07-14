# ADR 0017: Native feature source-of-truth reconciliation

## Status

Candidate, independently reviewed for architectural corrections but not yet ratified through the
manifest transition/product/evidence gates. Production activation remains blocked on the explicit
P1/P6/P7 validation and approval gates.

## Context

The CORD-owned implementation now contains the native identity, attestation, DotNS,
storage/provider/Drive/S3, sponsored transaction, and Broker control surfaces required by the
enterprise app model. The completion manifest still carried older planning rows that implied a
future code adoption from the immutable Web3 Storage reference and a future Game pallet. It also
omitted the stack-owned `CoretimeControl` pallet even though Origin and Orbis compose it at stable
index 221.

Those rows conflict with the CORD-only and clean-break decisions. External repositories are semantic
references, not writable implementation targets or runtime dependencies. Game is outside the
enterprise-first scope. A stale planning entry is not a compatibility promise and must not survive
as a second implementation path.

## Decision

1. Manifest version 17 records `pallet-coretime-control` as a CORD-owned pallet at index 221 and
   declares storage version 1 as the initial clean-genesis schema. No predecessor hook, legacy state
   import, ABI facade, or data migration is permitted.
2. The frozen Web3 Storage crate/module inventory is classified `excluded`. Its semantics remain
   provenance for the CORD-owned pallets and provider node; none of its code is planned for copying,
   packaging, or runtime composition.
3. Game remains excluded from the enterprise launch. Re-admission requires a separate approved
   product journey and a manifest-version change.
4. Token, Feeless and CoretimeControl benchmark definitions are wired into their applicable runtime
   registries. Conservative/current weights remain candidate-only and must be regenerated after the
   final accepted source diff in P7. Origin benchmarks only the provider-side CoretimeControl call;
   Orbis benchmarks the requester/receipt/control calls under the matching runtime feature profile.
   CoretimeControl benchmark setup fills bounded ledgers to their configured maximum, and held
   release uses an explicit bounded FIFO rather than a storage-read scan. Terminal receipts and
   bounded-history pruning atomically remove held entries so a stale head cannot wedge later work.
5. The runtime-alignment ledger owns the exact current Origin and Orbis pallet inventories, including
   both index-221 compositions. The historical manifest-v5 evidence remains historical and does not
   override the current candidate manifest.

## Consequences

The feature ledger no longer advertises an external code-adoption lane or an unapproved Game pallet.
The remaining gaps are validation/release work: live Broker/XCM and proof campaigns, final E/Q/C and
resource headroom, generated final weights, reproducible artifacts, soak/recovery, security review,
and explicit activation approvals. This ADR does not claim those gates or production readiness.

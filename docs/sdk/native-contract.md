# Native SDK contract

`native-version-matrix.json` retains the historical first-supported SDK inventory for the
clean-break Origin and Orbis network. Origin is the relay and deterministic control plane at
spec/transaction `9901/2`. The matrix's Orbis `29/8` identity and metadata hash
`0x50c8958f…dc45` are historical inputs only. The current Orbis source runtime is para `1006`,
spec/transaction `33/8`, with metadata hash `0x824731ed…f8e`; no metadata-bound native SDK is
admitted for that source runtime yet.

`origin-rs` and `product-sdk` both publish release `0.9.9` and retain those values in executable
source. The matrix records adopted runtime APIs, pallet storage schemas, the Orbis Names label
policy, and the provider protocol version. `npm --prefix product-sdk run validate:sdk-freeze`
checks the structural route policy and explicitly refuses to generate authoritative SDK evidence
while current-runtime metadata admission is false. The obsolete spec-29 coverage PASS report has
been removed.

The fail-closed network identity is the exact deterministic candidate genesis header
`0x2584c9d4…70fc`, bound through `docs/genesis/orbis-candidate-genesis-identity.json` at artifact
SHA-256 `bfac0f6c…c9e0`. Its frozen activation state is `candidate-pending` with
`production_activation_ready=false`. Rust callers must explicitly choose
`NetworkIdentity::orbis_candidate()` and TypeScript callers must explicitly choose
`ORBIS_CANDIDATE_NETWORK_BINDING`. Production access rejects this identity until a checked-in,
cryptographically verified activation envelope derives `production-approved`; the current unsigned
P5 envelope does not satisfy that gate.

`native-route-contract.json` is the authoritative typed host-route inventory and now contains 136
methods. Its generated TypeScript projection is updated independently of the network-bound
descriptor, which remains fail-closed until the live Commons metadata identity is reconciled. The
host request schema's route projection is reproducibly refreshed with
`generate-descriptor.ts --host-schema-only` while preserving its frozen network binding.
Each entry binds ordered parameters, result/finality, Rust query or command variant, TypeScript
callable, runtime API or pallet call, and the pallet/call indices used by current dispatch tables.
Rust and TypeScript harnesses execute all 136 canonical request samples. The checked-in Commons V14
SCALE metadata and byte-reproducible `polkadot-api` output are historical spec 29 inventory. They do
not bind the current spec 33 source runtime, so native SDK admission stays disabled until metadata is
regenerated from that runtime. The route contract remains an independent product-policy projection:
its closed payload schemas and canonical Rust/TypeScript factories describe the intended app methods
without claiming current metadata binding.
Product methods use exact, closed payload shapes and metadata-resolved transports. Raw SCALE,
pallet/call indices, migrated
domain Revive calls, contract ABIs, and contract-address aliases are not reference SDK surfaces.

The contract-to-native map is design coverage only, never a compatibility facade. It classifies
2,780 source-semantic design entries; 14 adopted semantic bindings are exact M5 bindings and the
remaining entries record intentional changes, retirements, or non-applicable source semantics.
Executable coverage is reported separately as 136 distinct Rust and TypeScript route cases. Retired and
not-applicable source symbols stay explicit in the census, but no legacy client, contract facade,
backward-compatibility layer, or data-migration path is shipped. This is a new network.

`generated/orbis-descriptor.json` is an evidence-bound snapshot, not an active route-admission
surface. It can retain retired provider method names until the current spec-33 metadata record is
reconciled with the spec-29 frozen SDK manifests and the full descriptor/evidence set is regenerated;
the full generator rejects that mismatch instead of silently rewriting the evidence snapshot.

The P5 ratification envelope binds the matrix, coverage map, native semantic vectors, descriptor,
host schema, and existing policy contracts. It is intentionally unsigned and the candidate genesis
is explicitly not production-approved. Production activation remains blocked until fresh owner signatures and a
separate final-genesis campaign; this SDK freeze does not claim production readiness.

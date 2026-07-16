# Native SDK contract

`native-version-matrix.json` is the canonical first-supported SDK/runtime contract for the
clean-break Origin and Orbis network. Origin is the relay and deterministic control plane at
spec/transaction `9901/2`; Orbis is the native application parachain at para `1006`,
spec/transaction `29/8`. The Orbis metadata identity is
`0x50c8958f…dc45`, reproduced from the current runtime Wasm with
`origin/orbis/runtime/tools/reproduce-metadata-hash.sh`.

`origin-rs` and `product-sdk` both publish release `0.9.9` and freeze those values in executable
source. The matrix also freezes each adopted runtime API, pallet storage schema, Orbis Names label policy,
and provider protocol version. `npm --prefix product-sdk run validate:sdk-freeze` verifies the
matrix against the runtime sources, both SDK exports, the generated descriptor, and
`sdk-native-coverage.report.json`.

The fail-closed network identity is the exact deterministic candidate genesis header
`0x2584c9d4…70fc`, bound through `docs/genesis/orbis-candidate-genesis-identity.json` at artifact
SHA-256 `bfac0f6c…c9e0`. Its frozen activation state is `candidate-pending` with
`production_activation_ready=false`. Rust callers must explicitly choose
`NetworkIdentity::orbis_candidate()` and TypeScript callers must explicitly choose
`ORBIS_CANDIDATE_NETWORK_BINDING`. Production access rejects this identity until a checked-in,
cryptographically verified activation envelope derives `production-approved`; the current unsigned
P5 envelope does not satisfy that gate.

`native-route-contract.json` is the authoritative typed host-route inventory and now contains 138
methods. Its generated TypeScript projection is updated independently of the network-bound
descriptor and host schema, which remain fail-closed until the live Commons metadata identity is
reconciled.
Each entry binds ordered parameters, result/finality, Rust query or command variant, TypeScript
callable, runtime API or pallet call, and the pallet/call indices used by current dispatch tables.
Rust and TypeScript harnesses execute all 138 canonical request samples. The checked-in Commons V14
SCALE metadata is extracted from the current runtime Wasm and drives a byte-reproducible
`polkadot-api` descriptor build. The route contract remains the authoritative product-policy
projection: its closed payload schemas and canonical Rust/TypeScript factories deliberately expose
only admitted app methods, while the generated PAPI descriptor retains the complete metadata truth.
Product methods use exact, closed payload shapes and metadata-resolved transports. Raw SCALE,
pallet/call indices, migrated
domain Revive calls, contract ABIs, and contract-address aliases are not reference SDK surfaces.

The contract-to-native map is design coverage only, never a compatibility facade. It classifies
2,780 source-semantic design entries; 14 adopted semantic bindings are exact M5 bindings and the
remaining entries record intentional changes, retirements, or non-applicable source semantics.
Executable coverage is reported separately as 138 distinct Rust and TypeScript route cases. Retired and
not-applicable source symbols stay explicit in the census, but no legacy client, contract facade,
backward-compatibility layer, or data-migration path is shipped. This is a new network.

The P5 ratification envelope binds the matrix, coverage map, native semantic vectors, descriptor,
host schema, and existing policy contracts. It is intentionally unsigned and the candidate genesis
is explicitly not production-approved. Production activation remains blocked until fresh owner signatures and a
separate final-genesis campaign; this SDK freeze does not claim production readiness.

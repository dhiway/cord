# Native SDK contract

Runtime metadata plus versioned runtime APIs are the exact chain truth. The executable P0 TypeScript foundation is in `product-sdk/`; `npm --prefix product-sdk test` validates the bootstrap descriptor, runtime fixtures, host boundary and E/Q/C contracts. The generated bootstrap descriptor is explicitly **not** a production PAPI domain descriptor.

The P0 TypeScript client is bound fail-closed to Orbis para 1006, spec 29, transaction 8, metadata hash `0x851955…9ef9`, the checked-in chain-spec source hash, descriptor-contract digest, and the explicitly unfinalized P0 fixture identity. The Rust client is a target only until it implements the same exact runtime-identity guard. The canonical envelope separates P0-target ratification from production activation: five external Ed25519 roles may ratify targets before final genesis, while production remains blocked on a later final-genesis/campaign payload. Empty, malformed, duplicate-role, unregistered, expired, revoked or invalid signatures never approve it.

Every product method uses a method-discriminated, closed payload. Recursive key normalization rejects SCALE, ABI, deployment/contract address and Revive-contract aliases at any depth; applications receive neither raw pallet indices nor a contract compatibility facade. Host permission, consent, signing and transport remain host-owned. Revocation is checked before and after signing, cancellation races deferred work and emits one terminal outcome, and composite reads use one finalized hash.

Every failure serializes as `native-error-v1.json`; lifecycle output follows `native-lifecycle-v1.json`. Retries preserve the intent ID. Rust/Subxt and future production TypeScript/PAPI domain clients must share identifiers, lifecycle transitions, exact current vector outcomes and finalized-hash semantics.

E/Q/C P0 execution validates targets, schema closure, topology/runtime identity and threshold recomputation only. It reports `performance_claim=false`. Network campaigns and any performance verdict remain prohibited until ratification and their P1/P6 gates.

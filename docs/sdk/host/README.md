# Fake-host contract

`host-request.schema.json` is the transport-neutral request contract and `fake-host-scenarios.json` is the mandatory hostile suite. The fake host must model desktop, iOS and Android without importing platform keys: a deterministic in-memory signer, permission ledger, consent clock, cancellation token and scripted transport are injected.

The host checks application scope and active consent before signing; binds request/application/network/spec/tx version; rejects replay; propagates cancellation exactly once; pins finalized reads; and reports typed transport/runtime/permission errors. It never administers chain roles or supplies raw SCALE/contract ABI product calls.

The P5 schema is generated from
`product-sdk/packages/descriptors/src/native-methods.ts`. It freezes every P2-P4 native
attestation, Orbis Names, Orbis Storage/provider-reference, provider, Drive and S3 method, its exact closed
payload field set, and its required `finalized` or `submit-and-finalize` route. Value types and
bounds are enforced by `product-sdk/packages/core/src/contract.ts`; no request accepts runtime-
encoded bytes, pallet indices, or migrated-domain ABI fields.

The transport-neutral harness is executable with `npm --prefix product-sdk run test:host`.
`product-sdk/packages/host/src/network-host.ts` supplies the CORD-owned typed network seam: exact
finalized-hash reads, runtime/descriptor binding, metadata-derived submission finality,
cancellation, and typed error/lifecycle mapping. Concrete generated PAPI clients and domain route
registration are injected; the adapter deliberately exposes no raw SCALE, storage-key, pallet-
index, or call-index fallback.

The schema and bootstrap descriptor are bound to the unsigned P5 SDK-freeze envelope; real
signatures from all required ratifier roles remain mandatory. Production desktop/iOS/Android
integration and journey evidence remain later activation work.

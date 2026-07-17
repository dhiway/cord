# Origin/Orbis native bootstrap

This procedure launches the new network directly on native Origin/Orbis state. It has no export,
import, checkpoint translation, contract deployment, restored address, dual write, compatibility
facade, or fallback path. The machine procedure is
[`origin-orbis-native-bootstrap.json`](origin-orbis-native-bootstrap.json); operators record the
proposal hash, approvers, exact runtime/metadata/descriptor hashes, finalizing block and post-check
for every step.

## Preconditions

1. Verify the checked-out release is `sm-update-sub-0x63`, the release bundle is independently
   approved, and Origin/Orbis WASM, chain specs and SDK descriptors match that bundle.
2. Fetch the finalized Origin and Orbis heads. Fail closed unless Origin reports spec 9901 and
   Orbis reports spec 29, transaction 8, para 1006, and the generated Product SDK network binding.
3. Confirm the key ceremony assigned distinct runtime, registrar, issuer, provider-governance, provider-service and
   provider-account keys. Secrets remain in an HSM/secret store, never JSON,
   shell history, logs or repository files.
4. Confirm every alert in `origin-orbis-monitoring.json` is loaded and links to an exercised
   incident action before admitting application traffic.

## Native domain registration

Execute the manifest steps in order and wait for finality after each write. Orbis Names registrars use
`names.set_registrar`; attestation issuers create an approved schema through
`attestation.create_schema`; provider governance uses `storage.register_provider`. Read the result back at the exact finalizing block. Never substitute
raw SCALE, a pallet/call index, a Revive call or a remembered deployment address.

The registration record includes actor and payer, scope, expiry, proposal/quorum, input hash,
extrinsic hash, finalizing block hash and the exact bounded values read back. A mismatch stops the
sequence; it does not continue with guessed state.

## Provider start and acceptance

Validate that the on-chain provider account, Ed25519 service key, endpoint and capacity exactly
match the service configuration. Start `origin-orbis-provider` and its
`origin-orbis-provider-outbox` finality consumer using the commands and secret environment names in
the JSON manifest. TLS and client identity terminate outside the provider process; do not expose
the bearer-protected listener directly.

Admission completes only when authenticated `/info`, `/stats`, `/replica/sync_status` and the
native `provider_by_id`, `provider_checkpoint` and `provider_root` reads agree at a finalized head.
The operator must also prove that a native heartbeat finalizes and its receipts-v3 journal is
durable. Any identity/key/root mismatch is an incident, not a bootstrap warning.

## Reference application

Run:

```sh
npm --prefix product-sdk run validate:reference-app
npm --prefix product-sdk run test:host
```

The Festival-like reference manifest grants only enumerated native methods. The executable
bootstrap proves active expiring consent, read/write caller routing, cancellation and revocation.
It exposes no migrated-domain contract call, deployment address, generated contract binding,
runtime encoding or pallet index.

## Stop and recovery

Before genesis, correct the signed inputs and regenerate. After genesis, suspend the affected
native registrar/issuer/provider or enter SafeMode, retain audit evidence and ship a normal
forward fix. Never restore an old contract path or import old-network state.

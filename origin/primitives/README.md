# cord-primitives

This crate exposes the shared data-transfer objects and helpers that span the
CORD runtime, pallets, and SDKs. The view-authorization helpers form the core
contract between off-chain clients and the runtime. The sections below
summarize the canonical payload layout, replay rules, and error taxonomy.

## View Authorization Payload (v1)

Runtime views expect an `Authorization<AccountId, Payload, Signature>` where
`Payload` follows this compact layout:

```
+-------------+--------+----------------------------------------------+
| Field       | Bytes  | Description                                  |
+=============+========+==============================================+
| digest      | 16     | `xxhash128(nonce || context || account || B)`|
| account     | 32     | SCALE-encoded signer `AccountId32`.          |
| ref_block   | 4      | Little-endian block number `B`.              |
+-------------+--------+----------------------------------------------+
```

- **Nonce**: arbitrary caller-provided entropy (SDKs default to 48 random
  bytes). It is not stored on-chain; only its 16-byte digest participates in the
  payload to keep signatures small.
- **Context binding**: `context` is the 16-byte tag returned by
  `AuthorizationBuilder::view_context("Pallet", "view_fn")`. While the runtime
  does not currently inspect it, all SDKs include the pallet/view pair to reduce
  accidental reuse and to prepare for future context validation.
- **Reference block + TTL**: the trailing block number represents when the SDK
  assembled the authorization. Pallets enforce
  `current_block < reference_block + ViewAuthorizationTTL`, so clients should
  fetch a fresh reference height shortly before invoking a view.
- **Size limit**: the payload must stay under `MaxAuthorizationLen`
  (256 bytes on Origin chains). The layout above occupies 52 bytes, leaving room
  for future extensions.

## Signature and Replay Rules

1. Compute the payload using the layout above, hashing with xxHash-128.
2. Sign the full payload byte array with the account’s multi-signature key.
3. Submit `Authorization { account, payload, signature }` alongside the typed
   view arguments.
4. Pallets validate the TTL (`reference_block + MaxAuthorizationTTL`) and the
   signature over the payload, then perform the requested view.

The SDK exposes `AuthorizationBuilder` utilities that take a Subxt signer,
construct the payload, and return the ready-to-use authorization structure.

## Error Codes

View functions return one of the following machine-readable codes whenever the
authorization fails:

- `AUTH_FAILED` – signature or signer data invalid.
- `EXPIRED` – the payload TTL elapsed.
- `REPLAY` – the `(account, payload, signature)` triple was already used.
- `INVALID_CONTEXT` – the payload was crafted for another pallet/view.
- `INVALID_REQUEST` – the payload’s request bytes do not match the provided
  arguments.
- `PERMISSION_DENIED` – caller lacks the required rights for the resource.
- `NOT_FOUND` – referenced resource is missing (when returned as an error rather
  than a `None`).
- `INTERNAL` – unexpected runtime failure.

Each error serializes as `CODE|short human-readable message`. The SDK surfaces
`AuthorizationError` directly so client applications can branch on the `code` while
logging the friendly detail.

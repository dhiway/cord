# cord-primitives

This crate exposes the shared data-transfer objects and helpers that span the
CORD runtime, pallets, and SDKs. The view-authorization helpers form the core
contract between off-chain clients and the runtime. The sections below
summarize the canonical payload layout, replay rules, and error taxonomy.

## View Authorization Payload (v1)

All view invocations must carry a `Authorization<AccountId, Payload,
Signature>` where `Payload` is a byte buffer produced via
`PayloadBuilder::build`. The binary layout is:

```
+------------+--------+------------------+-----------------+------------+---------------+
| Field      | Bytes  | Description      | Notes                            |
+============+========+==================+==================================+
| magic      | 8      | `CORDVIEW`       | Guards against accidental reuse. |
| version    | 1      | `0x01`           | Payload spec version.            |
| context    | 16     | blake2_128 hash  | `pallet || ':' || view`.        |
| expiry     | 8      | little-endian    | Absolute block number deadline.  |
| nonce_len  | 1      | u8               | Length of `nonce`.               |
| nonce      | var    | opaque bytes     | Unique caller-provided message.  |
| req_len    | 4      | little-endian    | Length of `request_bytes`.       |
| request    | var    | SCALE bytes      | Canonical encoding of view args. |
+------------+--------+------------------+----------------------------------+
```

- **Context binding**: the `context` hash is `blake2_128(pallet || ':' || view)`
  via `view_context_hash`. Pallets recompute it for each view and reject
  payloads with mismatched contexts.
- **Expiry**: `expiry` is the block number (u64) after which the request is
  invalid. A request is valid while `current_block <= expiry`. Clients should set
  the expiry to `current_block + ttl`, where `ttl` is a small buffer (e.g., 25
  blocks) depending on latency requirements.
- **Unique message**: the `nonce` is caller-controlled entropy. The SDK uses 32
  random bytes by default but any ≤255 byte sequence is accepted.
- **Request binding**: `request_bytes` must equal the SCALE encoding of the view
  arguments (excluding `auth`) in metadata order. Helper methods in the SDK
  reuse Subxt’s typed encoders to keep this canonical.
- **Size limit**: the runtime enforces `Payload::len() <= MaxAuthorizationLen`
  (currently 512 bytes in Origin runtimes). Builders should clamp requests
  before signing.

## Signature and Replay Rules

1. Compute the payload using the builder above.
2. Sign the entire payload byte array with the account key (MultiSignature).
3. Submit `Authorization { account, payload, signature }` alongside the
   typed view arguments.
4. Pallets recompute `blake2_256(account || payload || signature)` and maintain a
   bounded `(account, context)` replay window. Replays within the window return
   `AuthorizationErrorCode::Replay`.

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

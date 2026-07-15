# Storage encryption contract v1

The only admitted encrypted envelope is
`0x01 || nonce[24] || XChaCha20-Poly1305(ciphertext || tag[16])` using a 32-byte key. Stored length is
plaintext length plus 41. Associated data is canonical SCALE
`(network_genesis_hash:[u8;32], bucket_id:[u8;32], object_purpose:ObjectPurposeV1, plaintext_len:u64, key_version:u32)`. `ObjectPurposeV1` is a SCALE one-byte enum: `content=0`, `manifest=1`, `drive=2`, `s3=3`, `app_bundle=4`; lengths and versions use little-endian SCALE fixed integers. Encryption occurs
before CID and chunk construction, so the CID addresses the complete envelope. Reuse of a nonce with
the same key returns non-retryable `211 ENCRYPTION_NONCE_REUSE` before encryption or addressing.

The CORD host holds per-bucket keys. An app receives encrypt/decrypt intent operations, not raw keys.
Export/import requires fresh user consent and explicit wrapping. Rotation creates a new envelope/CID
and atomically changes references only after the new object is publishable. Events, provider APIs,
logs, and metrics may contain algorithm and key version but never keys, plaintext, user-to-nonce
mapping, or secret AAD.

Cross-language vectors MUST freeze key, nonce, SCALE AAD, plaintext generator, envelope digest/CID,
and literal envelope bytes for compact cases. Large boundary vectors use a named deterministic byte
generator plus length and digest so the exact bytes are reproducible without embedding 64 MiB JSON.
`streaming-aead`, `multi-recipient`, `hardware-keystore`, and `transparent-recovery` remain
`design-only`; they are not parity-complete until implemented or approved-excluded by P7.

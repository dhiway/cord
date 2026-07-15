# Storage content contract v1

## Canonical address and bounds

Launch writes MUST use CIDv1 base32lower, raw multicodec `0x55`, and BLAKE2b-256 multihash
`0xb220` with a 32-byte digest. Verification is BLAKE2b-256 over the complete stored object. DAG-PB,
SHA-2, alternative textual encodings, and compatibility decoders MUST NOT be accepted for writes.
Encryption, when selected, precedes addressing and chunking.

The stored chunk size is 262,144 bytes except the final chunk. A stored object is at most 67,108,864
bytes and 256 chunks. `none` permits 67,108,864 plaintext bytes. `xchacha20poly1305-v1` adds exactly
41 stored bytes and therefore permits 67,108,823 plaintext bytes. Empty `none` content has zero
chunks; encrypted empty content is one 41-byte envelope. A manifest is at most 4,194,304 stored bytes
(4,194,304 plaintext for `none`, 4,194,263 for v1 encryption). One application bundle is at most
4,096 objects. `storage-bounds-v1.toml` is the machine authority.

## Streaming and idempotency

A write descriptor is `(operation_id, bucket_id, expected_cid, object_len, chunk_index, bytes)`.
Indices start at zero and MUST be contiguous. A sender may have at most four chunks and 1,048,576
bytes unacknowledged. Admission MUST reject an impossible plaintext length before encryption and MUST
commit no partial object on out-of-order, oversized, missing, length-mismatched, or CID-mismatched
input.

`(bucket_id, operation_id)` is the idempotency key. A retry with byte-identical descriptor and chunks
returns the byte-identical prior receipt. A changed byte, declared length, or CID returns
`STORAGE_IDEMPOTENCY_CONFLICT`; it MUST NOT create a second effect.

## Reads and lifecycle

Every returned chunk, reconstructed length, and complete digest MUST verify before any bytes are
released. Ranges are half-open `[start,end)`, satisfy `0 <= start <= end <= object_len`, and return at
most 4,194,304 bytes. Invalid ranges return `STORAGE_RANGE_INVALID`. Corruption returns
`STORAGE_INTEGRITY_FAILED`, emits bounded evidence, and returns no object bytes.

A provider may call an object **durably readable** only after atomic rename/fsync and a signed receipt.
The platform may call it **publishable** only after a finalized bucket checkpoint includes its MMR
leaf and the primary plus two replicas confirm the same commitment. Upload completion MUST NOT be
represented as publishability. Plaintext length is authenticated host/manifest metadata; provider
byte counts, receipts, ranges, and CIDs always cover stored bytes.

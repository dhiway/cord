# Origin Orbis native provider

`origin-orbis-provider` is the CORD-owned off-chain byte/proof service for the native Orbis
`StorageProvider`, `Drive`, `S3`, and Orbis Storage `TransactionStorage` composition. Orbis runtime
state remains authoritative. The process refuses commits unless `StorageProviderApi` confirms at
one exact finalized hash that:

- the configured provider is active;
- the local sr25519 service key equals the registered provider service key;
- the agreement is active, unexpired, belongs to the provider, and matches the exact byte length;
- the agreement content commitment equals `blake2b-256(raw_content_bytes)`.

Protocol v4 uses the 32-byte raw Blake2b-256 digest in JSON and runtime calls. A CID-facing adapter
must declare a CID multicodec/multihash configuration whose digest is that exact Blake2b-256 value;
this service does not silently translate SHA-256, Blake3, UnixFS, or DAG commitments.

## Process boundary

The provider is a companion binary, not an alternative runtime or a fork of the pinned SDK. It
reads `StorageProviderApi::{provider,agreement,challenges_at}` using `state_call` at
`chain_getFinalizedHead`. Proof duties and deletion acknowledgements are written to an fsynced,
typed JSONL outbox. The included `origin-orbis-provider-outbox` process consumes that seam through
the existing `origin-rs` Orbis metadata-derived signer/nonce/finality pipeline and submits native
`StorageProvider::{submit_checkpoint,commit_provider_root,acknowledge_deletion}` calls. It records an fsynced finalized
block/extrinsic receipt before treating an idempotency key as complete. Neither process embeds a
pallet/call index, signed extension, nonce, governance key, or raw SCALE payload.

Required secrets are read from environment variables (defaults:
`ORBIS_PROVIDER_BEARER_TOKEN` and `ORBIS_PROVIDER_SERVICE_SURI`) and are not persisted. The bearer
token must contain at least 32 bytes. Mutating, content-read, proof, and replica endpoints require
the bearer token. TLS and external client identity are expected at the deployment proxy.

## HTTP surfaces

The bounded v4 routes are:

- operations: `GET /health`, `GET /info`, `GET /stats`, `GET /node`, `PUT /node`;
- content: `POST /exists`, `POST /commit`, `GET /read`, `GET /commitment`, `POST /delete`;
- proofs: `GET /mmr_proof`, `GET /chunk_proof`, `GET /mmr_peaks`, `GET /mmr_subtree`,
  `POST /fetch_nodes`;
- checkpoints: `GET /checkpoint-signature` (latest only), `POST /checkpoint/sign`,
  `GET /checkpoint/duty`;
- replicas: `GET /buckets`, `GET /replica/historical_roots`, `GET /replica/sync_status`.

`POST /delete` is fail-closed: the same finalized provider and agreement checks run again and the
agreement must already be finalized as `Cancelled` or `Expired`. The local agreement string is
never sufficient authorization. The store first atomically appends a tombstone leaf and a pending
deletion journal entry. It journals the exact tombstone leaf, then fsyncs a provider-root append followed by
`acknowledge_deletion(agreement, content_commitment, root_sequence, root, leaf_index, leaf_count,
inclusion_proof)` to the outbox before removing bytes. All pending root journal entries are flushed in ascending sequence under one shared mutation/outbox lock; a deletion root and acknowledgement are queued as one ordering unit. The runtime folds the exact leaf through its bounded frontier, derives root/count, and requires the root transaction to
finalize first, rejects sequence/leaf-count rollback, derives the canonical tombstone leaf from the
agreement, content and provider, and verifies the bounded duplicate-last Merkle proof. Agreement
pruning remains blocked until the provider-signed acknowledgement finalizes.

## Persistence and workers

Blob and index writes use same-directory temporary files, file/directory fsync, and atomic rename.
Content records are append-only proof leaves; deletion appends a distinct tombstone leaf, changing
the committed root while preserving historic proof indices. A pending-deletion journal is replayed
after crashes, and bytes are never removed before the acknowledgement outbox entry is durable.
Checkpoint history is bounded to 1024 entries, list pages to 100, replica node
fetches to 1024, bucket names to 255 bytes, and object keys to 1024 bytes.

The process runs three coordinators:

1. checkpoint coordinator signs the current domain-separated Blake2b Merkle root;
2. challenge responder rescans the finalized runtime's next 128 due-block indices while advancing
   its safe cursor only to finalized height, verifies the challenged root locally, and fsyncs a
   domain-separated proof bound to challenge, agreement, content, provider and root;
3. replica coordinator continuously verifies the local index/root/frontier/history state. Open challenges resolve their snapshotted root from the append history and sign that exact root and leaf count; unknown roots fail closed.

Run the consumer with the provider account secret URI in `ORBIS_PROVIDER_ACCOUNT_SURI`:

```text
origin-orbis-provider-outbox \
  --orbis-rpc ws://127.0.0.1:9944 \
  --outbox /var/lib/orbis-provider/provider-submissions-v3.jsonl
```

Peer discovery, remote replica transport, TLS termination, and Prometheus deployment wiring are
operational integrations rather than hidden in-process defaults.

JSONL producer and receipt journals repair only an incomplete final non-newline tail before retry. Malformed complete or non-final records fail closed; append and receipt writes are fsynced.

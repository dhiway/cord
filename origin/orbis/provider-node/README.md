# Origin Orbis native provider

`origin-orbis-provider` is the CORD-owned off-chain byte/proof service for the native Orbis
`StorageProvider`, `Drive`, and `S3` composition. Commons runtime state remains authoritative; the
service does not implement a second retention ledger or compatibility path. The process refuses commits unless `StorageProviderApi` confirms at
one exact finalized hash that:

- the configured provider is active;
- the local Ed25519 service key equals the registered provider service key;
- current-root checkpoint signatures verify as Ed25519 with that exact finalized service key;
- the agreement is active, unexpired, belongs to the provider, and matches the exact byte length;
- the agreement content commitment equals `blake2b-256(raw_content_bytes)`.

Protocol v6 uses the 32-byte raw Blake2b-256 digest in JSON and runtime calls. A CID-facing adapter
must declare a CID multicodec/multihash configuration whose digest is that exact Blake2b-256 value;
this service does not silently translate SHA-256, Blake3, UnixFS, or DAG commitments.

## Process boundary

The provider is a companion binary, not an alternative runtime or a fork of the pinned SDK. It
reads `StorageProviderApi::{provider,agreement,challenges_at}` using `state_call` at
`chain_getFinalizedHead`. Canonical manifest-deletion acknowledgements are written to an fsynced,
typed JSONL outbox. The included `origin-orbis-provider-outbox` process accepts only that record
and submits native `StorageProvider::acknowledge_manifest_deletion` through the existing
`origin-rs` Orbis metadata-derived signer/nonce/finality pipeline. It records an fsynced finalized
block/extrinsic receipt before treating an idempotency key as complete. Checkpoint v2 uses its
separate canonical outbox/worker and is not accepted by this manifest consumer. Neither process
embeds a pallet/call index, signed extension, nonce, governance key, or raw SCALE payload.

Required secrets are read from environment variables (defaults:
`ORBIS_PROVIDER_BEARER_TOKEN` and `ORBIS_PROVIDER_SERVICE_SURI`) and are not persisted. The bearer
token must contain at least 32 bytes. Mutating, content-read, proof, and replica endpoints require
the bearer token. TLS and external client identity are expected at the deployment proxy.

## HTTP surfaces

The bounded v6 routes are:

- operations: `GET /health`, `GET /info`, `GET /stats`, `GET /node`, `PUT /node`;
- content: `POST /exists`, `GET /read`, `GET /commitment`; the authenticated route identities
  `POST /commit` and `POST /delete` are reserved for the P4 atomic object-completion cutover and
  return `503 Service Unavailable` before parsing a body or mutating the store;
- proofs: `GET /mmr_proof`, `GET /chunk_proof`, `GET /mmr_peaks`, `GET /mmr_subtree`,
  `POST /fetch_nodes`;
- replicas: `GET /buckets`, `GET /replica/sync_status`.

Both reserved object-mutation routes are fail-closed before authorization, body parsing, byte
mutation, or pending-journal creation. The store retains its pending-root and pending-deletion
types only as a private P4 atomic-cutover bearer. No production route or worker drains those
journals, and they are not public SDK requests or Commons runtime calls. Canonical deletion
completion is driven independently from finalized manifest-deletion duties and
`acknowledge_manifest_deletion`.

## Persistence and workers

Blob and index writes use same-directory temporary files, file/directory fsync, and atomic rename.
The deferred P4 DiskStore model retains append-only proof leaves and private pending journals, but
production does not create or replay generic object-completion records. Replica node fetches are
bounded to 1024, bucket names to 255 bytes, and object keys to 1024 bytes.

The process runs only canonical runtime and private-data-plane coordinators:

1. checkpoint-v2 duty intake stages one fixed-finalized runtime snapshot for the canonical
   checkpoint stack;
2. checkpoint-v2 quorum, confirmation, and governed publication run through their dedicated
   metadata-derived lanes;
3. manifest-deletion intake signs and fsyncs canonical acknowledgement requests;
4. replication reconciliation and repair operate on the authenticated checkpoint-v2 data plane.

There is no production generic checkpoint signer, challenge checkpoint submitter, provider-root
flusher, agreement-deletion flusher, or generic checkpoint HTTP surface.

Run the consumer with the provider account secret URI in `ORBIS_PROVIDER_ACCOUNT_SURI`:

```text
origin-orbis-provider-outbox \
  --orbis-rpc ws://127.0.0.1:9944 \
  --outbox /var/lib/orbis-provider/provider-submissions-v3.jsonl
```

Peer discovery, remote replica transport, TLS termination, and Prometheus deployment wiring are
operational integrations rather than hidden in-process defaults.

JSONL producer and receipt journals repair only an incomplete final non-newline tail before retry. Malformed complete or non-final records fail closed; append and receipt writes are fsynced.

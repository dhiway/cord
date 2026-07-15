# Storage checkpoint, proof and replica contract v2

## Types and signed payload

The stable SCALE logical types are:

- `cord.storage.MmrLeaf.v1 = { data_root:H256, data_size:u64, total_size:u64 }`;
- `cord.storage.MmrProof.v1 = { peaks:Vec<H256>, leaf:MmrLeaf, leaf_proof:Vec<H256> }`;
- `cord.storage.Commitment.v1 = { mmr_root:H256, start_seq:u64, leaf_count:u64 }`;
- `cord.storage.ChunkLocation.v1 = { leaf_index:u64, chunk_index:u32 }`;
- `cord.storage.CommitmentPayload.v2 = { version:2, bucket_id:BucketId, commitment:Commitment,
  nonce:BlockNumber }`.

The signature message is `b"cord/storage/checkpoint/v2" || SCALE(CommitmentPayloadV2)`. The digest is BLAKE2b-256 of that complete message; the active provider service key produces Ed25519-64 over the 32-byte digest. `BlockNumber` is SCALE `u32` for this launch profile. Runtime metadata MUST publish the exact portable SCALE type IDs mapped to these logical names and the metadata hash. Numeric portable IDs are derived from the complete implementing registry and therefore do not exist before P1. Missing or drifting P1/P3 bindings fail closed before checkpoint dispatch; this is a derived implementation gate, not a P0 implementation blocker.

The nonce names a finalized block no more than 128 blocks old. Checkpoint rejection vocabulary is closed: `220 STORAGE_CHECKPOINT_WRONG_DOMAIN`, `221 STORAGE_CHECKPOINT_WRONG_VERSION`, `222 STORAGE_CHECKPOINT_WRONG_BUCKET`, `223 STORAGE_CHECKPOINT_WRONG_KEY`, `224 STORAGE_CHECKPOINT_STALE_NONCE`, `225 STORAGE_CHECKPOINT_WRONG_WINDOW`, `239 STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM`, `240 STORAGE_CHECKPOINT_SEQUENCE_INVALID`, and `241 STORAGE_CHECKPOINT_EQUIVOCATION`. Except equivocation, every rejection has zero state effects and emits zero runtime events. Equivocation append-only preserves both payloads, suspends eligibility, and emits exactly one evidence event. A snapshot stores commitment, finalized checkpoint block, primary
signer bitfield, nonce, and replica confirmations.

## Finality, cadence and equivocation

A checkpoint covers one contiguous sequence, is submitted at most every 100 blocks, and has a
20-block grace. At most 256 challenge/proof duties may be admitted per block and a worker scans at
most 128 records per tick. Eligibility decisions use finalized state only.

A finalized key rotation authorizes the new key. An old-key payload whose nonce predates rotation
remains auditable but never authorizes post-rotation state. An identical duplicate is idempotent.
Different roots for the same `(bucket_id, nonce, start_seq)` are `STORAGE_CHECKPOINT_EQUIVOCATION`: both signed
payloads are retained append-only and the provider becomes ineligible.

## Replica and repair law

A bucket has one primary and two to four replicas. Publish quorum is the primary plus two replica
confirmations. A failover candidate MUST be Active, have a currently valid organization/SLA, confirm
the latest finalized commitment, and have no overdue challenge. Selection orders first by highest
confirmed checkpoint and then by lexicographically smallest encoded provider ID. Finalized changes
emit `ProviderIneligible`, `ReplicaSelected`, and, on authority change, `PrimaryPromoted` with bucket,
old/new provider, and checkpoint only.

Replication is peer transfer, per-chunk verification, atomic commit, and confirmation—not a local
statistics loop. Repair resumes at the first absent or invalid chunk and cannot advance confirmation
until the complete CID and root verify.

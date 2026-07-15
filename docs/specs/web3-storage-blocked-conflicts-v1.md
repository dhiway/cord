# Web3 Storage blocked-conflict decision packet v1

## Purpose and authority

This packet resolves only `WSI-GUIDANCE-DESIGN` and `WSI-PROVIDER-BYTE-PLANE`. It is a bounded P0
research result for Architect/Critic ratification and a later inventory-ledger update; it does not
claim that either capability has been implemented or is production-ready.

Evidence was read from:

- the read-only `paritytech/web3-storage` checkout pinned at
  `48a38a1e8cdcce0a4947611252ec449ea04c1df3`; and
- CORD branch `sm-update-sub-0x65` at baseline
  `bff224f62b49428933a9b16063a25b12d2b902cb`.

All implementation authority remains in CORD. No upstream repository may be modified, depended on as
a runtime, or treated as production evidence. The upstream repository itself says that it is an
unaudited prototype and not production-ready
(`paritytech/web3-storage@48a38a1e:README.md:3-11`).

## Decision summary

| Ledger row | Decision | Recommended non-blocked maturity | Ledger effect |
| --- | --- | --- | --- |
| `WSI-GUIDANCE-DESIGN` | **Classify guidance-only.** Inventory the source and its claims, but give it no runtime, SDK, operator, or repository-rule authority in CORD. | `design-only` | Remove the code/design contradiction from this row. Link claims to the implementation row instead of making guidance depend on implementation. |
| `WSI-PROVIDER-BYTE-PLANE` | **Implement/adapt clean-room with frozen CORD semantics.** Reuse current CORD generic persistence/finality/outbox invariants only after tests; replace incomplete checkpoint discovery, shared-bearer authorization and local-only replication. | `prototype-partial` | Replace `blocked-conflict` with an explicit admitted transition and the executable contract below. P1/P2 implementation and gates remain outstanding. |

Neither decision is `approved-excluded`: developer-facing storage requires the byte plane, while the
guidance corpus remains useful as traceable non-normative design evidence. Neither decision silently
narrows an upstream claim: every admitted, changed and rejected checkpoint semantic is stated below.

## Decision 1: `WSI-GUIDANCE-DESIGN`

### Evidence

The source guidance mixes repository workflow, architecture description, product claims and proposed
design:

1. Root `CLAUDE.md` supplies agent/git/Cargo/formatting rules
   (`paritytech/web3-storage@48a38a1e:CLAUDE.md:3-20`). Those are rules for the upstream repository,
   not CORD rules.
2. The same file describes a staked/slashed public storage system and a two-node architecture
   (`paritytech/web3-storage@48a38a1e:CLAUDE.md:22-30`), which conflicts with CORD's governed
   zero-stake enterprise policy.
3. Its provider description says that the provider stores bytes, builds commitments, serves HTTP and
   signs checkpoints (`paritytech/web3-storage@48a38a1e:CLAUDE.md:274-292`). These statements are
   useful capability intent, not proof that every path is executable.
4. The provider-initiated checkpoint document labels the approach “Recommended” and promises
   autonomous provider coordination (`paritytech/web3-storage@48a38a1e:docs/design/provider-initiated-checkpoints.md:164-190,211-228`),
   but later sections are Rust sketches rather than linked, passing implementation
   (`paritytech/web3-storage@48a38a1e:docs/design/provider-initiated-checkpoints.md:276-316,589-683`).
5. That design includes checkpoint rewards, missed-checkpoint stake slashing and token-denominated
   recommendations (`paritytech/web3-storage@48a38a1e:docs/design/provider-initiated-checkpoints.md:398-460,746-769`),
   plus a compatibility migration with both client- and provider-initiated paths
   (`paritytech/web3-storage@48a38a1e:docs/design/provider-initiated-checkpoints.md:727-742`). CORD
   explicitly admits neither economics nor a compatibility phase.
6. Source skills are development procedures. For example, `test-pallet` prescribes format/lint/test
   commands and lists checkpoint test topics
   (`paritytech/web3-storage@48a38a1e:.claude/skills/test/pallet.md:19-82,159-180`); it is not a
   runtime conformance suite or evidence that those semantics pass. `run-local-uis` contains
   repository- and host-specific commands and absolute paths
   (`paritytech/web3-storage@48a38a1e:.claude/skills/run-local-uis/SKILL.md:27-75`).

### Resolution

`WSI-GUIDANCE-DESIGN` is a **guidance-only, design-only** row. Its artifacts and every extracted
heading/fence stay in the source census, but:

- they cannot define CORD behavior, package names, commands, maturity, production readiness or
  operator promises;
- upstream `CLAUDE.md`, commands and skills are not installed or copied as CORD instructions;
- an upstream document's “current”, “recommended”, “automatic” or “complete” wording is recorded as a
  source claim and linked to the owning capability row; code plus tests determine implementation
  maturity;
- economics, migration/compatibility, public-market, Revive/contract and upstream-branding guidance
  is not carried into current CORD developer/operator documentation; and
- a generic practice may be restated in CORD-owned documentation only when it conforms to CORD's
  repository rules and frozen specifications.

This distinguishes **source ingestion** from **runtime capability**: ingestion proves that CORD
visited and classified the source; it never proves that CORD implemented it. A contradiction between
design prose and code therefore changes the maturity/gap on the capability row, not the maturity of
the guidance row.

### Recommended ledger update

- `maturity`: `design-only`.
- `disposition`: retain `refactor` (CORD may synthesize conforming documentation; no source guidance
  is authoritative).
- `classification_scope`: `guidance`, `design-only` only.
- `dependencies`: remove implementation dependencies. Use artifact links from claims to capability
  rows instead.
- `conflicts`: replace the blocking conflict with a resolved note:
  `guidance claim is non-normative; implementation maturity is tracked by WSI-PROVIDER-BYTE-PLANE`.
- `vector_or_test`: inventory validator proves every guidance artifact/heading/fence is visited once;
  documentation validator rejects current implementation/production claims unless they cite a
  non-blocked capability row and its named test evidence.
- `deletion_targets`: current CORD docs or generated developer docs that expose upstream branding,
  public economics, compatibility phases, source-local commands, or an implementation claim without
  capability evidence.

**Unblock criterion:** Architect and Critic approve this authority boundary and the inventory maps
every guidance artifact once. No provider implementation spike is required to classify this row.

## Decision 2: `WSI-PROVIDER-BYTE-PLANE`

### What the upstream source proves—and does not prove

The upstream provider contains useful partial machinery:

- the module advertises provider-initiated checkpoint coordination
  (`paritytech/web3-storage@48a38a1e:provider-node/src/lib.rs:3-17`);
- a coordinator loop polls duties and attempts only leader duties
  (`paritytech/web3-storage@48a38a1e:provider-node/src/checkpoint_coordinator.rs:245-327`);
- force-by-bucket derives a local root and fetches interval/grace configuration
  (`paritytech/web3-storage@48a38a1e:provider-node/src/checkpoint_coordinator.rs:339-395`);
- peers compare local root/start/leaf count before signing
  (`paritytech/web3-storage@48a38a1e:provider-node/src/api.rs:712-767`); and
- the Subxt client builds a `provider_checkpoint` transaction and waits for finalized success
  (`paritytech/web3-storage@48a38a1e:provider-node/src/subxt_client.rs:469-525`).

It does **not** prove autonomous operation:

- polling always receives an empty duty set because chain discovery is a TODO
  (`paritytech/web3-storage@48a38a1e:provider-node/src/checkpoint_coordinator.rs:331-337`);
- force-by-bucket hard-codes the caller as leader and supplies no peers
  (`paritytech/web3-storage@48a38a1e:provider-node/src/checkpoint_coordinator.rs:382-392`);
- quorum is hard-coded to one instead of coming from the bucket
  (`paritytech/web3-storage@48a38a1e:provider-node/src/checkpoint_coordinator.rs:426-457`);
- the coordinator is optional and disabled unless explicitly enabled
  (`paritytech/web3-storage@48a38a1e:provider-node/src/cli.rs:173-179`,
  `paritytech/web3-storage@48a38a1e:provider-node/src/command.rs:201-227`); and
- coordinator tests exercise manual duty lookup/submission and force mode, but have no automatic
  discovery, leader/fallback, multi-provider quorum or restart/finality vector
  (`paritytech/web3-storage@48a38a1e:provider-node/tests/coordinators/checkpoint.rs:107-205`).

The design proposes hashed rotating leadership, reward/slash economics and a client-checkpoint
compatibility migration
(`paritytech/web3-storage@48a38a1e:docs/design/provider-initiated-checkpoints.md:278-315,398-460,727-742`).
Those are explicitly changed below rather than silently inherited.

### Current CORD baseline

CORD already has generic invariants worth retaining behind replacement tests:

- content commits authorize the exact provider, agreement, content commitment and length at one
  finalized hash (`origin/orbis/provider-node/src/chain.rs:141-175,228-271`);
- the current HTTP service authorizes protected routes with one configured bearer-token hash
  (`origin/orbis/provider-node/src/api.rs:50-60,180-195`), so it is not the audience-bound provider
  capability protocol required by the frozen contract;
- provider-root submissions are journaled and flushed in monotonic order
  (`origin/orbis/provider-node/src/workers.rs:78-121,377-410`);
- outbox records are fsynced before success
  (`origin/orbis/provider-node/src/workers.rs:141-203`);
- the worker performs a bounded finalized challenge scan and only advances its safe cursor when the
  batch is accepted (`origin/orbis/provider-node/src/chain.rs:274-326`,
  `origin/orbis/provider-node/src/workers.rs:246-303`); and
- current runtime replay of an identical challenge proof is idempotent, while a changed proof is
  rejected (`origin/orbis/pallets/storage-provider/src/tests.rs:353-445`).

CORD does not yet implement the frozen bucket checkpoint contract:

- its runtime API exposes challenge buckets and a provider-level latest checkpoint, but no paginated
  bucket checkpoint duties, primary/replica set or checkpoint window
  (`origin/orbis/runtime-api/storage/src/lib.rs:202-224`);
- `/checkpoint/duty` returns challenge-response duties, not autonomous bucket checkpoint duties
  (`origin/orbis/provider-node/src/api.rs:345-358`);
- the periodic checkpoint worker only signs and stores a local observation; it does not discover a
  due bucket, collect replica confirmations, or submit a bucket checkpoint
  (`origin/orbis/provider-node/src/workers.rs:225-267`);
- the “replica” worker only checks its own local statistics and peaks
  (`origin/orbis/provider-node/src/workers.rs:305-309`); and
- the current runtime `submit_checkpoint` proves one admin-created challenge and stores a
  provider-level record (`origin/orbis/pallets/storage-provider/src/lib.rs:804-905`), whereas the
  frozen contract requires a bucket snapshot, primary signer bitfield and replica confirmations.

### Resolution and semantic disposition

Use **implement/adapt clean-room with frozen CORD semantics**. Recommended ledger values are
`disposition = replace-clean-room` and `maturity = prototype-partial` until the P1/P2 gates pass.
Existing CORD persistence, finalized-read and durable-outbox code may be retained/refactored only with
the exact replacement tests below; upstream code is evidence, not a copied dependency.

Admitted upstream intent:

- provider nodes, not mobile/app clients, discover due work and keep checkpoints progressing;
- peers independently verify the same local commitment before signing;
- an eligible fallback provider can progress after the primary grace period; and
- the chain finalizes checkpoint accountability while provider nodes own bytes.

Explicit CORD changes:

- the configured active bucket **primary** is the normal initiator; after the 20-block grace, fallback
  is the eligible replica selected by highest confirmed checkpoint and then lexicographically smallest
  encoded provider ID—not upstream hashed rotating leadership;
- quorum is the primary plus two distinct eligible replica confirmations—not a hard-coded one or a
  generic majority;
- cadence is 100 blocks, grace is 20 blocks, and worker reconciliation is bounded to 128 records per
  tick;
- signatures use the frozen `cord/storage/checkpoint/v2` payload and active finalized Ed25519 service
  keys; and
- rewards, stake, slashing, public checkpoint pools, client checkpoint compatibility, parallel
  checkpoint state and contract/precompile routes are excluded.

The existing challenge responder remains a separate accountability transition. During P1 its
ambiguous current `submit_checkpoint(challenge_id, proof_commitment)` naming/surface must be replaced
by a challenge-proof operation or removed as part of the clean-break cutover; it cannot coexist as a
second meaning of “bucket checkpoint”.

### Minimum executable duty-discovery contract

The following is the minimum CORD behavior required to change this row from a conflicted prototype
claim to an executable, testable transition. Names may be adjusted atomically with the runtime
descriptor, but fields and behavior may not be weakened.

1. **Finalized, bounded discovery.** The provider client resolves a finalized `(hash,number)` and calls
   a versioned Commons runtime API at that explicit hash with `(provider,cursor,limit)`. It returns at
   most 128 `BucketCheckpointDutyV2` items plus `next_cursor`; every page in one scan is evaluated at
   the same hash. Before advancing the scan cursor, the provider atomically journals each returned
   duty under its snapshot-bound idempotency key. Processing failures remain pending in that work
   journal and do not block discovery of later duties; a fetch/decode/journal failure does not advance
   the page cursor. A changed finalized head starts a new scan after the old bounded scan completes.
   Non-finalized state is never eligibility evidence.
2. **Complete duty.** Each duty binds response version, Commons genesis/spec version, bucket ID,
   checkpoint window/due block/grace end, previous finalized commitment and checkpoint block, expected
   next `start_seq`, ordered primary/replica provider IDs, their active service-key versions and
   endpoint hashes, organization/SLA eligibility, overdue-challenge status, and required confirmations
   (`primary + 2`). A duty is emitted only to a current bucket member; inactive/ineligible providers
   remain visible as exclusion evidence but cannot sign or initiate.
3. **Deterministic initiator.** At `due_block <= finalized < grace_end`, only the eligible primary may
   initiate. At `finalized >= grace_end`, if the window still has no finalized checkpoint, exactly one
   eligible fallback is selected by the frozen highest-confirmed-checkpoint/encoded-provider-ID order.
   Suspension, organization/SLA invalidity, stale key or overdue challenge excludes a candidate.
4. **Contiguous local proposal.** The initiator reads the locally fsynced bucket append log, refuses an
   empty/no-change proposal, and requires `start_seq` to equal the duty's expected next sequence. It
   constructs `Commitment { mmr_root, start_seq, leaf_count }`, freezes the duty's finalized block as
   nonce, and signs `BLAKE2b-256("cord/storage/checkpoint/v2" || SCALE(payload_v2))` with the active
   provider service key. A nonce older than 128 finalized blocks is rediscovered, never refreshed
   inside the old proposal.
5. **Authenticated peer confirmation.** The initiator sends the byte-identical signed proposal to the
   ordered replicas. Each peer resolves the same finalized duty, verifies sender/recipient/bucket/key/
   window/nonce, verifies its complete local CID/chunk/MMR state and contiguous sequence, and only then
   signs the same payload. A peer with missing/corrupt bytes or a different root returns a typed
   refusal and never confirms.
6. **Quorum and durable submission.** Duplicate providers/signatures do not count. Only a primary
   signature plus two distinct eligible replica signatures enters the CORD durable outbox. The outbox
   idempotency key is derived from the complete payload and canonically ordered signer set; it is
   fsynced before send, uses the metadata-derived signer/nonce/finality pipeline, and retries
   byte-identically after restart.
7. **Runtime acceptance.** Commons verifies domain/version/genesis/bucket/window, finalized nonce age,
   active service-key versions, membership/eligibility, distinct quorum, contiguous sequence and MMR
   bounds. It stores the bucket snapshot, finalized checkpoint block, primary signer bitfield, nonce
   and replica confirmations. An identical replay is side-effect-free. A different root for the same
   `(bucket_id, nonce, start_seq)` stores both evidence hashes, returns `STORAGE_CHECKPOINT_EQUIVOCATION`, and
   makes the provider ineligible.
8. **Finality and publishability.** Provider/SDK status remains pending until the transaction is
   finalized and the stored snapshot contains quorum. Upload durability or a local signed root is not
   publishability. Lost leadership, stale snapshot, stale key and already-finalized windows cause
   rediscovery rather than an alternate local truth.

### Minimum tests to close the conflict

The row may lose `blocked-conflict` in P0 when this contract is ratified, but it may not be marked
implemented until all tests below exist and pass. These tests are narrower than full P2 production
readiness.

#### Runtime/API tests

1. A fixed finalized snapshot returns only provider-member duties, in stable bucket order, through
   `0/1/127/128/129` item pages without duplicates or omissions. Each page is durably journaled before
   cursor advance; a failed fetch/decode/journal does not advance it, while one pending duty does not
   starve later discovered duties.
2. Duties created after an earlier snapshot are found on the next finalized snapshot; non-finalized
   provider, key, org/SLA, membership and checkpoint changes do not affect the old snapshot.
3. Boundary vectors cover `due-1`, `due`, `grace-1`, `grace`, nonce age `127/128/129`, sequence
   contiguous/gap/overlap, and the 256 duties-per-block admission bound.
4. Primary selection and post-grace fallback are deterministic under inactive, suspended,
   organization/SLA-invalid, overdue-challenge and tied-replica cases.
5. Primary plus two distinct current replica signatures succeeds. Missing primary, one replica,
   duplicate signer, wrong key version, wrong domain/version/genesis/bucket/window/nonce and stale
   nonce each fail with the frozen distinct error and no snapshot/event.
6. Identical replay changes no state/event/reputation. A changed root for the same tuple records both
   evidence hashes and exactly one ineligibility transition/event.

#### Provider tests

7. An automatic poll—not a force endpoint—discovers a due bucket, constructs the exact golden SCALE
   payload/digest/signatures, collects two peer confirmations, writes one durable submission and marks
   publishable only after mocked/real finalized success.
8. A peer refuses missing bytes, corrupt chunk/CID/root, wrong sequence, wrong audience, stale duty,
   stale/rotated key and replay; no refused case is counted toward quorum or serves corrupt bytes.
9. Primary crash before proposal, after one confirmation, after outbox fsync and after submission is
   exercised. Retry is byte-identical and produces exactly one effect; after grace the deterministic
   replica completes without a competing submission.
10. Cursor and pending outbox survive provider restart. Finalized-head transport failure, page decode
    failure and transaction failure retain work and retry; no loop silently returns an empty success.

#### Deterministic integration test

11. One primary and two replicas ingest the same bounded content, discover one duty from the Commons
    runtime API, confirm one root and finalize one bucket checkpoint. Re-run with a corrupt/partitioned
    primary: corrupt bytes are never served, deterministic promotion occurs, and repair/reconnection or
    governed replacement restores a third eligible confirmation before the promoted primary finalizes
    the same commitment. The three-provider set converges within two 100-block intervals; the test must
    not weaken primary-plus-two quorum merely to make a one-fault fixture pass.

These tests prove an executable duty-discovery/checkpoint slice. They do **not** claim an audit,
production load qualification, public-network economics, general interoperability, full capability/
resume protocol completion, or full P2 readiness.

### Recommended ledger update

- `maturity`: `prototype-partial`.
- `disposition`: `replace-clean-room`.
- `cord_authority`: CORD `storage-checkpoints-v2`, Commons runtime metadata/API and CORD provider
  protocol; upstream docs/code are evidence only.
- `cord_transition`: replace challenge-only/periodic-local signing with the finalized bounded duty
  discovery, primary-plus-two confirmation and durable finality path above; preserve separately named
  challenge proof handling.
- `conflicts`: replace the blocking contradiction with a resolved evidence note: upstream autonomous
  behavior is unimplemented; CORD admits its intent through this frozen clean-room transition and does
  not claim upstream completeness.
- `vector_or_test`: bind the eleven tests above to P1 checkpoint contract and P2 three-provider
  evidence.
- `deletion_targets`: shared bearer authorization; `/checkpoint/duty` challenge alias; local-only
  replica-statistics worker; manual/force path as product behavior; any provider-level “checkpoint”
  state that duplicates the bucket snapshot; stale upstream economics/compatibility/API branding.

**Unblock criterion:** Architect and Critic ratify the semantic table and minimum executable contract.
The ledger may then become non-blocked and P1/P2 may implement it. Passing P1/P2 evidence—not this
paper decision—is required before capability or feature-complete claims.

## Reviewer decision checklist

- [ ] Guidance ingestion is explicitly non-normative and cannot claim runtime maturity.
- [ ] The provider row says `prototype-partial`, not implemented or production-ready.
- [ ] CORD primary/fallback/quorum semantics are an explicit change, not an implicit narrowing.
- [ ] Finalized snapshot paging and retry rules cannot silently lose duties.
- [ ] Checkpoint and challenge-proof transitions have distinct names and state.
- [ ] Primary plus two confirmations, contiguous sequence, nonce age and equivocation rules match the
      frozen CORD checkpoint specification.
- [ ] The deterministic test closes the empty-duty TODO without importing public economics,
      compatibility behavior or an upstream runtime.
- [ ] No external repository change or dependency is authorized.

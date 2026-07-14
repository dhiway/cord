# Members Pallet Design

## TL;DR

The Members pallet provides reusable ring-VRF membership management, extracted from `pallet-people`.
It manages independent collections of member keys organized into rings, enabling privacy-preserving
proof of membership through Bandersnatch ring-VRF. Collections are identified by a 32-byte
identifier, support two operational modes (append-only and flexible), and are maintained
automatically through authorized transactions submitted by an offchain worker.

## Objective

Provide a self-contained, reusable module for managing ring-VRF member sets that can serve multiple
independent use cases on the People chain, such as the people set, the lite people set, and
coinage.

## Motivation

The People Registry (`pallet-people`) originally contained all ring management logic inline. As
more member sets with similar properties were planned (lite people, coinage), duplicating
this logic became impractical. Extracting ring management into a separate pallet allows:

- **Reuse**: Multiple pallets can manage their own member collections without reimplementing ring
  logic.
- **Separation of concerns**: `pallet-people` focuses on personhood (personal IDs, alias accounts,
  key migration) while `pallet-members` handles ring mechanics.
- **Flexibility**: Different collections can use different ring sizes and operational modes.

## Background

The ring management logic is built on top of the Bandersnatch ring-VRF primitive. A ring-VRF
allows a member to prove they belong to a set (the ring) and produce a deterministic pseudonym
(contextual alias) for a given application context, without revealing which member they are. The
cryptographic commitment to a ring's members (the ring root) is what enables proof verification.

Building ring roots is computationally expensive because each member's key must be processed with
static chunks (precomputed cryptographic data). This cost, combined with the need to maintain
privacy through batch processing, drives much of the pallet's design around queues, batching, and
automated background processing.

## Scope

### In scope

- Manage multiple independent ring collections, segregated by identifier.
- Automated maintenance: onboarding, ring building, suspension cleanup, queue defragmentation,
  and collection deletion.
- Two operational modes: append-only (no removals, variable ring sizes) and flexible (supports
  removals, restricted ring size).
- Membership proof verification producing contextual aliases.
- Privacy protection through configurable minimum onboarding batch sizes.

### Out of scope

- Alias account management: up to collection owners (e.g. `pallet-people`) to manage alias
  accounts for their members.
- Key migration: implemented in `pallet-people` directly, since an owner can achieve the same
  effect by removing and re-adding a member with a new key.
- Permissionless collection creation for third parties (currently all collections are created by
  system-level pallets).

## Architecture

### High-level overview

The pallet organizes data around **collections**. Each collection holds zero or more **rings**, each
of which holds up to `ring_capacity` member keys. Members enter through an **onboarding queue** and
are moved into rings in batches. Ring **roots** (cryptographic commitments) are built after
onboarding to enable proof verification.

```
Collection (identifier)
├── Ring 0: [key, key, key, ...] → Root (commitment, revision)
├── Ring 1: [key, key, key, ...] → Root (commitment, revision)
├── Ring 2: [key, key, ...]      → Root (building...)
└── Onboarding Queue: [key, key, key, ...]
```

Collections are owned by a `Location` (e.g. a parachain or system pallet). The owner interacts
with the collection through the `AppendOnlyMembers` and `FlexibleMembers` traits.

### Ring sizing

Ring capacity is determined by the ring exponent: `capacity = 2^exponent - 257`. The 257 padding
is required by the ring-VRF implementation for internal use. The currently supported exponents are:

| Exponent | Capacity |
|----------|----------|
| 9        | 255      |
| 10       | 767      |
| 14       | 16,127   |

**Append-only** collections may use any supported ring size. **Flexible** collections are restricted
to ring sizes no larger than `MaxFlexibleRingExponent` (a runtime constant). This restriction
ensures each ring's keys fit in a single storage page, which is necessary because suspension
handling reconstructs the entire page of keys — multi-page reconstruction would be significantly
more complex and would require careful coordination across pages.

### Ring modes

**Append-only**: Members are only ever added. When a ring reaches full capacity and all members are
included in the root, it is marked with an `immutable_since` timestamp. This timestamp is exposed
through `RingStatus` and can be used by consumers to assess ring stability (e.g. only accepting
proofs from rings that have been stable for a minimum period).

**Flexible**: Members can be removed through suspension sessions. Rings are never marked immutable
since their membership can change at any time.

Both modes support the `FlexibleMembers` trait for member removal. The mode primarily governs ring
size constraints and immutability behavior rather than whether removal is allowed at all.

### Data model

#### Collections

```rust
/// Identifier type for a member collection.
pub type Identifier = [u8; 32];

/// Information about a collection.
pub struct CollectionInfo<Account, Location> {
    pub owner: CollectionOwner<Account, Location>,
    pub mode: RingMode,        // AppendOnly or Flexible
    pub ring_size: RingExponent,
}
```

Active collections live in `Collections`. When deletion is requested, they move to
`SuspendedCollections` where they are invisible to normal operations and are cleaned up
asynchronously. `IdentifiersOf` maps each owner to their collection identifiers.

#### Members

```rust
pub enum RingPosition {
    Onboarding { queue_page: PageIndex },
    Included { ring_index: RingIndex, ring_page: PageIndex, ring_position: u32 },
    Suspended,
}
```

The `Members` double map (keyed by collection identifier and member public key) tracks every member
in each collection. A key can belong to multiple collections.

#### Rings

```rust
pub struct RingRoot<T: Config> {
    pub root: MembersOf<T>,       // The cryptographic commitment
    pub revision: RevisionIndex,  // Incremented on each rebuild
    pub intermediate: IntermediateOf<T>,  // State for incremental building
}
```

Ring keys are stored in `RingKeys`, a paginated N-map keyed by `(Identifier, RingIndex, PageIndex)`.
Each page holds up to `MaxFlexibleRingExponent::ring_capacity()` keys. `RingKeysStatus` tracks
per-ring metadata: the total number of keys and how many are included in the current root.

`StaleRings` is a set of `(Identifier, RingIndex)` pairs marking rings that need their root
rebuilt. `PendingSuspensions` stores sorted indices of members pending removal from each ring.

#### Onboarding queue

The queue is a paginated double map keyed by `(Identifier, PageIndex)`, with `QueuePageIndices`
tracking the head and tail page for each collection. New members are appended at the tail; members
are dequeued from the head during onboarding.

#### Ring state

```rust
pub enum RingMembersState {
    AppendOnly,
    Mutating(u8),  // Semaphore count
}
```

`RingsState` tracks whether a collection is accepting incremental additions (`AppendOnly`) or
undergoing membership changes (`Mutating`). The semaphore allows multiple concurrent removal
sessions (e.g. if two pallets need to suspend members from the same collection simultaneously).

### APIs

The pallet exposes two trait interfaces for other pallets:

**`AppendOnlyMembers`**: The core interface.
- `create_collection(owner, identifier, onboarding_size, mode, ring_size)`: Create a new
  collection.
- `delete_collection(owner, identifier)`: Mark a collection for asynchronous deletion.
- `add_members(identifier, members)`: Add members to the onboarding queue. Re-adding a suspended
  member resumes their membership.
- `verify_membership(identifier, proof, ring_index, context, msg)`: Validate a ring-VRF proof
  and return a revised contextual alias.
- `remove_ring(identifier, ring_index)`: Queue a ring for deletion.
- `active_count`, `ring_status`, `ring_revision`, `member_status`, `ring_members`: Query
  functions.

**`FlexibleMembers`**: Extends `AppendOnlyMembers` with removal.
- `start_removal_session(identifier)`: Transition to `Mutating` state.
- `remove_members(identifier, suspensions)`: Queue member suspensions (must be within a session).
- `end_removal_session(identifier)`: Transition back to `AppendOnly`.
- `rings_state(identifier)`: Query the current state.

The pallet also provides dispatchable functions:
- `merge_rings`: Merge two underutilized rings.
- `set_onboarding_size`: Adjust onboarding batch size (root-only).

The following extrinsics are authorized and submitted by the offchain worker:
- `build_ring_authorized`: Build a ring root for a specific ring in a collection.
- `onboard_members_authorized`: Onboard members from the onboarding queue for a specific collection.
- `merge_queue_pages_authorized`: Merge the top two onboarding queue pages for a specific
  collection.
- `remove_suspended_keys_authorized`: Remove suspended keys from a specific ring in a collection.
- `delete_ring_page_authorized`: Delete a page for a specific ring in a collection.
- `enqueue_ring_deletion_authorized`: Enqueue a ring for deletion as part of collection deletion.
- `delete_onboarding_queue_page_authorized`: Delete an onboarding queue page as part of collection
  deletion.
- `finalize_collection_deletion_authorized`: Finalize collection deletion after all rings and queue
  pages have been cleaned up.
- `mark_ring_stale_authorized`: Mark a ring as stale so the offchain worker will rebuild it. This is
  a recovery mechanism that anyone can submit if a ring has unincluded members but is missing its
  `StaleRings` entry.

## Key flows

### Onboarding and ring building

Under normal operation, the entire pipeline is driven by the offchain worker:

1. An owner calls `add_members()` to register new members. Keys are validated and placed in the
   onboarding queue.
2. The offchain worker checks each collection for onboarding eligibility. If the queue has enough
   members (at least `OnboardingSize`) and the current ring has enough open slots,
   `onboard_members_authorized` is submitted to move members from the queue into the ring's key
   storage.
3. The ring is marked stale after onboarding. In the next cycle, `build_ring_authorized` is
   submitted to build (or extend) the ring root by processing the newly added keys through the
   ring-VRF with static chunks from the chunks-manager.
4. When a ring fills up, `CurrentRingIndex` is incremented and future members go into a new ring.

**Cohort rule**: Members must be onboarded in groups of at least `OnboardingSize`. After
onboarding, at least `OnboardingSize` slots must remain in the ring (unless the batch completely
fills it). This ensures future batches can also meet the minimum.

**Privacy rationale**: If members were onboarded one at a time, an observer could correlate the
timing of a key appearing in a ring with a specific registration event, potentially identifying who
owns which key. Requiring minimum batch sizes obscures this timing within the batch. The
`OnboardingSize` parameter is configurable per collection so that each owner can choose the
appropriate privacy/latency tradeoff.

### Suspension and removal

Member removal uses a session-based protocol to prevent conflicts between additions and removals:

1. **Start session**: `start_removal_session()` transitions the collection to `Mutating`. This
   blocks onboarding and ring building for the collection.
2. **Queue suspensions**: `remove_members()` processes each key:
   - **Included members**: Their ring position index is added to `PendingSuspensions` (kept sorted).
     Their status is updated to `Suspended`.
   - **Onboarding members**: Removed from the queue page immediately. Their status is updated to
     `Suspended`.
   - **Already suspended**: Emits a defensive warning but does not error.
3. **End session**: `end_removal_session()` transitions back to `AppendOnly`.
4. **Cleanup**: Suspended keys are removed from rings by reconstructing each ring's key list,
   skipping suspended indices. This avoids expensive in-place `Vec::remove` operations. The
   ring's intermediate state is reset, the ring is marked stale, and remaining members' positions
   are updated.
5. **Rebuild**: The ring root is rebuilt from scratch during the next ring building pass.

**Why full rebuild**: The ring-VRF commitment supports incremental addition but not incremental
removal. After removing members, the commitment must be recomputed from the remaining keys.

### Ring merging

When removals cause rings to become sparse, `merge_rings` combines two into one:

1. Both rings must be below 50% capacity, have no pending suspensions, and neither can be the
   current onboarding ring. A removal session must not be in progress.
2. All keys from the target ring are appended to the base ring. Each moved member's position is
   updated.
3. The base ring is marked stale for rebuilding; the target ring is deleted.

Merging only works for single-page rings (flexible collections).

### Collection deletion

Deletion is asynchronous and uses granular extrinsics to spread work across blocks:

1. `delete_collection()` moves the collection from `Collections` to `SuspendedCollections`.
   Normal operations immediately fail with `CollectionNotFound`.
2. The offchain worker iterates `SuspendedCollections` and submits granular authorized transactions
   for each unit of work:
   - `enqueue_ring_deletion_authorized`: For each ring, enqueues its pages into
     `RingDeletionQueue` and removes ring metadata (root, status, stale markers, pending
     suspensions).
   - `delete_ring_page_authorized`: Processes `RingDeletionQueue` entries, removing keys and
     associated `Members` entries.
   - `delete_onboarding_queue_page_authorized`: Removes a queue page and its associated `Members`
     entries. Only submitted after all rings and ring pages are deleted.
   - `finalize_collection_deletion_authorized`: Removes remaining per-collection storage items and
     the collection's entry from the owner's `IdentifiersOf` list. Only submitted after all
     onboarding queue pages are also deleted.
3. Multiple suspended collections can be deleted concurrently. Ordering constraints are enforced by
   the `ensure_can_*` authorization validators.

### Ring page deletion

`delete_ring_page_authorized` processes `RingDeletionQueue` entries, removing keys and the
associated `Members` entries. Both `remove_ring` and collection deletion enqueue into
`RingDeletionQueue`.

## Offchain worker

The pallet uses an offchain worker (OCW) that discovers pending maintenance work every
`OffchainWorkerInterval` blocks and submits authorized transactions in dependency order:

1. `remove_suspended_keys_authorized`
2. `merge_queue_pages_authorized`
3. `onboard_members_authorized`
4. `build_ring_authorized`
5. `delete_ring_page_authorized`
6. Collection deletion (for each suspended collection):
   a. `enqueue_ring_deletion_authorized`
   b. `delete_onboarding_queue_page_authorized`
   c. `finalize_collection_deletion_authorized`

Each authorized extrinsic performs the minimum unit of work (e.g., one collection, one ring, one
page, one step). The `authorize` closures validate preconditions before accepting transactions,
rejecting stale or redundant work.

## Security considerations

### Privacy when onboarding

The onboarding batch size mechanism protects new members from timing correlation. The ring contents
at any given time are opaque to external observers; only the ring root is publicly visible. The
responsibility for preserving anonymity when generating and using proofs falls on the user of the
pallet.

### Ring revisions

Each ring root carries a revision index that increments on every rebuild. Consumers of membership
proofs can compare the revision in a proof against the current revision to detect whether the ring
has been modified since the proof was generated. This enables staleness detection and time-based
validity policies (e.g. requiring a proof from a recent enough revision).

### Contextual aliases

Proof verification produces a contextual alias that is deterministic for a given member and
context. This allows applications to recognize the same member across interactions within a single
context while preventing cross-context tracking.

### Key reuse across collections

A key can belong to multiple collections. However, to preserve privacy, users should generate a
fresh key for each collection rather than reusing one across collections, since a shared key
could allow an observer to correlate activity across collections. This is best enforced at the
application/UI layer rather than at the protocol level, since the pallet cannot distinguish
intentional reuse from accidental reuse.

### Weight-based DoS resistance

All authorized extrinsics have explicit weight bounds. No single extrinsic can consume unbounded
weight.

## Assumptions

- **Small number of collections**: The offchain worker iterates over all collections and submits
  one authorized transaction per unit of work. A large number of collections could flood the
  transaction pool. Currently only system-level pallets create collections, so this is not a
  concern.
- **Chunks availability**: Static chunks for the relevant ring exponent must be loaded through the
  `ChunksManager` before any ring building can occur.
- **Single-page rings for flexible collections**: Suspension handling, ring merging, and several
  internal operations assume flexible collection rings fit in a single storage page. This is
  enforced at collection creation.
- **Rare onboarding-phase suspensions**: Suspending a member still in the onboarding queue
  requires shifting page contents, which is expensive in the worst case. This is assumed to be
  rare because members should be onboarded well before the next suspension round.

License: Apache-2.0

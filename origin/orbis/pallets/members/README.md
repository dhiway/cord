# Members Pallet

A pallet managing cryptographic membership rings for privacy-preserving proof of membership.

## Overview

The Members pallet stores and manages collections of member keys organized into rings. Using
Bandersnatch ring-VRF, it enables members to prove they belong to a set without revealing which
member they are. Each collection is identified by a unique 32-byte identifier and operates
independently, with its own ring size, operational mode, and onboarding configuration.

The pallet accepts new members through its API, available in `AppendOnlyMembers` and its extension
`FlexibleMembers`, queues them for onboarding, organizes their keys into rings, and builds
cryptographic commitments (ring roots) that enable ZK proofs of membership. It supports removing
members through a session-based suspension mechanism, merging underutilized rings, and deleting
entire collections.

## Key Features

- **Collection Management**: Creates and manages independent collections of members, each with its
  own configuration, ring size, and operational mode (append-only or flexible).
- **Ring-Based Privacy**: Organizes member keys into rings and builds cryptographic commitments that
  enable proof of membership without revealing individual identity.
- **Automated Maintenance**: An offchain worker periodically submits authorized transactions for
  onboarding, ring building, suspension cleanup, queue defragmentation, and collection deletion.
- **Flexible Membership**: Supports both append-only collections (members are never removed) and
  flexible collections (members can be suspended and removed through removal sessions).
- **Proof Verification**: Validates membership proofs against ring roots, producing contextual
  aliases that preserve anonymity across interactions.

## Interface

### Dispatchable Functions

- `merge_rings(identifier, base_ring_index, target_ring_index)`: Merge two rings that are each
  below half capacity into one. Both rings must have no pending suspensions and neither can be the
  current onboarding ring.
- `set_onboarding_size(identifier, onboarding_size)`: Set the minimum batch size for onboarding new
  members. Requires root privileges.

The following extrinsics are authorized and submitted by the offchain worker:

- `build_ring_authorized(identifier, ring_index, _revision)`: Build a ring root for a specific
  ring in a collection.
- `onboard_members_authorized(identifier, _ring_index, _head)`: Onboard members from the onboarding
  queue for a specific collection.
- `merge_queue_pages_authorized(identifier, initial_head, new_head)`: Merge the top two onboarding
  queue pages for a specific collection.
- `remove_suspended_keys_authorized(identifier, ring_index)`: Remove suspended keys from a
  specific ring in a collection.
- `delete_ring_page_authorized(identifier, ring_index, page_index)`: Delete a page for a specific
  ring in a collection.
- `enqueue_ring_deletion_authorized(identifier, ring_index)`: Enqueue a ring for deletion as part of
  collection deletion.
- `delete_onboarding_queue_page_authorized(identifier, page_index)`: Delete an onboarding queue page
  as part of collection deletion.
- `finalize_collection_deletion_authorized(identifier)`: Finalize collection deletion after all rings
  and queue pages have been cleaned up.
- `mark_ring_stale_authorized(identifier, ring_index)`: Mark a ring as stale so it gets rebuilt.
  Anyone can submit this if the ring has unincluded members but is not already marked stale.

### Automated tasks

- Ring building: Build or update a ring's cryptographic root. Keys are processed into a ring
  commitment in batches rather than individually. The batch size needs to be reasonably large to
  enhance privacy by obscuring the exact timing of when members' keys were added to the ring, making
  it more difficult to correlate specific members with their keys, but it cannot be too large due to
  the limited blockspace and expensive cryptographic operations.
- Member onboarding: Onboard members from the onboarding queue into a ring. Members can be
  onboarded only in batches of at least `OnboardingSize` and when the remaining open slots in a
  ring are at least `OnboardingSize`. This does not compute the root; that is done by ring building.
- Cleaning of suspended members: Remove members' keys marked as suspended from rings. The keys
  stored in `PendingSuspensions` are removed from rings and the ring's intermediate state is reset.
  The ring roots are subsequently rebuilt from scratch in the ring building phase.
- Onboarding queue page merging: Merge the two pages at the front of the onboarding queue. After
  a round of suspensions, queue pages may be left with too few members, causing the total count
  to fall below the required onboarding size and stalling the queue. This defragments the queue by
  combining pages which are both less than half full.
- Collection deletion: Process multi-stage deletion of collections that have been marked for
  removal, cleaning up rings, onboarding queues, and metadata incrementally across blocks.

### Custom Origin

The pallet provides the `MemberAlias` origin, identified by a collection identifier and a revised
contextual alias. This origin represents a member who proved their membership in a specific
collection through a ring-VRF proof.

## Usage

Other pallets interact with the Members pallet through two traits:

- `AppendOnlyMembers`: Core interface for creating collections, adding members, verifying proofs,
  querying ring and member status, removing rings, and deleting collections.
- `FlexibleMembers`: Extends `AppendOnlyMembers` with member removal via suspension sessions. A
  removal session must be started before suspending members and ended after all suspensions are
  queued.

License: Apache-2.0

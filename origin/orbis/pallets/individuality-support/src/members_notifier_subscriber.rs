// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Shared types for the members-notifier and members-subscriber pallets.

use crate::traits::{Identifier, RevisionIndex, RingIndex};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::{BoundedVec, Get},
	CloneNoBound, DebugNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use scale_info::TypeInfo;
use verifiable::GenerateVerifiable;

/// Sequence number for batches.
/// Used for ordering batches and detecting gaps in updates.
pub type SequenceNumber = u64;

/// Configuration trait for shared members types.
///
/// Both members-notifier and members-subscriber pallet Configs implement this trait,
/// allowing shared types to be generic over it.
pub trait MembersTypeConfig {
	/// Cryptographic implementation for ring membership verification.
	type Crypto: GenerateVerifiable;
	/// Maximum number of ring root updates per XCM batch.
	type MaxUpdatesPerBatch: Get<u32>;
	/// Maximum number of ring collections supported.
	type MaxCollections: Get<u32>;
}

/// Ring root members type from the crypto implementation.
pub type MembersOf<T> = <<T as MembersTypeConfig>::Crypto as GenerateVerifiable>::Members;

/// Describes whether a ring root was built or deleted.
#[derive(
	Clone, PartialEq, Eq, Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum RingRootOp<R> {
	/// A ring root was built or updated with the given revision and root data.
	Built { revision: RevisionIndex, root: R },
	/// A ring root was deleted.
	Deleted,
}

impl<R: Clone> RingRootOp<&R> {
	/// Converts from `RingRootOp<&R>` to `RingRootOp<R>` by cloning the root.
	pub fn cloned(&self) -> RingRootOp<R> {
		match self {
			RingRootOp::Built { revision, root } =>
				RingRootOp::Built { revision: *revision, root: (*root).clone() },
			RingRootOp::Deleted => RingRootOp::Deleted,
		}
	}
}

/// Represents a single ring root update sent between notifier and subscriber.
#[derive(
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	DebugNoBound,
	TypeInfo,
	MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct RingRootUpdate<T: MembersTypeConfig> {
	/// Ring index within the collection.
	pub ring_index: RingIndex,
	/// The operation performed on this ring root.
	pub op: RingRootOp<MembersOf<T>>,
}

/// Batch of ring root updates for a single collection, sent between notifier and subscriber.
#[derive(
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	DebugNoBound,
	TypeInfo,
	MaxEncodedLen,
	DefaultNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct RingRootUpdatesBatch<T: MembersTypeConfig> {
	/// Collection identifier for all updates in this batch.
	pub identifier: Identifier,
	/// Sequence number for this batch.
	pub sequence: SequenceNumber,
	/// Unix timestamp in seconds when this batch was created.
	pub source_time: u64,
	/// List of ring root updates in this batch.
	pub updates: BoundedVec<RingRootUpdate<T>, T::MaxUpdatesPerBatch>,
	/// Index of the next ring that will be built for this collection.
	/// The subscriber uses this to scan `0..next_ring_index` for missing indices.
	pub next_ring_index: u32,
}

/// Hook called when a ring root changes.
pub trait OnRingRootChange<RingRoot> {
	/// Called after a ring root is inserted, updated, or deleted.
	///
	/// - `identifier`: The collection this ring belongs to.
	/// - `ring_index`: The index of the ring that changed.
	/// - `op`: The operation performed on the ring root.
	fn on_ring_root_change(
		identifier: Identifier,
		ring_index: RingIndex,
		op: RingRootOp<&RingRoot>,
	);

	/// Seed the impl's storage so the next `on_ring_root_change` call exercises
	/// its worst-case branch. Callers invoke this from benchmark setup before
	/// the measured extrinsic; the default is a no-op.
	#[cfg(feature = "runtime-benchmarks")]
	fn bench_setup_worst_case() {}
}

impl<RingRoot> OnRingRootChange<RingRoot> for () {
	fn on_ring_root_change(_: Identifier, _: RingIndex, _: RingRootOp<&RingRoot>) {}
}

/// Provider of read-only access to ring roots.
/// Used by members-notifier pallet to query ring data for:
/// - Initializing subscribers with existing ring roots,
/// - Returning specific ring roots on request,
/// - Providing the index of the next ring for this collection.
pub trait RingRootsProvider<RingRoot> {
	/// Get specific ring roots by their indices.
	/// Returns a vector of (ring_index, root, revision).
	/// Output order must match the input `indices` order.
	fn get_ring_roots(
		identifier: Identifier,
		indices: &[RingIndex],
	) -> Vec<(RingIndex, RingRoot, RevisionIndex)>;

	/// Index of the next ring that will be built for this collection.
	/// The subscriber scans `0..next_ring_index` to detect missing or deleted rings.
	fn next_ring_index(identifier: Identifier) -> RingIndex;

	/// Get ring roots in a collection with pagination.
	/// Returns a vector of (ring_index, root, revision) starting after `after_key` (exclusive),
	/// up to `limit` items. Pass `None` to start from the beginning.
	fn get_ring_roots_paginated(
		identifier: Identifier,
		after_key: Option<RingIndex>,
		limit: u32,
	) -> Vec<(RingIndex, RingRoot, RevisionIndex)>;
}

impl<RingRoot> RingRootsProvider<RingRoot> for () {
	fn get_ring_roots(
		_identifier: Identifier,
		_indices: &[RingIndex],
	) -> Vec<(RingIndex, RingRoot, RevisionIndex)> {
		Vec::new()
	}

	fn next_ring_index(_identifier: Identifier) -> RingIndex {
		0
	}

	fn get_ring_roots_paginated(
		_identifier: Identifier,
		_after_key: Option<RingIndex>,
		_limit: u32,
	) -> Vec<(RingIndex, RingRoot, RevisionIndex)> {
		Vec::new()
	}
}

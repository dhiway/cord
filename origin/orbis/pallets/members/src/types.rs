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

//! Types for the members pallet.

#![allow(clippy::result_unit_err)]

use super::*;
use frame_support::pallet_prelude::*;

use indiv_support::traits::{PageIndex, RingExponent};

// Re-export shared types from the support crate.
pub use indiv_support::{
	traits::{Identifier, RevisionIndex, RingMode, RingPosition, RingStatus},
	utils::BigEndianU32,
};

/// A revision index encoded in big-endian format for correct storage iteration order.
pub type BigEndianRevisionIndex = BigEndianU32;

pub type MemberOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Member;
pub type MembersOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Members;
pub type IntermediateOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Intermediate;
pub type SecretOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Secret;
pub type SignatureOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Signature;
pub type CapacityOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Config;

use verifiable::DecodeUnchecked;

/// Ring root record. The `Decode` impl is hand-written so `root` and
/// `intermediate` route through [`DecodeUnchecked::decode_unchecked`],
/// skipping arkworks curve-point validation on storage reads. Values were
/// already validated when first written.
///
/// **Warning**: This type contains a `root` and `intermediate` which are trusted and decoded
/// without check, they must have been validated
#[derive(PartialEq, Eq, Clone, Encode, Debug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RingRoot<T: Config> {
	/// The ring root for the current ring.
	pub root: MembersOf<T>,
	/// The revision index of the ring.
	pub revision: RevisionIndex,
	/// An intermediate value if the ring is not full.
	pub intermediate: IntermediateOf<T>,
}

impl<T: Config> Decode for RingRoot<T> {
	fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
		Ok(Self {
			root: <MembersOf<T> as DecodeUnchecked>::decode_unchecked(input)?,
			revision: Decode::decode(input)?,
			intermediate: <IntermediateOf<T> as DecodeUnchecked>::decode_unchecked(input)?,
		})
	}
}

/// Owner of a collection.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub enum CollectionOwner<Account, Location> {
	/// External ownership from another chain/parachain.
	External(Location),
	/// Local account ownership. Not currently used, but will become useful once the pallet is open
	/// for permissionless use by regular users creating their collections.
	Local(Account),
}

/// Information about a collection.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct CollectionInfo<Account, Location> {
	/// The owner of this collection.
	pub owner: CollectionOwner<Account, Location>,
	/// The mode of ring operation for this collection.
	pub mode: RingMode,
	/// The ring size exponent for this collection.
	///
	/// The maximum ring capacity is 2^exponent - 257.
	pub ring_size: RingExponent,
	/// Minimum time in seconds a member must wait in the onboarding queue before they can
	/// self-include. `None` means self-inclusion is disabled for this collection.
	pub self_inclusion_delay: Option<u64>,
}

/// Describes the action to take after checking the first two pages of the onboarding queue for a
/// potential merge.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub(crate) enum QueueMergeAction<T: Config> {
	Merge {
		initial_head: PageIndex,
		new_head: PageIndex,
		first_key_page: BoundedVec<MemberOf<T>, T::OnboardingQueuePageSize>,
		second_key_page: BoundedVec<MemberOf<T>, T::OnboardingQueuePageSize>,
	},
	NoAction,
}

/// Extracts the ring capacity from the configured ring exponent for flexible collections.
pub struct RingCapacityFromExponent<T>(PhantomData<T>);
impl<T: Config> Get<u32> for RingCapacityFromExponent<T> {
	fn get() -> u32 {
		crate::Pallet::<T>::flexible_ring_capacity()
	}
}

/// An old ring root that is retained until the root cleanup is performed.
///
/// This allows proofs generated with the old root to remain valid during a grace period when a ring
/// is revised.
/// Archived ring root, retained during the post-revision grace period.
/// Hand-written `Decode` routes `root` through
/// [`DecodeUnchecked::decode_unchecked`] for the same reason as
/// [`RingRoot`].
///
/// **Warning**: This type contains a `root` which is trusted and decoded without check, it must
/// have been validated
#[derive(PartialEq, Eq, Clone, Encode, Debug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct OldRoot<T: Config> {
	/// The ring root commitment from the previous revision.
	pub root: MembersOf<T>,
	/// The timestamp (in seconds since Unix epoch) when this root was archived.
	pub archived_at: u64,
}

impl<T: Config> Decode for OldRoot<T> {
	fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
		Ok(Self {
			root: <MembersOf<T> as DecodeUnchecked>::decode_unchecked(input)?,
			archived_at: Decode::decode(input)?,
		})
	}
}

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

//! Traits concerned with modelling reality.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, FullCodec, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::{CloneNoBound, EqNoBound, Parameter, PartialEqNoBound};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::{traits::Member, BoundedVec, DispatchError, DispatchResult, Weight};
use verifiable::{
	ring::ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2, BatchProofItem, GenerateVerifiable,
};

/// Identity of personhood.
///
/// This is a persistent identifier for every individual. Regardless of what
/// else the individual changes within the system (such as identity documents, cryptographic keys,
/// etc...) this does not change. As such, it should never be used in application code.
pub type PersonalId = u64;

/// Identifier for a specific application in which we may wish to track individual people.
///
/// NOTE: This MUST remain equivalent to the type `Context` in the crate `verifiable`.
pub type Context = [u8; 32];

/// Identifier for a specific individual within an application context.
///
/// NOTE: This MUST remain equivalent to the type `Alias` in the crate `verifiable`.
pub type Alias = [u8; 32];

/// The type we use to identify different rings.
pub type RingIndex = u32;

/// The ring index 0.
pub const RI_ZERO: RingIndex = 0;

/// The type we use to represent ring sizes.
pub type RingSize = u32;

/// Allowed exponents for the ring size.
///
/// The actual ring size (capacity) for each exponent is `2^x - 257` where `x` is the exponent.
/// This accounts for the padding required by the ring-VRF implementation.
#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Debug,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	DecodeWithMemTracking,
)]
pub enum RingExponent {
	/// Ring exponent of 9 (ring capacity: 2^9 - 257 = 255)
	R2e9 = 9,
	/// Ring exponent of 10 (ring capacity: 2^10 - 257 = 767)
	R2e10 = 10,
	/// Ring exponent of 14 (ring capacity: 2^14 - 257 = 16127)
	R2e14 = 14,
}

impl RingExponent {
	/// Returns the exponent value as a u8.
	pub fn exponent(&self) -> u8 {
		*self as u8
	}

	/// Returns the ring capacity.
	///
	/// This is the maximum number of members that can be included in a ring.
	pub fn ring_capacity(&self) -> RingSize {
		self.domain_size().max_ring_size::<BandersnatchSha512Ell2>() as u32
	}

	/// Creates a `RingExponent` from its exponent value.
	///
	/// This function returns an error if the exponent is not supported by `RingExponent`.
	pub fn new_from_exponent(value: u8) -> Result<Self, u8> {
		match value {
			9 => Ok(RingExponent::R2e9),
			10 => Ok(RingExponent::R2e10),
			14 => Ok(RingExponent::R2e14),
			n => Err(n),
		}
	}

	// This is a private function to allow us to shift to a fallible API in the future.
	const fn domain_size(&self) -> verifiable::ring::RingDomainSize {
		match self {
			RingExponent::R2e9 => verifiable::ring::RingDomainSize::Domain11,
			RingExponent::R2e10 => verifiable::ring::RingDomainSize::Domain12,
			RingExponent::R2e14 => verifiable::ring::RingDomainSize::Domain16,
		}
	}

	/// Returns the greatest supported `RingExponent`.
	pub const fn max_ring_exponent() -> Self {
		RingExponent::R2e14
	}

	/// All available ring exponents.
	pub const ALL: [RingExponent; 3] =
		[RingExponent::R2e9, RingExponent::R2e10, RingExponent::R2e14];
}

impl TryFrom<RingExponent> for verifiable::ring::RingDomainSize {
	type Error = ();

	fn try_from(value: RingExponent) -> Result<verifiable::ring::RingDomainSize, Self::Error> {
		Ok(value.domain_size())
	}
}

/// Allows mock `Config = ()` types to convert from `RingExponent`.
impl TryFrom<RingExponent> for () {
	type Error = ();
	fn try_from(_: RingExponent) -> Result<Self, Self::Error> {
		Ok(())
	}
}

/// Identifier used for communication among people, usually a public key of a crypto type allowing
/// for symmetric key generation using public keys.
pub type CommunicationIdentifier = [u8; 65];

#[derive(
	Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
)]
pub struct ContextualAlias {
	pub alias: Alias,
	pub context: Context,
}

/// Trait to recognize people and handle personal id.
///
/// `PersonalId` goes through multiple state: free, reserved, used; a used personal id can belong
/// to a recognized person or a suspended person.
pub trait AddOnlyPeopleTrait {
	type Member: Parameter + MaxEncodedLen;
	/// Reserve a new id for a future person. This id is not recognized, not reserved, and has
	/// never been reserved in the past.
	fn reserve_new_id() -> PersonalId;
	/// Renew a reservation for a personal id. The id is not recognized, but has been reserved in
	/// the past.
	///
	/// An error is returned if the id is used or wasn't reserved before.
	fn renew_id_reservation(personal_id: PersonalId) -> Result<(), DispatchError>;
	/// Cancel the reservation for a personal id
	///
	/// An error is returned if the id wasn't reserved in the first place.
	fn cancel_id_reservation(personal_id: PersonalId) -> Result<(), DispatchError>;
	/// Recognized a person.
	///
	/// The personal id must be reserved or the person must have already been recognized and
	/// suspended in the past.
	/// If recognizing a new person, a key must be provided. If resuming the personhood then no key
	/// must be provided.
	///
	/// An error is returned if:
	/// * `maybe_key` is some and the personal id was not reserved or is used by a recognized or
	///   suspended person.
	/// * `maybe_key` is none and the personal id was not recognized before.
	fn recognize_personhood(
		who: PersonalId,
		maybe_key: Option<Self::Member>,
	) -> Result<(), DispatchError>;
	// All stuff for benchmarks.
	#[cfg(feature = "runtime-benchmarks")]
	type Secret;
	#[cfg(feature = "runtime-benchmarks")]
	fn mock_key(who: PersonalId) -> (Self::Member, Self::Secret);
	/// Initializes the people collection in the member service.
	#[cfg(feature = "runtime-benchmarks")]
	fn initialize_people_collection();
}

/// Trait to recognize and suspend people.
pub trait PeopleTrait: AddOnlyPeopleTrait {
	/// Suspend a set of people. This operation must be called within a mutation session.
	///
	/// An error is returned if:
	/// * a suspended personal id was already suspended.
	/// * a personal id doesn't belong to any person.
	fn suspend_personhood(suspensions: &[PersonalId]) -> DispatchResult;
	/// Return whether the mutation session can be started.
	///
	/// The result of this operation holds until any call to `start_people_set_mutation_session`
	/// and `end_people_set_mutation_session` is made.
	fn can_start_people_set_mutation_session() -> bool;
	/// Start a mutation session for setting people.
	///
	/// An error is returned if the mutation session can be started at the moment. It is expected
	/// to become startable later.
	fn start_people_set_mutation_session() -> DispatchResult;
	/// End a mutation session for setting people.
	///
	/// An error is returned if there is no mutation session ongoing.
	fn end_people_set_mutation_session() -> DispatchResult;
}

impl AddOnlyPeopleTrait for () {
	type Member = ();
	fn reserve_new_id() -> PersonalId {
		0
	}
	fn renew_id_reservation(_: PersonalId) -> Result<(), DispatchError> {
		Ok(())
	}
	fn cancel_id_reservation(_: PersonalId) -> Result<(), DispatchError> {
		Ok(())
	}
	fn recognize_personhood(_: PersonalId, _: Option<Self::Member>) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	type Secret = PersonalId;
	#[cfg(feature = "runtime-benchmarks")]
	fn mock_key(who: PersonalId) -> (Self::Member, Self::Secret) {
		((), who)
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn initialize_people_collection() {}
}

impl PeopleTrait for () {
	fn suspend_personhood(_: &[PersonalId]) -> DispatchResult {
		Ok(())
	}
	fn start_people_set_mutation_session() -> DispatchResult {
		Ok(())
	}
	fn end_people_set_mutation_session() -> DispatchResult {
		Ok(())
	}
	fn can_start_people_set_mutation_session() -> bool {
		true
	}
}

/// Identifier type for a member collection.
pub type Identifier = [u8; 32];

/// Identifier for the People collection.
pub const PEOPLE_IDENTIFIER: &Identifier = b"pop:polkadot.network/people     ";

/// Identifier for the People Lite collection.
pub const PEOPLE_LITE_IDENTIFIER: &Identifier = b"pop:polkadot.network/people-lite";

/// Index type for ring revisions.
pub type RevisionIndex = u32;

/// Index type for queue pages.
pub type PageIndex = u32;

/// Mode of ring operation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub enum RingMode {
	/// Append-only rings.
	AppendOnly,
	/// Flexible collection where keys may be removed from any ring.
	Flexible,
}

/// Information about the current key inclusion status in a ring.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct RingStatus {
	/// The number of keys in the ring.
	pub total: u32,
	/// The number of keys that have already been baked in.
	pub included: u32,
	/// If present, represents the timestamp, in seconds since the UNIX epoch, of the moment the
	/// ring became immutable; this happens for `AppendOnly` rings when they become full.
	pub immutable_since: Option<u64>,
}

/// The state of a member's key within the pallet along with its position in relevant structures.
///
/// Differentiates between members included in a ring, those being onboarded and the suspended
/// ones. For those already included, provides ring index, page, and position within the page. For
/// those being onboarded, provides queue page index.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub enum RingPosition {
	/// Coordinates within the onboarding queue for a member that doesn't belong to a ring yet.
	Onboarding {
		queue_page: PageIndex,
		/// Timestamp (seconds since UNIX epoch) when the member was added to the queue.
		queued_at: u64,
	},
	/// Coordinates within the rings for a member that was registered.
	Included { ring_index: RingIndex, ring_page: PageIndex, ring_position: u32 },
	/// The member is suspended and isn't part of any ring or onboarding queue page.
	Suspended,
}

impl RingPosition {
	/// Returns whether the member is suspended and has no position.
	pub fn suspended(&self) -> bool {
		matches!(self, Self::Suspended)
	}

	/// Returns the index of the ring if this member is included.
	pub fn ring_index(&self) -> Option<RingIndex> {
		match &self {
			Self::Included { ring_index, .. } => Some(*ring_index),
			_ => None,
		}
	}
}

/// The mutation mode of member rings.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	DecodeWithMemTracking,
	Default,
)]
pub enum RingMutationMode {
	/// The rings can accept new members sequentially if the maximum capacity has not been reached
	/// yet. Ring building is permitted in this state by building the ring roots on top of
	/// previously computed roots. In case a ring suffered mutations that invalidated a previous
	/// ring root through the removal of an included member, the existing ring root will be removed
	/// and ring building will start from scratch.
	#[default]
	AppendOnly,
	/// A semaphore counting the number of entities making changes to the ring members list which
	/// require the entire ring to be rebuilt. Whenever an entity would want to remove members, it
	/// would first need to increment this counter and then start submitting the suspended indices.
	/// After all indices are registered, the counter is decremented. When the counter reaches 0,
	/// the state should always transition to `AppendOnly` through the provided methods. Ring
	/// merges are allowed only when no entity is allowed to suspend keys, so while in `AppendOnly`
	/// mode.
	Mutating(u8),
}

/// The overarching state of all member rings for a given identifier, tracking both the mutation
/// mode and the number of rings with roots.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	DecodeWithMemTracking,
	Default,
)]
pub struct RingMembersState {
	/// The current mutation mode of the rings.
	pub mode: RingMutationMode,
}

impl RingMembersState {
	/// Returns whether the state allows only incremental additions to rings and their roots.
	pub fn append_only(&self) -> bool {
		matches!(self.mode, RingMutationMode::AppendOnly)
	}

	/// Returns whether the state allows mutating the member set of rings.
	pub fn mutating(&self) -> bool {
		matches!(self.mode, RingMutationMode::Mutating(_))
	}

	/// Move to a mutation state.
	pub fn start_mutation_session(mut self) -> Result<Self, Self> {
		self.mode = match self.mode {
			RingMutationMode::AppendOnly => RingMutationMode::Mutating(1),
			RingMutationMode::Mutating(n) =>
				RingMutationMode::Mutating(n.checked_add(1).ok_or(self.clone())?),
		};
		Ok(self)
	}

	/// Move out of a mutation state.
	pub fn end_mutation_session(mut self) -> Result<Self, Self> {
		self.mode = match self.mode {
			RingMutationMode::AppendOnly => return Err(self),
			RingMutationMode::Mutating(1) => RingMutationMode::AppendOnly,
			RingMutationMode::Mutating(n) => RingMutationMode::Mutating(n.saturating_sub(1)),
		};
		Ok(self)
	}
}

/// An alias [`Alias`] used in a specific ring revision.
///
/// The revision can be used to tell in the future if an alias may have been suspended. For
/// instance, if a person is suspended, then ring will get revised, the revised alias with the old
/// revision shows that the alias may not be owned by a valid person anymore.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct RevisedAlias {
	pub revision: RevisionIndex,
	pub ring: RingIndex,
	pub alias: Alias,
}

/// A contextual alias used in a specific ring revision.
///
/// The revision can be used to tell in the future if an alias may have been suspended.
/// For instance, if a member is suspended, then ring will get revised, the revised alias with the
/// old revision shows that the alias may not be owned by a valid member anymore.
#[derive(
	Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
)]
pub struct RevisedContextualAlias {
	pub revision: RevisionIndex,
	pub ring: RingIndex,
	pub ca: ContextualAlias,
}

/// Verify ring membership proofs against a collection's rings.
///
/// Implementors expose both single and batch verification, as well as
/// revision-aware variants that accept proofs built against an older root
/// still retained by the implementation (e.g. during a grace period or within
/// a sliding window of recent roots). [`Self::ring_revision`] and
/// [`Self::is_revision_valid`] let callers discover which revisions are
/// currently accepted.
pub trait MembershipProver {
	type Crypto: GenerateVerifiable<
		Member: Parameter + MaxEncodedLen,
		Proof: Parameter + Send + Sync,
		Signature: Parameter + Send + Sync,
	>;

	/// Check whether the provided proof is valid for a member of a particular collection.
	fn verify_membership(
		identifier: &Identifier,
		proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		context: Context,
		msg: &[u8],
	) -> Result<RevisedContextualAlias, DispatchError>;
	/// Check whether the provided proof is valid for a member at a specific revision.
	///
	/// This allows verification against old roots that are retained during the grace period.
	/// If the revision matches the current root, it verifies against the current root.
	/// Otherwise, it looks up the root for old revisions.
	///
	/// Revisions from deleted collections and removed rings are instantly expired and will
	/// fail verification.
	fn verify_membership_at_rev(
		identifier: &Identifier,
		proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		revision: RevisionIndex,
		context: Context,
		msg: &[u8],
	) -> Result<ContextualAlias, DispatchError>;
	/// Batch-verify multiple membership proofs against one ring in a collection.
	///
	/// Each item in `items` carries a proof together with the message and context it was created
	/// for. The returned vector preserves input order exactly: the i-th element corresponds to the
	/// i-th input item.
	///
	/// The implementation loads collection metadata and the ring root **once** for the whole
	/// batch, then delegates to the crypto batch verifier.
	///
	/// The verifier may aggregate proofs internally for speed. On failure, it reports only that
	/// the batch is invalid and does not identify which individual proof failed.
	fn verify_memberships_in_ring(
		identifier: &Identifier,
		ring_index: RingIndex,
		items: &[BatchProofItem<<Self::Crypto as GenerateVerifiable>::Proof>],
	) -> Result<Vec<RevisedContextualAlias>, DispatchError>;
	/// Batch-verify multiple membership proofs against a specific revision of a ring's root.
	///
	/// Combines the batch verification of [`Self::verify_memberships_in_ring`] with the
	/// revision-aware root lookup of [`Self::verify_membership_at_rev`].
	///
	/// This allows verification against old roots that are retained during the grace period.
	/// If the revision matches the current root, it verifies against the current root.
	/// Otherwise, it looks up the root for old revisions.
	///
	/// Revisions from deleted collections and removed rings are instantly expired and will
	/// fail verification.
	///
	/// Each item in `items` carries a proof together with the message and context it was created
	/// for. The returned vector preserves input order exactly: the i-th element corresponds to the
	/// i-th input item.
	fn verify_memberships_in_ring_at_rev(
		identifier: &Identifier,
		ring_index: RingIndex,
		revision: RevisionIndex,
		items: &[BatchProofItem<<Self::Crypto as GenerateVerifiable>::Proof>],
	) -> Result<Vec<ContextualAlias>, DispatchError>;
	/// Query the current revision of a particular ring.
	///
	/// Returns `None` if the collection does not exist, the ring has no built root available.
	fn ring_revision(identifier: &Identifier, ring_index: RingIndex) -> Option<RevisionIndex>;
	/// Check if a revision is valid for a particular ring.
	///
	/// A revision is valid if it matches the current revision OR if it's an old revision
	/// that is still within the retention/grace period.
	///
	/// Revisions from deleted collections and removed rings are instantly expired and will
	/// return `false`.
	fn is_revision_valid(
		identifier: &Identifier,
		ring_index: RingIndex,
		revision: RevisionIndex,
	) -> bool;
	/// Returns the unix timestamp in seconds at which the given revision was committed on
	/// the source chain.
	///
	/// Returns `None` if the collection, ring, or revision is not currently retained.
	fn revision_source_time(
		identifier: &Identifier,
		ring_index: RingIndex,
		revision: RevisionIndex,
	) -> Option<u64>;
}

/// Multi-context membership proof verification.
///
/// A single proof carries multiple aliases, corresponding 1:1 to the `contexts`. A valid proof is
/// guaranteed to have been created by a single ring member.
pub trait MembershipMultiProver: MembershipProver {
	/// Verify a multi-context proof against the current sliding window of recent roots.
	fn verify_membership_multi_context(
		identifier: &Identifier,
		proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		contexts: &[Context],
		msg: &[u8],
	) -> Result<Vec<RevisedContextualAlias>, DispatchError>;

	/// Verify a multi-context proof against a specific revision.
	fn verify_membership_multi_context_at_rev(
		identifier: &Identifier,
		proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		revision: RevisionIndex,
		contexts: &[Context],
		msg: &[u8],
	) -> Result<Vec<ContextualAlias>, DispatchError>;
}

/// Trait to manage append-only member sets, in addition to verifying proofs of membership to these
/// sets.
pub trait AppendOnlyMembers: MembershipProver {
	type Location: Parameter + MaxEncodedLen;

	/// Create a member set collection with a particular identifier.
	///
	/// The `ring_size` parameter specifies the exponent for the ring capacity (2^exponent - 257).
	/// The `self_inclusion_delay` parameter specifies the minimum time in seconds a member must
	/// wait in the onboarding queue before they can self-include via a signed extrinsic. `None`
	/// disables self-inclusion.
	fn create_collection(
		owner: Self::Location,
		identifier: &Identifier,
		onboarding_size: u32,
		mode: RingMode,
		ring_size: RingExponent,
		self_inclusion_delay: Option<u64>,
	) -> DispatchResult;
	/// Delete a member set collection with a particular identifier.
	///
	/// All of the information associated with the collection will be removed.
	fn delete_collection(owner: Self::Location, identifier: &Identifier) -> DispatchResult;
	/// Returns the number of active members in a set.
	fn active_count(identifier: &Identifier) -> u32;
	/// Add members in a particular collection.
	fn add_members(
		identifier: &Identifier,
		members: Vec<<Self::Crypto as GenerateVerifiable>::Member>,
	) -> DispatchResult;
	/// Remove a ring from a collection, but not the current building ring. Attempting to remove the
	/// top-most ring will result in an error.
	///
	/// Once a ring is removed, its revisions are instantly expired and all verification functions
	/// (`verify_membership_at_rev`, `verify_memberships_in_ring_at_rev`, `is_revision_valid`)
	/// will reject proofs against them, even if the old roots are still retained in storage.
	fn remove_ring(identifier: &Identifier, ring_index: RingIndex) -> DispatchResult;
	/// Query the status of a particular ring.
	fn ring_status(identifier: &Identifier, ring_index: RingIndex) -> Option<RingStatus>;
	/// Query the status of a particular member.
	fn member_status(
		identifier: &Identifier,
		member: &<Self::Crypto as GenerateVerifiable>::Member,
	) -> Option<RingPosition>;
	/// Query the list of all members of a ring in a collection.
	fn ring_members(
		identifier: &Identifier,
		ring_index: RingIndex,
	) -> Vec<<Self::Crypto as GenerateVerifiable>::Member>;
	/// Returns the number of active members in a set.
	#[cfg(feature = "runtime-benchmarks")]
	fn set_active_count(identifier: &Identifier, count: u32);
	/// Initializes the chunks used by the member set for a particular ring size.
	#[cfg(feature = "runtime-benchmarks")]
	fn initialize_chunks(ring_size: RingExponent);
	/// Onboards all queued members and builds the ring until all are included.
	#[cfg(feature = "runtime-benchmarks")]
	fn onboard_all_and_build_ring(identifier: &Identifier, ring_index: RingIndex)
		-> DispatchResult;
}

/// Provides relay-chain per-block randomness. Used wherever a winner check would be
/// vulnerable to grinding under the public, slowly-rotating epoch randomness.
///
/// The primary source is the relay chain per-block randomness
/// (`CURRENT_BLOCK_RANDOMNESS`). Per-block randomness is required because users may
/// have partial control over their account ID or alias and the winner threshold can
/// be low: any source that is fixed and public ahead of time (e.g.
/// `ONE_EPOCH_AGO_RANDOMNESS`, which only rotates each BABE epoch) lets a user grind
/// an account / alias / submission time that wins under the known value.
///
/// The tradeoff is that a collator scheduled to build at relay parent N can choose
/// silence, forcing the parachain to use relay parent N+1's randomness instead, at
/// the cost of the rewards that slot would have paid them. Each reroll costs one
/// block of rewards and only affects the slots that collator was assigned, so the
/// attack is bounded and metered on-chain.
///
/// Returns `None` when no usable randomness is available; callers must skip the
/// action (e.g. opening a prize pool, finalizing a scratch reward) rather than
/// substitute a predictable value.
pub trait CurrentBlockRandomness {
	/// Returns randomness for winner selection.
	fn randomness() -> Option<[u8; 32]>;

	/// Set up relay state so that `randomness()` returns a valid value. Used in
	/// benchmark setup.
	#[cfg(feature = "runtime-benchmarks")]
	fn setup_randomness();
}

/// Trait to add and remove members.
pub trait FlexibleMembers: AppendOnlyMembers {
	/// Remove a list of members. This operation must be called within a removal session.
	///
	/// An error is returned if a member in the `suspensions` list was already suspended.
	fn remove_members(
		identifier: &Identifier,
		suspensions: &[<Self::Crypto as GenerateVerifiable>::Member],
	) -> DispatchResult;
	/// Start a removal session for removing members.
	fn start_removal_session(identifier: &Identifier) -> DispatchResult;
	/// End a removal session for removing members.
	fn end_removal_session(identifier: &Identifier) -> DispatchResult;
	/// Fetches the R/W state of all rings of a collection.
	fn rings_state(identifier: &Identifier) -> RingMembersState;
}

/// Trait to get the total number of active members in a set.
pub trait CountedMembers {
	/// Returns the number of active members in the set.
	fn active_count() -> u32;

	/// Sets the number of active members in the set.
	#[cfg(feature = "runtime-benchmarks")]
	fn set_active_count(count: u32);
}

/// Username type used in individuality systems.
///
/// WARNING
///
/// Changing the maximum length of this type will require a migration in all pallets using it!
pub type Username = BoundedVec<u8, ConstU32<32>>;

/// Service for registering consumers.
pub trait ConsumerRegistrar<AccountId> {
	type Error;

	/// Register a lite consumer using the provided information.
	///
	/// IMPORTANT
	///
	/// This function does not check for authorization. The caller is responsible for ensuring
	/// the `account` to be registered is a lite person and that the user's consent was
	/// provided, usually through a signature verified by the caller.
	fn register_lite_consumer(
		account: AccountId,
		identifier_key: CommunicationIdentifier,
		username: Username,
		reserved_username: Option<Username>,
	) -> Result<(), Self::Error>;
}

impl<Account> ConsumerRegistrar<Account> for () {
	type Error = &'static str;

	fn register_lite_consumer(
		_account: Account,
		_identifier_key: CommunicationIdentifier,
		_username: Username,
		_reserved_username: Option<Username>,
	) -> Result<(), Self::Error> {
		Ok(())
	}
}

#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	DecodeWithMemTracking,
)]
pub enum Truth {
	/// The evidence can be taken as a clear indication that the statement is true. Doubt may still
	/// remain but it should be unlikely (no more than 1 chance in 20) that this doubt would be
	/// substantial enough to contravene the evidence.
	True,
	/// The evidence contradicts the statement.
	False,
}

#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	DecodeWithMemTracking,
)]
pub enum Judgement {
	/// A judgement on the truth of a statement.
	Truth(Truth),
	/// The evidence supplied probably (P > 50%) implies contempt for the system. Submitting
	/// evidence which clearly appears to be manipulated or intentionally provides no indication of
	/// truth for the statement would imply this outcome.
	Contempt,
}

impl Judgement {
	pub fn matches_intent(&self, j: Self) -> bool {
		use self::Truth::*;
		use Judgement::*;
		matches!(
			(self, j),
			(Truth(True), Truth(True)) | (Truth(False), Truth(False)) | (Contempt, Contempt)
		)
	}
}

pub type EvidenceHash = [u8; 32];

pub mod proof_of_ink {
	use super::*;

	pub type AccountId = [u8; 32];
	pub type ProceduralSeed = [u8; 4];
	pub type FamilyIndex = u16;
	pub type DesignIndex = u16;

	#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub enum InkSpec {
		DesignedElective(FamilyIndex, DesignIndex),
		ProceduralAccount(FamilyIndex, AccountId),
		ProceduralPersonal(FamilyIndex, PersonalId),
		Procedural(FamilyIndex, ProceduralSeed),
		// Not yet available, but planned.
		//		ProceduralShielded(FamilyIndex, Entropy, VariantIndex, ShieldCommit),
		//		ProceduralShieldedDerivative(PersonalId, Option<PersonalId>),
	}
}
pub use proof_of_ink::InkSpec;

pub mod identity {
	use super::*;

	/// Social platforms that statement oracles support.
	#[derive(
		Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
	)]
	pub enum Social {
		Twitter { username: Data },
		Github { username: Data },
		Discord { display_and_tag: Data },
	}

	impl Social {
		pub fn eq_platform(&self, other: &Social) -> bool {
			matches!(
				(&self, &other),
				(Social::Twitter { .. }, Social::Twitter { .. }) |
					(Social::Github { .. }, Social::Github { .. }) |
					(Social::Discord { .. }, Social::Discord { .. })
			)
		}
	}

	/// Data type for arbitrary information handled by the statement oracle.
	// #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub type Data = BoundedVec<u8, ConstU32<32>>;

	// impl TryFrom<&[u8]> for Data {
	// 	type Error = &'static str;

	// 	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
	// 		let mut buf = [0u8; 32];
	// 		let len = value.len();
	// 		if len <= 32 {
	// 			(&mut buf[..len]).copy_from_slice(&value[..len]);
	// 			Ok(Self(buf))
	// 		} else {
	// 			Err("data too long")
	// 		}
	// 	}
	// }
}

pub use identity::{Data as IdentityData, Social};

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum Statement {
	/// Ask for whether evidence exists to confirm that a particular tattoo uniquely exists at a
	/// particular place on somebody's body.
	///
	/// The particular tattoo is that which is defined by `design`. The place on the body it must
	/// exist is TODO.
	///
	/// The statement comes with `evidence` to provide a hint of what evidence should be considered
	/// in the decision making. This may not be comprehensive.
	///
	/// If `probable_acceptable` is `true`, then the evidence is expected only to provide at best a
	/// `Truth::Probable` judgement and further evidence will be supplied for a substantial
	/// judgement. If `probable_acceptable` is `false`, then the evidence is expected to provide a
	/// `Confident` outcome and `Truth::Probable` will be considered a failure of the evidence to
	/// substantiate the claim.
	ProofOfInk { design: proof_of_ink::InkSpec, evidence: EvidenceHash, probable_acceptable: bool },
	/// Ask for whether evidence exists to confirm that a particular social credential on a
	/// supported platform belongs to a person.
	IdentityCredential { platform: identity::Social, evidence: identity::Data },
	/// Ask for whether a username meets certain standards.
	///
	/// It is up to the oracle to decide upon username validity,
	/// but it may be assumed that a username is considered acceptable if it:
	/// - contains no offensive, discriminatory, or inappropriate content,
	/// - is visually distinct and readable in user interfaces,
	/// - complies with other oracle guidelines.
	UsernameValid { username: identity::Data },
}

pub const CONTEXT_SIZE: u32 = 64;
pub type JudgementContext = BoundedVec<u8, ConstU32<CONTEXT_SIZE>>;

#[derive(
	CloneNoBound, PartialEqNoBound, EqNoBound, Debug, Encode, Decode, MaxEncodedLen, TypeInfo,
)]
#[scale_info(skip_type_params(Params, RuntimeCall))]
#[codec(mel_bound())]
pub struct Callback<Params, RuntimeCall> {
	pallet_index: u8,
	call_index: u8,
	phantom: PhantomData<(Params, RuntimeCall)>,
}
impl<Params: Encode, RuntimeCall: Decode> Callback<Params, RuntimeCall> {
	pub const fn from_parts(pallet_index: u8, call_index: u8) -> Self {
		Self { pallet_index, call_index, phantom: PhantomData }
	}
	pub fn curry(&self, args: Params) -> Result<RuntimeCall, codec::Error> {
		(self.pallet_index, self.call_index, args).using_encoded(|mut d| Decode::decode(&mut d))
	}
}

/// A provider of wonderous magic: give it a `Statement` and it will tell you if it's true, with
/// some degree of resilience.
///
/// It's asynchronous, so you give it a callback in the form of a `RuntimeCall` stub.
pub trait StatementOracle<RuntimeCall> {
	/// A small piece of data which may be used to identify different ongoing judgements.
	type Ticket: Member + FullCodec + TypeInfo + MaxEncodedLen + Default;

	/// Judge a `statement` and get a Judgement.
	///
	/// We only care about the pallet/call index of `callback`; it must take exactly three
	/// arguments:
	///
	/// - `Self::Ticket`: The ticket which was returned here to identify the judgement.
	/// - `JudgementContext`: The value of `context` which was passed in to this call.
	/// - `Judgement`: The judgement given by the oracle.
	///
	/// It is assumed that all costs associated with this oraclisation have already been paid for
	/// or are absorbed by the system acting in its own interests.
	fn judge_statement(
		statement: Statement,
		context: JudgementContext,
		callback: Callback<(Self::Ticket, JudgementContext, Judgement), RuntimeCall>,
	) -> Result<Self::Ticket, DispatchError>;
}

impl<C> StatementOracle<C> for () {
	type Ticket = ();
	fn judge_statement(
		_: Statement,
		_: JudgementContext,
		_: Callback<(Self::Ticket, JudgementContext, Judgement), C>,
	) -> Result<(), DispatchError> {
		Err(DispatchError::Unavailable)
	}
}

/// Bundle of inputs to [`PersonhoodLookup::personhood_info_by_proof`].
pub struct PersonhoodProofRequest<'a, Proof> {
	/// Collection the proof is being verified against.
	pub identifier: Identifier,
	/// Ring-signature proof.
	pub proof: Proof,
	/// Alias the proof is claimed to derive in `context`.
	pub alias: Alias,
	/// Index of the ring within the collection.
	pub ring_index: RingIndex,
	/// Application context the alias is derived under.
	pub context: Context,
	/// Revision of the ring root the proof was created against.
	pub revision: RevisionIndex,
	/// Message bound into the proof.
	pub message: &'a [u8],
}

pub trait PersonhoodLookup<AccountId, Proof> {
	/// Worst-case weight that `personhood_info` may consume.
	fn personhood_info_weight() -> Weight;

	/// Returns `((collection, alias), actual_weight)`
	/// for the account in the given context.
	///
	/// Returns `None` if the account is not registered, the context doesn't match,
	/// the ring has been deleted, or the alias revision is too old (past the grace
	/// period).
	fn personhood_info(
		account: &AccountId,
		context: &Context,
	) -> (Option<(Identifier, Alias)>, Weight);

	/// Worst-case weight that `personhood_info_by_proof` may consume.
	fn personhood_info_by_proof_weight() -> Weight;

	/// Verifies the proof in `request` against the collection it specifies.
	/// Returns `false` when the ring/revision is unknown, the proof is invalid,
	/// or the derived alias differs from the claimed one.
	fn personhood_info_by_proof(request: PersonhoodProofRequest<'_, Proof>) -> (bool, Weight);
}

/// Trait for unconditionally cleaning up alias-to-account mappings.
pub trait CleanUpAlias {
	/// Remove an alias-to-account mapping unconditionally.
	///
	/// Returns `Ok(())` if the mapping was removed, or an error if it didn't exist.
	fn clean_up_alias(ca: ContextualAlias) -> DispatchResult;
}

impl CleanUpAlias for () {
	fn clean_up_alias(_ca: ContextualAlias) -> DispatchResult {
		Ok(())
	}
}

/// An abstract interface for allocating storage on a remote chain (e.g. the Bulletin chain).
pub trait AllocateStorage<AccountId> {
	fn allocate_storage(who: &AccountId, len: u64, count: u32) -> DispatchResult;
	fn refresh_allocation(who: &AccountId) -> DispatchResult;
}

impl<A> AllocateStorage<A> for () {
	fn allocate_storage(_: &A, _: u64, _: u32) -> DispatchResult {
		Ok(())
	}
	fn refresh_allocation(_: &A) -> DispatchResult {
		Ok(())
	}
}

/// Some weight information for `AppendOnlyMembers`.
pub trait AppendOnlyMembersWeightInfo {
	/// The amortised cost per key for background operations that happens when inserting a key
	/// into a member set.
	///
	/// In a typical implementation this would account for background operation such as onboarding
	/// and ring building if they happen outside of the `add_members` function.
	fn add_member_background_weight() -> frame_support::weights::Weight;
}

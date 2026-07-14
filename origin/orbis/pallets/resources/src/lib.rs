// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Personhood resource allowances and anonymous resource claims.
//!
//! Namespace ownership is intentionally absent: native Orbis Names is the sole owner of name
//! registration, reservation, and resolution for the clean-break Orbis network.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use core::mem::size_of;
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod extension;
pub mod types;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use frame_support::{
	dispatch::DispatchResultWithPostInfo,
	traits::{EnsureOriginWithArg, IsSubType, OriginTrait, UnixTime},
};
use frame_system::offchain::{CreateAuthorizedTransaction, SubmitTransaction};
use indiv_support::{
	traits::{
		Alias, AppendOnlyMembers, ClaimCleanupOutcome, CommunicationIdentifier, ConsumerRegistrar,
		Context, MembershipProver, ResourceClaimLifecycle, RingExponent, TwoPhaseStorage,
	},
	utils::BigEndianU32,
};
use sp_runtime::traits::{IdentifyAccount, Verify};
use types::{
	ConsumerInfo, Credibility, FriendRequestReference, LongTermStorageAllocation,
	MembershipCollection, ReservationId, ReservationPurpose, StmtStoreAllowanceEntry, StorageClaim,
};
use verifiable::GenerateVerifiable;

// TODO:
// - Get rid of the "friend request" naming.
// - Unify all the different allowances we have for the statement store (account allowance,
//   (lite)people and friend request), at least use the same grace period, claim time, etc. even if
//   the allowance amounts are different. Ideally, with the addition of batch claims, people can
//   claim multiple allowances into the same account all at once and we can use only one allowance
//   type per resource.

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Saturating, Zero};
	use sp_statement_store::{decrease_allowance_by, increase_allowance_by, StatementAllowance};

	pub const RESOURCES_CONTEXT: Context = *b"pop:polkadot.network/resources  ";

	const LOG_TARGET: &str = "runtime::indiv-pallet-resources";
	const FRIEND_REQUEST_CONTEXT_PREFIX: &[u8; 9] = b"FRND_REQ:";
	const STMT_STORE_SLOT_CONTEXT_PREFIX: &[u8; 9] = b"SSS_SLOT:";
	const LONG_TERM_STORAGE_CONTEXT_BASE: [u8; 24] = *b"pop:polkadot.net/rsc-lts";
	pub(crate) const SECONDS_PER_DAY: u64 = 86_400;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config<
			RuntimeOrigin: From<Origin<Self>>
			                   + From<<Self::RuntimeOrigin as OriginTrait>::PalletsOrigin>
			                   + OriginTrait<
				PalletsOrigin: From<Origin<Self>>
				                   + TryInto<
					Origin<Self>,
					Error = <Self::RuntimeOrigin as OriginTrait>::PalletsOrigin,
				>,
			>,
			RuntimeCall: IsSubType<Call<Self>>,
			AccountId: From<sp_statement_store::AccountId> + Into<sp_statement_store::AccountId>,
		> + CreateAuthorizedTransaction<Call<Self>>
		+ Send
		+ Sync
	{
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Trait allowing cryptographic proof of membership without exposing the underlying member.
		/// Normally a Ring-VRF.
		///
		/// This must be the member service for both people and lite people collections.
		type MemberService: AppendOnlyMembers
			+ MembershipProver<
				Crypto: GenerateVerifiable<
					Proof: Send + Sync + DecodeWithMemTracking,
					Signature: Send + Sync + DecodeWithMemTracking,
					Member: DecodeWithMemTracking,
					Config: TryFrom<RingExponent>,
				>,
			>;

		/// The duration of time, in seconds, for which a person's authorization is valid. After
		/// this period elapses, people will no longer be considered active, but their resource
		/// allowances should default to the same values used for lite people.
		#[pallet::constant]
		type PersonAuthDuration: Get<u32>;

		/// The minimum interval of time, in seconds, which must pass before updating a person's
		/// authorization.
		#[pallet::constant]
		type MinPersonAuthUpdateInterval: Get<u32>;

		/// The Statement Store allowance for the accounts API.
		#[pallet::constant]
		type AccountsApiAllowance: Get<StatementAllowance>;

		/// Maximum number of statement store slots a person can claim within one period.
		#[pallet::constant]
		type StmtStoreSlotsPerPeriod: Get<u32>;

		/// Maximum number of statement store slots a lite person can claim within one period.
		///
		/// Same semantics as `StmtStoreSlotsPerPeriod` but applied when the proof targets the
		/// lite-people collection via `MembershipCollection::LitePeople`.
		#[pallet::constant]
		type LiteStmtStoreSlotsPerPeriod: Get<u32>;

		/// Maximum number of stale statement store allowance entries to remove per cleanup call.
		#[pallet::constant]
		type StmtStoreCleanupLimit: Get<u32>;

		/// Minimum time, in seconds, that must pass before an alias can replace its own
		/// statement store allowance entry within the same period.
		#[pallet::constant]
		type StmtStoreReplacementCooldown: Get<u32>;

		/// Extra time, in seconds, during which statement-store allowances from an ended period
		/// remain active before cleanup may revoke them.
		///
		/// After this elapses, the allowances will eventually be cleaned by the OCW.
		#[pallet::constant]
		type StmtStoreGraceWindow: Get<u32>;

		/// The Statement Store allowance for friend request statement registration.
		#[pallet::constant]
		type FriendRequestAllowance: Get<StatementAllowance>;

		/// Maximum number of friend requests a person can send within one rate-limit period.
		///
		/// For example, if this is `8`, each person can send up to 8 friend requests during the
		/// period selected by `FriendRequestPeriodDuration`. When the period advances, the slots
		/// reset.
		#[pallet::constant]
		type FriendRequestSlotsPerPeriod: Get<u8>;

		/// Maximum number of friend requests a lite person can send within one rate-limit period.
		///
		/// Same semantics as `FriendRequestSlotsPerPeriod` but applied when the proof targets the
		/// lite-people collection via `MembershipCollection::LitePeople`.
		#[pallet::constant]
		type LiteFriendRequestSlotsPerPeriod: Get<u8>;

		/// Rolling time window for rate-limiting friend requests, in seconds.
		///
		/// Time is divided into fixed-duration periods. The period index is computed as
		/// `now_secs / FriendRequestPeriodDuration`.
		///
		/// For example, if this is `86_400` (24 hours), period `0` is the first 24 hours since the
		/// Unix epoch, period `1` is the next 24 hours, and so on. Combined with
		/// `FriendRequestSlotsPerPeriod`, this defines how many friend requests can be sent in each
		/// period.
		#[pallet::constant]
		type FriendRequestPeriodDuration: Get<u32>;

		/// Extra time, in seconds, during which the previous friend request period is still
		/// accepted after a rollover.
		///
		/// This allows transactions created close to a period boundary to still be included even if
		/// they are executed just after the next period begins.
		#[pallet::constant]
		type FriendRequestGraceWindow: Get<u32>;

		/// Duration for which friend request registrations will be retained. Specified in seconds.
		///
		/// A registration is created for a specific period. Once this period ends,
		/// the registration remains valid for the configured duration, after which
		/// it can be cleaned up. See `friend_request_expiration_time`.
		#[pallet::constant]
		type FriendRequestRetentionDuration: Get<u64>;

		/// Number of blocks between offchain-worker maintenance runs.
		#[pallet::constant]
		type OffchainWorkerInterval: Get<BlockNumberFor<Self>>;

		/// How to recognise an origin representing a person.
		type EnsurePerson: EnsureOriginWithArg<OriginFor<Self>, Context, Success = Alias>;

		/// How to recognise an origin representing a lite person.
		type EnsureLitePerson: EnsureOrigin<OriginFor<Self>, Success = Self::AccountId>;

		/// The origin allowed to perform privileged management operations on this pallet.
		/// The source of time.
		type Clock: UnixTime;

		/// Signature type for ensuring ownership of provided accounts in case of registrations
		/// through alias.
		type OffchainSignature: Verify<Signer: IdentifyAccount<AccountId = Self::AccountId>>
			+ Parameter;

		/// The limit for the statement store usage for lite people.
		type LitePersonStatementLimit: Get<StatementAllowance>;

		/// The limit for the statement store usage for people. Must be equal to or greater than the
		/// lite person limit.
		type PersonStatementLimit: Get<StatementAllowance>;

		/// The duration of a long-term storage claiming period, in seconds.
		///
		/// Time is divided into fixed-duration periods. The period index is computed as
		/// `now_secs / LongTermStoragePeriodDuration`. Each person can submit up to
		/// `LongTermStorageClaimsPerPeriod` claims per period.
		#[pallet::constant]
		type LongTermStoragePeriodDuration: Get<u32>;

		/// Maximum number of long-term storage claims per person per period.
		///
		/// Each claim uses a different counter value (0..claims_per_period) which produces a
		/// distinct alias in the proof context, ensuring one claim per counter slot.
		#[pallet::constant]
		type LongTermStorageClaimsPerPeriod: Get<u8>;

		/// Extra time, in seconds, during which the previous long-term storage period is still
		/// accepted after a rollover.
		///
		/// Same semantics as `FriendRequestGraceWindow` but applied to long-term storage claims.
		#[pallet::constant]
		type LongTermStorageGraceWindow: Get<u32>;

		/// The long-term storage allocation granted per claim for people.
		type LongTermStorageAllowanceForPeople: Get<LongTermStorageAllocation>;

		/// The long-term storage allocation granted per claim for lite people.
		type LongTermStorageAllowanceForLitePeople: Get<LongTermStorageAllocation>;

		/// Atomic isolated-capacity backend for real Orbis Storage content.
		type LongTermStorageDataStore: TwoPhaseStorage<
			Self::AccountId,
			ReservationId,
			ReservationPurpose,
			BlockNumberFor<Self>,
		>;

		#[pallet::constant]
		type MaxReservations: Get<u32>;

		#[pallet::constant]
		type StorageReservationDuration: Get<BlockNumberFor<Self>>;

		/// Maximum number of spent long-term storage aliases that can be cleared in a single
		/// `clear_expired_long_term_storage_aliases` call.
		///
		/// Bounds the worst-case weight of the cleanup extrinsic; callers must pass a `limit`
		/// no greater than this value.
		#[pallet::constant]
		type LongTermStorageCleanupLimit: Get<u32>;

		/// Benchmark helper trait.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: benchmarking::BenchmarkHelper<Self>;

		/// Runtime-specific account-bound Meta policy benchmark adapter.
		#[cfg(feature = "runtime-benchmarks")]
		type MetaPolicyBenchmarkHelper: benchmarking::MetaPolicyBenchmarkHelper;
	}

	#[pallet::origin]
	#[derive(
		CloneNoBound,
		PartialEqNoBound,
		EqNoBound,
		DebugNoBound,
		Encode,
		Decode,
		MaxEncodedLen,
		TypeInfo,
		DecodeWithMemTracking,
	)]
	#[scale_info(skip_type_params(T))]
	pub enum Origin<T: Config> {
		/// A friend request alias origin, produced by the `AsResources` transaction extension.
		FriendRequestAlias(Alias),
		/// A statement store slot alias origin, produced by the `AsResources` transaction
		/// extension after validating a ring-VRF proof for a specific slot context.
		StmtStoreAlias(Alias),
		/// A long-term storage claim origin, produced by the `AsResources` transaction extension.
		/// Carries the anonymous alias, the collection used for proof verification, and the
		/// validated signed extrinsic payer.
		LongTermStorageClaim { alias: Alias, collection: MembershipCollection, payer: T::AccountId },
	}

	/// Accounts used to identify consumers mapped to their consumer information.
	#[pallet::storage]
	pub type Consumers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ConsumerInfo>;

	/// Accounts associated with a statement store slot through an anonymous allowance, per period.
	///
	/// The period key is a big-endian encoded day number (seconds since Unix epoch / 86400) so
	/// that `Identity`-hashed iteration yields entries in chronological order to be removed by the
	/// offchain worker.
	#[pallet::storage]
	pub type StatementStoreAllowances<T: Config> = StorageDoubleMap<
		_,
		Identity,
		BigEndianU32,
		Blake2_128Concat,
		Alias,
		StmtStoreAllowanceEntry<T>,
		OptionQuery,
	>;

	/// Reverse lookup from a statement account to all its active anonymous allowances.
	///
	/// Keyed by `(AccountId, (BigEndianU32 period, u32 seq, Alias))` → `()`. Multiple
	/// entries per account are possible when the same statement account is authorized by
	/// different aliases or across grace-window overlaps.
	#[pallet::storage]
	pub type StmtStoreAllowanceByAccount<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		(BigEndianU32, u32, Alias),
		(),
		OptionQuery,
	>;

	/// Friend request registration by anonymous alias in friend request context.
	#[pallet::storage]
	pub type FriendRequestRegistrationByAlias<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Alias,
		types::FriendRequestRegistration<T::AccountId>,
		OptionQuery,
	>;

	/// Reverse lookup from friend request statement account to anonymous alias.
	#[pallet::storage]
	pub type FriendRequestAliasByAccount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Alias, OptionQuery>;

	/// Aliases that have already been used to claim long-term storage in a given period.
	///
	/// Keyed by `(period, alias)`. Each counter value in the proof context produces a unique
	/// alias, so a person can have up to `LongTermStorageClaimsPerPeriod` entries per period.
	/// Old periods can be cleaned up via `clear_expired_long_term_storage_aliases`.
	///
	/// The period key is `BigEndianU32` with `Identity` so iteration yields entries in
	/// chronological order, matching `StatementStoreAllowances`.
	#[pallet::storage]
	pub type SpentLongTermStorageAliases<T: Config> =
		StorageDoubleMap<_, Identity, BigEndianU32, Blake2_128Concat, Alias, (), OptionQuery>;

	#[pallet::storage]
	pub type NextStorageReservationId<T: Config> = StorageValue<_, ReservationId, ValueQuery>;

	#[pallet::storage]
	pub type StorageClaims<T: Config> =
		CountedStorageMap<_, Blake2_128Concat, ReservationId, StorageClaim<T>, OptionQuery>;

	#[pallet::storage]
	pub type StorageReservationByPurpose<T: Config> =
		StorageMap<_, Blake2_128Concat, ReservationPurpose, ReservationId, OptionQuery>;

	/// Reverse lookup from registered aliases to the `AccountId` used to register as a consumer.
	#[pallet::storage]
	pub type AccountOfAlias<T: Config> =
		StorageMap<_, Blake2_128Concat, Alias, T::AccountId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A person has registered as a consumer.
		PersonRegistered {
			alias: Alias,
			account: T::AccountId,
		},
		/// A lite person has registered as a consumer.
		LitePersonRegistered {
			account: T::AccountId,
		},
		/// Friend request statement usage has been assigned for a sequence.
		FriendRequestStmtUsageSet {
			alias: Alias,
			period: u32,
			seq: u8,
			account: T::AccountId,
		},
		/// Friend request statement usage has been removed.
		FriendRequestStmtUsageRemoved {
			account: T::AccountId,
		},
		/// A person's authorization was touched.
		PersonAuthorizationTouched {
			account: T::AccountId,
		},
		/// A consumer's identifier key was updated.
		IdentifierKeyUpdated {
			account: T::AccountId,
		},
		/// An anonymous statement store allowance was granted.
		StmtStoreAllowanceSet {
			alias: Alias,
			period: u32,
			seq: u32,
			account: T::AccountId,
		},
		/// Expired statement store allowances were cleaned up.
		StmtStoreAllowancesCleared {
			period: u32,
			first_key: Alias,
			count: u32,
		},
		/// A full person was demoted due to expired authorization.
		PersonDemoted {
			account: T::AccountId,
		},
		/// Isolated Orbis Storage capacity has been reserved for a membership claim.
		LongTermStorageReserved {
			reservation_id: ReservationId,
			alias: Alias,
			period: u32,
			counter: u8,
			account: T::AccountId,
			collection: MembershipCollection,
		},
		LongTermStorageReservationCancelled {
			reservation_id: ReservationId,
			account: T::AccountId,
		},
		LongTermStorageReservationExpired {
			reservation_id: ReservationId,
		},
		/// Expired long-term storage aliases have been cleared for a period.
		LongTermStorageAliasesCleared {
			period: u32,
			count: u32,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Consumer is already registered.
		AlreadyRegistered,
		/// Provided proof of ownership is invalid.
		InvalidProofOfOwnership,
		/// Person is not registered as a consumer.
		NotRegistered,
		/// Consumer is not a full person.
		NotFullPerson,
		/// Attempted to update person authorization too early.
		TouchNotReady,
		/// There is no lite consumer to be linked.
		NoLinkedIdentity,
		/// The lite consumer is already linked to a full person consumer.
		AlreadyLinked,
		/// The person's authorization has not expired yet.
		PersonAuthNotExpired,
		/// The person has already been demoted.
		AlreadyDemoted,
		/// Friend request sequence is invalid for the consumer.
		InvalidFriendRequestSequence,
		/// Friend request period is not the current period.
		InvalidFriendRequestPeriod,
		/// Friend request registration is not expired yet.
		FriendRequestRegistrationNotExpired,
		/// Friend request registration already exists for the alias/context.
		FriendRequestRegistrationAlreadyExists,
		/// The replacement cooldown has not elapsed since the entry was last set.
		StmtStoreReplacementTooEarly,
		/// The provided `limit` exceeds `LongTermStorageCleanupLimit`.
		LongTermStorageCleanupLimitExceeded,
		ReservationBackendFailed,
		ReservationIdOverflow,
		ReservationNotFound,
		NotReservationOwner,
		ClaimAlreadyReserved,
		CleanupLimitExceeded,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			if !(block_number % T::OffchainWorkerInterval::get()).is_zero() {
				return;
			}

			for registration in FriendRequestRegistrationByAlias::<T>::iter_values() {
				if !Self::should_clear_friend_request_registration(&registration) {
					continue;
				}

				let call = Call::clear_expired_friend_request_sequence {
					account: registration.account_id,
					seq: registration.reference.seq,
				};
				Self::submit_authorized_transaction(call, "Clear expired friend request sequence");
			}

			// Clean up stale statement store allowances.
			// Check the first (oldest) period key in the map. Because the period key is
			// big-endian encoded under `Identity`, iteration yields periods in ascending order.
			if let Some((oldest_period_key, first_alias, _)) =
				StatementStoreAllowances::<T>::iter().next()
			{
				let oldest_period: u32 = oldest_period_key.into();
				if Self::is_stmt_store_period_clearable(oldest_period) {
					let call = Call::clear_expired_stmt_store_allowances {
						period: oldest_period,
						first_entry: first_alias,
					};
					Self::submit_authorized_transaction(
						call,
						"Clear expired statement store allowances",
					);
				}
			}

			// Clean up stale long-term storage aliases. Same trick: the period key is
			// `BigEndianU32` under `Identity`, so iteration yields the oldest period first;
			// if the oldest isn't clearable, none are.
			if let Some((oldest_period_key, _alias)) =
				SpentLongTermStorageAliases::<T>::iter_keys().next()
			{
				let oldest_period: u32 = oldest_period_key.into();
				if Self::is_long_term_storage_period_clearable(oldest_period) {
					let call = Call::clear_expired_long_term_storage_aliases {
						period: oldest_period,
						limit: T::LongTermStorageCleanupLimit::get(),
					};
					Self::submit_authorized_transaction(
						call,
						"Clear expired long-term storage aliases",
					);
				}
			}
		}

		fn integrity_test() {
			assert!(
				T::FriendRequestSlotsPerPeriod::get() > 0,
				"FriendRequestSlotsPerPeriod must be non-zero",
			);
			assert!(
				T::LiteFriendRequestSlotsPerPeriod::get() > 0,
				"LiteFriendRequestSlotsPerPeriod must be non-zero",
			);
			assert!(
				T::LiteFriendRequestSlotsPerPeriod::get() <= T::FriendRequestSlotsPerPeriod::get(),
				"LiteFriendRequestSlotsPerPeriod must be <= FriendRequestSlotsPerPeriod",
			);
			assert!(
				T::FriendRequestPeriodDuration::get() > 0,
				"FriendRequestPeriodDuration must be non-zero",
			);
			assert!(
				T::FriendRequestGraceWindow::get() < T::FriendRequestPeriodDuration::get(),
				"FriendRequestGraceWindow must be smaller than FriendRequestPeriodDuration",
			);
			assert!(
				T::OffchainWorkerInterval::get() > Zero::zero(),
				"OffchainWorkerInterval must be greater than 0",
			);
			assert!(
				T::StmtStoreSlotsPerPeriod::get() > 0,
				"StmtStoreSlotsPerPeriod must be non-zero",
			);
			assert!(
				T::LiteStmtStoreSlotsPerPeriod::get() > 0,
				"LiteStmtStoreSlotsPerPeriod must be non-zero",
			);
			assert!(
				T::LiteStmtStoreSlotsPerPeriod::get() <= T::StmtStoreSlotsPerPeriod::get(),
				"LiteStmtStoreSlotsPerPeriod must be <= StmtStoreSlotsPerPeriod",
			);
			assert!(T::StmtStoreCleanupLimit::get() > 0, "StmtStoreCleanupLimit must be non-zero",);
			assert!(
				T::StmtStoreReplacementCooldown::get() > 0,
				"StmtStoreReplacementCooldown must be non-zero",
			);
			assert!(
				(T::StmtStoreReplacementCooldown::get() as u64) <= SECONDS_PER_DAY,
				"StmtStoreReplacementCooldown must be at most one day (the period length)",
			);
			assert!(T::StmtStoreGraceWindow::get() > 0, "StmtStoreGraceWindow must be non-zero",);
			assert!(
				T::LongTermStoragePeriodDuration::get() > 0,
				"LongTermStoragePeriodDuration must be non-zero",
			);
			assert!(
				T::LongTermStorageGraceWindow::get() < T::LongTermStoragePeriodDuration::get(),
				"LongTermStorageGraceWindow must be smaller than LongTermStoragePeriodDuration",
			);
			assert!(
				T::LongTermStorageClaimsPerPeriod::get() > 0,
				"LongTermStorageClaimsPerPeriod must be non-zero",
			);
			assert!(
				T::LongTermStorageCleanupLimit::get() > 0,
				"LongTermStorageCleanupLimit must be non-zero",
			);
		}

		#[cfg(feature = "try-runtime")]
		fn try_state(_now: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
			ensure!(
				StorageClaims::<T>::count() <= T::MaxReservations::get(),
				"Resources StorageClaims exceeds MaxReservations"
			);
			for (id, claim) in StorageClaims::<T>::iter() {
				ensure!(
					StorageReservationByPurpose::<T>::get(&claim.purpose) == Some(id),
					"Resources claim is missing its purpose reverse mapping"
				);
			}
			for (purpose, id) in StorageReservationByPurpose::<T>::iter() {
				ensure!(
					StorageClaims::<T>::get(id).is_some_and(|claim| claim.purpose == purpose),
					"Resources purpose reverse mapping is missing or mismatched"
				);
			}
			Ok(())
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a lite person as a consumer.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register_lite_person())]
		pub fn register_lite_person(
			origin: OriginFor<T>,
			identifier_key: CommunicationIdentifier,
		) -> DispatchResultWithPostInfo {
			// Ensure this is a lite person.
			let lite_person_account = T::EnsureLitePerson::ensure_origin(origin)?;
			Self::register_lite_consumer_inner(lite_person_account, identifier_key)?;
			Ok(Pays::No.into())
		}

		/// Register a proven person as a consumer.
		///
		/// The person must link a previously recognized lite identity, which will be upgraded to a
		/// full person consumer. In order to prove they hold the lite identity they want to link,
		/// users must provide a `lite_identity_proof` signature, created by signing the alias bytes
		/// using their lite consumer account.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::register_person())]
		pub fn register_person(
			origin: OriginFor<T>,
			linked_lite_identity: T::AccountId,
			lite_identity_proof: T::OffchainSignature,
		) -> DispatchResultWithPostInfo {
			let alias = T::EnsurePerson::ensure_origin(origin, &RESOURCES_CONTEXT)?;
			ensure!(!AccountOfAlias::<T>::contains_key(alias), Error::<T>::AlreadyRegistered);
			Self::register_person_inner(alias, &linked_lite_identity, &lite_identity_proof)
		}

		/// Update a person's authorization by ensuring they can still authenticate as people.
		///
		/// This call must be performed at least `MinPersonAuthUpdateInterval` seconds after the
		/// last update in order to prevent spam.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::touch_person_authorization())]
		pub fn touch_person_authorization(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			// Ensure this is a person.
			let alias = T::EnsurePerson::ensure_origin(origin, &RESOURCES_CONTEXT)?;
			let account = AccountOfAlias::<T>::get(alias).ok_or(Error::<T>::NotRegistered)?;
			let consumer_info = Consumers::<T>::get(&account).ok_or(Error::<T>::NotRegistered)?;
			let Credibility::Person { last_update, demoted: was_demoted, .. } =
				consumer_info.credibility
			else {
				return Err(Error::<T>::NotFullPerson.into());
			};
			// Ensure the authorization is old enough to be touched.
			let now = T::Clock::now().as_secs();
			ensure!(
				now > last_update.saturating_add(T::MinPersonAuthUpdateInterval::get() as u64),
				Error::<T>::TouchNotReady
			);

			// Set the consumer's updated record.
			Consumers::<T>::insert(
				&account,
				ConsumerInfo {
					credibility: Credibility::Person { alias, last_update: now, demoted: false },
					..consumer_info
				},
			);

			if was_demoted {
				// A person's allowance should be given back.
				let person_allowance = T::PersonStatementLimit::get();
				let lite_person_allowance = T::LitePersonStatementLimit::get();
				let remaining_allowance = person_allowance.saturating_sub(lite_person_allowance);
				increase_allowance_by(account.clone().into(), remaining_allowance);
			}

			Self::deposit_event(Event::PersonAuthorizationTouched { account });
			Ok(Pays::No.into())
		}

		/// Update the communication identifier key of a consumer.
		///
		/// The origin must be the account registered for that consumer, regardless of their
		/// credibility.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::update_identifier_key())]
		pub fn update_identifier_key(
			origin: OriginFor<T>,
			identifier_key: CommunicationIdentifier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut consumer_info = Consumers::<T>::get(&who).ok_or(Error::<T>::NotRegistered)?;
			consumer_info.identifier_key = identifier_key;
			Consumers::<T>::insert(&who, consumer_info);
			Self::deposit_event(Event::IdentifierKeyUpdated { account: who });
			Ok(())
		}

		/// Demote a full person to a lite person after their authorization has expired.
		///
		/// This is a permissionless call; the origin must be authorized.
		#[pallet::call_index(7)]
		#[pallet::authorize(|_source, account| {
		    Self::authorize_demote_auth_expired(account)
		})]
		#[pallet::weight_of_authorize(<T as Config>::WeightInfo::authorize_demote_auth_expired())]
		#[pallet::weight(<T as Config>::WeightInfo::demote_auth_expired())]
		pub fn demote_auth_expired(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			ensure_authorized(origin)?;
			let consumer_info = Self::validate_demotion(&account)?;
			if let Credibility::Person { alias, last_update, .. } = consumer_info.credibility {
				let allowance = T::PersonStatementLimit::get()
					.saturating_sub(T::LitePersonStatementLimit::get());
				decrease_allowance_by(account.clone().into(), allowance);
				Consumers::<T>::insert(
					&account,
					ConsumerInfo {
						credibility: Credibility::Person { alias, last_update, demoted: true },
						..consumer_info
					},
				);
			}
			Self::deposit_event(Event::PersonDemoted { account });
			Ok(())
		}

		/// Associate a statement account with a friend request context sequence.
		///
		/// The associated account can submit statements while this friend request registration is
		/// active.
		/// The origin must be `Origin::FriendRequestAlias`, created by the `AsResources`
		/// (`RegisterFriendRequestWithProof(..)`) transaction extension after proof validation.
		/// On success, increases statement allowance and stores registration state
		/// `{account_id, reference}`.
		///
		/// Parameters:
		/// * `reference`: friend request period/sequence pair.
		///   - `reference.period` must be in the accepted period window: `[period(now -
		///     FriendRequestGraceWindow), period(now)]`.
		///   - `reference.seq` must be in `0..=FriendRequestSlotsPerPeriod`.
		/// * `account_id`: statement account to authorize. Must not already be used by another
		///   friend request registration.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::set_friend_request_statement_account_for_sequence())]
		pub fn set_friend_request_statement_account_for_sequence(
			origin: OriginFor<T>,
			reference: FriendRequestReference,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			// Ensure this is a friend request alias origin produced by `AsResources`.
			let alias = Self::ensure_friend_request_alias(origin)?;
			// Fail fast if parameters are invalid.
			Self::validate_friend_request_period(reference.period)?;
			// `AsResources` enforces the collection-specific slot bound before dispatch. By the
			// time this call executes, only the broader catch-all friend request bound remains.
			Self::validate_friend_request_seq(reference.seq)?;

			Self::validate_friend_request_registration(alias, &account_id)?;

			increase_allowance_by(account_id.clone().into(), T::FriendRequestAllowance::get());
			FriendRequestRegistrationByAlias::<T>::insert(
				alias,
				types::FriendRequestRegistration { account_id: account_id.clone(), reference },
			);
			FriendRequestAliasByAccount::<T>::insert(&account_id, alias);

			Self::deposit_event(Event::FriendRequestStmtUsageSet {
				alias,
				period: reference.period,
				seq: reference.seq,
				account: account_id,
			});
			Ok(Pays::No.into())
		}

		/// Clear a stale friend request registration and revoke its statement allowance.
		///
		/// This is a permissionless call; the origin must be authorized.
		/// Succeeds only when the registration's period-derived expiry has elapsed.
		/// On success, removes friend request registration state and decreases statement allowance.
		///
		/// Parameters:
		/// * `account`: statement account previously associated with a friend request registration.
		/// * `seq`: friend request sequence to clear. Must match stored registration sequence and
		///   be in `0..=FriendRequestSlotsPerPeriod`.
		#[pallet::call_index(9)]
		#[pallet::authorize(|source, account, seq| {
			Self::authorize_clear_expired_friend_request_sequence(source, account, seq)
		})]
		#[pallet::weight_of_authorize(<T as Config>::WeightInfo::authorize_clear_expired_friend_request_sequence())]
		#[pallet::weight(<T as Config>::WeightInfo::clear_expired_friend_request_sequence())]
		pub fn clear_expired_friend_request_sequence(
			origin: OriginFor<T>,
			account: T::AccountId,
			_seq: u8,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;
			let alias = Self::friend_request_alias_for_account(&account)?;
			FriendRequestRegistrationByAlias::<T>::remove(alias);
			FriendRequestAliasByAccount::<T>::remove(&account);
			decrease_allowance_by(account.clone().into(), T::FriendRequestAllowance::get());

			Self::deposit_event(Event::FriendRequestStmtUsageRemoved { account });
			Ok(Pays::No.into())
		}

		/// Claim an anonymous statement store allowance for a target account.
		///
		/// The origin must be `Origin::StmtStoreAlias`, produced by the `AsResources`
		/// (`RegisterStatementStoreAllowance(..)`) transaction extension after proof validation.
		/// On success, increases the statement allowance for `target_account` and stores the
		/// mapping in `StatementStoreAllowances`.
		///
		/// Parameters:
		/// * `period`: day number since Unix epoch. Must be in the accepted period window.
		/// * `seq`: slot number within the period, bounded by the collection-specific limit.
		/// * `target_account`: statement account to authorize.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as Config>::WeightInfo::set_statement_store_account())]
		pub fn set_statement_store_account(
			origin: OriginFor<T>,
			period: u32,
			seq: u32,
			target_account: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let alias = Self::ensure_stmt_store_alias(origin)?;
			let period_key = BigEndianU32::from(period);
			let now = T::Clock::now().as_secs();

			// If an entry already exists for this alias in this period, the cooldown
			// since `existing.since` must have elapsed before it can be replaced.
			if let Some(existing) = StatementStoreAllowances::<T>::get(period_key, alias) {
				ensure!(
					now > existing
						.since
						.saturating_add(T::StmtStoreReplacementCooldown::get() as u64),
					Error::<T>::StmtStoreReplacementTooEarly
				);
				// Revoke the old allowance and clear the reverse lookup.
				decrease_allowance_by(
					existing.account_id.clone().into(),
					T::AccountsApiAllowance::get(),
				);
				StmtStoreAllowanceByAccount::<T>::remove(
					&existing.account_id,
					(period_key, existing.seq, alias),
				);
			}

			// Grant the statement store allowance to the target account.
			increase_allowance_by(target_account.clone().into(), T::AccountsApiAllowance::get());
			StatementStoreAllowances::<T>::insert(
				period_key,
				alias,
				StmtStoreAllowanceEntry { account_id: target_account.clone(), seq, since: now },
			);
			StmtStoreAllowanceByAccount::<T>::insert(&target_account, (period_key, seq, alias), ());
			Self::deposit_event(Event::StmtStoreAllowanceSet {
				alias,
				period,
				seq,
				account: target_account,
			});
			Ok(().into())
		}

		/// Remove expired statement store allowances for a past period.
		///
		/// This is a permissionless call; the origin must be authorized.
		/// Removes up to `StmtStoreCleanupLimit` entries from `StatementStoreAllowances` for
		/// the given `period`, decreasing the statement allowance for each removed account.
		#[pallet::call_index(11)]
		#[pallet::authorize(|source, period, first_entry| {
			Self::authorize_clear_expired_stmt_store_allowances(source, period, first_entry)
		})]
		#[pallet::weight_of_authorize(<T as Config>::WeightInfo::authorize_clear_expired_stmt_store_allowances())]
		#[pallet::weight(<T as Config>::WeightInfo::clear_expired_stmt_store_allowances(T::StmtStoreCleanupLimit::get()))]
		pub fn clear_expired_stmt_store_allowances(
			origin: OriginFor<T>,
			period: u32,
			first_entry: Alias,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let period_key = BigEndianU32::from(period);
			let limit = T::StmtStoreCleanupLimit::get();
			let mut count = 0u32;

			for (alias, entry) in
				StatementStoreAllowances::<T>::drain_prefix(period_key).take(limit as usize)
			{
				decrease_allowance_by(
					entry.account_id.clone().into(),
					T::AccountsApiAllowance::get(),
				);
				StmtStoreAllowanceByAccount::<T>::remove(
					entry.account_id,
					(period_key, entry.seq, alias),
				);
				count = count.saturating_add(1);
			}

			Self::deposit_event(Event::StmtStoreAllowancesCleared {
				period,
				first_key: first_entry,
				count,
			});
			Ok(Some(T::WeightInfo::clear_expired_stmt_store_allowances(count)).into())
		}

		/// Claim long-term storage on a remote chain using an anonymous membership proof.
		///
		/// The origin must be `Origin::LongTermStorageClaim { alias, collection, payer }`, created
		/// by the `AsResources` (`ClaimLongTermStorage(..)`) transaction extension after ring-VRF
		/// proof validation.
		///
		/// Parameters:
		/// * `period`: the claiming period. Must be the current period or the previous one if
		///   within the grace window.
		/// * `counter`: the claim counter within the period. Must be less than
		///   `LongTermStorageClaimsPerPeriod`. Each counter produces a distinct alias.
		/// * `account_id`: the account to authorize for storage on the remote chain.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_long_term_storage())]
		#[frame_support::transactional]
		pub fn claim_long_term_storage(
			origin: OriginFor<T>,
			period: u32,
			counter: u8,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let (alias, collection, payer) = Self::ensure_long_term_storage_claim(origin)?;
			ensure!(payer == account_id, DispatchError::BadOrigin);
			ensure!(
				StorageClaims::<T>::count() < T::MaxReservations::get(),
				Error::<T>::ReservationBackendFailed
			);
			let purpose = ReservationPurpose::Membership { period, alias, counter, collection };
			ensure!(
				!StorageReservationByPurpose::<T>::contains_key(&purpose),
				Error::<T>::ClaimAlreadyReserved
			);

			let allocation = match collection {
				MembershipCollection::People => T::LongTermStorageAllowanceForPeople::get(),
				MembershipCollection::LitePeople => T::LongTermStorageAllowanceForLitePeople::get(),
			};
			let reservation_id = NextStorageReservationId::<T>::get();
			let next = reservation_id.checked_add(1).ok_or(Error::<T>::ReservationIdOverflow)?;
			let now = frame_system::Pallet::<T>::block_number();
			let expires_at = now.saturating_add(T::StorageReservationDuration::get());

			NextStorageReservationId::<T>::put(next);
			StorageClaims::<T>::insert(
				reservation_id,
				StorageClaim::<T> {
					purpose: purpose.clone(),
					owner: account_id.clone(),
					created_at: now,
				},
			);
			StorageReservationByPurpose::<T>::insert(&purpose, reservation_id);
			T::LongTermStorageDataStore::reserve(
				reservation_id,
				&account_id,
				&purpose,
				allocation.bytes,
				allocation.transactions,
				expires_at,
			)
			.map_err(|_| Error::<T>::ReservationBackendFailed)?;
			// Consume the anonymous proof only after the backend reserve succeeds. The surrounding
			// storage transaction rolls every write back on any failure.
			SpentLongTermStorageAliases::<T>::insert(BigEndianU32::from(period), alias, ());

			Self::deposit_event(Event::LongTermStorageReserved {
				reservation_id,
				alias,
				period,
				counter,
				account: account_id,
				collection,
			});
			Ok(Pays::No.into())
		}

		/// Clear spent long-term storage aliases for an expired period.
		///
		/// This is a permissionless call authorized via the `authorize` attribute. It can be
		/// called by anyone once a period has fully expired (past the grace window).
		///
		/// Parameters:
		/// * `period`: the expired period to clear aliases for.
		/// * `limit`: the maximum number of entries to remove in this call.
		#[pallet::call_index(13)]
		#[pallet::authorize(|_source, period, _limit| {
			Self::authorize_clear_long_term_storage_aliases(period)
		})]
		#[pallet::weight_of_authorize(
			<T as Config>::WeightInfo::authorize_clear_expired_long_term_storage_aliases()
		)]
		#[pallet::weight(<T as Config>::WeightInfo::clear_expired_long_term_storage_aliases(*limit))]
		pub fn clear_expired_long_term_storage_aliases(
			origin: OriginFor<T>,
			period: u32,
			limit: u32,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;
			ensure!(
				limit <= T::LongTermStorageCleanupLimit::get(),
				Error::<T>::LongTermStorageCleanupLimitExceeded,
			);
			let mut count = 0u32;
			for _ in SpentLongTermStorageAliases::<T>::drain_prefix(BigEndianU32::from(period))
				.take(limit as usize)
			{
				count = count.saturating_add(1);
			}
			Self::deposit_event(Event::LongTermStorageAliasesCleared { period, count });
			Ok(Pays::No.into())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_long_term_storage_reservation())]
		#[frame_support::transactional]
		pub fn cancel_long_term_storage_reservation(
			origin: OriginFor<T>,
			reservation_id: ReservationId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let claim =
				StorageClaims::<T>::get(reservation_id).ok_or(Error::<T>::ReservationNotFound)?;
			ensure!(claim.owner == who, Error::<T>::NotReservationOwner);
			T::LongTermStorageDataStore::cancel(&who, reservation_id)
				.map_err(|_| Error::<T>::ReservationBackendFailed)?;
			Self::deposit_event(Event::LongTermStorageReservationCancelled {
				reservation_id,
				account: who,
			});
			Ok(Pays::Yes.into())
		}

		#[pallet::call_index(17)]
		#[pallet::weight(<T as Config>::WeightInfo::expire_long_term_storage_reservations(*limit))]
		#[frame_support::transactional]
		pub fn expire_long_term_storage_reservations(
			origin: OriginFor<T>,
			limit: u32,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			ensure!(
				limit <= T::LongTermStorageCleanupLimit::get(),
				Error::<T>::CleanupLimitExceeded
			);
			let now = frame_system::Pallet::<T>::block_number();
			let expired = T::LongTermStorageDataStore::expire_due(now, limit)
				.map_err(|_| Error::<T>::ReservationBackendFailed)?;
			for reservation_id in expired {
				Self::deposit_event(Event::LongTermStorageReservationExpired { reservation_id });
			}
			Ok(Pays::Yes.into())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		/// Returns the current statement store allowance period (day number since Unix epoch).
		pub fn current_stmt_store_period() -> u32 {
			Self::stmt_store_period_from_timestamp(T::Clock::now().as_secs())
		}

		/// Returns the proof context for a statement store slot claim at the given
		/// `period` and `seq`.
		///
		/// Layout: `SSS_SLOT:<period (4 bytes BE)><seq (4 bytes BE)>` padded to 32 bytes.
		pub fn stmt_store_slot_context_for(period: u32, seq: u32) -> Context {
			Self::stmt_store_slot_context(period, seq)
		}

		/// Returns the proof context for a friend request registration at the given
		/// `period` and `seq`.
		pub fn friend_request_context_for(period: u32, seq: u8) -> Context {
			Self::friend_request_context(FriendRequestReference { period, seq })
		}
	}

	impl<T: Config> Pallet<T> {
		fn authorize_demote_auth_expired(
			account: &T::AccountId,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::validate_demotion(account)
				.map_err(|_| crate::extension::CustomValidity::InvalidPersonDemotion)?;
			ValidTransaction::with_tag_prefix("PersonhoodResourcesDemoteAuthExpired")
				.and_provides(account)
				.propagate(true)
				.build()
				.map(|valid_tx| (valid_tx, Weight::zero()))
		}

		fn submit_authorized_transaction(call: Call<T>, description: &str) {
			let tx = T::create_authorized_transaction(call.into());
			match SubmitTransaction::<T, _>::submit_transaction(tx) {
				Ok(()) => log::debug!(
					target: LOG_TARGET,
					"offchain worker: submitted authorized transaction successfully for `{description}`",
				),
				Err(()) => log::warn!(
					target: LOG_TARGET,
					"offchain worker: failed to submit authorized transaction for `{description}`",
				),
			}
		}

		fn should_clear_friend_request_registration(
			registration: &types::FriendRequestRegistration<T::AccountId>,
		) -> bool {
			Self::validate_clear_friend_request_sequence(
				&registration.account_id,
				registration.reference.seq,
			)
			.is_ok()
		}

		/// Reject any non-local transaction source. Used by authorize closures of calls
		/// that are submitted exclusively by the offchain worker, so they should never
		/// arrive over the network from external peers.
		fn ensure_local_source(source: TransactionSource) -> Result<(), TransactionValidityError> {
			match source {
				TransactionSource::Local | TransactionSource::InBlock => Ok(()),
				TransactionSource::External => Err(InvalidTransaction::BadSigner.into()),
			}
		}

		fn authorize_clear_expired_friend_request_sequence(
			source: TransactionSource,
			account: &T::AccountId,
			seq: &u8,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			Self::validate_clear_friend_request_sequence(account, *seq).map_err(|_| {
				crate::extension::CustomValidity::InvalidExpiredFriendRequestCleanup
			})?;
			ValidTransaction::with_tag_prefix(
				"PersonhoodResourcesClearExpiredFriendRequestSequence",
			)
			.and_provides(account)
			.propagate(true)
			.build()
			.map(|valid_tx| (valid_tx, Weight::zero()))
		}

		fn authorize_clear_expired_stmt_store_allowances(
			source: TransactionSource,
			period: &u32,
			first_entry: &Alias,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			if !Self::is_stmt_store_period_clearable(*period) {
				return Err(crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup.into());
			}
			let Some((first_period, actual_first)) =
				StatementStoreAllowances::<T>::iter_keys().next()
			else {
				return Err(crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup.into());
			};
			if first_period.0 != *period {
				return Err(crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup.into());
			}
			if actual_first != *first_entry {
				return Err(crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup.into());
			}
			ValidTransaction::with_tag_prefix("PersonhoodResourcesClearExpiredStmtStore")
				.and_provides((period, first_entry))
				.propagate(true)
				.build()
				.map(|valid_tx| (valid_tx, Weight::zero()))
		}

		fn ensure_friend_request_alias(origin: OriginFor<T>) -> Result<Alias, DispatchError> {
			match origin.into_caller().try_into() {
				Ok(Origin::FriendRequestAlias(alias)) => Ok(alias),
				_ => Err(DispatchError::BadOrigin),
			}
		}

		fn ensure_long_term_storage_claim(
			origin: OriginFor<T>,
		) -> Result<(Alias, MembershipCollection, T::AccountId), DispatchError> {
			match origin.into_caller().try_into() {
				Ok(Origin::LongTermStorageClaim { alias, collection, payer }) => {
					Ok((alias, collection, payer))
				},
				_ => Err(DispatchError::BadOrigin),
			}
		}

		fn authorize_clear_long_term_storage_aliases(
			period: &u32,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::validate_clear_long_term_storage_period(*period)?;
			ensure!(
				SpentLongTermStorageAliases::<T>::iter_key_prefix(BigEndianU32::from(*period))
					.next()
					.is_some(),
				TransactionValidityError::from(
					crate::extension::CustomValidity::NothingToClearForLongTermStoragePeriod,
				)
			);
			ValidTransaction::with_tag_prefix(
				"PersonhoodResourcesClearExpiredLongTermStorageAliases",
			)
			.and_provides(period)
			.propagate(true)
			.build()
			.map(|valid_tx| (valid_tx, Weight::zero()))
		}

		/// Upgrade an existing lite consumer after proving control of its account.
		fn register_person_inner(
			alias: Alias,
			linked_lite_identity: &T::AccountId,
			lite_identity_proof: &T::OffchainSignature,
		) -> DispatchResultWithPostInfo {
			// Verify proof of ownership of the linked lite person.
			ensure!(
				lite_identity_proof.verify(&alias[..], linked_lite_identity),
				Error::<T>::InvalidProofOfOwnership,
			);
			// Ensure the linked lite person was not already linked to another full person.
			let mut linked_consumer_info =
				Consumers::<T>::get(linked_lite_identity).ok_or(Error::<T>::NoLinkedIdentity)?;
			ensure!(
				matches!(linked_consumer_info.credibility, Credibility::Lite),
				Error::<T>::AlreadyLinked
			);
			// Update the linked lite consumer's record with the full person credibility. From this
			// moment onward, this consumer will be registered as a full person through this
			// upgrade.
			let now = T::Clock::now().as_secs();
			linked_consumer_info.credibility =
				Credibility::Person { alias, last_update: now, demoted: false };

			// Mark the alias as used.
			AccountOfAlias::<T>::insert(alias, linked_lite_identity);

			// Set the consumer's record.
			Consumers::<T>::insert(linked_lite_identity, linked_consumer_info);

			// Increase the allowance by the difference between the lite person allowance the user
			// already has and the full person allowance they now have.
			let allowance =
				T::PersonStatementLimit::get().saturating_sub(T::LitePersonStatementLimit::get());
			increase_allowance_by(linked_lite_identity.clone().into(), allowance);

			Self::deposit_event(Event::PersonRegistered {
				alias,
				account: linked_lite_identity.clone(),
			});
			Ok(Pays::No.into())
		}

		pub fn friend_request_period_from_timestamp(now_secs: u64) -> u32 {
			let period_duration = u64::from(T::FriendRequestPeriodDuration::get().max(1));
			let period = now_secs.checked_div(period_duration).unwrap_or(0);
			period.try_into().unwrap_or(u32::MAX)
		}

		pub fn friend_request_expiration_time(period: u32) -> u64 {
			let period_end = u64::from(period.saturating_add(1))
				.saturating_mul(u64::from(T::FriendRequestPeriodDuration::get().max(1)));
			period_end.saturating_add(T::FriendRequestRetentionDuration::get())
		}

		/// Whether a friend request period is currently accepted.
		///
		/// This follows the same current-plus-grace-window pattern as
		/// `pallet_coinage::Pallet::<T>::current_free_unload_token_periods`.
		///
		/// The current period is always accepted. During the grace window immediately after a
		/// rollover, the previous period is also accepted so transactions created near the boundary
		/// do not fail just because they were included slightly later.
		fn is_accepted_friend_request_period(period: u32) -> bool {
			let now_secs = T::Clock::now().as_secs();
			let current_period = Self::friend_request_period_from_timestamp(now_secs);
			if period == current_period {
				return true;
			}
			let now_secs_minus_grace =
				now_secs.saturating_sub(u64::from(T::FriendRequestGraceWindow::get()));
			let previous_period_in_grace =
				Self::friend_request_period_from_timestamp(now_secs_minus_grace);
			period == previous_period_in_grace
		}

		pub fn friend_request_context(reference: FriendRequestReference) -> Context {
			let mut context = [b' '; 32];
			let required_len =
				FRIEND_REQUEST_CONTEXT_PREFIX.len() + size_of::<u32>() + size_of::<u8>();
			debug_assert!(
				required_len <= context.len(),
				"friend request context payload does not fit: required={required_len}, len={}",
				context.len()
			);
			let payload = FRIEND_REQUEST_CONTEXT_PREFIX
				.iter()
				.copied()
				.chain(reference.period.to_be_bytes())
				.chain([reference.seq]);
			for (dst, src) in context.iter_mut().zip(payload) {
				*dst = src;
			}
			context
		}

		/// Build the context for a statement store slot proof.
		///
		/// Layout: `SSS_SLOT:<period (4 bytes BE)><seq (4 bytes BE)>` padded to 32 bytes.
		pub fn stmt_store_slot_context(period: u32, seq: u32) -> Context {
			let mut context = [b' '; 32];
			let required_len =
				STMT_STORE_SLOT_CONTEXT_PREFIX.len() + size_of::<u32>() + size_of::<u32>();
			debug_assert!(
				required_len <= context.len(),
				"stmt store slot context payload does not fit: required={required_len}, len={}",
				context.len()
			);
			let payload = STMT_STORE_SLOT_CONTEXT_PREFIX
				.iter()
				.copied()
				.chain(period.to_be_bytes())
				.chain(seq.to_be_bytes());
			for (dst, src) in context.iter_mut().zip(payload) {
				*dst = src;
			}
			context
		}

		/// Compute the statement store period from a timestamp.
		///
		/// The period is the day number since the Unix epoch (`now_secs / 86400`).
		pub fn stmt_store_period_from_timestamp(now_secs: u64) -> u32 {
			let period = now_secs.checked_div(SECONDS_PER_DAY).unwrap_or(0);
			period.try_into().unwrap_or(u32::MAX)
		}

		/// Whether a statement store period is eligible for cleanup.
		///
		/// A period becomes clearable only after its end plus the grace window. This
		/// ensures that allowances from the previous period remain active for a short
		/// overlap into the new period, giving users continuous coverage while they
		/// claim fresh slots.
		///
		/// Period `P` ends at `(P + 1) * SECONDS_PER_DAY`. Cleanup is allowed when
		/// `now > period_end + StmtStoreGraceWindow`.
		fn is_stmt_store_period_clearable(period: u32) -> bool {
			let now_secs = T::Clock::now().as_secs();
			let period_end = u64::from(period.saturating_add(1)).saturating_mul(SECONDS_PER_DAY);
			let clearable_after =
				period_end.saturating_add(u64::from(T::StmtStoreGraceWindow::get()));
			now_secs > clearable_after
		}

		fn ensure_stmt_store_alias(origin: OriginFor<T>) -> Result<Alias, DispatchError> {
			match origin.into_caller().try_into() {
				Ok(Origin::StmtStoreAlias(alias)) => Ok(alias),
				_ => Err(DispatchError::BadOrigin),
			}
		}

		// Validation functions for friend request registration and clearing.
		// These are used both in the dispatchable calls and in the authorization logic.
		pub(crate) fn validate_friend_request_period(period: u32) -> Result<(), Error<T>> {
			ensure!(
				Self::is_accepted_friend_request_period(period),
				Error::<T>::InvalidFriendRequestPeriod
			);
			Ok(())
		}

		// The sequence number is an arbitrary identifier provided by the caller to
		// distinguish different registrations within the same period.
		// These are used both in the dispatchable calls and in the authorization logic.
		fn validate_friend_request_seq_with_limit(seq: u8, limit: u8) -> Result<(), Error<T>> {
			ensure!(seq <= limit, Error::<T>::InvalidFriendRequestSequence);
			Ok(())
		}

		pub(crate) fn validate_friend_request_seq(seq: u8) -> Result<(), Error<T>> {
			Self::validate_friend_request_seq_with_limit(seq, T::FriendRequestSlotsPerPeriod::get())
		}

		pub(crate) fn validate_lite_friend_request_seq(seq: u8) -> Result<(), Error<T>> {
			Self::validate_friend_request_seq_with_limit(
				seq,
				T::LiteFriendRequestSlotsPerPeriod::get(),
			)
		}

		// Friend request registration must reject duplicate account and alias registrations.
		// This helper is shared by dispatch and pre-dispatch validation.
		pub(crate) fn validate_friend_request_registration(
			alias: Alias,
			account_id: &T::AccountId,
		) -> Result<(), Error<T>> {
			ensure!(
				!FriendRequestRegistrationByAlias::<T>::contains_key(alias),
				Error::<T>::FriendRequestRegistrationAlreadyExists
			);
			if FriendRequestAliasByAccount::<T>::contains_key(account_id) {
				log::error!(
					target: LOG_TARGET,
					"friend request registration validation found an existing account mapping for a new alias",
				);
				return Err(Error::<T>::FriendRequestRegistrationAlreadyExists);
			}
			Ok(())
		}

		// To clear an expired friend request registration, the caller must provide
		// the account associated with the registration and the sequence number.
		fn validate_clear_friend_request_sequence(
			account: &T::AccountId,
			seq: u8,
		) -> Result<Alias, Error<T>> {
			let alias = Self::friend_request_alias_for_account(account)?;
			let Some(registration) = FriendRequestRegistrationByAlias::<T>::get(alias) else {
				log::error!(
					target: LOG_TARGET,
					"friend request storage corruption: missing registration for alias {alias:?} mapped from account {account:?}",
				);
				return Err(Error::<T>::InvalidFriendRequestSequence);
			};
			if registration.account_id != *account {
				log::error!(
					target: LOG_TARGET,
					"friend request storage corruption: alias {:?} maps from account {:?} but registration points to {:?}",
					alias,
					account,
					registration.account_id,
				);
				return Err(Error::<T>::InvalidFriendRequestSequence);
			}
			ensure!(registration.reference.seq == seq, Error::<T>::InvalidFriendRequestSequence);
			let now = T::Clock::now().as_secs();
			let expires_at = Self::friend_request_expiration_time(registration.reference.period);
			ensure!(now > expires_at, Error::<T>::FriendRequestRegistrationNotExpired);
			Ok(alias)
		}

		fn friend_request_alias_for_account(account: &T::AccountId) -> Result<Alias, Error<T>> {
			FriendRequestAliasByAccount::<T>::get(account)
				.ok_or(Error::<T>::InvalidFriendRequestSequence)
		}

		/// Construct the context for a long-term storage claim.
		pub fn long_term_storage_context(period: u32, counter: u8) -> Context {
			let mut context = [0u8; 32];
			context[..24].copy_from_slice(&LONG_TERM_STORAGE_CONTEXT_BASE);
			context[24..28].copy_from_slice(&period.to_be_bytes());
			context[28] = counter;
			context
		}

		pub fn long_term_storage_period_from_timestamp(now_secs: u64) -> u32 {
			(now_secs / T::LongTermStoragePeriodDuration::get() as u64) as u32
		}

		pub fn is_accepted_long_term_storage_period(period: u32) -> bool {
			let now_secs = T::Clock::now().as_secs();
			let current_period = Self::long_term_storage_period_from_timestamp(now_secs);
			if period == current_period {
				return true;
			}
			let now_secs_minus_grace =
				now_secs.saturating_sub(u64::from(T::LongTermStorageGraceWindow::get()));
			let previous_period_in_grace =
				Self::long_term_storage_period_from_timestamp(now_secs_minus_grace);
			period == previous_period_in_grace
		}

		/// Whether the long-term storage `period` is past its grace window and can be cleared.
		pub(crate) fn is_long_term_storage_period_clearable(period: u32) -> bool {
			let now_secs = T::Clock::now().as_secs();
			let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
			let grace = T::LongTermStorageGraceWindow::get() as u64;
			let period_claimable_until =
				(period as u64 + 1).saturating_mul(period_duration).saturating_add(grace);
			now_secs > period_claimable_until
		}

		pub(crate) fn validate_clear_long_term_storage_period(
			period: u32,
		) -> Result<(), TransactionValidityError> {
			ensure!(
				Self::is_long_term_storage_period_clearable(period),
				TransactionValidityError::from(
					crate::extension::CustomValidity::LongTermStoragePeriodNotExpired,
				)
			);
			Ok(())
		}

		/// Register a lite consumer using the provided information.
		///
		/// IMPORTANT
		///
		/// This function does not check for authorization. The caller is responsible for ensuring
		/// the `account` to be registered is a lite person and that the user's consent was
		/// provided, usually through a signature verified by the caller.
		pub fn register_lite_consumer_inner(
			account: T::AccountId,
			identifier_key: CommunicationIdentifier,
		) -> Result<(), Error<T>> {
			// Must not already be registered.
			ensure!(!Consumers::<T>::contains_key(&account), Error::<T>::AlreadyRegistered);
			// Set the consumer's record.
			Consumers::<T>::insert(
				&account,
				ConsumerInfo { identifier_key, credibility: Credibility::Lite },
			);
			frame_system::Pallet::<T>::inc_sufficients(&account);

			// A new lite person has been registered, so the initial allowance should be given
			increase_allowance_by(account.clone().into(), T::LitePersonStatementLimit::get());

			Self::deposit_event(Event::LitePersonRegistered { account });
			Ok(())
		}

		fn validate_demotion(account: &T::AccountId) -> Result<ConsumerInfo, Error<T>> {
			let consumer_info = Consumers::<T>::get(account).ok_or(Error::<T>::NotRegistered)?;
			let Credibility::Person { last_update, demoted, .. } = consumer_info.credibility else {
				return Err(Error::<T>::NotFullPerson);
			};
			let now = T::Clock::now().as_secs();
			ensure!(
				now > last_update.saturating_add(T::PersonAuthDuration::get() as u64),
				Error::<T>::PersonAuthNotExpired
			);
			ensure!(!demoted, Error::<T>::AlreadyDemoted);
			Ok(consumer_info)
		}
	}

	impl<T: Config> ConsumerRegistrar<T::AccountId> for Pallet<T> {
		type Error = Error<T>;

		fn register_lite_consumer(
			account: T::AccountId,
			identifier_key: CommunicationIdentifier,
		) -> Result<(), Error<T>> {
			Self::register_lite_consumer_inner(account, identifier_key)
		}
	}
}

impl<T: Config> ResourceClaimLifecycle<ReservationId, ReservationPurpose> for Pallet<T> {
	fn prune_claim(id: ReservationId) -> ClaimCleanupOutcome<ReservationId, ReservationPurpose> {
		let Some(claim) = StorageClaims::<T>::take(id) else {
			return ClaimCleanupOutcome { id, removed: false, purpose: None };
		};
		let reverse_matches = StorageReservationByPurpose::<T>::get(&claim.purpose) == Some(id);
		if reverse_matches {
			StorageReservationByPurpose::<T>::remove(&claim.purpose);
		}
		ClaimCleanupOutcome { id, removed: reverse_matches, purpose: Some(claim.purpose) }
	}
}

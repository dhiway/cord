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

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]
#![allow(clippy::borrowed_box)]
extern crate alloc;
use alloc::boxed::Box;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod extension;
pub mod types;
pub mod weights;
pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{
		extract_actual_weight, DispatchInfo, DispatchResultWithPostInfo, GetDispatchInfo,
		PostDispatchInfo,
	},
	traits::{EnsureOriginWithArg, IsSubType, OriginTrait},
};
use indiv_support::traits::{
	AddOnlyPeopleTrait, AppendOnlyMembers, CleanUpAlias, Context, ContextualAlias, CountedMembers,
	FlexibleMembers, Identifier, MembershipProver, PeopleTrait, PersonalId, RevisedAlias,
	RevisedContextualAlias, RingExponent, RingIndex, RingMode, PEOPLE_IDENTIFIER,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{BadOrigin, Dispatchable},
	transaction_validity::{InvalidTransaction, ValidTransaction},
	Debug, Saturating,
};
use verifiable::{Alias, GenerateVerifiable};

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use alloc::{vec, vec::Vec};
	use frame_support::{pallet_prelude::*, traits::Contains};
	use frame_system::{
		offchain::{CreateAuthorizedTransaction, SubmitTransaction},
		pallet_prelude::{ensure_authorized, ensure_signed, BlockNumberFor, OriginFor},
	};
	const LOG_TARGET: &str = "runtime::people";
	pub use indiv_support::traits::PEOPLE_IDENTIFIER as PEOPLE_MEMBER_IDENTIFIER;

	/// The onboarding size for the people collection.
	pub const PEOPLE_ONBOARDING_SIZE: u32 = 10;

	/// Maximum number of stale aliases cleaned up in a single bulk unsigned call
	/// ([`Pallet::clean_up_stale_aliases`]).
	pub const MAX_BULK_CLEANUP: u32 = 100;

	/// Reasons why an authorized cleanup transaction is invalid.
	#[derive(Clone)]
	#[repr(u8)]
	pub enum CustomInvalidity {
		/// The aliases list is empty.
		EmptyAliases = 0,
		/// An alias has no account mapping.
		InvalidAccount = 1,
		/// Alias mismatched.
		AliasMismatch = 3,
		/// The alias is not stale (context still active and ring still exists).
		AliasNotStale = 4,
	}

	impl From<CustomInvalidity> for TransactionValidityError {
		fn from(e: CustomInvalidity) -> Self {
			InvalidTransaction::Custom(e as u8).into()
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		CreateAuthorizedTransaction<Call<Self>>
		+ frame_system::Config<
			RuntimeOrigin: From<Origin>
			                   + From<<Self::RuntimeOrigin as OriginTrait>::PalletsOrigin>
			                   + OriginTrait<
				PalletsOrigin: From<Origin>
				                   + TryInto<
					Origin,
					Error = <Self::RuntimeOrigin as OriginTrait>::PalletsOrigin,
				>,
			>,
			RuntimeCall: Parameter
			                 + GetDispatchInfo
			                 + IsSubType<Call<Self>>
			                 + Dispatchable<
				RuntimeOrigin = Self::RuntimeOrigin,
				Info = DispatchInfo,
				PostInfo = PostDispatchInfo,
			>,
		>
	{
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		type MemberService: FlexibleMembers
			+ MembershipProver<
				Crypto: GenerateVerifiable<
					Proof: Send + Sync + DecodeWithMemTracking,
					Signature: Send + Sync + DecodeWithMemTracking,
					Member: DecodeWithMemTracking,
					Config: TryFrom<RingExponent>,
				>,
			>;

		/// The ring exponent used to operate the people member collection in `MemberService`.
		#[pallet::constant]
		type RingExponent: Get<RingExponent>;

		/// The location of the owner for the people collection.
		type CollectionOwner: Get<<Self::MemberService as AppendOnlyMembers>::Location>;

		/// Gates which contexts can set up account aliases for persons. Contexts
		/// can be defined on the pallet level (e.g. `MOB_CONTEXT`) or be
		/// generated dynamically (e.g. for individual airdrops).
		///
		/// Queried by [Pallet::set_alias_account] and the transaction extension
		/// [extension::AsPersonInfo::AsPersonalAliasWithProof].
		type AccountContexts: Contains<Context>;

		/// Maximum number of people included in an onboarding queue page before a new one is
		/// created.
		#[pallet::constant]
		type OnboardingQueuePageSize: Get<u32>;

		/// Interval (in blocks) at which the offchain worker runs stale alias cleanup.
		#[pallet::constant]
		type StaleAliasCleanupInterval: Get<BlockNumberFor<Self>>;

		/// Minimum time in seconds a member must wait in the onboarding queue before they
		/// can self-include.
		type SelfInclusionDelay: Get<u64>;

		/// The origin allowed to perform privileged management operations on this pallet.
		type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Helper for benchmarks.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<
			<<Self::MemberService as MembershipProver>::Crypto as GenerateVerifiable>::StaticChunk,
		>;
	}

	/// The current individuals we recognise, but not necessarily yet included in a ring.
	///
	/// Look-up from the crypto (public) key to the immutable ID of the individual (`PersonalId`). A
	/// person can have two different entries in this map if they queued a key migration which
	/// hasn't been enacted yet.
	#[pallet::storage]
	pub type Keys<T> = CountedStorageMap<_, Blake2_128Concat, MemberOf<T>, PersonalId>;

	/// The current individuals we recognise, but not necessarily yet included in a ring.
	///
	/// Immutable ID of the individual (`PersonalId`) to information about their key and status.
	#[pallet::storage]
	pub type People<T: Config> =
		StorageMap<_, Blake2_128Concat, PersonalId, PersonRecord<MemberOf<T>, T::AccountId>>;

	/// Conversion of a contextual alias to an account ID.
	#[pallet::storage]
	pub type AliasToAccount<T> = StorageMap<
		_,
		Blake2_128Concat,
		ContextualAlias,
		<T as frame_system::Config>::AccountId,
		OptionQuery,
	>;

	/// Conversion of an account ID to a contextual alias.
	#[pallet::storage]
	pub type AccountToAlias<T> = StorageMap<
		_,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		RevisedContextualAlias,
		OptionQuery,
	>;

	/// Association of an account ID to a personal ID.
	///
	/// Managed with `set_personal_id_account` and `unset_personal_id_account`.
	/// Reverse lookup is inside `People` storage, inside the record.
	#[pallet::storage]
	pub type AccountToPersonalId<T> = StorageMap<
		_,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		PersonalId,
		OptionQuery,
	>;

	/// The next free and never reserved personal ID.
	#[pallet::storage]
	pub type NextPersonalId<T> = StorageValue<_, PersonalId, ValueQuery>;

	/// Whether the people collection has been created.
	#[pallet::storage]
	pub type PeopleCollectionCreated<T> = StorageValue<_, bool, ValueQuery>;

	/// Candidates' reserved identities which we track.
	#[pallet::storage]
	pub type ReservedPersonalId<T: Config> =
		StorageMap<_, Twox64Concat, PersonalId, (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An individual has had their personhood recognised and indexed.
		PersonhoodRecognized { who: PersonalId, key: MemberOf<T> },
		/// An individual has had their personhood recognised again and indexed.
		PersonOnboarding { who: PersonalId, key: MemberOf<T> },
		/// A call was dispatched under an alias.
		AliasDispatched { alias: ContextualAlias, account: T::AccountId },
		/// An alias-to-account mapping was set or updated.
		AliasAccountSet { alias: ContextualAlias, account: T::AccountId },
		/// An alias-to-account mapping was removed.
		AliasAccountUnset { alias: ContextualAlias, account: T::AccountId },
		/// A personal ID-to-account mapping was set or updated.
		PersonalIdAccountSet { who: PersonalId, account: T::AccountId },
		/// A personal ID-to-account mapping was removed.
		PersonalIdAccountUnset { who: PersonalId, account: T::AccountId },
		/// The people collection was created.
		CollectionCreated,
		/// Personhood was forcefully recognized by root.
		ForcePersonhoodRecognized { people: Vec<MemberOf<T>> },
		/// An alias-to-account mapping was cleaned up.
		AliasCleanedUp { alias: ContextualAlias, account: T::AccountId },
	}

	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		/// The amount of block number tolerance we allow for a setup account transaction.
		///
		/// `set_alias_account` and `set_personal_id_account` calls contains
		/// `call_valid_at` as a parameter, those calls are valid if the block number is within
		/// the tolerance period.
		pub fn account_setup_time_tolerance() -> BlockNumberFor<T> {
			600u32.into()
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The supplied identifier does not represent a person.
		NotPerson,
		/// The given person has no associated key.
		NoKey,
		/// The context is not a member of those allowed to have account aliases held.
		InvalidContext,
		/// The account is not known.
		InvalidAccount,
		/// The account is already in use under another alias.
		AccountInUse,
		/// The proof is invalid.
		InvalidProof,
		/// The signature is invalid.
		InvalidSignature,
		/// There are not yet any members of our personhood set.
		NoMembers,
		/// The root cannot be finalized as there are still unpushed members.
		Incomplete,
		/// The root is still fresh.
		StillFresh,
		/// Too many members have been pushed.
		TooManyMembers,
		/// Key already in use by another person.
		KeyAlreadyInUse,
		/// The old key was not found when expected.
		KeyNotFound,
		/// Could not push member into the ring.
		CouldNotPush,
		/// The record is already using this key.
		SameKey,
		/// Personal Id was not reserved.
		PersonalIdNotReserved,
		/// Personal Id has never been reserved.
		PersonalIdReservationCannotRenew,
		/// Personal Id was not reserved or not already recognized.
		PersonalIdNotReservedOrNotRecognized,
		/// Ring cannot be merged if it's the top ring.
		InvalidRing,
		/// Ring cannot be built while there are suspensions pending.
		SuspensionsPending,
		/// Ring cannot be merged if it's not below 1/2 capacity.
		RingAboveMergeThreshold,
		/// Suspension indices provided are invalid.
		InvalidSuspensions,
		/// An mutating action was queued when there was no mutation session in progress.
		NoMutationSession,
		/// An mutating session could not be started.
		CouldNotStartMutationSession,
		/// Cannot merge rings while a suspension session is in progress.
		SuspensionSessionInProgress,
		/// The alias mapping is not stale.
		AliasNotStale,
		/// Call is too late or too early.
		TimeOutOfRange,
		/// Alias <-> Account is already set and up to date.
		AliasAccountAlreadySet,
		/// Personhood cannot be resumed if it is not suspended.
		NotSuspended,
		/// Personhood is suspended.
		Suspended,
		/// Invalid state for attempted key migration.
		InvalidKeyMigration,
		/// Invalid suspension of a key belonging to a person whose index in the ring has already
		/// been included in the pending suspensions list.
		KeyAlreadySuspended,
		/// The onboarding size must not exceed the maximum ring size.
		InvalidOnboardingSize,
		/// The member key is not valid for the crypto.
		InvalidMemberKey,
		/// The people collection has already been created.
		PeopleCollectionAlreadyExists,
		/// The provided alias does not match the account's current alias mapping.
		AliasMismatch,
		/// None of the supplied aliases were stale.
		NoStaleAliases,
	}

	#[pallet::origin]
	#[derive(
		Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
	)]
	pub enum Origin {
		PersonalIdentity(PersonalId),
		PersonalAlias(RevisedContextualAlias),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Scans for aliases whose context is no longer in [`Config::AccountContexts`]
		/// and submits a [`Pallet::clean_up_stale_aliases`] transaction to remove them.
		///
		/// Missing context is the only criterion used here; authorize and
		/// dispatch re-validate independently. Context removal is permanent,
		/// so staleness cannot resolve between the offchain check and
		/// on-chain execution. Combined with the `"stale-alias-cleanup"`
		/// pool tag (see [`Pallet::authorize_clean_up_stale_aliases`]) that
		/// prevents concurrent cleanup transactions, no submitted alias can
		/// be cleaned up by another tx before this one executes.
		///
		/// At most [`MAX_BULK_CLEANUP`] aliases are submitted per run; any
		/// remainder is picked up in subsequent runs.
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			let interval = T::StaleAliasCleanupInterval::get();
			if interval == 0u32.into() || !(block_number % interval).is_zero() {
				return;
			}

			let stale = AccountToAlias::<T>::iter()
				// Missing context implies staleness since [`Self::ensure_alias_is_stale`]
				// returns `Ok(())`.
				.filter(|(_, rev_ca)| !T::AccountContexts::contains(&rev_ca.ca.context))
				.map(|(_, rev_ca)| rev_ca.ca)
				.take(MAX_BULK_CLEANUP as usize)
				.collect::<Vec<_>>();

			if !stale.is_empty() {
				let aliases = BoundedVec::truncate_from(stale);
				let call = Call::<T>::clean_up_stale_aliases { aliases };
				let xt = T::create_authorized_transaction(call.into());
				if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(xt) {
					log::debug!(target: LOG_TARGET, "Failed to submit stale alias cleanup: {e:?}");
				}
			}
		}

		fn integrity_test() {
			// Ring size and chunk management constraints are enforced by the members pallet.
			assert!(
				!T::StaleAliasCleanupInterval::get().is_zero(),
				"StaleAliasCleanupInterval must not be zero"
			);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dispatch a call under an alias using the `account <-> alias` mapping.
		///
		/// This is a call version of the transaction extension `AsPersonalAliasWithAccount`.
		/// It is recommended to use the transaction extension instead when suitable.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::under_alias().saturating_add(call.get_dispatch_info().call_weight))]
		pub fn under_alias(
			origin: OriginFor<T>,
			call: Box<<T as frame_system::Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin.clone())?;
			let rev_ca = AccountToAlias::<T>::get(&account).ok_or(Error::<T>::InvalidAccount)?;
			ensure!(T::AccountContexts::contains(&rev_ca.ca.context), Error::<T>::InvalidContext);
			let ring_revision =
				T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, rev_ca.ring)
					.ok_or(Error::<T>::InvalidAccount)?;
			ensure!(rev_ca.revision == ring_revision, DispatchError::BadOrigin);

			Self::deposit_event(Event::<T>::AliasDispatched { alias: rev_ca.ca.clone(), account });

			let derivation_weight = T::WeightInfo::under_alias();
			let local_origin = Origin::PersonalAlias(rev_ca);
			Self::derivative_call(origin, local_origin, *call, derivation_weight)
		}

		/// This transaction is refunded if successful and no alias was previously set.
		///
		/// The call is valid from `call_valid_at` until
		/// `call_valid_at + account_setup_time_tolerance`.
		/// `account_setup_time_tolerance` is a constant available in the metadata.
		///
		/// Parameters:
		/// - `account`: The account to set the alias for.
		/// - `call_valid_at`: The block number when the call becomes valid.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_alias_account())]
		pub fn set_alias_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			call_valid_at: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			let rev_ca = Self::ensure_revised_personal_alias(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			let time_tolerance = Self::account_setup_time_tolerance();
			ensure!(
				call_valid_at <= now && now <= call_valid_at.saturating_add(time_tolerance),
				Error::<T>::TimeOutOfRange
			);
			ensure!(T::AccountContexts::contains(&rev_ca.ca.context), Error::<T>::InvalidContext);
			ensure!(!AccountToPersonalId::<T>::contains_key(&account), Error::<T>::AccountInUse);

			let old_account = AliasToAccount::<T>::get(&rev_ca.ca);
			let old_rev_ca = old_account.as_ref().and_then(AccountToAlias::<T>::get);

			let needs_revision = old_rev_ca.is_some_and(|old_rev_ca| {
				old_rev_ca.revision != rev_ca.revision || old_rev_ca.ring != rev_ca.ring
			});

			// Ensure it changes the account associated, or it needs revision.
			ensure!(
				old_account.as_ref() != Some(&account) || needs_revision,
				Error::<T>::AliasAccountAlreadySet
			);

			// If the old account is different from the new one:
			// * decrease the sufficients of the old account
			// * increase the sufficients of the new account
			// * check new account is not already in use
			if old_account.as_ref() != Some(&account) {
				ensure!(!AccountToAlias::<T>::contains_key(&account), Error::<T>::AccountInUse);
				if let Some(old_account) = &old_account {
					frame_system::Pallet::<T>::dec_sufficients(old_account);
					AccountToAlias::<T>::remove(old_account);
				}
				frame_system::Pallet::<T>::inc_sufficients(&account);
			}

			AccountToAlias::<T>::insert(&account, &rev_ca);
			AliasToAccount::<T>::insert(&rev_ca.ca, &account);
			Self::deposit_event(Event::<T>::AliasAccountSet { alias: rev_ca.ca, account });

			if old_account.is_none() || needs_revision {
				Ok(Pays::No.into())
			} else {
				Ok(Pays::Yes.into())
			}
		}

		/// Remove the mapping from a particular alias to its registered account.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::unset_alias_account())]
		pub fn unset_alias_account(origin: OriginFor<T>) -> DispatchResult {
			let alias = Self::ensure_personal_alias(origin)?;
			let account = AliasToAccount::<T>::take(&alias).ok_or(Error::<T>::InvalidAccount)?;
			AccountToAlias::<T>::remove(&account);
			frame_system::Pallet::<T>::dec_sufficients(&account);
			Self::deposit_event(Event::<T>::AliasAccountUnset { alias, account });

			Ok(())
		}

		/// Recognize a set of people without any additional checks.
		///
		/// The people are identified by the provided list of keys and will each be assigned, in
		/// order, the next available personal ID.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::force_recognize_personhood(people.len().try_into().unwrap_or(0)))]
		pub fn force_recognize_personhood(
			origin: OriginFor<T>,
			people: Vec<MemberOf<T>>,
		) -> DispatchResultWithPostInfo {
			T::ManagerOrigin::ensure_origin_or_root(origin)?;
			for key in people.iter() {
				let personal_id = Self::reserve_new_id();
				Self::recognize_personhood(personal_id, Some(key.clone()))?;
			}
			Self::deposit_event(Event::<T>::ForcePersonhoodRecognized { people });
			Ok(().into())
		}

		/// Set a personal id account.
		///
		/// The account can then be used to sign transactions on behalf of the personal id, and
		/// provide replay protection with the nonce.
		///
		/// This transaction is refunded if successful and no account was previously set for the
		/// personal id.
		///
		/// The call is valid from `call_valid_at` until
		/// `call_valid_at + account_setup_time_tolerance`.
		/// `account_setup_time_tolerance` is a constant available in the metadata.
		///
		/// Parameters:
		/// - `account`: The account to set the alias for.
		/// - `call_valid_at`: The block number when the call becomes valid.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_personal_id_account())]
		pub fn set_personal_id_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			call_valid_at: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			let id = Self::ensure_personal_identity(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			let time_tolerance = Self::account_setup_time_tolerance();
			ensure!(
				call_valid_at <= now && now <= call_valid_at.saturating_add(time_tolerance),
				Error::<T>::TimeOutOfRange
			);
			ensure!(!AccountToPersonalId::<T>::contains_key(&account), Error::<T>::AccountInUse);
			ensure!(!AccountToAlias::<T>::contains_key(&account), Error::<T>::AccountInUse);
			let mut record = People::<T>::get(id).ok_or(Error::<T>::NotPerson)?;
			let pays = if let Some(old_account) = record.account {
				frame_system::Pallet::<T>::dec_sufficients(&old_account);
				AccountToPersonalId::<T>::remove(&old_account);
				Pays::Yes
			} else {
				Pays::No
			};
			record.account = Some(account.clone());
			frame_system::Pallet::<T>::inc_sufficients(&account);
			AccountToPersonalId::<T>::insert(&account, id);
			People::<T>::insert(id, &record);
			Self::deposit_event(Event::<T>::PersonalIdAccountSet { who: id, account });

			Ok(pays.into())
		}

		/// Unset the personal id account.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::unset_personal_id_account())]
		pub fn unset_personal_id_account(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let id = Self::ensure_personal_identity(origin)?;
			let mut record = People::<T>::get(id).ok_or(Error::<T>::NotPerson)?;
			let account = record.account.take().ok_or(Error::<T>::InvalidAccount)?;
			AccountToPersonalId::<T>::take(&account).ok_or(Error::<T>::InvalidAccount)?;
			frame_system::Pallet::<T>::dec_sufficients(&account);
			People::<T>::insert(id, &record);
			Self::deposit_event(Event::<T>::PersonalIdAccountUnset { who: id, account });

			Ok(Pays::Yes.into())
		}

		/// Create the people collection.
		///
		/// This call is valid only if the collection doesn't exist yet. Once created,
		/// this call cannot be executed again.
		///
		/// The collection is created with a fixed configuration:
		/// - Owner: Configured via `CollectionOwner` type
		/// - Onboarding size: `PEOPLE_ONBOARDING_SIZE` (10)
		/// - Mode: `Flexible`
		/// - Ring size: `R2e9`
		#[pallet::call_index(6)]
		#[pallet::authorize(Pallet::<T>::authorize_create_people_collection)]
		#[pallet::weight(T::WeightInfo::create_people_collection())]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_create_people_collection())]
		pub fn create_people_collection(origin: OriginFor<T>) -> DispatchResult {
			ensure_authorized(origin)?;

			T::MemberService::create_collection(
				T::CollectionOwner::get(),
				PEOPLE_MEMBER_IDENTIFIER,
				PEOPLE_ONBOARDING_SIZE,
				RingMode::Flexible,
				T::RingExponent::get(),
				Some(T::SelfInclusionDelay::get()),
			)?;

			PeopleCollectionCreated::<T>::put(true);
			Self::deposit_event(Event::<T>::CollectionCreated);

			Ok(())
		}

		/// Remove stale alias <-> account mappings.
		///
		/// A mapping is stale when:
		/// - its context has been removed from [`Config::AccountContexts`] (governance
		///   reconfiguration, ended airdrop etc.), or
		/// - its ring has been deleted.
		///
		/// Revision mismatches do not render an alias stale. The user can continue
		/// transacting via `AsPersonalAliasWithAccountRevised` without having to
		/// redo account setup.
		///
		/// Typically submitted by the OCW, but the dispatch does not trust
		/// the caller. Each alias is re-validated via
		/// `Self::ensure_alias_is_stale`; those that are not stale are
		/// skipped.
		///
		/// At most [`MAX_BULK_CLEANUP`] aliases are processed per call.
		///
		/// The transaction source must be local or in-block. Thus, external
		/// invocations are not permitted.
		#[pallet::call_index(7)]
		#[pallet::authorize(Pallet::<T>::authorize_clean_up_stale_aliases)]
		#[pallet::weight(T::WeightInfo::clean_up_stale_alias(aliases.len() as u32))]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_clean_up_stale_alias(aliases.len() as u32))]
		pub fn clean_up_stale_aliases(
			origin: OriginFor<T>,
			aliases: BoundedVec<ContextualAlias, ConstU32<MAX_BULK_CLEANUP>>,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let mut cleaned = 0u64;

			for ca in aliases {
				let Some(account) = AliasToAccount::<T>::get(&ca) else { continue };
				let Some(rev_ca) = AccountToAlias::<T>::get(&account) else { continue };

				if rev_ca.ca != ca {
					log::error!(
						target: LOG_TARGET,
						"Inconsistent alias state: AliasToAccount points to {account:?} \
						 but AccountToAlias points to a different alias"
					);

					continue;
				}

				AliasToAccount::<T>::remove(&ca);
				AccountToAlias::<T>::remove(&account);
				frame_system::Pallet::<T>::dec_sufficients(&account);

				Self::deposit_event(Event::<T>::AliasCleanedUp { alias: ca, account });
				cleaned += 1;
			}

			ensure!(cleaned > 0, Error::<T>::NoStaleAliases);

			Ok(Some(T::WeightInfo::clean_up_stale_alias(cleaned as u32)).into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Validates that the aliases can be cleaned up.
		///
		/// Only accepts transactions from local or in-block sources. Each alias
		/// is checked for existence, that the account still points to the
		/// supplied alias, and that it is stale (see `Self::ensure_alias_is_stale`).
		pub fn authorize_clean_up_stale_aliases(
			source: TransactionSource,
			aliases: &BoundedVec<ContextualAlias, ConstU32<MAX_BULK_CLEANUP>>,
		) -> TransactionValidityWithRefund {
			if !matches!(source, TransactionSource::Local | TransactionSource::InBlock) {
				return Err(InvalidTransaction::BadSigner.into());
			}

			ensure!(!aliases.is_empty(), CustomInvalidity::EmptyAliases);

			for ca in aliases {
				let account =
					AliasToAccount::<T>::get(ca).ok_or(CustomInvalidity::InvalidAccount)?;
				let rev_ca =
					AccountToAlias::<T>::get(&account).ok_or(CustomInvalidity::InvalidAccount)?;

				if &rev_ca.ca != ca {
					log::error!(
						target: LOG_TARGET,
						"Inconsistent alias state: AliasToAccount points to {account:?} \
						 but AccountToAlias points to a different alias"
					);

					return Err(CustomInvalidity::AliasMismatch.into());
				}

				Self::ensure_alias_is_stale(&rev_ca)
					.map_err(|_| CustomInvalidity::AliasNotStale)?;
			}

			let validity = ValidTransaction::with_tag_prefix("pallet-people-cleanup")
				// Only one cleanup tx in the pool at a time; newer submissions replace
				// stale ones. This guarantees that when a cleanup tx executes on-chain,
				// no concurrent cleanup was in the pool that might have already removed
				// some of the targeted aliases.
				.and_provides("stale-alias-cleanup")
				.longevity(5)
				.propagate(false)
				.into();

			Ok((validity, Weight::zero()))
		}

		/// Validates that the people collection can be created.
		///
		/// Returns a valid transaction if the collection doesn't exist yet,
		/// or `InvalidTransaction::Call` if it already exists.
		pub fn authorize_create_people_collection(
			_source: TransactionSource,
		) -> TransactionValidityWithRefund {
			if PeopleCollectionCreated::<T>::get() {
				return Err(InvalidTransaction::Call.into());
			}
			let validity = ValidTransaction::with_tag_prefix("pallet-people")
				.and_provides("create_people_collection")
				.into();
			Ok((validity, Weight::zero()))
		}

		fn derivative_call(
			mut origin: OriginFor<T>,
			local_origin: Origin,
			call: <T as frame_system::Config>::RuntimeCall,
			derivation_weight: Weight,
		) -> DispatchResultWithPostInfo {
			origin.set_caller_from(<T::RuntimeOrigin as OriginTrait>::PalletsOrigin::from(
				local_origin,
			));
			let info = call.get_dispatch_info();
			let result = call.dispatch(origin);
			let weight = derivation_weight.saturating_add(extract_actual_weight(&result, &info));
			result
				.map(|p| PostDispatchInfo { actual_weight: Some(weight), pays_fee: p.pays_fee })
				.map_err(|mut err| {
					err.post_info = Some(weight).into();
					err
				})
		}

		/// Ensure that the origin `o` represents a person.
		/// Returns `Ok` with the base identity of the person on success.
		pub fn ensure_personal_identity(
			origin: T::RuntimeOrigin,
		) -> Result<PersonalId, DispatchError> {
			Ok(ensure_personal_identity(origin.into_caller())?)
		}

		/// Ensure that the origin `o` represents a person.
		/// Returns `Ok` with the alias of the person together with the context in which it can
		/// be used on success.
		pub fn ensure_personal_alias(
			origin: T::RuntimeOrigin,
		) -> Result<ContextualAlias, DispatchError> {
			Ok(ensure_personal_alias(origin.into_caller())?)
		}

		/// Verify that an account <-> alias mapping is stale.
		///
		/// A mapping is stale when:
		/// - its context has been removed from [`Config::AccountContexts`] (governance
		///   reconfiguration, ended airdrop etc.), or
		/// - its ring has been deleted.
		fn ensure_alias_is_stale(rev_ca: &RevisedContextualAlias) -> DispatchResult {
			// The context is missing from AccountContexts in scenarios such as governance
			// reconfiguration removing a known context.
			if !T::AccountContexts::contains(&rev_ca.ca.context) {
				return Ok(());
			}

			let ring_revision =
				T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, rev_ca.ring);

			match ring_revision {
				None => Ok(()),
				Some(_) => Err(Error::<T>::AliasNotStale.into()),
			}
		}

		/// Ensure that the origin `o` represents a person.
		/// On success returns `Ok` with the revised alias of the person together with the context
		/// in which it can be used and the revision of the ring the person is in.
		pub fn ensure_revised_personal_alias(
			origin: T::RuntimeOrigin,
		) -> Result<RevisedContextualAlias, DispatchError> {
			Ok(ensure_revised_personal_alias(origin.into_caller())?)
		}
	}

	impl<T: Config> AddOnlyPeopleTrait for Pallet<T> {
		type Member = MemberOf<T>;

		fn reserve_new_id() -> PersonalId {
			let new_id = NextPersonalId::<T>::mutate(|id| {
				let new_id = *id;
				id.saturating_inc();
				new_id
			});
			ReservedPersonalId::<T>::insert(new_id, ());
			new_id
		}

		fn cancel_id_reservation(personal_id: PersonalId) -> Result<(), DispatchError> {
			ReservedPersonalId::<T>::take(personal_id).ok_or(Error::<T>::PersonalIdNotReserved)?;
			Ok(())
		}

		fn renew_id_reservation(personal_id: PersonalId) -> Result<(), DispatchError> {
			if NextPersonalId::<T>::get() <= personal_id ||
				People::<T>::contains_key(personal_id) ||
				ReservedPersonalId::<T>::contains_key(personal_id)
			{
				return Err(Error::<T>::PersonalIdReservationCannotRenew.into());
			}
			ReservedPersonalId::<T>::insert(personal_id, ());
			Ok(())
		}

		fn recognize_personhood(
			who: PersonalId,
			maybe_key: Option<Self::Member>,
		) -> Result<(), DispatchError> {
			match maybe_key {
				Some(key) => {
					// If the key is already in use by another person then error.
					ensure!(!Keys::<T>::contains_key(&key), Error::<T>::KeyAlreadyInUse);
					// This is a first time key, so it must be reserved.
					ensure!(
						// (We can't use `take` here because `add_members` can still fail).
						ReservedPersonalId::<T>::get(who).is_some(),
						Error::<T>::PersonalIdNotReservedOrNotRecognized
					);
					T::MemberService::add_members(PEOPLE_MEMBER_IDENTIFIER, vec![key.clone()])?;
					ReservedPersonalId::<T>::remove(who);
					let record = PersonRecord { key, account: None };
					Keys::<T>::insert(&record.key, who);
					People::<T>::insert(who, &record);
					Self::deposit_event(Event::<T>::PersonhoodRecognized { who, key: record.key });
				},
				None => {
					let record = People::<T>::get(who).ok_or(Error::<T>::NotPerson)?;
					ensure!(Keys::<T>::get(&record.key) == Some(who), Error::<T>::NoKey);
					T::MemberService::add_members(
						PEOPLE_MEMBER_IDENTIFIER,
						vec![record.key.clone()],
					)?;
					Self::deposit_event(Event::<T>::PersonOnboarding { who, key: record.key });
				},
			}
			Ok(())
		}

		#[cfg(feature = "runtime-benchmarks")]
		type Secret = SecretOf<T>;

		#[cfg(feature = "runtime-benchmarks")]
		fn mock_key(who: PersonalId) -> (Self::Member, Self::Secret) {
			let mut buf = [0u8; 32];
			buf[..core::mem::size_of::<PersonalId>()].copy_from_slice(&who.to_le_bytes()[..]);
			let secret = CryptoOf::<T>::new_secret(buf);
			(CryptoOf::<T>::member_from_secret(&secret), secret)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn initialize_people_collection() {
			T::MemberService::create_collection(
				T::CollectionOwner::get(),
				PEOPLE_MEMBER_IDENTIFIER,
				PEOPLE_ONBOARDING_SIZE,
				RingMode::Flexible,
				T::RingExponent::get(),
				Some(T::SelfInclusionDelay::get()),
			)
			.expect("couldn't create the people collection in the member service");
			PeopleCollectionCreated::<T>::put(true);
		}
	}

	impl<T: Config> PeopleTrait for Pallet<T> {
		fn suspend_personhood(suspensions: &[PersonalId]) -> DispatchResult {
			let mut keys = vec![];
			for personal_id in suspensions {
				let mut record = People::<T>::get(personal_id).ok_or(Error::<T>::NotPerson)?;
				if let Some(account) = record.account {
					AccountToPersonalId::<T>::remove(account);
					record.account = None;
				}
				People::<T>::insert(personal_id, &record);
				keys.push(record.key);
			}
			T::MemberService::remove_members(PEOPLE_MEMBER_IDENTIFIER, &keys[..])
		}
		fn can_start_people_set_mutation_session() -> bool {
			let current_state = T::MemberService::rings_state(PEOPLE_MEMBER_IDENTIFIER);
			current_state.start_mutation_session().is_ok()
		}
		fn start_people_set_mutation_session() -> DispatchResult {
			T::MemberService::start_removal_session(PEOPLE_MEMBER_IDENTIFIER)
		}
		fn end_people_set_mutation_session() -> DispatchResult {
			T::MemberService::end_removal_session(PEOPLE_MEMBER_IDENTIFIER)
		}
	}

	impl<T: Config> CleanUpAlias for Pallet<T> {
		fn clean_up_alias(ca: ContextualAlias) -> DispatchResult {
			let account = AliasToAccount::<T>::get(&ca).ok_or(Error::<T>::InvalidAccount)?;
			let rev_ca = AccountToAlias::<T>::get(&account).ok_or(Error::<T>::InvalidAccount)?;

			if rev_ca.ca != ca {
				log::error!(
					target: LOG_TARGET,
					"Inconsistent alias state: AliasToAccount points to {account:?} \
					 but AccountToAlias points to a different alias"
				);

				return Err(Error::<T>::AliasMismatch.into());
			}

			AliasToAccount::<T>::remove(&ca);
			AccountToAlias::<T>::remove(&account);
			frame_system::Pallet::<T>::dec_sufficients(&account);

			Self::deposit_event(Event::<T>::AliasCleanedUp { alias: ca, account });
			Ok(())
		}
	}

	/// Ensure that the origin `o` represents an extrinsic (i.e. transaction) from a personal
	/// identity. Returns `Ok` with the personal identity that signed the extrinsic or an `Err`
	/// otherwise.
	pub fn ensure_personal_identity<OuterOrigin>(o: OuterOrigin) -> Result<PersonalId, BadOrigin>
	where
		OuterOrigin: TryInto<Origin, Error = OuterOrigin>,
	{
		match o.try_into() {
			Ok(Origin::PersonalIdentity(m)) => Ok(m),
			_ => Err(BadOrigin),
		}
	}

	/// Ensure that the origin `o` represents an extrinsic (i.e. transaction) from a personal alias.
	/// Returns `Ok` with the personal alias that signed the extrinsic or an `Err` otherwise.
	pub fn ensure_personal_alias<OuterOrigin>(o: OuterOrigin) -> Result<ContextualAlias, BadOrigin>
	where
		OuterOrigin: TryInto<Origin, Error = OuterOrigin>,
	{
		match o.try_into() {
			Ok(Origin::PersonalAlias(rev_ca)) => Ok(rev_ca.ca),
			_ => Err(BadOrigin),
		}
	}

	/// Guard to ensure that the given origin is a person. The underlying identity of the person is
	/// provided on success.
	pub struct EnsurePersonalIdentity<T>(PhantomData<T>);
	impl<T: Config> EnsureOrigin<OriginFor<T>> for EnsurePersonalIdentity<T> {
		type Success = PersonalId;

		fn try_origin(o: OriginFor<T>) -> Result<Self::Success, OriginFor<T>> {
			ensure_personal_identity(o.clone().into_caller()).map_err(|_| o)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin() -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalIdentity(0).into())
		}
	}

	frame_support::impl_ensure_origin_with_arg_ignoring_arg! {
		impl<{ T: Config, A }>
			EnsureOriginWithArg< OriginFor<T>, A> for EnsurePersonalIdentity<T>
		{}
	}

	impl<T: Config> CountedMembers for EnsurePersonalIdentity<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}

	/// Guard to ensure that the given origin is a person. The contextual alias of the person is
	/// provided on success.
	pub struct EnsurePersonalAlias<T>(PhantomData<T>);
	impl<T: Config> EnsureOrigin<OriginFor<T>> for EnsurePersonalAlias<T> {
		type Success = ContextualAlias;

		fn try_origin(o: OriginFor<T>) -> Result<Self::Success, OriginFor<T>> {
			ensure_personal_alias(o.clone().into_caller()).map_err(|_| o)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin() -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalAlias(RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1; 32], context: [0; 32] },
			})
			.into())
		}
	}

	frame_support::impl_ensure_origin_with_arg_ignoring_arg! {
		impl<{ T: Config, A }>
			EnsureOriginWithArg< OriginFor<T>, A> for EnsurePersonalAlias<T>
		{}
	}

	impl<T: Config> CountedMembers for EnsurePersonalAlias<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}

	/// Guard to ensure that the given origin is a person. The alias of the person within the
	/// context provided as an argument is returned on success.
	pub struct EnsurePersonalAliasInContext<T>(PhantomData<T>);
	impl<T: Config> EnsureOriginWithArg<OriginFor<T>, Context> for EnsurePersonalAliasInContext<T> {
		type Success = Alias;

		fn try_origin(o: OriginFor<T>, arg: &Context) -> Result<Self::Success, OriginFor<T>> {
			match ensure_personal_alias(o.clone().into_caller()) {
				Ok(ca) if &ca.context == arg => Ok(ca.alias),
				_ => Err(o),
			}
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin(context: &Context) -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalAlias(RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1; 32], context: *context },
			})
			.into())
		}
	}

	/// Same as [`EnsurePersonalAliasInContext`] but also yields the collection identifier the
	/// proof was verified against (`PEOPLE_IDENTIFIER`). Lets callers that admit multiple
	/// collections (combined via `EitherOf`) tell which collection authenticated.
	pub struct EnsurePersonalAliasInContextWithCollection<T>(PhantomData<T>);
	impl<T: Config> EnsureOriginWithArg<OriginFor<T>, Context>
		for EnsurePersonalAliasInContextWithCollection<T>
	{
		type Success = (Alias, Identifier);

		fn try_origin(o: OriginFor<T>, arg: &Context) -> Result<Self::Success, OriginFor<T>> {
			match ensure_personal_alias(o.clone().into_caller()) {
				Ok(ca) if &ca.context == arg => Ok((ca.alias, *PEOPLE_IDENTIFIER)),
				_ => Err(o),
			}
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin(context: &Context) -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalAlias(RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1; 32], context: *context },
			})
			.into())
		}
	}

	impl<T: Config> CountedMembers for EnsurePersonalAliasInContext<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}

	impl<T: Config> CountedMembers for EnsurePersonalAliasInContextWithCollection<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}

	/// Ensure that the origin `o` represents an extrinsic (i.e. transaction) from a personal alias
	/// with revision information.
	///
	/// Returns `Ok` with the revised personal alias that signed the extrinsic or an `Err`
	/// otherwise.
	pub fn ensure_revised_personal_alias<OuterOrigin>(
		o: OuterOrigin,
	) -> Result<RevisedContextualAlias, BadOrigin>
	where
		OuterOrigin: TryInto<Origin, Error = OuterOrigin>,
	{
		match o.try_into() {
			Ok(Origin::PersonalAlias(rev_ca)) => Ok(rev_ca),
			_ => Err(BadOrigin),
		}
	}

	/// Guard to ensure that the given origin is a person.
	///
	/// The revised contextual alias of the person is provided on success. The revision can be used
	/// to tell in the future if an alias may have been suspended. See [`RevisedContextualAlias`].
	pub struct EnsureRevisedPersonalAlias<T>(PhantomData<T>);
	impl<T: Config> EnsureOrigin<OriginFor<T>> for EnsureRevisedPersonalAlias<T> {
		type Success = RevisedContextualAlias;

		fn try_origin(o: OriginFor<T>) -> Result<Self::Success, OriginFor<T>> {
			ensure_revised_personal_alias(o.clone().into_caller()).map_err(|_| o)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin() -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalAlias(RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1; 32], context: [0; 32] },
			})
			.into())
		}
	}

	frame_support::impl_ensure_origin_with_arg_ignoring_arg! {
		impl<{ T: Config, A }>
			EnsureOriginWithArg< OriginFor<T>, A> for EnsureRevisedPersonalAlias<T>
		{}
	}

	impl<T: Config> CountedMembers for EnsureRevisedPersonalAlias<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}

	/// Guard to ensure that the given origin is a person.
	///
	/// The revised alias of the person within the context provided as an argument is returned on
	/// success. The revision can be used to tell in the future if an alias may have been suspended.
	/// See [`RevisedAlias`].
	pub struct EnsureRevisedPersonalAliasInContext<T>(PhantomData<T>);
	impl<T: Config> EnsureOriginWithArg<OriginFor<T>, Context>
		for EnsureRevisedPersonalAliasInContext<T>
	{
		type Success = RevisedAlias;

		fn try_origin(o: OriginFor<T>, arg: &Context) -> Result<Self::Success, OriginFor<T>> {
			match ensure_revised_personal_alias(o.clone().into_caller()) {
				Ok(ca) if &ca.ca.context == arg =>
					Ok(RevisedAlias { revision: ca.revision, ring: ca.ring, alias: ca.ca.alias }),
				_ => Err(o),
			}
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin(context: &Context) -> Result<OriginFor<T>, ()> {
			Ok(Origin::PersonalAlias(RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1; 32], context: *context },
			})
			.into())
		}
	}

	impl<T: Config> CountedMembers for EnsureRevisedPersonalAliasInContext<T> {
		fn active_count() -> u32 {
			T::MemberService::active_count(PEOPLE_MEMBER_IDENTIFIER)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_active_count(count: u32) {
			T::MemberService::set_active_count(PEOPLE_MEMBER_IDENTIFIER, count)
		}
	}
}

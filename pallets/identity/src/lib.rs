// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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
//

//! # Identity Pallet
//! Modified to work with CORD primitives
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! A federated naming system, allowing for multiple registrars to be added from
//! a specified origin. Registrars can set a fee to provide
//! identity-verification service. Anyone can put forth a proposed identity for
//! a review by any number of registrars. Registrar judgements are given as an
//! `enum`, allowing for sophisticated, multi-tier opinions.
//!
//! Some judgements are identified as *sticky*, which means they cannot be
//! removed except by complete removal of the identity, or by the registrar.
//!
//! A super-user can remove accounts.
//!
//! All accounts may also have a limited number of sub-accounts which may be specified by the owner;
//! by definition, these have equivalent ownership and each has an individual name.
//!
//! The number of registrars should be limited, and the deposit made sufficiently large, to ensure
//! no state-bloat attack is viable.
//!
//! ### Usernames
//!
//! The pallet provides functionality for username authorities to issue usernames. When an account
//! receives a username, they get a default instance of `IdentityInfo`. Usernames also serve as a
//! reverse lookup from username to account.
//!
//! Username authorities are given an allocation by governance to prevent state bloat. Usernames
//! impose no cost or deposit on the user.
//!
//! Users can have multiple usernames that map to the same `AccountId`, however one `AccountId` can
//! only map to a single username, known as the _primary_.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! #### For general users
//! * `set_identity` - Set the associated identity details of accounts
//! * `clear_identity` - Remove an account's associated identity details
//! * `request_judgement` - Request a judgement from a registrar.
//! * `cancel_request` - Cancel the previous request for a judgement.
//!
//!
//! #### For registrars
//! * `provide_judgement` - Provide a judgement to an identity.
//!
//! #### For super-users
//! * `add_registrar` - Add a new registrar to the system.
//! * `remove_registrar` - Remove a registrar from the system.
//! * `kill_identity` - Forcibly remove the associated identity
//!
//! [`Call`]: ./enum.Call.html
//! [`Config`]: ./trait.Config.html

#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
pub mod legacy;

#[cfg(test)]
mod tests;
mod types;
pub mod weights;

use crate::types::{AuthorityPropertiesOf, Suffix, Username};
use codec::Encode;
use frame_support::{
	ensure,
	pallet_prelude::{DispatchError, DispatchResult},
	traits::{Get, StorageVersion},
	BoundedVec,
};
pub use pallet::*;
use sp_runtime::traits::{
	AppendZerosInput, Hash, IdentifyAccount, Saturating, StaticLookup, Verify,
};
use sp_std::prelude::*;
pub use types::{
	Data, IdentityInformationProvider, Judgement, RegistrarIndex, RegistrarInfo, Registration,
};
pub use weights::WeightInfo;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of sub-accounts allowed per identified account.
		#[pallet::constant]
		type MaxSubAccounts: Get<u32>;

		/// Structure holding information about an identity.
		type IdentityInformation: IdentityInformationProvider;

		/// Maxmimum number of registrars allowed in the system. Needed to bound
		/// the complexity of, e.g., updating judgements.
		#[pallet::constant]
		type MaxRegistrars: Get<u32>;

		/// The origin which may add or remove registrars as well as remove
		/// identities. Root can always do this.
		type RegistrarOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Signature type for pre-authorizing usernames off-chain.
		///
		/// Can verify whether an `Self::SigningPublicKey` created a signature.
		type OffchainSignature: Verify<Signer = Self::SigningPublicKey> + Parameter;

		/// Public key that corresponds to an on-chain `Self::AccountId`.
		type SigningPublicKey: IdentifyAccount<AccountId = Self::AccountId>;

		/// The origin which may add or remove username authorities. Root can always do this.
		type UsernameAuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The number of blocks within which a username grant must be accepted.
		#[pallet::constant]
		type PendingUsernameExpiration: Get<BlockNumberFor<Self>>;

		/// The maximum length of a suffix.
		#[pallet::constant]
		type MaxSuffixLength: Get<u32>;

		/// The maximum length of a username, including its suffix and any system-added delimiters.
		#[pallet::constant]
		type MaxUsernameLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Information that is pertinent to identify the entity behind an account.
	#[pallet::storage]
	pub(super) type IdentityOf<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		(Registration<T::AccountId, T::MaxRegistrars, T::IdentityInformation>, Option<Username<T>>),
		OptionQuery,
	>;
	/// The super-identity of an alternative "sub" identity together with its name, within that
	/// context. If the account is not some other account's sub-identity, then just `None`.
	#[pallet::storage]
	pub(super) type SuperOf<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (T::AccountId, Data), OptionQuery>;

	/// Alternative "sub" identities of this account.
	///
	/// The first item is the deposit, the second is a vector of the accounts.
	///
	/// TWOX-NOTE: OK ― `AccountId` is a secure hash.
	#[pallet::storage]
	pub(super) type SubsOf<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<T::AccountId, T::MaxSubAccounts>,
		ValueQuery,
	>;

	/// The set of registrars. Not expected to get very big as can only be added
	/// through a special origin (likely a council motion).
	#[pallet::storage]
	pub(super) type Registrars<T: Config> = StorageValue<
		_,
		BoundedVec<
			Option<
				RegistrarInfo<
					T::AccountId,
					<T::IdentityInformation as IdentityInformationProvider>::FieldsIdentifier,
				>,
			>,
			T::MaxRegistrars,
		>,
		ValueQuery,
	>;

	/// A map of the accounts who are authorized to grant usernames.
	#[pallet::storage]
	pub(super) type UsernameAuthorities<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, AuthorityPropertiesOf<T>, OptionQuery>;

	/// Reverse lookup from `username` to the `AccountId` that has registered it. The value should
	/// be a key in the `IdentityOf` map, but it may not if the user has cleared their identity.
	///
	/// Multiple usernames may map to the same `AccountId`, but `IdentityOf` will only map to one
	/// primary username.
	#[pallet::storage]
	pub(super) type AccountOfUsername<T: Config> =
		StorageMap<_, Blake2_128Concat, Username<T>, T::AccountId, OptionQuery>;

	/// Usernames that an authority has granted, but that the account controller has not confirmed
	/// that they want it. Used primarily in cases where the `AccountId` cannot provide a signature
	/// because they are a pure proxy, multisig, etc. In order to confirm it, they should call
	/// [`Call::accept_username`].
	///
	/// First tuple item is the account and second is the acceptance deadline.
	#[pallet::storage]
	pub type PendingUsernames<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Username<T>,
		(T::AccountId, BlockNumberFor<T>),
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Too many subs-accounts.
		TooManySubAccounts,
		/// Account isn't found.
		NotFound,
		/// Registrar not found.
		RegistrarNotFound,
		/// Registrar already exists.
		RegistrarAlreadyExists,
		/// Account isn't named.
		NotNamed,
		/// Empty index.
		EmptyIndex,
		/// No identity found.
		NoIdentity,
		/// Sticky judgement.
		StickyJudgement,
		/// Judgement given.
		JudgementGiven,
		/// Invalid judgement.
		InvalidJudgement,
		/// The index is invalid.
		InvalidIndex,
		/// The target is invalid.
		InvalidTarget,
		/// Too many additional fields.
		TooManyFields,
		/// Maximum amount of registrars reached. Cannot add any more.
		TooManyRegistrars,
		/// Account ID is already named.
		AlreadyClaimed,
		/// Sender is not a sub-account.
		NotSub,
		/// Sub-account isn't owned by sender.
		NotOwned,
		/// The provided judgement was for a different identity.
		JudgementForDifferentIdentity,
		/// Error that occurs when there is an issue paying for judgement.
		JudgementPaymentFailed,
		/// The provided suffix is too long.
		InvalidSuffix,
		/// The sender does not have permission to issue a username.
		NotUsernameAuthority,
		/// The authority cannot allocate any more usernames.
		NoAllocation,
		/// The signature on a username was not valid.
		InvalidSignature,
		/// Setting this username requires a signature, but none was provided.
		RequiresSignature,
		/// The username does not meet the requirements.
		InvalidUsername,
		/// The username is already taken.
		UsernameTaken,
		/// The requested username does not exist.
		NoUsername,
		/// The username cannot be forcefully removed because it can still be accepted.
		NotExpired,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A name was set or reset (which will remove all judgements).
		IdentitySet { who: T::AccountId },
		/// A name was cleared, and the given balance returned.
		IdentityCleared { who: T::AccountId },
		/// A name was removed and the given balance slashed.
		IdentityKilled { who: T::AccountId },
		/// A judgement was asked from a registrar.
		JudgementRequested { who: T::AccountId, registrar: T::AccountId },
		/// A judgement request was retracted.
		JudgementUnrequested { who: T::AccountId, registrar: T::AccountId },
		/// A judgement was given by a registrar.
		JudgementGiven { target: T::AccountId, registrar: T::AccountId },
		/// A registrar was added.
		RegistrarAdded { registrar_index: RegistrarIndex },
		/// A registrar was removed.
		RegistrarRemoved { registrar: T::AccountId },
		/// A sub-identity was added to an identity and the deposit paid.
		SubIdentityAdded { sub: T::AccountId, main: T::AccountId },
		/// A sub-identity was removed from an identity and the deposit freed.
		SubIdentityRemoved { sub: T::AccountId, main: T::AccountId },
		/// A sub-identity was cleared, and the given deposit repatriated from the
		/// main identity account to the sub-identity account.
		SubIdentityRevoked { sub: T::AccountId, main: T::AccountId },
		/// A username authority was added.
		AuthorityAdded { authority: T::AccountId },
		/// A username authority was removed.
		AuthorityRemoved { authority: T::AccountId },
		/// A username was set for `who`.
		UsernameSet { who: T::AccountId, username: Username<T> },
		/// A username was queued, but `who` must accept it prior to `expiration`.
		UsernameQueued { who: T::AccountId, username: Username<T>, expiration: BlockNumberFor<T> },
		/// A queued username passed its expiration without being claimed and was removed.
		PreapprovalExpired { whose: T::AccountId },
		/// A username was set as a primary and can be looked up from `who`.
		PrimaryUsernameSet { who: T::AccountId, username: Username<T> },
		/// A dangling username (as in, a username corresponding to an account that has removed its
		/// identity) has been removed.
		DanglingUsernameRemoved { who: T::AccountId, username: Username<T> },
	}

	#[pallet::call]
	/// Identity pallet declaration.
	impl<T: Config> Pallet<T> {
		/// Add a registrar to the system.
		///
		/// The dispatch origin for this call must be `T::RegistrarOrigin`.
		///
		/// - `account`: the account of the registrar.
		///
		/// Emits `RegistrarAdded` if successful.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_registrar(T::MaxRegistrars::get()))]
		pub fn add_registrar(
			origin: OriginFor<T>,
			account: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?;
			let account = T::Lookup::lookup(account)?;

			let (i, registrar_count) = <Registrars<T>>::try_mutate(
				|registrars| -> Result<(RegistrarIndex, usize), DispatchError> {
					ensure!(
						!registrars.iter().any(|registrar| match registrar {
							Some(registrar_info) => registrar_info.account == account,
							None => false,
						}),
						Error::<T>::RegistrarAlreadyExists
					);
					registrars
						.try_push(Some(RegistrarInfo { account, fields: Default::default() }))
						.map_err(|_| Error::<T>::TooManyRegistrars)?;
					Ok(((registrars.len() - 1) as RegistrarIndex, registrars.len()))
				},
			)?;

			Self::deposit_event(Event::RegistrarAdded { registrar_index: i });

			Ok(Some(T::WeightInfo::add_registrar(registrar_count as u32)).into())
		}

		/// Set an account's identity information
		///
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `info`: The identity information.
		///
		/// Emits `IdentitySet` if successful.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_identity(T::MaxRegistrars::get()))]
		pub fn set_identity(
			origin: OriginFor<T>,
			info: Box<T::IdentityInformation>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let (id, username) = match <IdentityOf<T>>::get(&sender) {
				Some((mut id, maybe_username)) => (
					{
						// Only keep non-positive judgements.
						id.judgements.retain(|j| j.1.is_sticky());
						id.info = *info;
						id
					},
					maybe_username,
				),
				None => (Registration { info: *info, judgements: BoundedVec::default() }, None),
			};

			let judgements = id.judgements.len();
			<IdentityOf<T>>::insert(&sender, (id, username));
			Self::deposit_event(Event::IdentitySet { who: sender });

			Ok(Some(T::WeightInfo::set_identity(judgements as u32)).into())
		}

		/// Set the sub-accounts of the sender.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// identity.
		///
		/// - `subs`: The identity's (new) sub-accounts.
		// TODO: This whole extrinsic screams "not optimized". For example we could
		// filter any overlap between new and old subs, and avoid reading/writing
		// to those values... We could also ideally avoid needing to write to
		// N storage items for N sub accounts. Right now the weight on this function
		// is a large overestimate due to the fact that it could potentially write
		// to 2 x T::MaxSubAccounts::get().
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_subs_old(T::MaxSubAccounts::get())
			.saturating_add(T::WeightInfo::set_subs_new(subs.len() as u32))
		)]
		pub fn set_subs(
			origin: OriginFor<T>,
			subs: Vec<(T::AccountId, Data)>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(<IdentityOf<T>>::contains_key(&sender), Error::<T>::NotFound);
			ensure!(
				subs.len() <= T::MaxSubAccounts::get() as usize,
				Error::<T>::TooManySubAccounts
			);

			let old_ids = <SubsOf<T>>::get(&sender);

			let not_other_sub =
				subs.iter().filter_map(|i| SuperOf::<T>::get(&i.0)).all(|i| i.0 == sender);
			ensure!(not_other_sub, Error::<T>::AlreadyClaimed);

			for s in old_ids.iter() {
				<SuperOf<T>>::remove(s);
			}
			let mut ids = BoundedVec::<T::AccountId, T::MaxSubAccounts>::default();
			for (id, name) in subs {
				<SuperOf<T>>::insert(&id, (sender.clone(), name));
				ids.try_push(id).expect("subs length is less than T::MaxSubAccounts; qed");
			}
			let new_subs = ids.len();

			if ids.is_empty() {
				<SubsOf<T>>::remove(&sender);
			} else {
				<SubsOf<T>>::insert(&sender, ids);
			}

			Ok(Some(
				T::WeightInfo::set_subs_old(old_ids.len() as u32) // P: Real number of old accounts removed.
					// S: New subs added
					.saturating_add(T::WeightInfo::set_subs_new(new_subs as u32)),
			)
			.into())
		}
		/// Clear an account's identity info and all sub-accounts
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// Emits `IdentityCleared` if successful.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::clear_identity(
			T::MaxRegistrars::get(),
			T::MaxSubAccounts::get(),
		))]
		pub fn clear_identity(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let sub_ids = <SubsOf<T>>::take(&sender);
			let (id, maybe_username) =
				<IdentityOf<T>>::take(&sender).ok_or(Error::<T>::NoIdentity)?;
			for sub in sub_ids.iter() {
				<SuperOf<T>>::remove(sub);
			}
			if let Some(username) = maybe_username {
				AccountOfUsername::<T>::remove(username);
			}

			Self::deposit_event(Event::IdentityCleared { who: sender });

			#[allow(deprecated)]
			Ok(Some(T::WeightInfo::clear_identity(
				id.judgements.len() as u32,
				sub_ids.len() as u32,
			))
			.into())
		}

		/// Request a judgement from a registrar.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// - `reg_index`: The index of the registrar whose judgement is requested.
		///
		/// Emits `JudgementRequested` if successful.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::request_judgement(T::MaxRegistrars::get(),))]
		pub fn request_judgement(
			origin: OriginFor<T>,
			registrar: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let registrars = <Registrars<T>>::get();

			let registrar_acc = registrars
				.iter()
				.find_map(|reg_info| match reg_info {
					Some(info) if info.account == registrar => Some(info.account.clone()),
					_ => None,
				})
				.ok_or(Error::<T>::RegistrarNotFound)?;

			let (mut id, username) = <IdentityOf<T>>::get(&sender).ok_or(Error::<T>::NoIdentity)?;

			let item = (registrar_acc, Judgement::Requested);

			match id.judgements.binary_search_by_key(&registrar, |x| x.0.clone()) {
				Ok(i) =>
					if id.judgements[i].1.is_sticky() {
						return Err(Error::<T>::StickyJudgement.into());
					} else {
						id.judgements[i] = item
					},
				Err(i) =>
					id.judgements.try_insert(i, item).map_err(|_| Error::<T>::TooManyRegistrars)?,
			}

			let judgements = id.judgements.len();
			<IdentityOf<T>>::insert(&sender, (id, username));

			Self::deposit_event(Event::JudgementRequested { who: sender, registrar });

			Ok(Some(T::WeightInfo::request_judgement(judgements as u32)).into())
		}

		/// Cancel a previous request.
		///
		/// Payment: A previously reserved deposit is returned on success.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// - `reg_index`: The index of the registrar whose judgement is no longer requested.
		///
		/// Emits `JudgementUnrequested` if successful.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::cancel_request(T::MaxRegistrars::get()))]
		pub fn cancel_request(
			origin: OriginFor<T>,
			registrar: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let (mut id, username) = <IdentityOf<T>>::get(&sender).ok_or(Error::<T>::NoIdentity)?;

			let pos = id
				.judgements
				.binary_search_by_key(&registrar, |x| x.0.clone())
				.map_err(|_| Error::<T>::NotFound)?;

			if let Judgement::Requested = id.judgements[pos].1 {
				// Judgement is in the "Requested" state, proceed with the cancellation
				id.judgements.remove(pos);
			} else {
				return Err(Error::<T>::JudgementGiven.into());
			}

			let judgements = id.judgements.len();
			<IdentityOf<T>>::insert(&sender, (id, username));

			Self::deposit_event(Event::JudgementUnrequested { who: sender, registrar });

			Ok(Some(T::WeightInfo::cancel_request(judgements as u32)).into())
		}

		/// Change the account associated with a registrar.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must be the account of the registrar whose index is `index`.
		///
		/// - `index`: the index of the registrar whose fee is to be set.
		/// - `new`: the new account ID.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::set_account_id(T::MaxRegistrars::get()))]
		pub fn set_account_id(
			origin: OriginFor<T>,
			new: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let new = T::Lookup::lookup(new)?;

			let registrars =
				<Registrars<T>>::mutate(|registrars| -> Result<usize, DispatchError> {
					let registrar_found = registrars.iter_mut().any(|registrar_option| {
						if let Some(registrar) = registrar_option {
							if registrar.account == who {
								registrar.account = new.clone();
								return true;
							}
						}
						false
					});

					if !registrar_found {
						return Err(DispatchError::from(Error::<T>::RegistrarNotFound));
					}

					Ok(registrars.len())
				})?;

			Ok(Some(T::WeightInfo::set_account_id(registrars as u32)).into())
		}

		/// Set the field information for a registrar.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must be the account of the registrar whose index is `index`.
		///
		/// - `index`: the index of the registrar whose fee is to be set.
		/// - `fields`: the fields that the registrar concerns themselves with.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::set_fields(T::MaxRegistrars::get()))]
		pub fn set_fields(
			origin: OriginFor<T>,
			fields: <T::IdentityInformation as IdentityInformationProvider>::FieldsIdentifier,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let registrars =
				<Registrars<T>>::mutate(|registrars| -> Result<usize, DispatchError> {
					let registrar = registrars.iter_mut().any(|registrar_option| {
						if let Some(registrar) = registrar_option {
							if registrar.account == who {
								registrar.fields = fields.clone();
								return true;
							}
						}
						false
					});

					if !registrar {
						return Err(DispatchError::from(Error::<T>::RegistrarNotFound));
					}

					Ok(registrars.len())
				})?;

			Ok(Some(T::WeightInfo::set_fields(registrars as u32)).into())
		}

		/// Provide a judgement for an account's identity.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must be the account of the registrar whose index is `reg_index`.
		///
		/// - `reg_index`: the index of the registrar whose judgement is being made.
		/// - `target`: the account whose identity the judgement is upon. This must be an account
		///   with a registered identity.
		/// - `judgement`: the judgement of the registrar of index `reg_index` about `target`.
		/// - `identity`: The hash of the [`IdentityInfo`] for that the judgement is provided.
		///
		/// Emits `JudgementGiven` if successful.
		///
		/// ## Complexity
		/// - `O(R + X)`.
		///   - where `R` registrar-count (governance-bounded).
		///   - where `X` additional-field-count (deposit-bounded and code-bounded).
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::provide_judgement(T::MaxRegistrars::get()))]
		pub fn provide_judgement(
			origin: OriginFor<T>,
			target: AccountIdLookupOf<T>,
			judgement: Judgement,
			identity: T::Hash,
		) -> DispatchResultWithPostInfo {
			let registrar = ensure_signed(origin)?;
			let target = T::Lookup::lookup(target)?;
			ensure!(!judgement.has_requested(), Error::<T>::InvalidJudgement);

			let registrars = Registrars::<T>::get();
			let registrar_found =
				registrars.iter().any(|registrar_option| match registrar_option {
					Some(r) => r.account == registrar,
					None => false,
				});

			ensure!(registrar_found, Error::<T>::RegistrarNotFound);

			let (mut id, username) =
				<IdentityOf<T>>::get(&target).ok_or(Error::<T>::InvalidTarget)?;

			if T::Hashing::hash_of(&id.info) != identity {
				return Err(Error::<T>::JudgementForDifferentIdentity.into());
			}

			let item = (registrar.clone(), judgement);
			match id.judgements.binary_search_by_key(&registrar, |x| x.0.clone()) {
				Ok(position) => id.judgements[position] = item,
				Err(position) => id
					.judgements
					.try_insert(position, item)
					.map_err(|_| Error::<T>::TooManyRegistrars)?,
			}

			let judgements = id.judgements.len();
			<IdentityOf<T>>::insert(&target, (id, username));
			Self::deposit_event(Event::JudgementGiven { target, registrar });

			Ok(Some(T::WeightInfo::provide_judgement(judgements as u32)).into())
		}

		/// Remove an account's identity
		///
		/// The dispatch origin for this call must match `T::RegistrarOrigin`.
		///
		/// - `target`: the account whose identity the judgement is upon. This must be an account
		///   with a registered identity.
		///
		/// Emits `IdentityKilled` if successful.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::kill_identity(
			T::MaxRegistrars::get(), // R
			T::MaxSubAccounts::get(),
		))]
		pub fn kill_identity(
			origin: OriginFor<T>,
			target: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?;

			// Figure out who we're meant to be clearing.
			let target = T::Lookup::lookup(target)?;
			let sub_ids = <SubsOf<T>>::take(&target);
			let (id, maybe_username) =
				<IdentityOf<T>>::take(&target).ok_or(Error::<T>::NoIdentity)?;
			for sub in sub_ids.iter() {
				<SuperOf<T>>::remove(sub);
			}
			if let Some(username) = maybe_username {
				AccountOfUsername::<T>::remove(username);
			}
			Self::deposit_event(Event::IdentityKilled { who: target });

			#[allow(deprecated)]
			Ok(Some(T::WeightInfo::kill_identity(id.judgements.len() as u32, sub_ids.len() as u32))
				.into())
		}

		/// Add the given account to the sender's subs.
		///
		/// Payment: Balance reserved by a previous `set_subs` call for one sub will be repatriated
		/// to the sender.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// sub identity of `sub`.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::add_sub(T::MaxSubAccounts::get()))]
		pub fn add_sub(
			origin: OriginFor<T>,
			sub: AccountIdLookupOf<T>,
			data: Data,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let sub = T::Lookup::lookup(sub)?;
			ensure!(IdentityOf::<T>::contains_key(&sender), Error::<T>::NoIdentity);

			// Check if it's already claimed as sub-identity.
			ensure!(!SuperOf::<T>::contains_key(&sub), Error::<T>::AlreadyClaimed);

			SubsOf::<T>::try_mutate(&sender, |ref mut sub_ids| {
				// Ensure there is space and that the deposit is paid.
				ensure!(
					sub_ids.len() < T::MaxSubAccounts::get() as usize,
					Error::<T>::TooManySubAccounts
				);

				SuperOf::<T>::insert(&sub, (sender.clone(), data));
				sub_ids.try_push(sub.clone()).expect("sub ids length checked above; qed");

				Self::deposit_event(Event::SubIdentityAdded { sub, main: sender.clone() });
				Ok(())
			})
		}

		/// Alter the associated name of the given sub-account.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// sub identity of `sub`.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::rename_sub(T::MaxSubAccounts::get()))]
		pub fn rename_sub(
			origin: OriginFor<T>,
			sub: AccountIdLookupOf<T>,
			data: Data,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let sub = T::Lookup::lookup(sub)?;
			ensure!(IdentityOf::<T>::contains_key(&sender), Error::<T>::NoIdentity);
			ensure!(SuperOf::<T>::get(&sub).map_or(false, |x| x.0 == sender), Error::<T>::NotOwned);
			SuperOf::<T>::insert(&sub, (sender, data));
			Ok(())
		}

		/// Remove the given account from the sender's subs.
		///
		/// Payment: Balance reserved by a previous `set_subs` call for one sub will be repatriated
		/// to the sender.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// sub identity of `sub`.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::remove_sub(T::MaxSubAccounts::get()))]
		pub fn remove_sub(origin: OriginFor<T>, sub: AccountIdLookupOf<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(IdentityOf::<T>::contains_key(&sender), Error::<T>::NoIdentity);
			let sub = T::Lookup::lookup(sub)?;
			let (sup, _) = SuperOf::<T>::get(&sub).ok_or(Error::<T>::NotSub)?;
			ensure!(sup == sender, Error::<T>::NotOwned);
			SuperOf::<T>::remove(&sub);
			SubsOf::<T>::mutate(&sup, |ref mut sub_ids| {
				sub_ids.retain(|x| x != &sub);
				Self::deposit_event(Event::SubIdentityRemoved { sub, main: sender });
			});
			Ok(())
		}

		/// Remove the sender as a sub-account.
		///
		/// Payment: Balance reserved by a previous `set_subs` call for one sub will be repatriated
		/// to the sender (*not* the original depositor).
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// super-identity.
		///
		/// NOTE: This should not normally be used, but is provided in the case that the non-
		/// controller of an account is maliciously registered as a sub-account.
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::quit_sub(T::MaxSubAccounts::get()))]
		pub fn quit_sub(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let (sup, _) = SuperOf::<T>::take(&sender).ok_or(Error::<T>::NotSub)?;
			SubsOf::<T>::mutate(&sup, |ref mut sub_ids| {
				sub_ids.retain(|x| x != &sender);
				Self::deposit_event(Event::SubIdentityRevoked { sub: sender, main: sup.clone() });
			});
			Ok(())
		}

		/// Add an `AccountId` with permission to grant usernames with a given `suffix` appended.
		///
		/// The authority can grant up to `allocation` usernames. To top up their allocation, they
		/// should just issue (or request via governance) a new `add_username_authority` call.
		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::add_username_authority())]
		pub fn add_username_authority(
			origin: OriginFor<T>,
			authority: AccountIdLookupOf<T>,
			suffix: Vec<u8>,
			allocation: u32,
		) -> DispatchResult {
			T::UsernameAuthorityOrigin::ensure_origin(origin)?;
			let authority = T::Lookup::lookup(authority)?;
			// We don't need to check the length because it gets checked when casting into a
			// `BoundedVec`.
			Self::validate_suffix(&suffix).map_err(|_| Error::<T>::InvalidSuffix)?;
			let suffix = Suffix::<T>::try_from(suffix).map_err(|_| Error::<T>::InvalidSuffix)?;
			// The authority may already exist, but we don't need to check. They might be changing
			// their suffix or adding allocation, so we just want to overwrite whatever was there.
			UsernameAuthorities::<T>::insert(
				&authority,
				AuthorityPropertiesOf::<T> { suffix, allocation },
			);
			Self::deposit_event(Event::AuthorityAdded { authority });
			Ok(())
		}

		/// Remove `authority` from the username authorities.
		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::remove_username_authority())]
		pub fn remove_username_authority(
			origin: OriginFor<T>,
			authority: AccountIdLookupOf<T>,
		) -> DispatchResult {
			T::UsernameAuthorityOrigin::ensure_origin(origin)?;
			let authority = T::Lookup::lookup(authority)?;
			UsernameAuthorities::<T>::take(&authority).ok_or(Error::<T>::NotUsernameAuthority)?;
			Self::deposit_event(Event::AuthorityRemoved { authority });
			Ok(())
		}

		/// Set the username for `who`. Must be called by a username authority.
		///
		/// The authority must have an `allocation`. Users can either pre-sign their usernames or
		/// accept them later.
		///
		/// Usernames must:
		///   - Only contain lowercase ASCII characters or digits.
		///   - When combined with the suffix of the issuing authority be _less than_ the
		///     `MaxUsernameLength`.
		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::set_username_for())]
		pub fn set_username_for(
			origin: OriginFor<T>,
			who: AccountIdLookupOf<T>,
			username: Vec<u8>,
			signature: Option<T::OffchainSignature>,
		) -> DispatchResult {
			// Ensure origin is a Username Authority and has an allocation. Decrement their
			// allocation by one.
			let sender = ensure_signed(origin)?;
			let suffix = UsernameAuthorities::<T>::try_mutate(
				&sender,
				|maybe_authority| -> Result<Suffix<T>, DispatchError> {
					let properties =
						maybe_authority.as_mut().ok_or(Error::<T>::NotUsernameAuthority)?;
					ensure!(properties.allocation > 0, Error::<T>::NoAllocation);
					properties.allocation.saturating_dec();
					Ok(properties.suffix.clone())
				},
			)?;

			// Ensure that the username only contains allowed characters. We already know the suffix
			// does.
			let username_length = username.len().saturating_add(suffix.len()) as u32;
			Self::validate_username(&username, Some(username_length))?;

			// Concatenate the username with suffix and cast into a BoundedVec. Should be infallible
			// since we already ensured it is below the max length.
			let mut full_username =
				Vec::with_capacity(username.len().saturating_add(suffix.len()).saturating_add(1));
			full_username.extend(username);
			full_username.extend(b"@");
			full_username.extend(suffix);
			let bounded_username =
				Username::<T>::try_from(full_username).map_err(|_| Error::<T>::InvalidUsername)?;

			// Usernames must be unique. Ensure it's not taken.
			ensure!(
				!AccountOfUsername::<T>::contains_key(&bounded_username),
				Error::<T>::UsernameTaken
			);
			ensure!(
				!PendingUsernames::<T>::contains_key(&bounded_username),
				Error::<T>::UsernameTaken
			);

			// Insert or queue.
			let who = T::Lookup::lookup(who)?;
			if let Some(s) = signature {
				// Account has pre-signed an authorization. Verify the signature provided and grant
				// the username directly.
				let encoded = Encode::encode(&bounded_username.to_vec());
				Self::validate_signature(&encoded, &s, &who)?;
				Self::insert_username(&who, bounded_username);
			} else {
				// The user must accept the username, therefore, queue it.
				Self::queue_acceptance(&who, bounded_username);
			}
			Ok(())
		}

		/// Accept a given username that an `authority` granted. The call must include the full
		/// username, as in `username.suffix`.
		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::accept_username())]
		pub fn accept_username(origin: OriginFor<T>, username: Username<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let (approved_for, _) =
				PendingUsernames::<T>::take(&username).ok_or(Error::<T>::NoUsername)?;
			ensure!(approved_for == who.clone(), Error::<T>::InvalidUsername);
			Self::insert_username(&who, username.clone());
			Self::deposit_event(Event::UsernameSet { who: who.clone(), username });
			Ok(())
		}

		/// Remove an expired username approval. The username was approved by an authority but never
		/// accepted by the user and must now be beyond its expiration. The call must include the
		/// full username, as in `username.suffix`.
		#[pallet::call_index(18)]
		#[pallet::weight(T::WeightInfo::remove_expired_approval())]
		pub fn remove_expired_approval(
			origin: OriginFor<T>,
			username: Username<T>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			if let Some((who, expiration)) = PendingUsernames::<T>::take(&username) {
				let now = frame_system::Pallet::<T>::block_number();
				ensure!(now > expiration, Error::<T>::NotExpired);
				Self::deposit_event(Event::PreapprovalExpired { whose: who.clone() });
				Ok(())
			} else {
				Err(Error::<T>::NoUsername.into())
			}
		}

		/// Set a given username as the primary. The username should include the suffix.
		#[pallet::call_index(19)]
		#[pallet::weight(T::WeightInfo::set_primary_username())]
		pub fn set_primary_username(origin: OriginFor<T>, username: Username<T>) -> DispatchResult {
			// ensure `username` maps to `origin` (i.e. has already been set by an authority).
			let who = ensure_signed(origin)?;
			ensure!(AccountOfUsername::<T>::contains_key(&username), Error::<T>::NoUsername);
			let (registration, _maybe_username) =
				IdentityOf::<T>::get(&who).ok_or(Error::<T>::NoIdentity)?;
			IdentityOf::<T>::insert(&who, (registration, Some(username.clone())));
			Self::deposit_event(Event::PrimaryUsernameSet { who: who.clone(), username });
			Ok(())
		}

		/// Remove a username that corresponds to an account with no identity. Exists when a user
		/// gets a username but then calls `clear_identity`.
		#[pallet::call_index(20)]
		#[pallet::weight(T::WeightInfo::remove_dangling_username())]
		pub fn remove_dangling_username(
			origin: OriginFor<T>,
			username: Username<T>,
		) -> DispatchResult {
			// ensure `username` maps to `origin` (i.e. has already been set by an authority).
			let _ = ensure_signed(origin)?;
			let who = AccountOfUsername::<T>::take(&username).ok_or(Error::<T>::NoUsername)?;
			ensure!(!IdentityOf::<T>::contains_key(&who), Error::<T>::InvalidUsername);
			Self::deposit_event(Event::DanglingUsernameRemoved { who: who.clone(), username });
			Ok(())
		}
		/// Remove a registrar from the system.
		///
		/// The dispatch origin for this call must be `T::RegistrarOrigin`.
		///
		/// - `account`: the account of the registrar.
		///
		/// Emits `RegistrarRemoved` if successful.
		#[pallet::call_index(21)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_registrar(T::MaxRegistrars::get()))]
		pub fn remove_registrar(
			origin: OriginFor<T>,
			account: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?;
			let account_id = T::Lookup::lookup(account)?;

			let registrar_count =
				<Registrars<T>>::try_mutate(|registrars| -> Result<usize, DispatchError> {
					let position = registrars
						.iter()
						.position(|registrar_option| {
							if let Some(registrar_info) = registrar_option {
								registrar_info.account == account_id
							} else {
								false
							}
						})
						.ok_or(Error::<T>::RegistrarNotFound)?;
					registrars.remove(position);
					Ok(registrars.len())
				})?;

			Self::deposit_event(Event::RegistrarRemoved { registrar: account_id });

			Ok(Some(T::WeightInfo::remove_registrar(registrar_count as u32)).into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Get the subs of an account.
	pub fn subs(who: &T::AccountId) -> Vec<(T::AccountId, Data)> {
		SubsOf::<T>::get(who)
			.into_iter()
			.filter_map(|a| SuperOf::<T>::get(&a).map(|(_super_account, data)| (a, data)))
			.collect()
	}

	/// Check if the account has corresponding identity information by the
	/// identity field.
	pub fn has_identity(
		who: &T::AccountId,
		fields: <T::IdentityInformation as IdentityInformationProvider>::FieldsIdentifier,
	) -> bool {
		IdentityOf::<T>::get(who)
			.map_or(false, |(registration, _username)| (registration.info.has_identity(fields)))
	}
	/// Validate that a username conforms to allowed characters/format.
	///
	/// The function will validate the characters in `username` and that `length` (if `Some`)
	/// conforms to the limit. It is not expected to pass a fully formatted username here (i.e. one
	/// with any protocol-added characters included, such as a `.`). The suffix is also separately
	/// validated by this function to ensure the full username conforms.
	fn validate_username(username: &Vec<u8>, length: Option<u32>) -> DispatchResult {
		// Verify input length before allocating a Vec with the user's input. `<` instead of `<=`
		// because it needs one element for the point (`username` + `.` + `suffix`).
		if let Some(l) = length {
			ensure!(l < T::MaxUsernameLength::get(), Error::<T>::InvalidUsername);
		}
		// Usernames cannot be empty.
		ensure!(!username.is_empty(), Error::<T>::InvalidUsername);
		// Username must be lowercase and alphanumeric.
		ensure!(Self::is_valid_username(username), Error::<T>::InvalidUsername);
		Ok(())
	}

	fn is_valid_username(input: &[u8]) -> bool {
		// Check if input is empty, does not start with a lowercase letter, or ends with a period
		if input.is_empty() || !input[0].is_ascii_lowercase() || *input.last().unwrap() == b'.' {
			return false;
		}

		// Check for valid characters and consecutive periods using a fold operation
		let is_valid = input
			.iter()
			.fold((true, None), |(valid, last_char), &cur_char| {
				(
					valid &&
						(cur_char.is_ascii_lowercase() ||
							cur_char.is_ascii_digit() || cur_char == b'.') &&
						!(last_char == Some(b'.') && cur_char == b'.'),
					Some(cur_char),
				)
			})
			.0;

		is_valid
	}

	/// Validate that a suffix conforms to allowed characters/format.
	fn validate_suffix(suffix: &Vec<u8>) -> DispatchResult {
		// Suffix name cannot be empty.
		ensure!(!suffix.is_empty(), Error::<T>::InvalidSuffix);
		// Suffix name must be lowercase.
		ensure!(suffix.iter().all(|byte| byte.is_ascii_lowercase()), Error::<T>::InvalidSuffix);
		Ok(())
	}

	/// Validate a signature. Supports signatures on raw `data` or `data` wrapped in HTML `<Bytes>`.
	pub fn validate_signature(
		data: &Vec<u8>,
		signature: &T::OffchainSignature,
		signer: &T::AccountId,
	) -> DispatchResult {
		// Happy path, user has signed the raw data.
		if signature.verify(&data[..], &signer) {
			return Ok(());
		}
		// NOTE: for security reasons modern UIs implicitly wrap the data requested to sign into
		// `<Bytes> + data + </Bytes>`, so why we support both wrapped and raw versions.
		let prefix = b"<Bytes>";
		let suffix = b"</Bytes>";
		let mut wrapped: Vec<u8> = Vec::with_capacity(data.len() + prefix.len() + suffix.len());
		wrapped.extend(prefix);
		wrapped.extend(data);
		wrapped.extend(suffix);

		ensure!(signature.verify(&wrapped[..], &signer), Error::<T>::InvalidSignature);

		Ok(())
	}

	/// A username has met all conditions. Insert the relevant storage items.
	pub fn insert_username(who: &T::AccountId, username: Username<T>) {
		// Check if they already have a primary. If so, leave it. If not, set it.
		// Likewise, check if they have an identity. If not, give them a minimal one.
		let (reg, primary_username, new_is_primary) = match <IdentityOf<T>>::get(&who) {
			// User has an existing Identity and a primary username. Leave it.
			Some((reg, Some(primary))) => (reg, primary, false),
			// User has an Identity but no primary. Set the new one as primary.
			Some((reg, None)) => (reg, username.clone(), true),
			// User does not have an existing Identity. Give them a fresh default one and set
			// their username as primary.
			None => (
				Registration { info: Default::default(), judgements: Default::default() },
				username.clone(),
				true,
			),
		};

		// Enter in identity map. Note: In the case that the user did not have a pre-existing
		// Identity, we have given them the storage item for free. If they ever call
		// `set_identity` with identity info, then they will need to place the normal identity
		// deposit.
		IdentityOf::<T>::insert(&who, (reg, Some(primary_username)));
		// Enter in username map.
		AccountOfUsername::<T>::insert(username.clone(), &who);
		Self::deposit_event(Event::UsernameSet { who: who.clone(), username: username.clone() });
		if new_is_primary {
			Self::deposit_event(Event::PrimaryUsernameSet { who: who.clone(), username });
		}
	}

	/// A username was granted by an authority, but must be accepted by `who`. Put the username
	/// into a queue for acceptance.
	pub fn queue_acceptance(who: &T::AccountId, username: Username<T>) {
		let now = frame_system::Pallet::<T>::block_number();
		let expiration = now.saturating_add(T::PendingUsernameExpiration::get());
		PendingUsernames::<T>::insert(&username, (who.clone(), expiration));
		Self::deposit_event(Event::UsernameQueued { who: who.clone(), username, expiration });
	}

	/// Reap an identity, clearing associated storage items and refunding any deposits. This
	/// function is very similar to (a) `clear_identity`, but called on a `target` account instead
	/// of self; and (b) `kill_identity`, but without imposing a slash.
	///
	/// Parameters:
	/// - `target`: The account for which to reap identity state.
	///
	/// Return type is a tuple of the number of registrars, `IdentityInfo` bytes, and sub accounts,
	/// respectively.
	///
	/// NOTE: This function is here temporarily for migration of Identity info from the Polkadot
	/// Relay Chain into a system parachain. It will be removed after the migration.
	pub fn reap_identity(who: &T::AccountId) -> Result<(u32, u32, u32), DispatchError> {
		// `take` any storage items keyed by `target`
		// identity
		let (id, _maybe_username) = <IdentityOf<T>>::take(&who).ok_or(Error::<T>::NoIdentity)?;
		let registrars = id.judgements.len() as u32;
		let encoded_byte_size = id.info.encoded_size() as u32;

		// subs
		let sub_ids = <SubsOf<T>>::take(&who);
		let actual_subs = sub_ids.len() as u32;
		for sub in sub_ids.iter() {
			<SuperOf<T>>::remove(sub);
		}

		Ok((registrars, encoded_byte_size, actual_subs))
	}

	/// Set an identity with zero deposit. Used for benchmarking and XCM emulator tests that involve
	/// `rejig_deposit`.
	#[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
	pub fn set_identity_no_deposit(
		who: &T::AccountId,
		info: T::IdentityInformation,
	) -> DispatchResult {
		IdentityOf::<T>::insert(
			&who,
			(
				Registration { judgements: Default::default(), info: info.clone() },
				None::<Username<T>>,
			),
		);
		Ok(())
	}

	/// Set subs with zero deposit and default name. Only used for benchmarks that involve
	/// `rejig_deposit`.
	#[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
	pub fn set_subs_no_deposit(
		who: &T::AccountId,
		subs: Vec<(T::AccountId, Data)>,
	) -> DispatchResult {
		let mut sub_accounts = BoundedVec::<T::AccountId, T::MaxSubAccounts>::default();
		for (sub, name) in subs {
			<SuperOf<T>>::insert(&sub, (who.clone(), name));
			sub_accounts
				.try_push(sub)
				.expect("benchmark should not pass more than T::MaxSubAccounts");
		}
		SubsOf::<T>::insert::<&T::AccountId, BoundedVec<T::AccountId, T::MaxSubAccounts>>(
			&who,
			sub_accounts,
		);
		Ok(())
	}
}

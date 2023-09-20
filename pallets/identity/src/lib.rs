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
#[cfg(test)]
mod tests;
mod types;
pub mod weights;

use sp_runtime::traits::{AppendZerosInput, Hash, StaticLookup};
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub use pallet::*;
pub use types::{Data, IdentityField, IdentityFields, IdentityInfo, Judgement, Registration};

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

		/// Maximum number of additional fields that may be stored in an ID.
		/// Needed to bound the I/O required to access an identity, but can be
		/// pretty high.
		#[pallet::constant]
		type MaxAdditionalFields: Get<u32>;

		/// Maxmimum number of registrars allowed in the system. Needed to bound
		/// the complexity of, e.g., updating judgements.
		#[pallet::constant]
		type MaxRegistrars: Get<u32>;

		/// The origin which may add or remove registrars as well as remove
		/// identities. Root can always do this.
		type RegistrarOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Information that is pertinent to identify the entity behind an account.
	#[pallet::storage]
	#[pallet::getter(fn identity)]
	pub(super) type IdentityOf<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		Registration<T::AccountId, T::MaxRegistrars, T::MaxAdditionalFields>,
		OptionQuery,
	>;

	/// The set of registrars. Not expected to get very big as can only be added
	/// through a special origin (likely a council motion).
	#[pallet::storage]
	#[pallet::getter(fn registrars)]
	pub(super) type Registrars<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxRegistrars>, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
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
		/// The target is invalid.
		InvalidTarget,
		/// Too many additional fields.
		TooManyFields,
		/// Maximum amount of registrars reached. Cannot add any more.
		TooManyRegistrars,
		/// Account ID is already named.
		AlreadyClaimed,
		/// The provided judgement was for a different identity.
		JudgementForDifferentIdentity,
		/// Error that occurs when there is an issue paying for judgement.
		JudgementPaymentFailed,
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
		JudgementRequested { who: T::AccountId, registrar: T::AccountId, digest: T::Hash },
		/// A judgement request was retracted.
		JudgementUnrequested { who: T::AccountId, registrar: T::AccountId },
		/// A judgement was given by a registrar.
		JudgementGiven { target: T::AccountId, registrar: T::AccountId },
		/// A registrar was added.
		RegistrarAdded { registrar: T::AccountId },
		/// A registrar was removed.
		RegistrarRemoved { registrar: T::AccountId },
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

			let registrar_count =
				<Registrars<T>>::try_mutate(|registrars| -> Result<usize, DispatchError> {
					ensure!(!registrars.contains(&account), Error::<T>::RegistrarAlreadyExists); // Ensure the registrar does not already exist
					registrars
						.try_push(account.clone())
						.map_err(|_| Error::<T>::TooManyRegistrars)?;
					Ok(registrars.len())
				})?;

			Self::deposit_event(Event::RegistrarAdded { registrar: account });

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
		#[pallet::weight( T::WeightInfo::set_identity(
			T::MaxAdditionalFields::get(), // R
		))]
		pub fn set_identity(
			origin: OriginFor<T>,
			info: Box<IdentityInfo<T::MaxAdditionalFields>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let extra_fields = info.additional.len() as u32;
			ensure!(extra_fields <= T::MaxAdditionalFields::get(), Error::<T>::TooManyFields);

			// Check if identity already exists
			ensure!(!<IdentityOf<T>>::contains_key(&sender), Error::<T>::AlreadyClaimed);

			// Create a new Registration struct
			let id = Registration { info: *info, judgements: BoundedVec::default() };

			// Insert the new identity into storage
			<IdentityOf<T>>::insert(&sender, id);

			Self::deposit_event(Event::IdentitySet { who: sender });

			Ok(Some(T::WeightInfo::set_identity(
				extra_fields, // R
			))
			.into())
		}

		/// Clear an account's identity info and all sub-accounts and return all
		/// deposits.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// Emits `IdentityCleared` if successful.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::clear_identity(
			T::MaxRegistrars::get(), // R
			T::MaxAdditionalFields::get(), // X
		))]
		pub fn clear_identity(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let id = <IdentityOf<T>>::take(&sender).ok_or(Error::<T>::NotNamed)?;

			Self::deposit_event(Event::IdentityCleared { who: sender });

			Ok(Some(T::WeightInfo::clear_identity(
				id.judgements.len() as u32,      // R
				id.info.additional.len() as u32, // X
			))
			.into())
		}

		/// Request a judgement from a registrar.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// - `reg_index`: The index of the registrar whose judgement is
		///   requested.
		///
		/// Emits `JudgementRequested` if successful.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::request_judgement(
			T::MaxRegistrars::get(), // R
			T::MaxAdditionalFields::get(), // X
		))]
		pub fn request_judgement(
			origin: OriginFor<T>,
			registrar: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(<Registrars<T>>::get().contains(&registrar), Error::<T>::RegistrarNotFound);

			let mut id = <IdentityOf<T>>::get(&sender).ok_or(Error::<T>::NoIdentity)?;

			let item = (registrar.clone(), Judgement::Requested);
			match id.judgements.binary_search_by_key(&registrar, |x| x.0.clone()) {
				Ok(i) =>
					if id.judgements[i].1.is_sticky() {
						return Err(Error::<T>::StickyJudgement.into())
					} else {
						id.judgements[i] = item
					},
				Err(i) =>
					id.judgements.try_insert(i, item).map_err(|_| Error::<T>::TooManyRegistrars)?,
			}

			let judgements = id.judgements.len();
			let extra_fields = id.info.additional.len();
			let digest = T::Hashing::hash_of(&id.info);
			<IdentityOf<T>>::insert(&sender, id);

			Self::deposit_event(Event::JudgementRequested { who: sender, registrar, digest });

			Ok(Some(T::WeightInfo::request_judgement(judgements as u32, extra_fields as u32))
				.into())
		}

		/// Cancel a previous request.
		///
		/// Payment: A previously reserved deposit is returned on success.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must have a registered identity.
		///
		/// - `reg_index`: The index of the registrar whose judgement is no
		///   longer requested.
		///
		/// Emits `JudgementUnrequested` if successful.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::cancel_request(
			T::MaxRegistrars::get(), // R
			T::MaxAdditionalFields::get(), // X
		))]
		pub fn cancel_request(
			origin: OriginFor<T>,
			registrar: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let mut id = <IdentityOf<T>>::get(&sender).ok_or(Error::<T>::NoIdentity)?;

			let pos = id
				.judgements
				.binary_search_by_key(&registrar, |x| x.0.clone())
				.map_err(|_| Error::<T>::NotFound)?;

			if let Judgement::Requested = id.judgements[pos].1 {
				// Judgement is in the "Requested" state, proceed with the cancellation
				id.judgements.remove(pos);
			} else {
				return Err(Error::<T>::JudgementGiven.into())
			}

			let judgements = id.judgements.len();
			let extra_fields = id.info.additional.len();
			<IdentityOf<T>>::insert(&sender, id);

			Self::deposit_event(Event::JudgementUnrequested { who: sender, registrar });

			Ok(Some(T::WeightInfo::cancel_request(judgements as u32, extra_fields as u32)).into())
		}

		/// Provide a judgement for an account's identity.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender
		/// must be the account of the registrar whose index is `reg_index`.
		///
		/// - `reg_index`: the index of the registrar whose judgement is being
		///   made.
		/// - `target`: the account whose identity the judgement is upon. This
		///   must be an account with a registered identity.
		/// - `judgement`: the judgement of the registrar of index `reg_index`
		///   about `target`.
		/// - `identity`: The hash of the [`IdentityInfo`] for that the
		///   judgement is provided.
		///
		/// Emits `JudgementGiven` if successful.
		///
		/// ## Complexity
		/// - `O(R + X)`.
		///   - where `R` registrar-count (governance-bounded).
		///   - where `X` additional-field-count (deposit-bounded and
		///     code-bounded).
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::provide_judgement(
			T::MaxRegistrars::get(), // R
			T::MaxAdditionalFields::get(), // X
		))]
		pub fn provide_judgement(
			origin: OriginFor<T>,
			target: AccountIdLookupOf<T>,
			judgement: Judgement,
			digest: T::Hash,
		) -> DispatchResultWithPostInfo {
			let registrar = ensure_signed(origin)?;
			let target = T::Lookup::lookup(target)?;
			ensure!(!judgement.has_requested(), Error::<T>::InvalidJudgement);
			ensure!(Registrars::<T>::get().contains(&registrar), Error::<T>::RegistrarNotFound);

			let mut id = <IdentityOf<T>>::get(&target).ok_or(Error::<T>::InvalidTarget)?;

			if T::Hashing::hash_of(&id.info) != digest {
				return Err(Error::<T>::JudgementForDifferentIdentity.into())
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
			let extra_fields = id.info.additional.len();
			<IdentityOf<T>>::insert(&target, id);
			Self::deposit_event(Event::JudgementGiven { target, registrar });

			Ok(Some(T::WeightInfo::provide_judgement(judgements as u32, extra_fields as u32))
				.into())
		}

		/// Remove an account's identity
		///
		/// The dispatch origin for this call must match `T::RegistrarOrigin`.
		///
		/// - `target`: the account whose identity the judgement is upon. This
		///   must be an account with a registered identity.
		///
		/// Emits `IdentityKilled` if successful.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::kill_identity(
			T::MaxRegistrars::get(), // R
			T::MaxAdditionalFields::get(), // X
		))]
		pub fn kill_identity(
			origin: OriginFor<T>,
			target: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?;

			// Figure out who we're meant to be clearing.
			let target = T::Lookup::lookup(target)?;
			// Grab their deposit (and check that they have one).
			let id = <IdentityOf<T>>::take(&target).ok_or(Error::<T>::NotNamed)?;

			Self::deposit_event(Event::IdentityKilled { who: target });

			Ok(Some(T::WeightInfo::kill_identity(
				id.judgements.len() as u32,      // R
				id.info.additional.len() as u32, // X
			))
			.into())
		}

		/// Remove a registrar from the system.
		///
		/// The dispatch origin for this call must be `T::RegistrarOrigin`.
		///
		/// - `account`: the account of the registrar.
		///
		/// Emits `RegistrarRemoved` if successful.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::remove_registrar())]
		pub fn remove_registrar(
			origin: OriginFor<T>,
			account: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?; // Ensure the origin is authorized
			let account = T::Lookup::lookup(account)?; // Look up the AccountId

			// Try to remove the registrar
			<Registrars<T>>::try_mutate(|registrars| -> Result<(), DispatchError> {
				let position = registrars
					.iter()
					.position(|x| *x == account)
					.ok_or(Error::<T>::RegistrarNotFound)?;
				registrars.remove(position);
				Ok(())
			})?;

			// Emit an event to log the removal of the registrar
			Self::deposit_event(Event::RegistrarRemoved { registrar: account });

			// Return the weight for the transaction
			Ok(Some(T::WeightInfo::remove_registrar()).into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Check if the account has corresponding identity information by the
	/// identity field.
	pub fn has_identity(who: &T::AccountId, fields: u64) -> bool {
		IdentityOf::<T>::get(who)
			.map_or(false, |registration| (registration.info.fields().0.bits() & fields) == fields)
	}
}

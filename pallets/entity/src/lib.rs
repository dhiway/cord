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

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;
pub mod entity;
pub mod weights;

extern crate alloc;
use alloc::{boxed::Box, fmt::Debug, vec::Vec};
use codec::{Encode, EncodeLike};
use cord_primitives::{
	doket::{Attribute, DoketInformationProvider, DoketUpdateError, DoketUpdateOp, Element},
	identifier::Ss58Identifier,
};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, StorageVersion},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use pallet_identifier::{EventBlock, EventTypeOf, Identifier};
use sp_runtime::traits::Hash;
pub use weights::WeightInfo;

pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
pub type UpdateOpOf<T> = <<T as Config>::EntityInfoDoket as DoketInformationProvider>::UpdateOp;
pub type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;
pub type AttributeUpdateKeyOpOf<T> = (Vec<u8>, DataOf<T>);

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Ideentifier Type
		type Identifier: Identifier<Self, Hash = Self::Hash>;

		/// The maximum number of sub-accounts allowed per identified account.
		#[pallet::constant]
		type MaxSubAccounts: Get<u32>;

		/// Structure holding information about an entity,
		type EntityInfoDoket: DoketInformationProvider<
				FieldMask = u64,
				MaxRawDataLength = Self::MaxRawDataLength,
				MaxAdditionalAttributes = Self::MaxAdditionalAttributes,
				UpdateOp = DoketUpdateOp<Self::MaxRawDataLength>,
			> + Encode
			+ Decode
			+ MaxEncodedLen
			+ TypeInfo
			+ EncodeLike
			+ Clone
			+ PartialEq
			+ Eq
			+ Debug
			+ Default;

		/// Maximum size for raw data fields.
		#[pallet::constant]
		type MaxRawDataLength: Get<u32>;

		/// Maximum number of additional attributes allowed.
		#[pallet::constant]
		type MaxAdditionalAttributes: Get<u32>;

		/// Max length for username prefix (before the dot).
		#[pallet::constant]
		type MaxUsernameLength: Get<u32>;

		/// The origin which may forcibly set or remove a name. Root can always do this.
		type ForceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// The on‐chain entity registration for an account.
	#[pallet::storage]
	pub type EntityInfoOf<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::EntityInfoDoket, OptionQuery>;

	/// What Ss58‐ID does this account currently hold?
	#[pallet::storage]
	pub type Ss58OfActiveAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	/// Linked sub-accounts for each entity doken.
	#[pallet::storage]
	pub type SubAccounts<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		BoundedVec<T::AccountId, T::MaxSubAccounts>,
		ValueQuery,
	>;

	/// Which account currently “owns” this Ss58‐ID?
	#[pallet::storage]
	pub type ControllerOfSs58<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::AccountId, OptionQuery>;

	/// When was this account un-bound from this entity doken?
	#[pallet::storage]
	pub type Ss58OfAccountHistory<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		T::AccountId,
		EventBlock,
		OptionQuery,
	>;

	/// Username attached to an entity doken.
	#[pallet::storage]
	pub type Ss58IdNameOf<T: Config> =
		StorageMap<_, Twox64Concat, Ss58Identifier, Username<T>, OptionQuery>;

	/// Reverse lookup: username → (owner, provider).
	#[pallet::storage]
	pub type NameSs58IdOf<T: Config> =
		StorageMap<_, Twox64Concat, Username<T>, Ss58Identifier, OptionQuery>;

	/// Version counter for each (doken, attribute key).
	#[pallet::storage]
	pub type Ss58OfAttributeVersion<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		Attribute,
		u64,
		ValueQuery,
	>;

	/// All history entries: (doken, (key, version)) → (old_value,  block).
	#[pallet::storage]
	pub type Ss58OfAttributeHistory<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		(Attribute, u64),
		(DataOf<T>, EventBlock),
		OptionQuery,
	>;
	#[pallet::error]
	pub enum Error<T> {
		///Bad Origin
		BadOrigin,
		/// Too many subs-accounts.
		TooManySubAccounts,
		/// Account isn't found.
		AccountNotFound,
		/// Account is a controller.
		AlreadyController,
		/// Account is a controller.
		ControllerAccount,
		/// Account isn't named.
		NotNamed,
		/// Empty index.
		EmptyIndex,
		/// Fee is changed.
		FeeChanged,
		/// No identity found.
		NoIdentity,
		/// Doken Exists.
		DokenAlreadyExists,
		/// Doken not found.
		DokenNotFound,
		/// Account mapped to an entity.
		EntitySubAccount,
		/// The index is invalid.
		InvalidIndex,
		/// The target is invalid.
		InvalidTarget,
		// /// The entity length is not valid.
		// InvalidIdentifierLength,
		/// The docken inputs are not valid.
		DokenCreationFailed,
		/// The provided event type is invalid.
		InvalidEventType,
		/// Maximum amount of registrars reached. Cannot add any more.
		TooManyRegistrars,
		/// Account ID is already named.
		AlreadyClaimed,
		/// Sender is not a sub-account.
		NotSub,
		/// Sub-account isn't owned by sender.
		NotOwned,
		/// Not enough free balance to pay the per-byte identity fee.
		InsufficientFunds,
		/// Setting this username requires a signature, but none was provided.
		RequiresSignature,
		/// Sub account is already mapped to an entity.
		SubAccountAlreadyClaimed,
		/// Sub account not found.
		SubAccountNotFound,
		/// Sub account is not linked to the entity
		SubAccountNotLinked,
		/// Sub account not found.
		SubAccountExists,
		/// The username does not meet the requirements.
		InvalidSs58IdName,
		/// The username does not meet the requirements.
		InvalidAttributeEntry,
		/// The username does not meet the requirements.
		DuplicateAttributeKey,
		/// The username is already taken.
		Ss58IdNameTaken,
		/// The requested username does not exist.
		NoUsername,
		/// The action cannot be performed because of insufficient privileges (e.g. authority
		/// trying to unbind a username provided by the system).
		InsufficientPrivileges,
		/// Tried to add an attribute that already exists.
		AttributeExists,
		/// Exceeded the maximum number of additional attribute/value pairs.
		TooManyAttributes,
		/// Tried to update or remove an attribute that doesn't exist.
		AttributeNotFound,
		// State Update Failed
		StateUpdateFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A name was set or reset (which will remove all judgements).
		EntityInfoSet { who: T::AccountId, doken: Ss58Identifier },
		/// An entity info was updated.
		EntityInfoUpdated { who: T::AccountId, doken: Ss58Identifier },
		/// An entity attribute was updated.
		EntityAttributeUpdated { who: T::AccountId, doken: Ss58Identifier },
		/// An entity attribute was removed.
		EntityAttributeRemoved { who: T::AccountId, doken: Ss58Identifier, attr: Attribute },
		/// An entity attribute was rotated.
		EntityAttributeRotated { who: T::AccountId, doken: Ss58Identifier, attr: Attribute },
		/// A sub-account was added to an entity.
		EntitySubAccountAdded { sub: T::AccountId, doken: Ss58Identifier },
		/// A sub-identity was revoked
		EntitySubAccountRevoked { sub: T::AccountId, doken: Ss58Identifier },
		/// A sub-identity was revoked by root or council
		EntitySubAccountRevokedFor { sub: T::AccountId, doken: Ss58Identifier },
		/// A controller was rotated.
		EntityControllerRotated { doken: Ss58Identifier, new: T::AccountId },
		/// A controller was rotated by root or council.
		EntityControllerRotatedFor { doken: Ss58Identifier, new: T::AccountId },
		/// A name was cleared.
		EntityInfoCleared { doken: Ss58Identifier },
		/// A name was cleared by root or council.
		EntityInfoClearedFor { doken: Ss58Identifier },
		/// A username was set for `who`.
		Ss58IdNameAdded { doken: Ss58Identifier, name: Username<T> },
		/// A username has been removed.
		Ss58IdNameRemoved { doken: Ss58Identifier },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set an entity's information and generate an entity doken.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_info(info.encoded_size() as u32))]
		pub fn set_info(origin: OriginFor<T>, info: Box<T::EntityInfoDoket>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!Ss58OfActiveAccounts::<T>::contains_key(&who), Error::<T>::EntitySubAccount);

			let info = *info;
			if let Some(attributes) = info.attributes() {
				ensure!(
					!attributes.iter().any(|(key, _)| key.is_empty()),
					Error::<T>::InvalidAttributeEntry
				);
				let mut seen = Vec::with_capacity(attributes.len());
				for (key, _) in attributes.iter() {
					let raw_key = key.as_ref();
					ensure!(
						!seen.iter().any(|existing: &&[u8]| *existing == raw_key),
						Error::<T>::DuplicateAttributeKey
					);
					seen.push(raw_key);
				}
			}

			let digest = T::Hashing::hash(&(&info, b"IdentityInfoSet" as &[u8]).encode());
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let doken = T::Identifier::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::DokenCreationFailed)?;

			EntityInfoOf::<T>::try_mutate_exists(&doken, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::DokenAlreadyExists);
				*opt = Some(info);
				Ok(())
			})?;

			Ss58OfActiveAccounts::<T>::insert(&who, doken.clone());
			ControllerOfSs58::<T>::insert(&doken, who.clone());

			Self::record_activity(&doken, digest, b"EntityInfoSet")?;
			Self::deposit_event(Event::EntityInfoSet { who, doken });

			Ok(())
		}

		/// Update attributes in an existing entity.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_info(ops.encoded_size() as u32))]
		pub fn update_info(
			origin: OriginFor<T>,
			ops: Vec<AttributeUpdateKeyOpOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			let controller = Self::lookup_controller_of(&doken)?;
			ensure!(who == controller, Error::<T>::BadOrigin);

			EntityInfoOf::<T>::try_mutate(&doken, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::DokenNotFound)?;
				for (raw_key, val) in ops.iter() {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					let op = DoketUpdateOp::UpdateAttribute(key.clone(), val.clone());
					info.apply_update(&op).map_err(|e| match e {
						DoketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
						_ => Error::<T>::InvalidAttributeEntry,
					})?;
				}
				Ok(())
			})?;

			let digest = T::Hashing::hash(&(&doken, &ops, b"EntityInfoUpdated" as &[u8]).encode());
			Self::record_activity(&doken, digest, b"EntityInfoUpdated")?;
			Self::deposit_event(Event::EntityInfoUpdated { who, doken });

			Ok(())
		}

		/// Add entity attributes key->Data.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::add_attributes( ops.encoded_size()  as u32))]
		pub fn add_attributes(
			origin: OriginFor<T>,
			ops: Vec<AttributeUpdateKeyOpOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			EntityInfoOf::<T>::try_mutate(&doken, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::DokenNotFound)?;
				for (raw_key, val) in ops.iter() {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					info.apply_update(&DoketUpdateOp::AddAttribute(key.clone(), val.clone()))
						.map_err(|e| match e {
							DoketUpdateError::AttributeExists => Error::<T>::AttributeExists,
							DoketUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes,
							_ => Error::<T>::InvalidAttributeEntry,
						})?;
				}
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&doken, &ops, b"EntityAttributeUpdated" as &[u8]).encode());

			Self::record_activity(&doken, digest, b"EntityAttributeUpdated")?;

			Self::deposit_event(Event::EntityAttributeUpdated { who, doken });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::remove_attribute( key.len()  as u32))]
		pub fn remove_attribute(origin: OriginFor<T>, key: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
			EntityInfoOf::<T>::try_mutate(&doken, |opt| -> DispatchResult {
				let info = opt.as_mut().ok_or(Error::<T>::DokenNotFound)?;
				info.apply_update(&DoketUpdateOp::RemoveAttribute(attr.clone()))
					.map_err(|_| Error::<T>::AttributeNotFound)?;
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&doken, &key, b"EntityAttributeRemoved" as &[u8]).encode());

			Self::record_activity(&doken, digest, b"EntityAttributeRemoved")?;

			Self::deposit_event(Event::EntityAttributeRemoved { who, doken, attr });
			Ok(())
		}

		/// “Rotate” (update) an existing attribute: record the old value in history, bump the version,
		/// then overwrite. Fails if the key is missing or invalid.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::rotate_attribute( key.len() as u32 + val.as_ref().len() as u32))]
		pub fn rotate_attribute(
			origin: OriginFor<T>,
			key: Vec<u8>,
			val: DataOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;

			EntityInfoOf::<T>::try_mutate(&doken, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::DokenNotFound)?;
				let old_val = info.get_key(&key);

				info.apply_update(&DoketUpdateOp::UpdateAttribute(attr.clone(), val.clone()))
					.map_err(|_| Error::<T>::AttributeNotFound)?;

				let ver = Ss58OfAttributeVersion::<T>::get(&doken, &attr).saturating_add(1);
				Ss58OfAttributeVersion::<T>::insert(&doken, &attr, ver);
				Ss58OfAttributeHistory::<T>::insert(
					&doken,
					(attr.clone(), ver),
					(old_val.clone(), EventBlock::current::<T>()),
				);

				Ok(())
			})?;

			let digest = T::Hashing::hash(
				&(&doken, &key, &val, b"EntityAttributeRotated" as &[u8]).encode(),
			);

			Self::record_activity(&doken, digest, b"EntityAttributeRotated")?;
			Self::deposit_event(Event::EntityAttributeRotated { who, doken, attr });

			Ok(())
		}

		/// Set a sub-account of the sender.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_sub_account(sub.encoded_size() as u32))]
		pub fn set_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			ensure!(
				!Ss58OfActiveAccounts::<T>::contains_key(&sub),
				Error::<T>::SubAccountAlreadyClaimed
			);

			SubAccounts::<T>::try_mutate(&doken, |list| {
				ensure!(
					list.len() < T::MaxSubAccounts::get() as usize,
					Error::<T>::TooManySubAccounts
				);
				list.try_push(sub.clone()).map_err(|_| Error::<T>::TooManySubAccounts)
			})?;

			Ss58OfActiveAccounts::<T>::insert(&sub, doken.clone());

			let digest =
				T::Hashing::hash(&(&doken, &sub, b"EntitySubAccountAdded" as &[u8]).encode());
			Self::record_activity(&doken, digest, b"EntitySubAccountAdded")?;

			Self::deposit_event(Event::EntitySubAccountAdded { sub, doken });

			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account(sub.encoded_size() as u32))]
		pub fn revoke_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);
			ensure!(sub != who, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&doken, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, doken });
			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account_for(sub.encoded_size() as u32))]
		pub fn revoke_sub_account_for(
			origin: OriginFor<T>,
			doken: Ss58Identifier,
			sub: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&doken), Error::<T>::DokenNotFound);
			ensure!(sub != Self::lookup_controller_of(&doken)?, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&doken, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, doken });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::rotate_controller(new_controller.encoded_size() as u32)
            .saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller(
			origin: OriginFor<T>,
			doken: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);
			ensure!(new_controller != who, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&who);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, doken.clone());
			ControllerOfSs58::<T>::insert(&doken, new_controller.clone());
			Ss58OfAccountHistory::<T>::insert(&doken, &who, EventBlock::current::<T>());

			let digest = T::Hashing::hash(
				&(&doken, &new_controller, b"EntityControllerRotated" as &[u8]).encode(),
			);
			Self::record_activity(&doken, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { doken, new: new_controller });

			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::rotate_controller_for(new_controller.encoded_size() as u32)
		.saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller_for(
			origin: OriginFor<T>,
			doken: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&doken), Error::<T>::DokenNotFound);
			let current_controller = Self::lookup_controller_of(&doken)?;
			ensure!(current_controller != new_controller, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&current_controller);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, doken.clone());
			ControllerOfSs58::<T>::insert(&doken, new_controller.clone());

			Ss58OfAccountHistory::<T>::insert(
				&doken,
				&current_controller,
				EventBlock::current::<T>(),
			);

			let digest = T::Hashing::hash(
				&(&doken, &new_controller, b"EntityControllerRotated" as &[u8]).encode(),
			);
			Self::record_activity(&doken, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { doken, new: new_controller });

			Ok(())
		}

		/// Remove all entity details from storage
		#[pallet::call_index(10)]
		// #[pallet::weight(T::WeightInfo::clear_identity())]
		#[pallet::weight({
		    let sub_count = SubAccounts::<T>::get(&doken).len() as u32;
		    T::WeightInfo::clear_everything(sub_count)
		})]
		pub fn clear_everything(origin: OriginFor<T>, doken: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);
			ensure!(EntityInfoOf::<T>::contains_key(&doken), Error::<T>::DokenNotFound);
			Self::do_clear_everything(&doken)?;
			Self::deposit_event(Event::EntityInfoCleared { doken });
			Ok(())
		}

		/// Council/Root Remove all details of an entity from storage
		#[pallet::call_index(11)]
		// #[pallet::weight(T::WeightInfo::clear_identity_for())]
		#[pallet::weight({
		    let sub_count = SubAccounts::<T>::get(&doken).len() as u32;
		    T::WeightInfo::clear_everything_for(sub_count)
		})]
		pub fn clear_everything_for(origin: OriginFor<T>, doken: Ss58Identifier) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&doken), Error::<T>::DokenNotFound);
			Self::do_clear_everything(&doken)?;
			Self::deposit_event(Event::EntityInfoCleared { doken });
			Ok(())
		}

		/// Add an entity doken name under the constant suffix ".myn.social", always stored lowercase.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::set_id_name(prefix.len() as u32))]
		pub fn set_id_name(origin: OriginFor<T>, mut prefix: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let doken = Self::lookup_doken_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			for b in &mut prefix {
				*b = b.to_ascii_lowercase();
			}
			ensure!(Self::is_valid_user_name_prefix(&prefix), Error::<T>::InvalidSs58IdName);
			prefix.extend(b".myn.social");

			let bounded_uname: Username<T> =
				prefix.try_into().map_err(|_| Error::<T>::InvalidSs58IdName)?;

			NameSs58IdOf::<T>::try_mutate_exists(&bounded_uname, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::Ss58IdNameTaken);
				*opt = Some(doken.clone());
				Ok(())
			})?;

			Ss58IdNameOf::<T>::insert(&doken, &bounded_uname);
			let digest =
				T::Hashing::hash(&(&doken, &bounded_uname, b"Ss58IdNameAdded" as &[u8]).encode());

			Self::record_activity(&doken, digest, b"Ss58IdNameAdded")?;
			Self::deposit_event(Event::Ss58IdNameAdded { doken, name: bounded_uname });

			Ok(())
		}

		/// Remove an existing username under the suffix "myn.social".
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::remove_id_name())]
		pub fn remove_id_name(origin: OriginFor<T>, doken: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&doken)?, Error::<T>::BadOrigin);

			let uname = Ss58IdNameOf::<T>::take(&doken).ok_or(Error::<T>::NoUsername)?;
			NameSs58IdOf::<T>::remove(&uname);

			let digest =
				T::Hashing::hash(&(&doken, &uname, b"Ss58IdNameRemoved" as &[u8]).encode());

			Self::record_activity(&doken, digest, b"Ss58IdNameRemoved")?;
			Self::deposit_event(Event::Ss58IdNameRemoved { doken });
			Ok(())
		}
	}

	#[pallet::view_functions_experimental]
	impl<T: Config> Pallet<T> {
		/// Get every attribute change for `doken` as
		/// `(key_bytes, version, old_value_bytes, block_number)`.
		pub fn get_attribute_history(
			doken: Ss58Identifier,
		) -> Vec<(Vec<u8>, u64, Vec<u8>, EventBlock)> {
			Ss58OfAttributeHistory::<T>::iter_prefix(&doken)
				.map(|((key, version), (old, block))| {
					(key.to_vec(), version, old.as_ref().to_vec(), block)
				})
				.collect()
		}

		/// Get the change history of a single `key` for `doken` as
		/// `(version, old_value_bytes, block_number)`.
		pub fn get_attribute_history_for_key(
			doken: Ss58Identifier,
			key: Vec<u8>,
		) -> Vec<(u64, Vec<u8>, EventBlock)> {
			let key_bounded: Attribute =
				key.try_into().expect("caller should provide valid-length key");
			Ss58OfAttributeHistory::<T>::iter_prefix(&doken)
				.filter_map(|((k, version), (old, block))| {
					if k == key_bounded {
						Some((version, old.as_ref().to_vec(), block))
					} else {
						None
					}
				})
				.collect()
		}

		/// Fetch a single history entry by `doken`, `key`, and `version`, returning
		/// `(old_value_bytes, block_number)` if it exists.
		pub fn get_attribute_history_entry(
			doken: Ss58Identifier,
			key: Vec<u8>,
			version: u64,
		) -> Option<(Vec<u8>, EventBlock)> {
			let key_bounded: Attribute = key.try_into().ok()?;
			Ss58OfAttributeHistory::<T>::get(&doken, (key_bounded, version))
				.map(|(old, block)| (old.as_ref().to_vec(), block))
		}
	}
}

impl<T: Config> Pallet<T> {
	// Revoke sub-account helper
	fn do_revoke_sub_account(doken: &Ss58Identifier, sub: &T::AccountId) -> DispatchResult {
		let bound = Ss58OfActiveAccounts::<T>::get(sub).ok_or(Error::<T>::SubAccountNotFound)?;
		ensure!(bound == *doken, Error::<T>::SubAccountNotLinked);
		Ss58OfActiveAccounts::<T>::remove(sub);
		let now = EventBlock::current::<T>();
		Ss58OfAccountHistory::<T>::insert(doken, sub, now);
		SubAccounts::<T>::try_mutate(doken, |list| {
			if let Some(pos) = list.iter().position(|x| x == sub) {
				list.swap_remove(pos);
				Ok(())
			} else {
				Err(Error::<T>::SubAccountNotLinked)
			}
		})?;
		let digest = T::Hashing::hash(&(doken, sub, b"EntitySubAccountRevoked" as &[u8]).encode());
		Self::record_activity(doken, digest, b"EntitySubAccountRevoked")?;
		Ok(())
	}

	// Clear Identitiy Helper
	fn do_clear_everything(doken: &Ss58Identifier) -> DispatchResult {
		let now = EventBlock::current::<T>();
		for sub in SubAccounts::<T>::take(doken).into_iter() {
			Ss58OfActiveAccounts::<T>::remove(&sub);
			Ss58OfAccountHistory::<T>::insert(doken, &sub, now.clone());
		}
		if let Some(ctrl) = ControllerOfSs58::<T>::take(doken) {
			Ss58OfActiveAccounts::<T>::remove(&ctrl);
			Ss58OfAccountHistory::<T>::insert(doken, &ctrl, now.clone());
		}
		EntityInfoOf::<T>::remove(doken);
		if let Some(uname) = Ss58IdNameOf::<T>::take(doken) {
			NameSs58IdOf::<T>::remove(&uname);
		}
		let digest = T::Hashing::hash(&(doken, b"EntityInfoCleared" as &[u8]).encode());
		Self::record_activity(doken, digest, b"EntityInfoCleared")?;
		Ok(())
	}

	/// Get the current Ss58 ID of `who`, or an error if none.
	pub fn lookup_doken_of(who: &T::AccountId) -> Result<Ss58Identifier, Error<T>> {
		Ss58OfActiveAccounts::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	/// Get the controller account of `doken`, or error if none.
	pub fn lookup_controller_of(doken: &Ss58Identifier) -> Result<T::AccountId, Error<T>> {
		ControllerOfSs58::<T>::get(doken).ok_or(Error::<T>::DokenNotFound)
	}

	/// Get the full history for `doken` as `(AccountId, BlockNumber)` pairs.
	pub fn lookup_history(doken: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Ss58OfAccountHistory::<T>::iter_prefix(doken).collect()
	}

	/// Check if `who` has _all_ of the requested `fields` in their on-chain identity.
	pub fn has_info_fields(
		who: &T::AccountId,
		mask: <T::EntityInfoDoket as DoketInformationProvider>::FieldMask,
	) -> bool {
		Ss58OfActiveAccounts::<T>::get(who)
			.and_then(|doken| EntityInfoOf::<T>::get(&doken))
			.map_or(false, |info| info.has_info_fields(mask))
	}

	/// Validates a username prefix
	fn is_valid_user_name_prefix(input: &[u8]) -> bool {
		// reject empty, too long, leading/trailing period, or consecutive periods
		let max_len = T::MaxUsernameLength::get() as usize;
		if input.is_empty() || input.len() > max_len {
			return false;
		}
		let (_, ok) = input.iter().copied().fold((false, true), |(prev_dot, ok), b| {
			let is_dot = b == b'.';
			let ok_char = matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.');
			let no_consec = ok && !(prev_dot && is_dot);
			(is_dot, ok_char && no_consec)
		});
		ok && input.first() != Some(&b'.') && input.last() != Some(&b'.')
	}

	/// Records an activity using a provided event message.
	pub fn record_activity(
		identifier: &Ss58Identifier,
		digest: T::Hash,
		msg: &[u8],
	) -> DispatchResult {
		let action: EventTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
		let stamp = EventBlock::current::<T>();
		T::Identifier::state_event(identifier, digest, action, stamp)
			.map_err(|_| Error::<T>::StateUpdateFailed)?;
		Ok(())
	}
}

/// A thin API for entity lookups, so that *any* pallet can call:
pub trait EntityLookup<T: frame_system::Config> {
	/// The error returned by the fallible lookups.
	type Error;
	/// The username type (e.g. `Username<T>`).
	type Username;

	/// Get the current Ss58 ID of `who`, or Err if none.
	fn lookup_doken_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error>;

	/// Get the controller account of `doken`, or Err if none.
	fn lookup_controller_of(doken: &Ss58Identifier) -> Result<T::AccountId, Self::Error>;

	/// Get the full history for `doken` as `(AccountId, EventBlock)`.
	fn lookup_history(doken: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)>;

	/// Fetch the (optional) username attached to an entity doken.
	fn lookup_name_of_identifier(doken: &Ss58Identifier) -> Option<Self::Username>;

	/// Reverse lookup: given a username, get the attached entity doken (if any).
	fn lookup_identifier_of_name(name: &Self::Username) -> Option<Ss58Identifier>;
}

impl<T: Config> EntityLookup<T> for Pallet<T> {
	type Error = Error<T>;
	type Username = Username<T>;

	fn lookup_doken_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error> {
		Ss58OfActiveAccounts::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	fn lookup_controller_of(doken: &Ss58Identifier) -> Result<T::AccountId, Self::Error> {
		ControllerOfSs58::<T>::get(doken).ok_or(Error::<T>::DokenNotFound)
	}

	fn lookup_history(doken: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Ss58OfAccountHistory::<T>::iter_prefix(doken).collect()
	}

	fn lookup_name_of_identifier(doken: &Ss58Identifier) -> Option<Username<T>> {
		Ss58IdNameOf::<T>::get(doken)
	}

	fn lookup_identifier_of_name(name: &Username<T>) -> Option<Ss58Identifier> {
		NameSs58IdOf::<T>::get(name)
	}
}

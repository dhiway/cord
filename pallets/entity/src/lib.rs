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

// mod benchmarking;
pub mod entity;
pub mod types;
pub mod weights;

extern crate alloc;
use crate::types::Attribute;
use alloc::{boxed::Box, vec::Vec};
use codec::{Encode, EncodeLike};
use cord_primitives::identifier::Ss58Identifier;
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
pub use types::{Data, EntityInformationProvider, EntityUpdateError, EntityUpdateOp};
pub use weights::WeightInfo;

pub type DataOf<T> = Data<<T as Config>::MaxRawDataLength>;
pub type UpdateOpOf<T> = <<T as Config>::EntityInformation as EntityInformationProvider>::UpdateOp;
pub type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_identifier::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of sub-accounts allowed per identified account.
		#[pallet::constant]
		type MaxSubAccounts: Get<u32>;

		/// Structure holding information about an entity,
		/// containing up to `Self::MaxAdditionalFields` fields of at most `Self::MaxDataLength` bytes.
		/// Must have `FieldsIdentifier = u64` and use `EntityUpdateOp<Self::MaxDataLength>` for updates.
		type EntityInformation: EntityInformationProvider<
				FieldsIdentifier = u64,
				MaxRawDataLength = Self::MaxRawDataLength,
				UpdateOp = EntityUpdateOp<Self::MaxRawDataLength>,
			> + EncodeLike;

		// /// Maximum number of additional fields that may be stored in an ID.
		// #[pallet::constant]
		// type MaxAdditionalFields: Get<u32> + TypeInfo;

		/// Maximum size for raw data fields.
		#[pallet::constant]
		type MaxRawDataLength: Get<u32>;

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
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::EntityInformation, OptionQuery>;

	/// What Ss58‐ID does this account currently hold?
	#[pallet::storage]
	pub type Ss58OfActiveAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	/// Linked sub-accounts for each identifier.
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

	/// When was this account un-bound from this identifier?
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

	/// Username attached to an identifier.
	#[pallet::storage]
	pub type Ss58IdNameOf<T: Config> =
		StorageMap<_, Twox64Concat, Ss58Identifier, Username<T>, OptionQuery>;

	/// Reverse lookup: username → (owner, provider).
	#[pallet::storage]
	pub type NameSs58IdOf<T: Config> =
		StorageMap<_, Twox64Concat, Username<T>, Ss58Identifier, OptionQuery>;

	/// Version counter for each (identifier, attribute key).
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

	/// All history entries: (id, (key, version)) → (old_value,  block).
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
		/// Identifier Exists.
		IdentifierAlreadyExists,
		/// Identifier not found.
		IdentifierNotFound,
		/// Account mapped to an Identifier.
		IdentifierSubAccount,
		/// The index is invalid.
		InvalidIndex,
		/// The target is invalid.
		InvalidTarget,
		/// The identifier length is not valid.
		InvalidIdentifierLength,
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
		/// Sub account is already mapped to an identifier.
		SubAccountAlreadyClaimed,
		/// Sub account not found.
		SubAccountNotFound,
		/// Sub account is not linked to the identifier
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
		EntityInfoSet { who: T::AccountId, id: Ss58Identifier },
		/// An entity info was updated.
		EntityInfoUpdated { who: T::AccountId, id: Ss58Identifier },
		/// An Attribute was added to the identifier.
		EntityAttributeUpdated { who: T::AccountId, id: Ss58Identifier, attribute: Attribute },
		/// A sub-account was added to an identifier.
		EntitySubAccountAdded { sub: T::AccountId, id: Ss58Identifier },
		/// A sub-identity was revoked
		EntitySubAccountRevoked { sub: T::AccountId, id: Ss58Identifier },
		/// A sub-identity was revoked by root or council
		EntitySubAccountRevokedFor { sub: T::AccountId, id: Ss58Identifier },
		/// A controller was rotated.
		EntityControllerRotated { id: Ss58Identifier, new: T::AccountId },
		/// A controller was rotated by root or council.
		EntityControllerRotatedFor { id: Ss58Identifier, new: T::AccountId },
		/// A name was cleared.
		EntityInfoCleared { id: Ss58Identifier },
		/// A name was cleared by root or council.
		EntityInfoClearedFor { id: Ss58Identifier },
		/// A username was set for `who`.
		Ss58IdNameAdded { id: Ss58Identifier, name: Username<T> },
		/// A username has been removed.
		Ss58IdNameRemoved { id: Ss58Identifier },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set an entity's information and generate identifier.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_info(info.encoded_size() as u32))]
		pub fn set_info(origin: OriginFor<T>, info: Box<T::EntityInformation>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				!Ss58OfActiveAccounts::<T>::contains_key(&who),
				Error::<T>::IdentifierSubAccount
			);

			let info: T::EntityInformation = *info;
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

			let digest = T::Hashing::hash(&(info.clone(), b"IdentityInfoSet".to_vec()).encode());
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let id = <pallet_identifier::Pallet<T> as Identifier<T>>::build(
				&digest.encode()[..],
				pallet_name,
			)?;

			EntityInfoOf::<T>::try_mutate_exists(&id, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::IdentifierAlreadyExists);
				*opt = Some(info.clone());
				Ok(())
			})?;

			Ss58OfActiveAccounts::<T>::insert(&who, id.clone());
			ControllerOfSs58::<T>::insert(&id, who.clone());

			Self::record_activity(&id, digest, b"EntityInfoSet")?;
			Self::deposit_event(Event::EntityInfoSet { who, id });

			Ok(())
		}

		/// Update *any* combination of fields in an existing entity.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_info(ops.encoded_size() as u32))]
		pub fn update_info(origin: OriginFor<T>, ops: Vec<UpdateOpOf<T>>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let id = Self::lookup_id_of(&who)?;
			let controller = Self::lookup_controller_of(&id)?;
			ensure!(who == controller, Error::<T>::BadOrigin);

			EntityInfoOf::<T>::try_mutate(&id, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::IdentifierNotFound)?;
				let mut history: Vec<(Attribute, DataOf<T>)> = Vec::with_capacity(ops.len());

				for op in ops.iter() {
					if let EntityUpdateOp::SetKey(ref key, _) = op {
						let old = info.get_key(&key[..]);
						history.push((key.clone(), old));
					}
					info.apply_update(op).map_err(|e| match e {
						EntityUpdateError::AttributeExists => Error::<T>::AttributeExists,
						EntityUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes,
						EntityUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
					})?;
				}

				for (key, old) in history {
					let ver = Ss58OfAttributeVersion::<T>::get(&id, &key).saturating_add(1);
					Ss58OfAttributeVersion::<T>::insert(&id, &key, ver);
					Ss58OfAttributeHistory::<T>::insert(
						&id,
						(key.clone(), ver),
						(old.clone(), EventBlock::current::<T>()),
					);
				}

				let digest = T::Hashing::hash(
					&(id.clone(), ops.clone(), b"EntityInfoUpdated".to_vec()).encode(),
				);
				Self::record_activity(&id, digest, b"EntityInfoUpdated")?;
				Self::deposit_event(Event::EntityInfoUpdated { who: who.clone(), id: id.clone() });

				Ok(())
			})
		}

		/// Add a single arbitrary attribute key->Data.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::add_attribute( key.len() as u32 + val.as_ref().len() as u32))]
		pub fn add_attribute(origin: OriginFor<T>, key: Vec<u8>, val: DataOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let id = Self::lookup_id_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
			EntityInfoOf::<T>::try_mutate(&id, |opt| -> DispatchResult {
				let info = opt.as_mut().ok_or(Error::<T>::IdentifierNotFound)?;
				let op = EntityUpdateOp::SetKey(attr.clone(), val.clone());
				info.apply_update(&op).map_err(|e| match e {
					EntityUpdateError::AttributeExists => Error::<T>::AttributeExists,
					EntityUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes,
					EntityUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
				})?;
				Ok(())
			})?;
			let digest = T::Hashing::hash(
				&(id.clone(), attr.clone(), val.clone(), b"EntityAttributeUpdated".to_vec())
					.encode(),
			);

			Self::record_activity(&id, digest, b"EntityAttributeUpdated")?;

			Self::deposit_event(Event::EntityAttributeUpdated { who, id, attribute: attr });
			Ok(())
		}

		/// Set a sub-account of the sender.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_sub_account(sub.encoded_size() as u32))]
		pub fn set_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let id = Self::lookup_id_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);

			ensure!(
				!Ss58OfActiveAccounts::<T>::contains_key(&sub),
				Error::<T>::SubAccountAlreadyClaimed
			);

			SubAccounts::<T>::try_mutate(&id, |list| {
				ensure!(
					list.len() < T::MaxSubAccounts::get() as usize,
					Error::<T>::TooManySubAccounts
				);
				list.try_push(sub.clone()).map_err(|_| Error::<T>::TooManySubAccounts)
			})?;

			Ss58OfActiveAccounts::<T>::insert(&sub, id.clone());
			let digest = T::Hashing::hash(
				&(id.clone(), sub.clone(), b"EntitySubAccountAdded".to_vec()).encode(),
			);
			Self::record_activity(&id, digest, b"EntitySubAccountAdded")?;

			Self::deposit_event(Event::EntitySubAccountAdded { sub, id });

			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account(sub.encoded_size() as u32))]
		pub fn revoke_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let id = Self::lookup_id_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);
			ensure!(sub != who, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&id, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, id });
			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account_for(sub.encoded_size() as u32))]
		pub fn revoke_sub_account_for(
			origin: OriginFor<T>,
			id: Ss58Identifier,
			sub: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&id), Error::<T>::IdentifierNotFound);
			ensure!(sub != Self::lookup_controller_of(&id)?, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&id, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, id });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::rotate_controller(new_controller.encoded_size() as u32)
            .saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller(
			origin: OriginFor<T>,
			id: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);
			ensure!(new_controller != who, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&who);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, id.clone());
			ControllerOfSs58::<T>::insert(&id, new_controller.clone());
			Ss58OfAccountHistory::<T>::insert(&id, &who, EventBlock::current::<T>());

			let digest = T::Hashing::hash(
				&(id.clone(), new_controller.clone(), b"EntityControllerRotated".to_vec()).encode(),
			);
			Self::record_activity(&id, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { id, new: new_controller });

			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::rotate_controller_for(new_controller.encoded_size() as u32)
		.saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller_for(
			origin: OriginFor<T>,
			id: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&id), Error::<T>::IdentifierNotFound);
			let current_controller = Self::lookup_controller_of(&id)?;
			ensure!(current_controller != new_controller, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&current_controller);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, id.clone());
			ControllerOfSs58::<T>::insert(&id, new_controller.clone());

			Ss58OfAccountHistory::<T>::insert(&id, &current_controller, EventBlock::current::<T>());

			let digest = T::Hashing::hash(
				&(id.clone(), new_controller.clone(), b"EntityControllerRotated".to_vec()).encode(),
			);
			Self::record_activity(&id, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { id, new: new_controller });

			Ok(())
		}

		/// Remove all entity details from storage
		#[pallet::call_index(8)]
		// #[pallet::weight(T::WeightInfo::clear_identity())]
		#[pallet::weight({
    let sub_count = SubAccounts::<T>::get(&id).len() as u32;
    T::WeightInfo::clear_everything(sub_count)
})]
		pub fn clear_everything(origin: OriginFor<T>, id: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);
			ensure!(EntityInfoOf::<T>::contains_key(&id), Error::<T>::IdentifierNotFound);
			Self::do_clear_everything(&id)?;
			Self::deposit_event(Event::EntityInfoCleared { id });
			Ok(())
		}

		/// Council/Root Remove all details of an entity from storage
		#[pallet::call_index(9)]
		// #[pallet::weight(T::WeightInfo::clear_identity_for())]
		#[pallet::weight({
    let sub_count = SubAccounts::<T>::get(&id).len() as u32;
    T::WeightInfo::clear_everything_for(sub_count)
})]
		pub fn clear_everything_for(origin: OriginFor<T>, id: Ss58Identifier) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&id), Error::<T>::IdentifierNotFound);
			Self::do_clear_everything(&id)?;
			Self::deposit_event(Event::EntityInfoCleared { id });
			Ok(())
		}

		/// Add an identifier name under the constant suffix ".myn.social", always stored lowercase.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::set_id_name(prefix.len() as u32))]
		pub fn set_id_name(origin: OriginFor<T>, prefix: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let id = Self::lookup_id_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);

			let lower: Vec<u8> = prefix.iter().map(|b| b.to_ascii_lowercase()).collect();
			ensure!(Self::is_valid_user_name_prefix(&lower), Error::<T>::InvalidSs58IdName);

			let mut uname = lower.clone();
			uname.extend(b".myn.social");
			let bounded_uname =
				Username::<T>::try_from(uname).map_err(|_| Error::<T>::InvalidSs58IdName)?;

			NameSs58IdOf::<T>::try_mutate_exists(&bounded_uname, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::Ss58IdNameTaken);
				*opt = Some(id.clone());
				Ok(())
			})?;

			Ss58IdNameOf::<T>::insert(&id, &bounded_uname);
			let digest = T::Hashing::hash(
				&(id.clone(), bounded_uname.clone(), b"Ss58IdNameAdded".to_vec()).encode(),
			);

			Self::record_activity(&id, digest, b"Ss58IdNameAdded")?;
			Self::deposit_event(Event::Ss58IdNameAdded { id: id.clone(), name: bounded_uname });

			Ok(())
		}

		/// Remove an existing username under the suffix "myn.social".
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::remove_id_name())]
		pub fn remove_id_name(origin: OriginFor<T>, id: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&id)?, Error::<T>::BadOrigin);

			let uname = Ss58IdNameOf::<T>::take(&id).ok_or(Error::<T>::NoUsername)?;
			NameSs58IdOf::<T>::remove(&uname);

			let digest = T::Hashing::hash(
				&(id.clone(), uname.clone(), b"Ss58IdNameRemoved".to_vec()).encode(),
			);

			Pallet::<T>::record_activity(&id, digest, b"Ss58IdNameRemoved")
				.map_err(|_| Error::<T>::StateUpdateFailed)?;

			Self::deposit_event(Event::Ss58IdNameRemoved { id: id.clone() });
			Ok(())
		}
	}

	#[pallet::view_functions_experimental]
	impl<T: Config> Pallet<T> {
		/// Get every attribute change for `id` as
		/// `(key_bytes, version, old_value_bytes, block_number)`.
		pub fn get_attribute_history(
			id: Ss58Identifier,
		) -> Vec<(Vec<u8>, u64, Vec<u8>, EventBlock)> {
			Ss58OfAttributeHistory::<T>::iter_prefix(&id)
				.map(|((key, version), (old, block))| {
					(key.to_vec(), version, old.as_ref().to_vec(), block)
				})
				.collect()
		}

		/// Get the change history of a single `key` for `id` as
		/// `(version, old_value_bytes, block_number)`.
		pub fn get_attribute_history_for_key(
			id: Ss58Identifier,
			key: Vec<u8>,
		) -> Vec<(u64, Vec<u8>, EventBlock)> {
			let key_bounded: Attribute =
				key.try_into().expect("caller should provide valid-length key");
			Ss58OfAttributeHistory::<T>::iter_prefix(&id)
				.filter_map(|((k, version), (old, block))| {
					if k == key_bounded {
						Some((version, old.as_ref().to_vec(), block))
					} else {
						None
					}
				})
				.collect()
		}

		/// Fetch a single history entry by `id`, `key`, and `version`, returning
		/// `(old_value_bytes, block_number)` if it exists.
		pub fn get_attribute_history_entry(
			id: Ss58Identifier,
			key: Vec<u8>,
			version: u64,
		) -> Option<(Vec<u8>, EventBlock)> {
			let key_bounded: Attribute = key.try_into().ok()?;
			Ss58OfAttributeHistory::<T>::get(&id, (key_bounded, version))
				.map(|(old, block)| (old.as_ref().to_vec(), block))
		}
	}
}

impl<T: Config> Pallet<T> {
	// Revoke sub-account helper
	fn do_revoke_sub_account(id: &Ss58Identifier, sub: &T::AccountId) -> DispatchResult {
		let bound = Ss58OfActiveAccounts::<T>::get(sub).ok_or(Error::<T>::SubAccountNotFound)?;
		ensure!(bound == *id, Error::<T>::SubAccountNotLinked);
		Ss58OfActiveAccounts::<T>::remove(sub);
		let now = EventBlock::current::<T>();
		Ss58OfAccountHistory::<T>::insert(id, sub, now);
		SubAccounts::<T>::try_mutate(id, |list| {
			if let Some(pos) = list.iter().position(|x| x == sub) {
				list.swap_remove(pos);
				Ok(())
			} else {
				Err(Error::<T>::SubAccountNotLinked)
			}
		})?;
		let digest = T::Hashing::hash(
			&(id.clone(), sub.clone(), b"EntitySubAccountRevoked".to_vec()).encode(),
		);
		<Pallet<T>>::record_activity(id, digest, b"EntitySubAccountRevoked")?;
		Ok(())
	}

	// Clear Identitiy Helper
	fn do_clear_everything(id: &Ss58Identifier) -> DispatchResult {
		let now = EventBlock::current::<T>();
		for sub in SubAccounts::<T>::take(id).into_iter() {
			Ss58OfActiveAccounts::<T>::remove(&sub);
			Ss58OfAccountHistory::<T>::insert(id, &sub, now.clone());
		}
		if let Some(ctrl) = ControllerOfSs58::<T>::take(id) {
			Ss58OfActiveAccounts::<T>::remove(&ctrl);
			Ss58OfAccountHistory::<T>::insert(id, &ctrl, now.clone());
		}
		EntityInfoOf::<T>::remove(id);
		if let Some(uname) = Ss58IdNameOf::<T>::take(id) {
			NameSs58IdOf::<T>::remove(&uname);
		}
		let digest = T::Hashing::hash(&(id.clone(), b"EntityInfoCleared".to_vec()).encode());
		<Pallet<T>>::record_activity(id, digest, b"EntityInfoCleared")?;
		Ok(())
	}

	/// Get the current Ss58 ID of `who`, or an error if none.
	pub fn lookup_id_of(who: &T::AccountId) -> Result<Ss58Identifier, Error<T>> {
		Ss58OfActiveAccounts::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	/// Get the controller account of `id`, or error if none.
	pub fn lookup_controller_of(id: &Ss58Identifier) -> Result<T::AccountId, Error<T>> {
		ControllerOfSs58::<T>::get(id).ok_or(Error::<T>::IdentifierNotFound)
	}

	/// Get the full history for `id` as `(AccountId, BlockNumber)` pairs.
	pub fn lookup_history(id: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Ss58OfAccountHistory::<T>::iter_prefix(id).collect()
	}

	/// Check if `who` has _all_ of the requested `fields` in their on-chain identity.
	pub fn has_info_fields(
		who: &T::AccountId,
		fields: <T::EntityInformation as EntityInformationProvider>::FieldsIdentifier,
	) -> bool {
		Ss58OfActiveAccounts::<T>::get(who)
			.and_then(|id| EntityInfoOf::<T>::get(&id))
			.map_or(false, |info| info.has_info_fields(fields))
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
		<pallet_identifier::Pallet<T> as Identifier<T>>::state_event(
			identifier, digest, action, stamp,
		)
		.map_err(|_| Error::<T>::StateUpdateFailed)?;
		Ok(())
	}
}

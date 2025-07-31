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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod register;
pub mod weights;

extern crate alloc;
use crate::register::{
	Attribute, DoketInformationProvider, DoketUpdateError, DoketUpdateOp, Element, ExtraAttributes,
	FeeModel, Fees, LookupDefs, Permissions, RegisterField, RegisterInfo, RegisterType, Schema,
	Terms,
};
use alloc::{boxed::Box, fmt::Debug, vec::Vec};
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
use pallet_doken::{Doken, EventBlock, EventTypeOf};
use sp_runtime::traits::Hash;
pub use weights::WeightInfo;

pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
// pub type UpdateOpOf<T> = <<T as Config>::EntityInfoDoket as DoketInformationProvider>::UpdateOp;
// pub type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;
pub type AttributeUpdateKeyOpOf<T> = (Vec<u8>, DataOf<T>);

#[frame_support::pallet]
pub mod pallet {
	use cord_primitives::Attributes;

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		///  Doken Type
		type Doken: Doken<Self, Hash = Self::Hash>;

		/// The EntityLookup implementation (comes from `pallet_entity::Pallet`).
		type EntityLookupProvider: EntityLookup<Self, Error = Self::RuntimeError>;

		/// Maximum size for raw data fields.
		#[pallet::constant]
		type MaxRawDataLength: Get<u32>;

		/// Maximum number of additional attributes allowed.
		#[pallet::constant]
		type MaxAdditionalAttributes: Get<u32>;

		/// Max number of update operations in a single call
		#[pallet::constant]
		type MaxUpdateAttributeOps: Get<u32>;

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
	pub type RegisterInfoOf<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		RegisterInfo<T::MaxRawDataLength, T::MaxAdditionalAttributes>,
		OptionQuery,
	>;

	/// Permissions bitmask store.
	#[pallet::storage]
	pub type RegisterDelegatesOf<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		Ss58Identifier,
		Permissions,
		OptionQuery,
	>;

	/// Version counter for each (register, attribute key)
	#[pallet::storage]
	pub type RegisterAttributeVersionOf<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		Attribute,
		u64,
		ValueQuery,
	>;

	/// All history entries: (register, (key, version)) -> (old_value,  block)
	#[pallet::storage]
	pub type RegisterAttributeHistoryOf<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		(Attribute, u64),
		(Element<T::MaxRawDataLength>, EventBlock),
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		///Bad Origin
		BadOrigin,
		/// Register already exists
		RegisterAlreadyExists,
		/// Register not found
		RegisterNotFound,
		/// Invalid key length
		InvalidKey,
		/// Permission denied
		PermissionDenied,
		/// Empty index.
		EmptyIndex,
		/// Reserved Attribute.
		ReservedAttribute,
		/// Register Maintainer.
		RegisterMaintainer,
		/// Register Maintainer not found.
		MaintainerNotFound,
		/// Doken Exists.
		DokenAlreadyExists,
		/// Doken not found.
		DokenNotFound,
		/// Invalid Identifier.
		Invalididentifier,
		/// The index is invalid.
		InvalidIndex,
		/// The target is invalid.
		InvalidTarget,
		/// The doken inputs are not valid.
		DokenCreationFailed,
		/// The provided event type is invalid.
		InvalidEventType,
		/// Delegate not found.
		DelegateNotFound,
		/// Account ID is already named.
		AlreadyClaimed,
		/// Sender is not a sub-account.
		NotSub,
		/// Sub-account isn't owned by sender.
		NotOwned,
		/// Not enough free balance to pay the per-byte identity fee.
		InsufficientFunds,
		/// The username does not meet the requirements.
		InvalidAttributeEntry,
		/// The username does not meet the requirements.
		DuplicateAttributeKey,
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
		/// A new register was created: (creator, reg_id)
		RegisterCreated { doken: Ss58Identifier, who: Ss58Identifier },
		/// A register info was updated.
		RegisterInfoUpdated { doken: Ss58Identifier, who: Ss58Identifier },
		/// A register delegate was added.
		DelegateSet { doken: Ss58Identifier, who: Ss58Identifier },
		/// A register delegate was removed.
		DelegateRemoved { doken: Ss58Identifier, who: Ss58Identifier },
		/// Generic attribute update event
		AttributesAdded { doken: Ss58Identifier, who: Ss58Identifier },
		/// Generic attribute update event
		AttributeUpdated { doken: Ss58Identifier, who: Ss58Identifier },
		/// Generic attribute update event
		AttributeRemoved { doken: Ss58Identifier, who: Ss58Identifier, key: Vec<u8> },
		/// A register attribute was “rotated” (old value saved, version bumped).
		AttributeRotated { who: T::AccountId, register: Ss58Identifier, key: Vec<u8> },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new registry.
		#[pallet::call_index(0)]
		#[pallet::weight(
    	T::WeightInfo::create_register( info.encoded_size() as u32
        + attributes.as_ref().map_or(0, |a| a.encoded_size() as u32))
		)]
		pub fn create_register(
			origin: OriginFor<T>,
			info: Element<T::MaxRawDataLength>,
			attributes: Option<
				BoundedVec<(Attribute, Element<T::MaxRawDataLength>), T::MaxAdditionalAttributes>,
			>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let maintainer = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let mut reg =
				RegisterInfo::<T::MaxRawDataLength, T::MaxAdditionalAttributes>::default();
			reg.set_info(info);
			reg.set_maintainer(maintainer.clone());
			reg.attributes = attributes;

			reg.validate_attributes().map_err(|_| Error::<T>::InvalidAttributeEntry)?;

			let digest = T::Hashing::hash(&(&reg, b"RegisterCreated" as &[u8]).encode());
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let doken = T::Doken::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::DokenCreationFailed)?;

			RegisterInfoOf::<T>::try_mutate_exists(&doken, |slot| -> DispatchResult {
				ensure!(slot.is_none(), Error::<T>::RegisterAlreadyExists);
				*slot = Some(reg);
				Ok(())
			})?;

			RegisterDelegatesOf::<T>::insert(&doken, &maintainer, Permissions::ADMIN);

			Self::record_activity(&doken, digest, b"RegisterCreated")?;
			Self::deposit_event(Event::RegisterCreated { doken, who: maintainer });
			Ok(())
		}

		/// Overwrite the permissions bitmask for a specific register.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_delegate(perms.len() as u32))]
		pub fn set_delegate(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			delegate: T::AccountId,
			perms: Vec<Permissions>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegator = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let perm = RegisterDelegatesOf::<T>::get(&register, &delegator)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perm.has_delegate(), Error::<T>::PermissionDenied);

			let add_delegate = T::EntityLookupProvider::lookup_doken_of(&delegate)
				.map_err(|_| Error::<T>::DokenNotFound)?;
			let perm_mask = Permissions::from_list(&perms);
			RegisterDelegatesOf::<T>::insert(&register, &add_delegate, perm_mask);

			let digest = T::Hashing::hash(
				&(&register, &add_delegate, &perm_mask, b"DelegateSet" as &[u8]).encode(),
			);
			Self::record_activity(&register, digest, b"DelegateSet")?;

			Self::deposit_event(Event::DelegateSet { doken: register, who: add_delegate });
			Ok(())
		}

		/// Overwrite the permissions bitmask for a specific register.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::remove_delegate())]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			delegate: Ss58Identifier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegator = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let maintainer = RegisterInfoOf::<T>::get(&register)
				.ok_or(Error::<T>::RegisterNotFound)?
				.maintainer()
				.cloned()
				.ok_or(Error::<T>::MaintainerNotFound)?;

			ensure!(delegate != maintainer, Error::<T>::RegisterMaintainer);

			let perm = RegisterDelegatesOf::<T>::get(&register, &delegator)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perm.has_delegate(), Error::<T>::PermissionDenied);

			RegisterDelegatesOf::<T>::take(&register, &delegate)
				.ok_or(Error::<T>::DelegateNotFound)?;

			let digest = T::Hashing::hash(
				&(&register, &delegator, &delegate, b"DelegateRemoved" as &[u8]).encode(),
			);
			Self::record_activity(&register, digest, b"DelegateRemoved")?;

			Self::deposit_event(Event::DelegateRemoved { doken: register, who: delegate });
			Ok(())
		}

		/// Update attributes in an existing entity.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::update_info(info.encoded_size() as u32))]
		pub fn update_info(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			info: Element<T::MaxRawDataLength>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let register_admin = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			RegisterInfoOf::<T>::try_mutate(&register, |maybe| -> DispatchResult {
				let reg = maybe.as_mut().ok_or(Error::<T>::RegisterNotFound)?;
				if Some(&register_admin) != reg.maintainer() {
					let perms = RegisterDelegatesOf::<T>::get(&register, &register_admin)
						.ok_or(Error::<T>::PermissionDenied)?;
					ensure!(perms.has_admin(), Error::<T>::PermissionDenied);
				}
				reg.set_info(info.clone());
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&register, &info, b"RegisterInfoUpdated" as &[u8]).encode());
			Self::record_activity(&register, digest, b"RegisterInfoUpdated")?;
			Self::deposit_event(Event::RegisterInfoUpdated {
				doken: register,
				who: register_admin,
			});

			Ok(())
		}

		/// Add register attributes key->Data.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::add_attributes( ops.encoded_size()  as u32))]
		pub fn add_attributes(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			ops: Vec<AttributeUpdateKeyOpOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let register_admin = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let perms = RegisterDelegatesOf::<T>::get(&register, &register_admin)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perms.has_admin(), Error::<T>::PermissionDenied);

			RegisterInfoOf::<T>::try_mutate(&register, |maybe_reg| -> DispatchResult {
				let reg = maybe_reg.as_mut().ok_or(Error::<T>::RegisterNotFound)?;

				for (raw_key, val) in ops.iter() {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;

					reg.apply_update(&DoketUpdateOp::AddAttribute(key.clone(), val.clone()))
						.map_err(|e| match e {
							DoketUpdateError::AttributeExists => Error::<T>::AttributeExists,
							DoketUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes,
							DoketUpdateError::ReservedAttribute => Error::<T>::ReservedAttribute,
							_ => Error::<T>::InvalidAttributeEntry,
						})?;
				}

				reg.validate_attributes().map_err(|_| Error::<T>::InvalidAttributeEntry)?;

				Ok(())
			})?;
			let digest = T::Hashing::hash(&(&register, &ops, b"AttributesAdded" as &[u8]).encode());
			Self::record_activity(&register, digest, b"AttributesAdded")?;
			Self::deposit_event(Event::AttributesAdded { doken: register, who: register_admin });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::remove_attribute( key.len()  as u32))]
		pub fn remove_attribute(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			key: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let register_admin = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let perm = RegisterDelegatesOf::<T>::get(&register, &register_admin)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perm.has_revoke(), Error::<T>::PermissionDenied);

			let attr: Attribute = key.clone().try_into().map_err(|_| Error::<T>::InvalidKey)?;

			ensure!(
				RegisterField::from_bytes(attr.as_ref()).is_none(),
				Error::<T>::InvalidAttributeEntry
			);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;

			RegisterInfoOf::<T>::try_mutate(&register, |maybe| -> DispatchResult {
				let reg = maybe.as_mut().ok_or(Error::<T>::RegisterNotFound)?;
				reg.apply_update(&DoketUpdateOp::RemoveAttribute(attr.clone())).map_err(
					|e| match e {
						DoketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
						_ => Error::<T>::InvalidAttributeEntry,
					},
				)?;
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&register, &key, b"AttributeRemoved" as &[u8]).encode());
			Self::record_activity(&register, digest, b"AttributeRemoved")?;
			Self::deposit_event(Event::AttributeRemoved { doken: register, who: register_admin });
			Ok(())
		}

		/// Update a single custom attribute on a registry.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::update_attribute(key.len() as u32 + new_val.encoded_size() as u32))]
		pub fn update_attribute(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			key: Vec<u8>,
			new_val: DataOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let register_admin = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let perm = RegisterDelegatesOf::<T>::get(&register, &register_admin)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perm.has_admin(), Error::<T>::PermissionDenied);

			let attr: Attribute = key.clone().try_into().map_err(|_| Error::<T>::InvalidKey)?;

			ensure!(
				RegisterField::from_bytes(attr.as_ref()).is_none(),
				Error::<T>::InvalidAttributeEntry
			);

			RegisterInfoOf::<T>::try_mutate(&register, |maybe| -> DispatchResult {
				let reg = maybe.as_mut().ok_or(Error::<T>::RegisterNotFound)?;
				let op = DoketUpdateOp::UpdateAttribute(attr.clone(), new_val.clone());
				reg.apply_update(&op).map_err(|e| match e {
					DoketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
					_ => Error::<T>::InvalidAttributeEntry,
				})?;

				reg.validate_attributes().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
				Ok(())
			})?;

			let digest = T::Hashing::hash(
				&(&register, &key, &new_val, b"AttributeUpdated" as &[u8]).encode(),
			);
			Self::record_activity(&register, digest, b"AttributeUpdated")?;
			Self::deposit_event(Event::AttributeUpdated { doken: register, who: register_admin });

			Ok(())
		}

		/// “Rotate” (update) an existing attribute: record the old value in history, bump the version,
		/// then overwrite. Fails if the key is missing or invalid.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::rotate_attribute( key.len() as u32 + val.as_ref().len() as u32))]
		pub fn rotate_attribute(
			origin: OriginFor<T>,
			register: Ss58Identifier,
			key: Vec<u8>,
			val: DataOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let register_admin = T::EntityLookupProvider::lookup_doken_of(&who)
				.map_err(|_| Error::<T>::DokenNotFound)?;

			let perms = RegisterDelegatesOf::<T>::get(&register, &register_admin)
				.ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perms.has_admin(), Error::<T>::PermissionDenied);

			let attr: Attribute = key.clone().try_into().map_err(|_| Error::<T>::InvalidKey)?;

			ensure!(
				RegisterField::from_bytes(attr.as_ref()).is_none(),
				Error::<T>::InvalidAttributeEntry
			);

			RegisterInfoOf::<T>::try_mutate(&register, |maybe| -> DispatchResult {
				let reg = maybe.as_mut().ok_or(Error::<T>::RegisterNotFound)?;

				let old =
					reg.get_attribute(attr.as_ref()).ok_or(Error::<T>::AttributeNotFound)?.clone();
				let op = DoketUpdateOp::UpdateAttribute(attr.clone(), new_val.clone());
				reg.apply_update(&op).map_err(|e| match e {
					DoketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
					_ => Error::<T>::InvalidAttributeEntry,
				})?;

				let ver = RegisterAttributeVersionOf::<T>::get(&register, &attr).saturating_add(1);
				RegisterAttributeVersionOf::<T>::insert(&register, &attr, ver);
				RegisterAttributeHistoryOf::<T>::insert(
					&register,
					(attr.clone(), ver),
					(old, EventBlock::current::<T>()),
				);
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&register, &key, &val, b"AttributeRotated" as &[u8]).encode());
			Self::record_activity(&register, digest, b"AttributeRotated")?;
			Self::deposit_event(Event::AttributeRotated { who, register, key });

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
	pub fn record_activity(doken: &Ss58Identifier, digest: T::Hash, msg: &[u8]) -> DispatchResult {
		let action: EventTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
		let stamp = EventBlock::current::<T>();
		T::Doken::state_event(doken, digest, action, stamp)
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

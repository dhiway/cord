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

pub mod entity;
pub mod signature;
pub mod weights;

extern crate alloc;
use alloc::{boxed::Box, fmt::Debug, vec::Vec};
use codec::{Encode, EncodeLike};
use cord_primitives::{
	identifier::Ss58Identifier,
	packet::{Attribute, Element, PacketInformationProvider, PacketUpdateError, PacketUpdateOp},
};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, StorageVersion},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use pallet_token::{EventBlock, EventTypeOf, Token};
use signature::{SignatureVerificationError, SignatureVerificationResult, VerifySignature};
use sp_runtime::traits::Hash;
pub use weights::WeightInfo;

pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
pub type UpdateOpOf<T> = <<T as Config>::EntityInfoPacket as PacketInformationProvider>::UpdateOp;
pub type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;
pub type AttributeUpdateKeyOpOf<T> = (Vec<u8>, DataOf<T>);

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Token Type
		type Token: Token<Self, Hash = Self::Hash>;

		/// The maximum number of sub-accounts allowed per identified account.
		#[pallet::constant]
		type MaxSubAccounts: Get<u32>;

		/// Structure holding information about an entity,
		type EntityInfoPacket: PacketInformationProvider<
				FieldMask = u64,
				MaxRawDataLength = Self::MaxRawDataLength,
				MaxAdditionalAttributes = Self::MaxAdditionalAttributes,
				UpdateOp = PacketUpdateOp<Self::MaxRawDataLength>,
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
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::EntityInfoPacket, OptionQuery>;

	/// What Ss58‐ID does this account currently hold?
	#[pallet::storage]
	pub type Ss58OfActiveAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	/// Linked sub-accounts for each entity token.
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

	/// When was this account un-bound from this entity token?
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

	/// Username attached to an entity token.
	#[pallet::storage]
	pub type Ss58IdNameOf<T: Config> =
		StorageMap<_, Twox64Concat, Ss58Identifier, Username<T>, OptionQuery>;

	/// Reverse lookup: username → (owner, provider).
	#[pallet::storage]
	pub type NameSs58IdOf<T: Config> =
		StorageMap<_, Twox64Concat, Username<T>, Ss58Identifier, OptionQuery>;

	/// Version counter for each (token, attribute key).
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

	/// All history entries: (token, (key, version)) → (old_value,  block).
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
		/// Token Exists.
		TokenAlreadyExists,
		/// Token not found.
		TokenNotFound,
		/// Account mapped to an entity.
		EntitySubAccount,
		/// The index is invalid.
		InvalidIndex,
		/// The target is invalid.
		InvalidTarget,
		// /// The entity length is not valid.
		// InvalidIdentifierLength,
		/// The token inputs are not valid.
		TokenCreationFailed,
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
		EntityInfoSet { who: T::AccountId, token: Ss58Identifier },
		/// An entity info was updated.
		EntityInfoUpdated { who: T::AccountId, token: Ss58Identifier },
		/// An entity attribute was updated.
		EntityAttributeUpdated { who: T::AccountId, token: Ss58Identifier },
		/// An entity attribute was removed.
		EntityAttributeRemoved { who: T::AccountId, token: Ss58Identifier, attr: Attribute },
		/// An entity attribute was rotated.
		EntityAttributeRotated { who: T::AccountId, token: Ss58Identifier, attr: Attribute },
		/// A sub-account was added to an entity.
		EntitySubAccountAdded { sub: T::AccountId, token: Ss58Identifier },
		/// A sub-identity was revoked
		EntitySubAccountRevoked { sub: T::AccountId, token: Ss58Identifier },
		/// A sub-identity was revoked by root or council
		EntitySubAccountRevokedFor { sub: T::AccountId, token: Ss58Identifier },
		/// A controller was rotated.
		EntityControllerRotated { token: Ss58Identifier, new: T::AccountId },
		/// A controller was rotated by root or council.
		EntityControllerRotatedFor { token: Ss58Identifier, new: T::AccountId },
		/// A name was cleared.
		EntityInfoCleared { token: Ss58Identifier },
		/// A name was cleared by root or council.
		EntityInfoClearedFor { token: Ss58Identifier },
		/// A username was set for `who`.
		Ss58IdNameAdded { token: Ss58Identifier, name: Username<T> },
		/// A username has been removed.
		Ss58IdNameRemoved { token: Ss58Identifier },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set an entity's information and generate an entity token.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_info(info.encoded_size() as u32))]
		pub fn set_info(origin: OriginFor<T>, info: Box<T::EntityInfoPacket>) -> DispatchResult {
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

			for reserved in
				[&b"display"[..], &b"legal"[..], &b"web"[..], &b"email"[..], &b"twitter"[..]]
			{
				info.get_key(reserved)
					.validate()
					.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
			}
			if let Some(attributes) = info.attributes() {
				for (_, value) in attributes.iter() {
					value.validate().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
				}
			}

			let digest = T::Hashing::hash(&(&info, b"EntityInfoSet" as &[u8]).encode());
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let token = T::Token::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::TokenCreationFailed)?;

			EntityInfoOf::<T>::try_mutate_exists(&token, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::TokenAlreadyExists);
				*opt = Some(info);
				Ok(())
			})?;

			Ss58OfActiveAccounts::<T>::insert(&who, token.clone());
			ControllerOfSs58::<T>::insert(&token, who.clone());

			Self::record_activity(&token, digest, b"EntityInfoSet")?;
			Self::deposit_event(Event::EntityInfoSet { who, token });

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
			let token = Self::lookup_token_of(&who)?;
			let controller = Self::lookup_controller_of(&token)?;
			ensure!(who == controller, Error::<T>::BadOrigin);

			EntityInfoOf::<T>::try_mutate(&token, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				for (raw_key, val) in ops.iter() {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					let op = PacketUpdateOp::UpdateAttribute(key.clone(), val.clone());
					info.apply_update(&op).map_err(|e| match e {
						PacketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound,
						_ => Error::<T>::InvalidAttributeEntry,
					})?;
				}
				Ok(())
			})?;

			let digest = T::Hashing::hash(&(&token, &ops, b"EntityInfoUpdated" as &[u8]).encode());
			Self::record_activity(&token, digest, b"EntityInfoUpdated")?;
			Self::deposit_event(Event::EntityInfoUpdated { who, token });

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
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			EntityInfoOf::<T>::try_mutate(&token, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				for (raw_key, val) in ops.iter() {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					info.apply_update(&PacketUpdateOp::AddAttribute(key.clone(), val.clone()))
						.map_err(|e| match e {
							PacketUpdateError::AttributeExists => Error::<T>::AttributeExists,
							PacketUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes,
							_ => Error::<T>::InvalidAttributeEntry,
						})?;
				}
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&token, &ops, b"EntityAttributeUpdated" as &[u8]).encode());

			Self::record_activity(&token, digest, b"EntityAttributeUpdated")?;

			Self::deposit_event(Event::EntityAttributeUpdated { who, token });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::remove_attribute( key.len()  as u32))]
		pub fn remove_attribute(origin: OriginFor<T>, key: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
			EntityInfoOf::<T>::try_mutate(&token, |opt| -> DispatchResult {
				let info = opt.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				info.apply_update(&PacketUpdateOp::RemoveAttribute(attr.clone()))
					.map_err(|_| Error::<T>::AttributeNotFound)?;
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&token, &key, b"EntityAttributeRemoved" as &[u8]).encode());

			Self::record_activity(&token, digest, b"EntityAttributeRemoved")?;

			Self::deposit_event(Event::EntityAttributeRemoved { who, token, attr });
			Ok(())
		}

		/// “Rotate” (update) an existing attribute: record the old value in history, bump the
		/// version, then overwrite. Fails if the key is missing or invalid.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::rotate_attribute( key.len() as u32 + val.as_ref().len() as u32))]
		pub fn rotate_attribute(
			origin: OriginFor<T>,
			key: Vec<u8>,
			val: DataOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;

			EntityInfoOf::<T>::try_mutate(&token, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				let old_val = info.get_key(&key);

				info.apply_update(&PacketUpdateOp::UpdateAttribute(attr.clone(), val.clone()))
					.map_err(|_| Error::<T>::AttributeNotFound)?;

				let ver = Ss58OfAttributeVersion::<T>::get(&token, &attr).saturating_add(1);
				Ss58OfAttributeVersion::<T>::insert(&token, &attr, ver);
				Ss58OfAttributeHistory::<T>::insert(
					&token,
					(attr.clone(), ver),
					(old_val.clone(), EventBlock::current::<T>()),
				);

				Ok(())
			})?;

			let digest = T::Hashing::hash(
				&(&token, &key, &val, b"EntityAttributeRotated" as &[u8]).encode(),
			);

			Self::record_activity(&token, digest, b"EntityAttributeRotated")?;
			Self::deposit_event(Event::EntityAttributeRotated { who, token, attr });

			Ok(())
		}

		/// Set a sub-account of the sender.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_sub_account(sub.encoded_size() as u32))]
		pub fn set_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			ensure!(
				!Ss58OfActiveAccounts::<T>::contains_key(&sub),
				Error::<T>::SubAccountAlreadyClaimed
			);

			SubAccounts::<T>::try_mutate(&token, |list| {
				ensure!(
					list.len() < T::MaxSubAccounts::get() as usize,
					Error::<T>::TooManySubAccounts
				);
				list.try_push(sub.clone()).map_err(|_| Error::<T>::TooManySubAccounts)
			})?;

			Ss58OfActiveAccounts::<T>::insert(&sub, token.clone());

			let digest =
				T::Hashing::hash(&(&token, &sub, b"EntitySubAccountAdded" as &[u8]).encode());
			Self::record_activity(&token, digest, b"EntitySubAccountAdded")?;

			Self::deposit_event(Event::EntitySubAccountAdded { sub, token });

			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account(sub.encoded_size() as u32))]
		pub fn revoke_sub_account(origin: OriginFor<T>, sub: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			ensure!(sub != who, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&token, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, token });
			Ok(())
		}

		/// Remove a previously-added sub-account.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::revoke_sub_account_for(sub.encoded_size() as u32))]
		pub fn revoke_sub_account_for(
			origin: OriginFor<T>,
			token: Ss58Identifier,
			sub: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			ensure!(sub != Self::lookup_controller_of(&token)?, Error::<T>::ControllerAccount);
			Self::do_revoke_sub_account(&token, &sub)?;
			Self::deposit_event(Event::EntitySubAccountRevoked { sub, token });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::rotate_controller(new_controller.encoded_size() as u32)
            .saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller(
			origin: OriginFor<T>,
			token: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			ensure!(new_controller != who, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&who);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, token.clone());
			ControllerOfSs58::<T>::insert(&token, new_controller.clone());
			Ss58OfAccountHistory::<T>::insert(&token, &who, EventBlock::current::<T>());

			let digest = T::Hashing::hash(
				&(&token, &new_controller, b"EntityControllerRotated" as &[u8]).encode(),
			);
			Self::record_activity(&token, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { token, new: new_controller });

			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::rotate_controller_for(new_controller.encoded_size() as u32)
		.saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		pub fn rotate_controller_for(
			origin: OriginFor<T>,
			token: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			let current_controller = Self::lookup_controller_of(&token)?;
			ensure!(current_controller != new_controller, Error::<T>::AlreadyController);

			Ss58OfActiveAccounts::<T>::remove(&current_controller);
			Ss58OfActiveAccounts::<T>::insert(&new_controller, token.clone());
			ControllerOfSs58::<T>::insert(&token, new_controller.clone());

			Ss58OfAccountHistory::<T>::insert(
				&token,
				&current_controller,
				EventBlock::current::<T>(),
			);

			let digest = T::Hashing::hash(
				&(&token, &new_controller, b"EntityControllerRotated" as &[u8]).encode(),
			);
			Self::record_activity(&token, digest, b"EntityControllerRotated")?;

			Self::deposit_event(Event::EntityControllerRotated { token, new: new_controller });

			Ok(())
		}

		/// Remove all entity details from storage
		#[pallet::call_index(10)]
		// #[pallet::weight(T::WeightInfo::clear_identity())]
		#[pallet::weight({
		    let sub_count = SubAccounts::<T>::get(&token).len() as u32;
		    T::WeightInfo::clear_everything(sub_count)
		})]
		pub fn clear_everything(origin: OriginFor<T>, token: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			Self::do_clear_everything(&token)?;
			Self::deposit_event(Event::EntityInfoCleared { token });
			Ok(())
		}

		/// Council/Root Remove all details of an entity from storage
		#[pallet::call_index(11)]
		// #[pallet::weight(T::WeightInfo::clear_identity_for())]
		#[pallet::weight({
		    let sub_count = SubAccounts::<T>::get(&token).len() as u32;
		    T::WeightInfo::clear_everything_for(sub_count)
		})]
		pub fn clear_everything_for(origin: OriginFor<T>, token: Ss58Identifier) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			Self::do_clear_everything(&token)?;
			Self::deposit_event(Event::EntityInfoCleared { token });
			Ok(())
		}

		/// Add an entity token name under the constant suffix ".myn.social", always stored
		/// lowercase.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::set_id_name(prefix.len() as u32))]
		pub fn set_id_name(origin: OriginFor<T>, mut prefix: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			for b in &mut prefix {
				*b = b.to_ascii_lowercase();
			}
			ensure!(Self::is_valid_user_name_prefix(&prefix), Error::<T>::InvalidSs58IdName);
			prefix.extend(b".myn.social");

			let bounded_uname: Username<T> =
				prefix.try_into().map_err(|_| Error::<T>::InvalidSs58IdName)?;

			NameSs58IdOf::<T>::try_mutate_exists(&bounded_uname, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::Ss58IdNameTaken);
				*opt = Some(token.clone());
				Ok(())
			})?;

			Ss58IdNameOf::<T>::insert(&token, &bounded_uname);
			let digest =
				T::Hashing::hash(&(&token, &bounded_uname, b"Ss58IdNameAdded" as &[u8]).encode());

			Self::record_activity(&token, digest, b"Ss58IdNameAdded")?;
			Self::deposit_event(Event::Ss58IdNameAdded { token, name: bounded_uname });

			Ok(())
		}

		/// Remove an existing username under the suffix "myn.social".
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::remove_id_name())]
		pub fn remove_id_name(origin: OriginFor<T>, token: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let uname = Ss58IdNameOf::<T>::take(&token).ok_or(Error::<T>::NoUsername)?;
			NameSs58IdOf::<T>::remove(&uname);

			let digest =
				T::Hashing::hash(&(&token, &uname, b"Ss58IdNameRemoved" as &[u8]).encode());

			Self::record_activity(&token, digest, b"Ss58IdNameRemoved")?;
			Self::deposit_event(Event::Ss58IdNameRemoved { token });
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		/// Get every attribute change for `token` as
		/// `(key_bytes, version, old_value_bytes, block_number)`.
		pub fn get_attribute_history(
			token: Ss58Identifier,
		) -> Vec<(Vec<u8>, u64, Vec<u8>, EventBlock)> {
			Ss58OfAttributeHistory::<T>::iter_prefix(&token)
				.map(|((key, version), (old, block))| {
					(key.to_vec(), version, old.as_ref().to_vec(), block)
				})
				.collect()
		}

		/// Get the change history of a single `key` for `token` as
		/// `(version, old_value_bytes, block_number)`.
		pub fn get_attribute_history_for_key(
			token: Ss58Identifier,
			key: Vec<u8>,
		) -> Vec<(u64, Vec<u8>, EventBlock)> {
			let key_bounded: Attribute =
				key.try_into().expect("caller should provide valid-length key");
			Ss58OfAttributeHistory::<T>::iter_prefix(&token)
				.filter_map(|((k, version), (old, block))| {
					if k == key_bounded {
						Some((version, old.as_ref().to_vec(), block))
					} else {
						None
					}
				})
				.collect()
		}

		/// Fetch a single history entry by `token`, `key`, and `version`, returning
		/// `(old_value_bytes, block_number)` if it exists.
		pub fn get_attribute_history_entry(
			token: Ss58Identifier,
			key: Vec<u8>,
			version: u64,
		) -> Option<(Vec<u8>, EventBlock)> {
			let key_bounded: Attribute = key.try_into().ok()?;
			Ss58OfAttributeHistory::<T>::get(&token, (key_bounded, version))
				.map(|(old, block)| (old.as_ref().to_vec(), block))
		}
	}
}

impl<T: Config> Pallet<T> {
	// Revoke sub-account helper
	fn do_revoke_sub_account(token: &Ss58Identifier, sub: &T::AccountId) -> DispatchResult {
		let bound = Ss58OfActiveAccounts::<T>::get(sub).ok_or(Error::<T>::SubAccountNotFound)?;
		ensure!(bound == *token, Error::<T>::SubAccountNotLinked);
		Ss58OfActiveAccounts::<T>::remove(sub);
		let now = EventBlock::current::<T>();
		Ss58OfAccountHistory::<T>::insert(token, sub, now);
		SubAccounts::<T>::try_mutate(token, |list| {
			if let Some(pos) = list.iter().position(|x| x == sub) {
				list.swap_remove(pos);
				Ok(())
			} else {
				Err(Error::<T>::SubAccountNotLinked)
			}
		})?;
		let digest = T::Hashing::hash(&(token, sub, b"EntitySubAccountRevoked" as &[u8]).encode());
		Self::record_activity(token, digest, b"EntitySubAccountRevoked")?;
		Ok(())
	}

	// Clear Identitiy Helper
	fn do_clear_everything(token: &Ss58Identifier) -> DispatchResult {
		let now = EventBlock::current::<T>();
		for sub in SubAccounts::<T>::take(token).into_iter() {
			Ss58OfActiveAccounts::<T>::remove(&sub);
			Ss58OfAccountHistory::<T>::insert(token, &sub, now.clone());
		}
		if let Some(ctrl) = ControllerOfSs58::<T>::take(token) {
			Ss58OfActiveAccounts::<T>::remove(&ctrl);
			Ss58OfAccountHistory::<T>::insert(token, &ctrl, now.clone());
		}
		EntityInfoOf::<T>::remove(token);
		if let Some(uname) = Ss58IdNameOf::<T>::take(token) {
			NameSs58IdOf::<T>::remove(&uname);
		}
		let digest = T::Hashing::hash(&(token, b"EntityInfoCleared" as &[u8]).encode());
		Self::record_activity(token, digest, b"EntityInfoCleared")?;
		Ok(())
	}

	/// Get the current Ss58 ID of `who`, or an error if none.
	pub fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Error<T>> {
		Ss58OfActiveAccounts::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	/// Get the controller account of `token`, or error if none.
	pub fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Error<T>> {
		ControllerOfSs58::<T>::get(token).ok_or(Error::<T>::TokenNotFound)
	}

	/// Get the full history for `token` as `(AccountId, BlockNumber)` pairs.
	pub fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Ss58OfAccountHistory::<T>::iter_prefix(token).collect()
	}

	/// Check if `who` has _all_ of the requested `fields` in their on-chain identity.
	pub fn has_info_fields(
		who: &T::AccountId,
		mask: <T::EntityInfoPacket as PacketInformationProvider>::FieldMask,
	) -> bool {
		Ss58OfActiveAccounts::<T>::get(who)
			.and_then(|token| EntityInfoOf::<T>::get(&token))
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
	pub fn record_activity(token: &Ss58Identifier, digest: T::Hash, msg: &[u8]) -> DispatchResult {
		let action: EventTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
		let stamp = EventBlock::current::<T>();
		T::Token::state_event(token, digest, action, stamp)
			.map_err(|_| Error::<T>::StateUpdateFailed)?;
		Ok(())
	}

	/// Resolve the controller account for the supplied entity token.
	pub fn controller_account(
		token: &Ss58Identifier,
	) -> Result<T::AccountId, SignatureVerificationError> {
		ControllerOfSs58::<T>::get(token)
			.ok_or(SignatureVerificationError::SignerInformationNotPresent)
	}

	/// Verify a payload using the supplied signature strategy against the controller of `token`.
	pub fn verify_signature_with<V>(
		token: &Ss58Identifier,
		payload: Vec<u8>,
		signature: &V::Signature,
	) -> SignatureVerificationResult
	where
		V: VerifySignature<SignerId = T::AccountId, Payload = Vec<u8>>,
	{
		let controller = Self::controller_account(token)?;
		V::verify(&controller, &payload, signature)
	}
}

/// A thin API for entity lookups, so that *any* pallet can call:
pub trait EntityLookup<T: frame_system::Config> {
	/// The error returned by the fallible lookups.
	type Error;
	/// The username type (e.g. `Username<T>`).
	type Username;

	/// Get the current Ss58 ID of `who`, or Err if none.
	fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error>;

	/// Get the controller account of `token`, or Err if none.
	fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Self::Error>;

	/// Get the full history for `token` as `(AccountId, EventBlock)`.
	fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)>;

	/// Fetch the (optional) username attached to an entity token.
	fn lookup_name_of_identifier(token: &Ss58Identifier) -> Option<Self::Username>;

	/// Reverse lookup: given a username, get the attached entity token (if any).
	fn lookup_identifier_of_name(name: &Self::Username) -> Option<Ss58Identifier>;
}

impl<T: Config> EntityLookup<T> for Pallet<T> {
	type Error = Error<T>;
	type Username = Username<T>;

	fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error> {
		Ss58OfActiveAccounts::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Self::Error> {
		ControllerOfSs58::<T>::get(token).ok_or(Error::<T>::TokenNotFound)
	}

	fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Ss58OfAccountHistory::<T>::iter_prefix(token).collect()
	}

	fn lookup_name_of_identifier(token: &Ss58Identifier) -> Option<Username<T>> {
		Ss58IdNameOf::<T>::get(token)
	}

	fn lookup_identifier_of_name(name: &Username<T>) -> Option<Ss58Identifier> {
		NameSs58IdOf::<T>::get(name)
	}
}

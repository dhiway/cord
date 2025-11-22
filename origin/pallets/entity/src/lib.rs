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

use alloc::{boxed::Box, collections::BTreeSet, fmt::Debug, vec::Vec};
use codec::{Decode, Encode, EncodeLike};
use core::convert::TryInto;
use frame_support::{
	ensure,
	pallet_prelude::*,
	storage::{with_transaction, TransactionOutcome},
	traits::{CallerTrait, Get, StorageVersion},
	BoundedVec,
};
use frame_system::pallet_prelude::*;

use crate::entity::{
	AccountUnbindEntryView, AttributeHistoryEntryView, EntityField, EntityInfoView,
	EntityStateView, EventBlockView,
};
use crate::signature::{verify_multisignature, SignatureVerificationError};
use origin_primitives::{
	attribute::{Attribute, AttributeValueView, Element},
	authorization::{
		ensure_authorization_ttl, extract_valid_until, Authorization as ViewAuthorization,
		AuthorizationError,
	},
	element::ElementView,
	identifier::Ss58Identifier,
	packet::{PacketInformationProvider, PacketUpdateError, PacketUpdateOp},
	Signature,
};
use pallet_feeless::FeelessAccounts;
use pallet_token::{EventBlock, EventTypeOf, Token};
use sp_runtime::{
	traits::{Hash, UniqueSaturatedInto, Verify},
	AccountId32,
};

pub use pallet::*;
pub use weights::WeightInfo;

pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
pub type UpdateOpOf<T> = <<T as Config>::EntityInfoPacket as PacketInformationProvider>::UpdateOp;
pub type EntityNym<T> = BoundedVec<u8, <T as Config>::MaxEntityNymLength>;
pub type AttributeUpdateKeyOpOf<T> = (Vec<u8>, DataOf<T>);

/// Authorization payload supplied for entity authorization requests.
pub type AuthorizationPayloadOf<T> = BoundedVec<u8, <T as Config>::MaxAuthorizationLen>;
/// Authorization structure reused by authorization-gated queries.
pub type Authorization<T> =
	ViewAuthorization<<T as frame_system::Config>::AccountId, AuthorizationPayloadOf<T>, Signature>;
pub type AuthorizationOf<T> = Authorization<T>;
pub type LinkedAccountsListOf<T> =
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxLinkedAccounts>;
pub type SignerId = <Signature as Verify>::Signer;

const ENTITY_NYM_SUFFIX: &[u8] = b".nym.org.in";

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

		/// The maximum number of linked accounts allowed per entity token.
		#[pallet::constant]
		type MaxLinkedAccounts: Get<u32>;

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

		/// Max length for an entity nym prefix (before the suffix).
		#[pallet::constant]
		type MaxEntityNymLength: Get<u32>;

		/// Default number of history rows returned from the composite overview view.
		#[pallet::constant]
		type DefaultEntityOverviewHistory: Get<u32>;

		/// Maximum number of history rows returned from the composite overview view.
		#[pallet::constant]
		type MaxEntityOverviewHistory: Get<u32>;

		/// Maximum payload length for view authorizations.
		#[pallet::constant]
		type MaxAuthorizationLen: Get<u32>;

		/// Maximum number of blocks an authorization remains valid.
		#[pallet::constant]
		type MaxAuthorizationTTL: Get<u32>;

		/// Source of feeless account information.
		type Feeless: FeelessAccounts<Self::AccountId>;

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

	/// Which entity token (if any) is currently bound to this account?
	#[pallet::storage]
	pub type EntityTokenOfAccount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	/// Linked accounts for each entity token.
	#[pallet::storage]
	pub type LinkedAccounts<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		BoundedVec<T::AccountId, T::MaxLinkedAccounts>,
		ValueQuery,
	>;

	/// Which account currently controls this entity token?
	#[pallet::storage]
	pub type ControllerAccountOf<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::AccountId, OptionQuery>;

	/// When was this account unbound from this entity token?
	#[pallet::storage]
	pub type AccountUnbindHistory<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		T::AccountId,
		EventBlock,
		OptionQuery,
	>;

	/// Entity nym attached to an entity token.
	#[pallet::storage]
	pub type EntityNymOf<T: Config> =
		StorageMap<_, Twox64Concat, Ss58Identifier, EntityNym<T>, OptionQuery>;

	/// Reverse lookup: nym → entity token.
	#[pallet::storage]
	pub type EntityNymIndex<T: Config> =
		StorageMap<_, Twox64Concat, EntityNym<T>, Ss58Identifier, OptionQuery>;

	/// Version counter for each (token, attribute key).
	#[pallet::storage]
	pub type AttributeVersionOf<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		Attribute,
		u64,
		ValueQuery,
	>;

	/// All history entries: (token, (key, version)) → (old_value, block).
	#[pallet::storage]
	pub type AttributeHistoryOf<T: Config> = StorageDoubleMap<
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
		/// Too many linked accounts.
		TooManyLinkedAccounts,
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
		/// Account already mapped to an entity.
		AccountAlreadyLinked,
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
		/// Sender is not a linked account.
		NotSub,
		/// Sub-account isn't owned by sender.
		NotOwned,
		/// Not enough free balance to pay the per-byte identity fee.
		InsufficientFunds,
		/// Setting this entity nym requires a signature, but none was provided.
		RequiresSignature,
		/// Linked account is already mapped to an entity.
		LinkedAccountAlreadyClaimed,
		/// Linked account not found.
		LinkedAccountNotFound,
		/// Linked account is not linked to the entity
		LinkedAccountNotLinked,
		/// Linked account already exists.
		LinkedAccountExists,
		/// The entity nym does not meet the requirements.
		InvalidEntityNym,
		/// The attribute value is invalid.
		InvalidAttributeEntry,
		/// Duplicate attribute key found.
		DuplicateAttributeKey,
		/// The entity nym is already taken.
		EntityNymTaken,
		/// The entity already has an assigned nym.
		EntityNymAlreadySet,
		/// No entity nym exists for this token.
		NoEntityNym,
		/// The action cannot be performed because of insufficient privileges (e.g. authority
		/// trying to unbind a nym provided by the system).
		InsufficientPrivileges,
		/// Tried to add an attribute that already exists.
		AttributeExists,
		/// Exceeded the maximum number of additional attribute/value pairs.
		TooManyAttributes,
		/// Tried to update or remove an attribute that doesn't exist.
		AttributeNotFound,
		/// Tried to remove a reserved/preset attribute key.
		ReservedAttribute,
		// State Update Failed
		StateUpdateFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A name was set or reset (which will remove all judgements).
		EntityInfoSet { who: T::AccountId, token: Ss58Identifier },
		/// An entity attribute was updated.
		EntityAttributeUpdated { who: T::AccountId, token: Ss58Identifier },
		/// An entity attribute was removed.
		EntityAttributeRemoved { who: T::AccountId, token: Ss58Identifier, attr: Attribute },
		/// An entity attribute was rotated.
		EntityAttributeRotated { who: T::AccountId, token: Ss58Identifier, attr: Attribute },
		/// A linked account was added to an entity.
		EntityLinkedAccountAdded { account: T::AccountId, token: Ss58Identifier },
		/// A linked account was revoked by the controller.
		EntityLinkedAccountRevoked { account: T::AccountId, token: Ss58Identifier },
		/// A linked account was revoked by root or council.
		EntityLinkedAccountRevokedFor { account: T::AccountId, token: Ss58Identifier },
		/// A controller was rotated.
		EntityControllerRotated { token: Ss58Identifier, new: T::AccountId },
		/// A controller was rotated by root or council.
		EntityControllerRotatedFor { token: Ss58Identifier, new: T::AccountId },
		/// A name was cleared.
		EntityInfoCleared { token: Ss58Identifier },
		/// A name was cleared by root or council.
		EntityInfoClearedFor { token: Ss58Identifier },
		/// A nym was set for `who`.
		EntityNymAdded { token: Ss58Identifier, name: EntityNym<T> },
		/// A nym has been removed.
		EntityNymRemoved { token: Ss58Identifier },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set an entity's information and generate an entity token.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_info(info.encoded_size() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _info: &Box<T::EntityInfoPacket>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn set_info(origin: OriginFor<T>, info: Box<T::EntityInfoPacket>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				!EntityTokenOfAccount::<T>::contains_key(&who),
				Error::<T>::AccountAlreadyLinked
			);

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

			for reserved in [&b"display"[..], &b"web"[..], &b"email"[..]] {
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

			Self::do_set_linked_account(&token, &who)?;
			ControllerAccountOf::<T>::insert(&token, who.clone());

			Self::record_activity(&token, digest, b"EntityInfoSet")?;
			Self::deposit_event(Event::EntityInfoSet { who, token });

			Ok(())
		}

		/// Rotate multiple attributes in a single dispatch, recording history per key.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::rotate_attributes(ops.encoded_size() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _ops: &Vec<AttributeUpdateKeyOpOf<T>>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn rotate_attributes(
			origin: OriginFor<T>,
			ops: Vec<AttributeUpdateKeyOpOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			let controller = Self::lookup_controller_of(&token)?;
			ensure!(who == controller, Error::<T>::BadOrigin);

			let parsed: Vec<(Attribute, DataOf<T>)> = ops
				.into_iter()
				.map(|(raw_key, val)| {
					let attr: Attribute =
						raw_key.try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					Ok((attr, val))
				})
				.collect::<Result<_, Error<T>>>()?;
			let mut seen = BTreeSet::new();
			for (attr, _) in parsed.iter() {
				let inserted = seen.insert(attr.clone().into_inner());
				ensure!(inserted, Error::<T>::DuplicateAttributeKey);
			}

			with_transaction(|| {
				for (attr, val) in parsed.iter() {
					if let Err(err) = Self::do_rotate_attribute(&token, attr, val) {
						return TransactionOutcome::Rollback(Err(err));
					}
				}
				TransactionOutcome::Commit(Ok(()))
			})?;

			for (attr, _) in parsed.iter() {
				Self::deposit_event(Event::EntityAttributeRotated {
					who: who.clone(),
					token: token.clone(),
					attr: attr.clone(),
				});
			}

			Ok(())
		}

		/// Add entity attributes key->Data.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::add_attributes( ops.encoded_size()  as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _ops: &Vec<AttributeUpdateKeyOpOf<T>>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn add_attributes(
			origin: OriginFor<T>,
			ops: Vec<AttributeUpdateKeyOpOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let parsed: Vec<(Attribute, DataOf<T>)> = ops
				.iter()
				.map(|(raw_key, val)| {
					let key: Attribute = raw_key
						.clone()
						.try_into()
						.map_err(|_| Error::<T>::InvalidAttributeEntry)?;
					Ok((key, val.clone()))
				})
				.collect::<Result<_, Error<T>>>()?;
			let mut seen = BTreeSet::new();
			for (attr, _) in parsed.iter() {
				let inserted = seen.insert(attr.clone().into_inner());
				ensure!(inserted, Error::<T>::DuplicateAttributeKey);
			}

			EntityInfoOf::<T>::try_mutate(&token, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				for (key, val) in parsed.iter() {
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
		#[pallet::feeless_if(|origin: &OriginFor<T>, _key: &Vec<u8>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn remove_attribute(origin: OriginFor<T>, key: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let attr: Attribute =
				key.clone().try_into().map_err(|_| Error::<T>::InvalidAttributeEntry)?;
			ensure!(
				EntityField::from_bytes(attr.as_slice()).is_none(),
				Error::<T>::ReservedAttribute
			);
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
		#[pallet::feeless_if(|origin: &OriginFor<T>, _key: &Vec<u8>, _val: &DataOf<T>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
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
			Self::do_rotate_attribute(&token, &attr, &val)?;
			Self::deposit_event(Event::EntityAttributeRotated { who, token, attr });

			Ok(())
		}

		/// Link an additional account to the sender's entity token.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_linked_account(account.encoded_size() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _account: &T::AccountId| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn set_linked_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			Self::do_set_linked_account(&token, &account)?;

			let digest = T::Hashing::hash(
				&(&token, &account, b"EntityLinkedAccountAdded" as &[u8]).encode(),
			);
			Self::record_activity(&token, digest, b"EntityLinkedAccountAdded")?;

			Self::deposit_event(Event::EntityLinkedAccountAdded { account, token });

			Ok(())
		}

		/// Remove a previously-added linked account.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::revoke_linked_account(account.encoded_size() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _account: &T::AccountId| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn revoke_linked_account(
			origin: OriginFor<T>,
			account: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			Self::do_revoke_linked_account(&token, &account)?;
			Self::deposit_event(Event::EntityLinkedAccountRevoked { account, token });
			Ok(())
		}

		/// Remove a previously-added linked account via privileged origin.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::revoke_linked_account_for(account.encoded_size() as u32))]
		pub fn revoke_linked_account_for(
			origin: OriginFor<T>,
			token: Ss58Identifier,
			account: T::AccountId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			Self::do_revoke_linked_account(&token, &account)?;
			Self::deposit_event(Event::EntityLinkedAccountRevokedFor { account, token });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::rotate_controller(new_controller.encoded_size() as u32)
            .saturating_add(T::DbWeight::get().reads_writes(2, 3)))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _token: &Ss58Identifier, _new_controller: &T::AccountId| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn rotate_controller(
			origin: OriginFor<T>,
			token: Ss58Identifier,
			new_controller: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			ensure!(new_controller != who, Error::<T>::AlreadyController);
			Self::ensure_account_linked(&token, &new_controller)?;

			EntityTokenOfAccount::<T>::remove(&who);
			EntityTokenOfAccount::<T>::insert(&new_controller, token.clone());
			ControllerAccountOf::<T>::insert(&token, new_controller.clone());
			AccountUnbindHistory::<T>::insert(&token, &who, EventBlock::current::<T>());

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
			Self::ensure_account_linked(&token, &new_controller)?;

			EntityTokenOfAccount::<T>::remove(&current_controller);
			EntityTokenOfAccount::<T>::insert(&new_controller, token.clone());
			ControllerAccountOf::<T>::insert(&token, new_controller.clone());

			AccountUnbindHistory::<T>::insert(
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
		#[pallet::weight({
		    let sub_count = LinkedAccounts::<T>::get(&token).len() as u32;
		    T::WeightInfo::clear_everything(sub_count)
		})]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _token: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
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
		    let sub_count = LinkedAccounts::<T>::get(&token).len() as u32;
		    T::WeightInfo::clear_everything_for(sub_count)
		})]
		pub fn clear_everything_for(origin: OriginFor<T>, token: Ss58Identifier) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), Error::<T>::TokenNotFound);
			Self::do_clear_everything(&token)?;
			Self::deposit_event(Event::EntityInfoClearedFor { token });
			Ok(())
		}

		/// Add an entity token name under the constant suffix ".nym.org.in", always stored
		/// lowercase.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::set_entity_nym(prefix.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _prefix: &Vec<u8>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn set_entity_nym(origin: OriginFor<T>, mut prefix: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let token = Self::lookup_token_of(&who)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);
			ensure!(!EntityNymOf::<T>::contains_key(&token), Error::<T>::EntityNymAlreadySet);

			for b in &mut prefix {
				*b = b.to_ascii_lowercase();
			}
			ensure!(Self::is_valid_entity_nym_prefix(&prefix), Error::<T>::InvalidEntityNym);
			prefix.extend_from_slice(ENTITY_NYM_SUFFIX);

			let bounded_uname: EntityNym<T> =
				prefix.try_into().map_err(|_| Error::<T>::InvalidEntityNym)?;

			EntityNymIndex::<T>::try_mutate_exists(&bounded_uname, |opt| -> DispatchResult {
				ensure!(opt.is_none(), Error::<T>::EntityNymTaken);
				*opt = Some(token.clone());
				Ok(())
			})?;

			EntityNymOf::<T>::insert(&token, &bounded_uname);
			let digest =
				T::Hashing::hash(&(&token, &bounded_uname, b"EntityNymAdded" as &[u8]).encode());

			Self::record_activity(&token, digest, b"EntityNymAdded")?;
			Self::deposit_event(Event::EntityNymAdded { token, name: bounded_uname });

			Ok(())
		}

		/// Remove an existing entity nym under the suffix "nym.org.in".
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::remove_entity_nym())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _token: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn remove_entity_nym(origin: OriginFor<T>, token: Ss58Identifier) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::lookup_controller_of(&token)?, Error::<T>::BadOrigin);

			let uname = EntityNymOf::<T>::take(&token).ok_or(Error::<T>::NoEntityNym)?;
			EntityNymIndex::<T>::remove(&uname);

			let digest = T::Hashing::hash(&(&token, &uname, b"EntityNymRemoved" as &[u8]).encode());

			Self::record_activity(&token, digest, b"EntityNymRemoved")?;
			Self::deposit_event(Event::EntityNymRemoved { token });
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: Clone + Into<AccountId32>,
	{
		/// Return the entity info
		pub fn details(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<EntityInfoView, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			let info = EntityInfoOf::<T>::get(&token).ok_or(AuthorizationError::NotFound)?;
			Ok(Self::entity_info_view(&info))
		}

		/// Resolve the entity token bound to the supplied account.
		pub fn account_token(
			auth: AuthorizationOf<T>,
			account: T::AccountId,
		) -> Result<Ss58Identifier, AuthorizationError> {
			Self::authorize_account_lookup(&auth, &account)?;
			let token =
				EntityTokenOfAccount::<T>::get(&account).ok_or(AuthorizationError::NotFound)?;
			ensure!(EntityInfoOf::<T>::contains_key(&token), AuthorizationError::NotFound);
			Ok(token)
		}

		/// All linked accounts for the supplied entity token.
		pub fn linked_accounts(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<Vec<T::AccountId>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			Ok(LinkedAccounts::<T>::get(&token).into_inner())
		}

		/// Controller account for the supplied entity token.
		pub fn controller_account(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<T::AccountId, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			ControllerAccountOf::<T>::get(&token).ok_or(AuthorizationError::NotFound)
		}

		/// Historical unbind events (account + block) for this token.
		pub fn account_history(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<Vec<AccountUnbindEntryView<T::AccountId>>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			Ok(AccountUnbindHistory::<T>::iter_prefix(&token)
				.map(|(account, block)| AccountUnbindEntryView {
					account,
					block: EventBlockView { height: block.height, index: block.index },
				})
				.collect())
		}

		/// Entity nym (e.g. `foo.nym.org.in`) as raw bytes.
		pub fn entity_nym(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<Vec<u8>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			EntityNymOf::<T>::get(&token)
				.map(|name| name.into_inner())
				.ok_or(AuthorizationError::NotFound)
		}

		/// Composite overview: info, nym, linked accounts, and recent attribute history.
		pub fn overview(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			history_limit: Option<u32>,
		) -> Result<EntityStateView<T::AccountId>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			let info = EntityInfoOf::<T>::get(&token).ok_or(AuthorizationError::NotFound)?;
			let entity_info_view = Self::entity_info_view(&info);

			let nym = EntityNymOf::<T>::get(&token).map(|n| n.into_inner());
			let linked_accounts = LinkedAccounts::<T>::get(&token);

			let cap = T::MaxEntityOverviewHistory::get();
			let def = T::DefaultEntityOverviewHistory::get();
			let hist_len = history_limit.unwrap_or(def).max(1).min(cap) as usize;

			let mut history: Vec<AttributeHistoryEntryView> =
				crate::Pallet::<T>::attribute_history_plain(&token)
					.into_iter()
					.map(|(key, version, old, block)| AttributeHistoryEntryView {
						key,
						version,
						old_value: old,
						block: EventBlockView { height: block.height, index: block.index },
					})
					.collect();

			history.sort_by(|a, b| {
				(b.block.height, b.block.index).cmp(&(a.block.height, a.block.index))
			});
			history.truncate(hist_len);

			Ok(EntityStateView {
				info: entity_info_view,
				nym,
				linked_accounts: linked_accounts.into_inner(),
				history,
			})
		}

		/// Current version counter for a specific attribute key.
		pub fn attribute_version(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			key: Attribute,
		) -> Result<u64, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			Ok(AttributeVersionOf::<T>::get(&token, key))
		}

		/// All attribute versions as (key, version) pairs.
		pub fn attribute_versions(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<Vec<(Vec<u8>, u64)>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			let entries = AttributeVersionOf::<T>::iter_prefix(&token)
				.map(|(attribute, version)| (attribute.into_inner(), version))
				.collect();
			Ok(entries)
		}

		/// Full attribute history across all keys.
		pub fn attribute_history(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
		) -> Result<Vec<AttributeHistoryEntryView>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;

			let mut rows: Vec<AttributeHistoryEntryView> =
				crate::Pallet::<T>::attribute_history_plain(&token)
					.into_iter()
					.map(|(key, version, old, block)| AttributeHistoryEntryView {
						key,
						version,
						old_value: old,
						block: EventBlockView { height: block.height, index: block.index },
					})
					.collect();

			rows.sort_by(|a, b| {
				(b.block.height, b.block.index).cmp(&(a.block.height, a.block.index))
			});
			Ok(rows)
		}

		/// History for a single attribute key.
		pub fn attribute_history_for_key(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			key: Attribute,
		) -> Result<Vec<AttributeHistoryEntryView>, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			let key_bytes = key.into_inner();

			let mut rows: Vec<AttributeHistoryEntryView> =
				crate::Pallet::<T>::attribute_history_for_key_plain(&token, &key_bytes)
					.into_iter()
					.map(|(version, old, block)| AttributeHistoryEntryView {
						key: key_bytes.clone(),
						version,
						old_value: old,
						block: EventBlockView { height: block.height, index: block.index },
					})
					.collect();

			rows.sort_by(|a, b| {
				(b.block.height, b.block.index).cmp(&(a.block.height, a.block.index))
			});
			Ok(rows)
		}

		/// Single history entry for a specific key + version.
		pub fn attribute_history_entry(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			key: Attribute,
			version: u64,
		) -> Result<AttributeHistoryEntryView, AuthorizationError> {
			Self::authorize_account_query(&auth)?;
			let key_bytes = key.into_inner();

			let (old, block) =
				crate::Pallet::<T>::attribute_history_entry_plain(&token, &key_bytes, version)
					.ok_or(AuthorizationError::NotFound)?;

			Ok(AttributeHistoryEntryView {
				key: key_bytes,
				version,
				old_value: old,
				block: EventBlockView { height: block.height, index: block.index },
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Returns `true` if the supplied origin is signed by an approved feeless account.
	pub fn is_origin_feeless(origin: &OriginFor<T>) -> bool {
		origin.caller().as_signed().map(T::Feeless::is_feeless).unwrap_or(false)
	}

	/// Raw attribute history entries for all keys.
	pub fn attribute_history_plain(
		token: &Ss58Identifier,
	) -> Vec<(Vec<u8>, u64, Vec<u8>, EventBlock)> {
		AttributeHistoryOf::<T>::iter_prefix(token)
			.map(|((key, version), (old, block))| {
				(key.to_vec(), version, old.as_ref().to_vec(), block)
			})
			.collect()
	}

	/// Raw attribute history entries for a specific key.
	pub fn attribute_history_for_key_plain(
		token: &Ss58Identifier,
		key: &[u8],
	) -> Vec<(u64, Vec<u8>, EventBlock)> {
		let key_attr: Attribute = match key.to_vec().try_into() {
			Ok(attr) => attr,
			Err(_) => return Vec::new(),
		};
		AttributeHistoryOf::<T>::iter_prefix(token)
			.filter_map(|((k, version), (old, block))| {
				if k == key_attr {
					Some((version, old.as_ref().to_vec(), block))
				} else {
					None
				}
			})
			.collect()
	}

	/// Raw attribute history entry for a specific key + version.
	pub fn attribute_history_entry_plain(
		token: &Ss58Identifier,
		key: &[u8],
		version: u64,
	) -> Option<(Vec<u8>, EventBlock)> {
		let key_attr: Attribute = key.to_vec().try_into().ok()?;
		AttributeHistoryOf::<T>::get(token, (key_attr, version))
			.map(|(old, block)| (old.as_ref().to_vec(), block))
	}

	/// Flatten `EntityInfoPacket` into the pallet-local view.
	fn entity_info_view(info: &T::EntityInfoPacket) -> EntityInfoView {
		let attributes: Option<Vec<AttributeValueView>> = info
			.attributes()
			.map(|attrs| attrs.iter().map(AttributeValueView::from).collect());

		EntityInfoView {
			display: ElementView::from(&info.get_key(b"display")),
			web: ElementView::from(&info.get_key(b"web")),
			email: ElementView::from(&info.get_key(b"email")),
			attributes,
		}
	}

	fn ensure_authorization_valid(payload: &[u8]) -> Result<(), AuthorizationError> {
		let issued_at = extract_valid_until(payload).ok_or(AuthorizationError::InvalidInput)?;
		let now: u32 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
		let ttl = T::MaxAuthorizationTTL::get();
		ensure_authorization_ttl(now, issued_at, ttl)
	}

	fn check_authorization_signature(
		auth: &AuthorizationOf<T>,
		expected: &T::AccountId,
	) -> Result<(), AuthorizationError>
	where
		T::AccountId: Clone + Into<AccountId32>,
	{
		// TTL check
		Self::ensure_authorization_valid(auth.payload.as_slice())?;

		// Correct account?
		if &auth.account != expected {
			return Err(AuthorizationError::InvalidInput);
		}
		let signer: AccountId32 = auth.account.clone().into();

		if !auth.signature.verify(auth.payload.as_slice(), &signer) {
			return Err(AuthorizationError::Unauthorized);
		}

		Ok(())
	}

	fn authorize_account_query(auth: &AuthorizationOf<T>) -> Result<(), AuthorizationError>
	where
		T::AccountId: Clone + Into<AccountId32>,
	{
		Self::check_authorization_signature(auth, &auth.account)
	}

	fn authorize_account_lookup(
		auth: &AuthorizationOf<T>,
		account: &T::AccountId,
	) -> Result<(), AuthorizationError>
	where
		T::AccountId: Clone + Into<AccountId32>,
	{
		Self::check_authorization_signature(auth, account)
	}

	fn do_set_linked_account(token: &Ss58Identifier, account: &T::AccountId) -> DispatchResult {
		ensure!(
			!EntityTokenOfAccount::<T>::contains_key(account),
			Error::<T>::LinkedAccountAlreadyClaimed
		);
		LinkedAccounts::<T>::try_mutate(token, |list| {
			ensure!(
				list.len() < T::MaxLinkedAccounts::get() as usize,
				Error::<T>::TooManyLinkedAccounts
			);
			list.try_push(account.clone()).map_err(|_| Error::<T>::TooManyLinkedAccounts)
		})?;
		EntityTokenOfAccount::<T>::insert(account, token.clone());
		Ok(())
	}

	fn ensure_account_linked(token: &Ss58Identifier, account: &T::AccountId) -> DispatchResult {
		let already_linked = LinkedAccounts::<T>::get(token).iter().any(|acct| acct == account);
		if already_linked {
			let bound =
				EntityTokenOfAccount::<T>::get(account).ok_or(Error::<T>::LinkedAccountNotFound)?;
			ensure!(bound == *token, Error::<T>::LinkedAccountNotLinked);
			Ok(())
		} else {
			Self::do_set_linked_account(token, account)
		}
	}

	// Revoke linked-account helper
	fn do_revoke_linked_account(token: &Ss58Identifier, account: &T::AccountId) -> DispatchResult {
		let controller = ControllerAccountOf::<T>::get(token).ok_or(Error::<T>::TokenNotFound)?;
		ensure!(controller != *account, Error::<T>::ControllerAccount);
		let bound =
			EntityTokenOfAccount::<T>::get(account).ok_or(Error::<T>::LinkedAccountNotFound)?;
		ensure!(bound == *token, Error::<T>::LinkedAccountNotLinked);
		EntityTokenOfAccount::<T>::remove(account);
		let now = EventBlock::current::<T>();
		AccountUnbindHistory::<T>::insert(token, account, now);
		LinkedAccounts::<T>::try_mutate(token, |list| {
			if let Some(pos) = list.iter().position(|x| x == account) {
				list.swap_remove(pos);
				Ok(())
			} else {
				Err(Error::<T>::LinkedAccountNotLinked)
			}
		})?;
		let digest =
			T::Hashing::hash(&(token, account, b"EntityLinkedAccountRevoked" as &[u8]).encode());
		Self::record_activity(token, digest, b"EntityLinkedAccountRevoked")?;
		Ok(())
	}

	// Clear Identitiy Helper
	fn do_clear_everything(token: &Ss58Identifier) -> DispatchResult {
		let now = EventBlock::current::<T>();
		for sub in LinkedAccounts::<T>::take(token).into_iter() {
			EntityTokenOfAccount::<T>::remove(&sub);
			AccountUnbindHistory::<T>::insert(token, &sub, now.clone());
		}
		if let Some(ctrl) = ControllerAccountOf::<T>::take(token) {
			if EntityTokenOfAccount::<T>::take(&ctrl).is_some() {
				AccountUnbindHistory::<T>::insert(token, &ctrl, now.clone());
			}
		}
		EntityInfoOf::<T>::remove(token);
		if let Some(uname) = EntityNymOf::<T>::take(token) {
			EntityNymIndex::<T>::remove(&uname);
		}
		let digest = T::Hashing::hash(&(token, b"EntityInfoCleared" as &[u8]).encode());
		Self::record_activity(token, digest, b"EntityInfoCleared")?;
		Ok(())
	}

	/// Get the current Ss58 ID of `who`, or an error if none.
	pub fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Error<T>> {
		EntityTokenOfAccount::<T>::get(who).ok_or(Error::<T>::AccountNotFound)
	}

	fn do_rotate_attribute(
		token: &Ss58Identifier,
		attr: &Attribute,
		val: &DataOf<T>,
	) -> DispatchResult {
		let key_vec: Vec<u8> = attr.clone().into_inner();
		EntityInfoOf::<T>::try_mutate(token, |maybe_info| -> DispatchResult {
			let info = maybe_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
			let old_val = info.get_key(&key_vec);
			info.apply_update(&PacketUpdateOp::UpdateAttribute(attr.clone(), val.clone()))
				.map_err(|_| Error::<T>::AttributeNotFound)?;
			let ver = AttributeVersionOf::<T>::get(token, attr).saturating_add(1);
			AttributeVersionOf::<T>::insert(token, attr, ver);
			AttributeHistoryOf::<T>::insert(
				token,
				(attr.clone(), ver),
				(old_val, EventBlock::current::<T>()),
			);
			Ok(())
		})?;
		let digest =
			T::Hashing::hash(&(&token, &key_vec, val, b"EntityAttributeRotated" as &[u8]).encode());
		Self::record_activity(token, digest, b"EntityAttributeRotated")?;
		Ok(())
	}

	pub fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Error<T>> {
		ControllerAccountOf::<T>::get(token).ok_or(Error::<T>::TokenNotFound)
	}

	pub fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		AccountUnbindHistory::<T>::iter_prefix(token).collect()
	}

	/// Check if `who` has _all_ of the requested `fields` in their on-chain identity.
	pub fn has_info_fields(
		who: &T::AccountId,
		mask: <T::EntityInfoPacket as PacketInformationProvider>::FieldMask,
	) -> bool {
		EntityTokenOfAccount::<T>::get(who)
			.and_then(|token| EntityInfoOf::<T>::get(&token))
			.map_or(false, |info| info.has_info_fields(mask))
	}

	fn is_valid_entity_nym_prefix(input: &[u8]) -> bool {
		let max_len = T::MaxEntityNymLength::get() as usize;
		let suffix_len = ENTITY_NYM_SUFFIX.len();
		if input.is_empty() || input.len() + suffix_len > max_len {
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
	pub fn resolve_controller_account(
		token: &Ss58Identifier,
	) -> Result<T::AccountId, SignatureVerificationError> {
		ControllerAccountOf::<T>::get(token)
			.ok_or(SignatureVerificationError::SignerInformationNotPresent)
	}

	/// Verify a payload using the supplied signature strategy against the controller of `token`.
	/// Verify a multi-signature against the specified `account` and return its entity identifier.
	pub fn verify_account_signature(
		account: &T::AccountId,
		payload: &[u8],
		signature: &Signature,
	) -> Result<Ss58Identifier, SignatureVerificationError>
	where
		T::AccountId: Clone + Into<sp_runtime::AccountId32>,
	{
		let token = EntityTokenOfAccount::<T>::get(account)
			.ok_or(SignatureVerificationError::SignerInformationNotPresent)?;
		verify_multisignature(account, payload, signature)?;
		Ok(token)
	}
}

/// A thin API for entity lookups, so that *any* pallet can call:
pub trait EntityLookup<T: frame_system::Config> {
	/// The error returned by the fallible lookups.
	type Error;
	/// The entity nym type (e.g. `EntityNym<T>`).
	type EntityNym;

	/// Get the current Ss58 ID of `who`, or Err if none.
	fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error>;

	/// Get the controller account of `token`, or Err if none.
	fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Self::Error>;

	/// Get the full history for `token` as `(AccountId, EventBlock)`.
	fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)>;

	/// Fetch the (optional) entity nym attached to an entity token.
	fn lookup_nym_of_identifier(token: &Ss58Identifier) -> Option<Self::EntityNym>;

	/// Reverse lookup: given a nym, get the attached entity token (if any).
	fn lookup_identifier_of_nym(name: &Self::EntityNym) -> Option<Ss58Identifier>;

	/// Verify that `signature` was produced by `account` over `payload`, returning its entity
	/// token.
	fn verify_account_signature(
		account: &T::AccountId,
		payload: &[u8],
		signature: &Signature,
	) -> Result<Ss58Identifier, SignatureVerificationError>
	where
		T::AccountId: Clone + Into<sp_runtime::AccountId32>;
}

impl<T: Config> EntityLookup<T> for Pallet<T> {
	type Error = Error<T>;
	type EntityNym = EntityNym<T>;

	fn lookup_token_of(who: &T::AccountId) -> Result<Ss58Identifier, Self::Error> {
		Pallet::<T>::lookup_token_of(who)
	}

	fn lookup_controller_of(token: &Ss58Identifier) -> Result<T::AccountId, Self::Error> {
		Pallet::<T>::lookup_controller_of(token)
	}

	fn lookup_history(token: &Ss58Identifier) -> Vec<(T::AccountId, EventBlock)> {
		Pallet::<T>::lookup_history(token)
	}

	fn lookup_nym_of_identifier(token: &Ss58Identifier) -> Option<EntityNym<T>> {
		EntityNymOf::<T>::get(token)
	}

	fn lookup_identifier_of_nym(name: &EntityNym<T>) -> Option<Ss58Identifier> {
		EntityNymIndex::<T>::get(name)
	}

	fn verify_account_signature(
		account: &T::AccountId,
		payload: &[u8],
		signature: &Signature,
	) -> Result<Ss58Identifier, SignatureVerificationError>
	where
		T::AccountId: Clone + Into<sp_runtime::AccountId32>,
	{
		Self::verify_account_signature(account, payload, signature)
	}
}

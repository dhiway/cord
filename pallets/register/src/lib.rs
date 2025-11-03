// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![warn(unused_crate_dependencies)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod register;
pub mod weights;

extern crate alloc;
use alloc::vec::Vec;
use codec::Encode;
use cord_primitives::{
	identifier::Ss58Identifier,
	packet::{Attribute, Element, PacketUpdateError},
};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, PalletInfoAccess, StorageVersion},
	BoundedVec,
};
use frame_system::{ensure_root, pallet_prelude::*};
pub use pallet::*;
use pallet_entity::EntityLookup;
use pallet_token::{EventBlock, EventTypeOf, Token};
use register::{LookupSpec, RegistryFieldError, RegistryInfo, RegistryKind, RegistryPermissions};
use sp_runtime::traits::Hash;
pub use weights::WeightInfo;

/// Convenience type aliases bound to pallet Config.
pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
pub type ExtraAttributeListOf<T> = BoundedVec<
	(Attribute, Element<<T as Config>::MaxRawDataLength>),
	<T as Config>::MaxAdditionalAttributes,
>;
pub type TokenSpecOf<T> = LookupSpec<<T as Config>::MaxAdditionalAttributes>;
pub type LookupSpecListOf<T> = BoundedVec<
	LookupSpec<<T as Config>::MaxAdditionalAttributes>,
	<T as Config>::MaxAdditionalAttributes,
>;

/// Registry packet type alias for storage.
pub type RegistryPacketOf<T> =
	RegistryInfo<<T as Config>::MaxRawDataLength, <T as Config>::MaxAdditionalAttributes>;

pub trait RegistryInspector<T: Config> {
	fn registry_packet(token: &Ss58Identifier) -> Option<RegistryPacketOf<T>>;
	fn attribute_keys(token: &Ss58Identifier) -> Option<Vec<Vec<u8>>>;
	/// Keys (in order) that are hashed to derive the registry token.
	fn token_fields(token: &Ss58Identifier) -> Option<Vec<Vec<u8>>>;
	/// Lookup specifications expressed as lists of attribute keys (single key => len 1).
	fn lookup_specs(token: &Ss58Identifier) -> Option<Vec<Vec<Vec<u8>>>>;
	fn has_permissions(
		token: &Ss58Identifier,
		delegate: &Ss58Identifier,
		required: RegistryPermissions,
	) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Token trait to derive registry ids and post state-events.
		type Token: Token<Self, Hash = Self::Hash>;

		/// Resolve an account to the entity’s Ss58Identifier
		type EntityLookup: EntityLookup<Self>;

		/// Max size for info blob and each attribute.
		#[pallet::constant]
		type MaxRawDataLength: Get<u32>;

		/// Max number of additional attributes.
		#[pallet::constant]
		type MaxAdditionalAttributes: Get<u32>;

		/// Weight instrumentation.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_); // unit struct

	/// Primary storage mapping registry token -> packet.
	#[pallet::storage]
	pub type Registries<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, RegistryPacketOf<T>, OptionQuery>;

	/// Per-registry delegate permissions.
	#[pallet::storage]
	pub type RegistryDelegates<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		Ss58Identifier,
		RegistryPermissions,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry was created.
		RegistryCreated { registry: Ss58Identifier, maintainer: Ss58Identifier },
		/// The info blob was updated.
		RegistryInfoUpdated { registry: Ss58Identifier, who: Ss58Identifier },
		/// The registry status flag changed.
		RegistryStatusChanged {
			registry: Ss58Identifier,
			who: Option<Ss58Identifier>,
			is_active: bool,
		},
		/// A delegate with permissions was set/updated.
		RegistryDelegateSet {
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
			permissions: RegistryPermissions,
		},
		/// A delegate was removed.
		RegistryDelegateRemoved { registry: Ss58Identifier, delegate: Ss58Identifier },
	}

	#[pallet::error]
	pub enum Error<T> {
		RegistryAlreadyExists,
		RegistryNotFound,
		TokenNotFound,
		TokenCreationFailed,
		PermissionDenied,
		AttributeExists,
		TooManyAttributes,
		AttributeNotFound,
		InvalidAttributeKey,
		DuplicateTokenField,
		UnknownTokenField,
		DuplicateLookupField,
		UnknownLookupField,
		NoAttributes,
		NoTokenFields,
		CannotRemoveMaintainer,
		InvalidEventType,
		StateUpdateFailed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new registry with:
		/// - info blob
		/// - attributes (KV pairs)
		/// - token_spec (subset or combination of attributes used in token-material digest)
		/// - lookup_specs (subset of attribute-key combinations intended as index keys; may be
		///   empty)
		#[pallet::call_index(0)]
		#[pallet::weight({
		let info_size = info.using_encoded(|d| d.len() as u32);
		let attr_size = attributes.iter().fold(0u32, |acc, (k, v)| {
			acc + k.len() as u32 + v.using_encoded(|d| d.len() as u32)
		});
		let token_key_size = token_spec.total_key_bytes() as u32;
		let lookup_key_size = lookup_specs
			.iter()
			.fold(0u32, |acc, spec| acc + spec.total_key_bytes() as u32);
		T::WeightInfo::create_registry(info_size + attr_size + token_key_size + lookup_key_size)
	})]
		pub fn create_registry(
			origin: OriginFor<T>,
			info: DataOf<T>,
			kind: RegistryKind,
			is_active: bool,
			attributes: ExtraAttributeListOf<T>,
			token_spec: TokenSpecOf<T>,
			lookup_specs: LookupSpecListOf<T>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let maintainer = Self::resolve_account_token(&signer)?;
			ensure!(!attributes.is_empty(), Error::<T>::NoAttributes);
			ensure!(token_spec.key_count() > 0, Error::<T>::NoTokenFields);

			let mut packet = RegistryPacketOf::<T> {
				info: Element::default(),
				maintainer: maintainer.clone(),
				attributes: BoundedVec::default(),
				token_spec: LookupSpec::Combo(BoundedVec::<
					Attribute,
					<T as Config>::MaxAdditionalAttributes,
				>::default()),
				lookup_specs: BoundedVec::default(),
				kind: RegistryKind::Raw,
				is_active: true,
			};

			packet.set_info(info);
			packet.set_kind(kind.clone());
			packet.set_status(is_active);
			packet.set_maintainer(maintainer.clone());
			packet.set_attributes(attributes.clone()).map_err(Self::map_attribute_error)?;
			packet.set_token_spec(token_spec.clone()).map_err(Self::map_token_field_error)?;
			packet
				.set_lookup_specs(lookup_specs.clone())
				.map_err(Self::map_token_field_error_for_lookups)?;

			let digest_material = packet.resolve_token_material();
			let digest = T::Hashing::hash(&(digest_material, kind, is_active).encode());
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let registry_token = T::Token::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::TokenCreationFailed)?;

			Registries::<T>::try_mutate_exists(&registry_token, |slot| -> DispatchResult {
				ensure!(slot.is_none(), Error::<T>::RegistryAlreadyExists);
				*slot = Some(packet);
				Ok(())
			})?;

			RegistryDelegates::<T>::insert(
				&registry_token,
				&maintainer,
				RegistryPermissions::ADMIN.with_implied_view(),
			);
			Self::record_activity(&registry_token, digest, b"RegistryCreated")?;

			Self::deposit_event(Event::RegistryCreated {
				registry: registry_token.clone(),
				maintainer,
			});
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_registry_delegate(roles.len() as u32))]
		pub fn set_registry_delegate(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			delegate: T::AccountId,
			roles: Vec<RegistryPermissions>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_account_token(&signer)?;
			Self::ensure_can_delegate(&registry, &actor)?;

			let delegate_token = Self::resolve_account_token(&delegate)?;
			let perms = RegistryPermissions::from_list(&roles);
			RegistryDelegates::<T>::insert(&registry, &delegate_token, perms);

			let digest = T::Hashing::hash(
				&(&registry, &delegate_token, &perms, b"RegistryDelegateSet" as &[u8]).encode(),
			);
			Self::record_activity(&registry, digest, b"RegistryDelegateSet")?;
			Self::deposit_event(Event::RegistryDelegateSet {
				registry,
				delegate: delegate_token,
				permissions: perms,
			});
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::remove_registry_delegate())]
		pub fn remove_registry_delegate(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_account_token(&signer)?;
			Self::ensure_can_delegate(&registry, &actor)?;

			let packet = Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			if packet.maintainer() == &delegate {
				return Err(Error::<T>::CannotRemoveMaintainer.into());
			}

			RegistryDelegates::<T>::take(&registry, &delegate)
				.ok_or(Error::<T>::PermissionDenied)?;

			let digest = T::Hashing::hash(
				&(&registry, &delegate, b"RegistryDelegateRemoved" as &[u8]).encode(),
			);
			Self::record_activity(&registry, digest, b"RegistryDelegateRemoved")?;
			Self::deposit_event(Event::RegistryDelegateRemoved { registry, delegate });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::update_registry_info(info.using_encoded(|d| d.len() as u32)))]
		pub fn update_registry_info(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			info: DataOf<T>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_account_token(&signer)?;
			Self::ensure_admin(&registry, &actor)?;

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let packet = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				packet.set_info(info.clone());
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&registry, &info, b"RegistryInfoUpdated" as &[u8]).encode());
			Self::record_activity(&registry, digest, b"RegistryInfoUpdated")?;
			Self::deposit_event(Event::RegistryInfoUpdated { registry, who: actor });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_registry_status())]
		pub fn set_registry_status(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			is_active: bool,
		) -> DispatchResult {
			let actor_token = if ensure_root(origin.clone()).is_ok() {
				None
			} else {
				let signer = ensure_signed(origin)?;
				let actor = Self::resolve_account_token(&signer)?;
				Self::ensure_admin(&registry, &actor)?;
				Some(actor)
			};

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let packet = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				packet.set_status(is_active);
				Ok(())
			})?;

			let digest = T::Hashing::hash(
				&(&registry, is_active, b"RegistryStatusChanged" as &[u8]).encode(),
			);
			Self::record_activity(&registry, digest, b"RegistryStatusChanged")?;
			Self::deposit_event(Event::RegistryStatusChanged {
				registry,
				who: actor_token,
				is_active,
			});
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		/// Returns the full registry packet if `viewer` has view rights.
		pub fn view_registry(
			viewer: Ss58Identifier,
			registry: Ss58Identifier,
		) -> Option<RegistryPacketOf<T>> {
			let packet = Registries::<T>::get(&registry)?;
			if Self::has_view_rights(&registry, &packet, &viewer) {
				Some(packet)
			} else {
				None
			}
		}

		/// Returns the value of a specific attribute if `viewer` has view rights.
		pub fn view_registry_attribute(
			viewer: Ss58Identifier,
			registry: Ss58Identifier,
			key: Vec<u8>,
		) -> Option<Element<<T as Config>::MaxRawDataLength>> {
			let key_bounded: Attribute = key.try_into().ok()?;
			let packet = Registries::<T>::get(&registry)?;
			if !Self::has_view_rights(&registry, &packet, &viewer) {
				return None;
			}

			packet
				.attributes
				.iter()
				.find(|(attr, _)| attr == &key_bounded)
				.map(|(_, value)| value.clone())
		}
	}

	impl<T: Config> Pallet<T> {
		fn resolve_account_token(account: &T::AccountId) -> Result<Ss58Identifier, DispatchError> {
			T::EntityLookup::lookup_token_of(account).map_err(|_| Error::<T>::TokenNotFound.into())
		}

		fn ensure_admin(registry: &Ss58Identifier, actor: &Ss58Identifier) -> DispatchResult {
			let packet = Registries::<T>::get(registry).ok_or(Error::<T>::RegistryNotFound)?;
			if packet.maintainer() == actor {
				return Ok(());
			}
			let perms =
				RegistryDelegates::<T>::get(registry, actor).ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perms.has_admin(), Error::<T>::PermissionDenied);
			Ok(())
		}

		fn ensure_can_delegate(
			registry: &Ss58Identifier,
			actor: &Ss58Identifier,
		) -> DispatchResult {
			let packet = Registries::<T>::get(registry).ok_or(Error::<T>::RegistryNotFound)?;
			if packet.maintainer() == actor {
				return Ok(());
			}
			let perms =
				RegistryDelegates::<T>::get(registry, actor).ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perms.has_delegate(), Error::<T>::PermissionDenied);
			Ok(())
		}

		fn has_view_rights(
			registry: &Ss58Identifier,
			packet: &RegistryPacketOf<T>,
			viewer: &Ss58Identifier,
		) -> bool {
			if packet.maintainer() == viewer {
				return true;
			}

			RegistryDelegates::<T>::get(registry, viewer).map_or(false, |perms| perms.has_view())
		}

		fn map_attribute_error(err: PacketUpdateError) -> DispatchError {
			match err {
				PacketUpdateError::AttributeExists => Error::<T>::AttributeExists.into(),
				PacketUpdateError::TooManyAttributes => Error::<T>::TooManyAttributes.into(),
				PacketUpdateError::AttributeNotFound => Error::<T>::AttributeNotFound.into(),
				PacketUpdateError::ReservedAttribute => Error::<T>::InvalidAttributeKey.into(),
				PacketUpdateError::InvalidIdentifier => Error::<T>::InvalidAttributeKey.into(),
				PacketUpdateError::InvalidElement => Error::<T>::InvalidAttributeKey.into(),
			}
		}

		fn map_token_field_error(err: RegistryFieldError) -> DispatchError {
			match err {
				RegistryFieldError::DuplicateKey => Error::<T>::DuplicateTokenField.into(),
				RegistryFieldError::UnknownKey => Error::<T>::UnknownTokenField.into(),
				RegistryFieldError::EmptySpec => Error::<T>::NoTokenFields.into(),
				RegistryFieldError::DuplicateSpec => Error::<T>::DuplicateTokenField.into(),
			}
		}

		fn map_token_field_error_for_lookups(err: RegistryFieldError) -> DispatchError {
			match err {
				RegistryFieldError::DuplicateKey => Error::<T>::DuplicateLookupField.into(),
				RegistryFieldError::UnknownKey => Error::<T>::UnknownLookupField.into(),
				RegistryFieldError::EmptySpec => Error::<T>::UnknownLookupField.into(),
				RegistryFieldError::DuplicateSpec => Error::<T>::DuplicateLookupField.into(),
			}
		}

		fn record_activity(token: &Ss58Identifier, digest: T::Hash, msg: &[u8]) -> DispatchResult {
			let action: EventTypeOf =
				msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
			let stamp = EventBlock::current::<T>();
			T::Token::state_event(token, digest, action, stamp)
				.map_err(|_| Error::<T>::StateUpdateFailed)?;
			Ok(())
		}
	}
}

impl<T: Config> RegistryInspector<T> for Pallet<T> {
	fn registry_packet(token: &Ss58Identifier) -> Option<RegistryPacketOf<T>> {
		Registries::<T>::get(token)
	}

	fn attribute_keys(token: &Ss58Identifier) -> Option<Vec<Vec<u8>>> {
		Registries::<T>::get(token).map(|packet| packet.attribute_keys())
	}

	fn token_fields(token: &Ss58Identifier) -> Option<Vec<Vec<u8>>> {
		Registries::<T>::get(token).map(|packet| {
			packet.token_spec.cloned_keys().into_iter().map(|key| key.to_vec()).collect()
		})
	}

	fn lookup_specs(token: &Ss58Identifier) -> Option<Vec<Vec<Vec<u8>>>> {
		Registries::<T>::get(token).map(|packet| {
			packet
				.lookup_specs
				.iter()
				.map(|spec| spec.cloned_keys().into_iter().map(|key| key.to_vec()).collect())
				.collect()
		})
	}

	fn has_permissions(
		token: &Ss58Identifier,
		delegate: &Ss58Identifier,
		required: RegistryPermissions,
	) -> bool {
		if let Some(packet) = Registries::<T>::get(token) {
			if packet.maintainer() == delegate {
				return true;
			}
		}
		RegistryDelegates::<T>::get(token, delegate).map_or(false, |perms| match required {
			RegistryPermissions::ADMIN => perms.has_admin(),
			RegistryPermissions::DELEGATE => perms.has_delegate(),
			RegistryPermissions::ENTRY => perms.has_entry(),
			RegistryPermissions::VIEW => perms.has_view(),
			_ => perms.contains(required),
		})
	}
}

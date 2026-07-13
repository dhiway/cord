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

pub mod packet;
pub mod register;
pub mod view;
pub mod weights;

extern crate alloc;
use alloc::{collections::BTreeMap, vec, vec::Vec};
use codec::Encode;
use core::convert::TryInto;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{CallerTrait, Get, PalletInfoAccess, StorageVersion},
	BoundedVec,
};
use frame_system::{ensure_root, pallet_prelude::*};
use log as _;
use origin_primitives::{
	attribute::{Attribute, Element, ElementType},
	authorization::{
		ensure_authorization_ttl, extract_valid_until, Authorization as ViewAuthorization,
		AuthorizationError,
	},
	identifier::Ss58Identifier,
	packet::{PacketPointer, PacketStateView, PacketStatus, PacketUpdateError},
	registry::{
		LookupSpec as LookupSpecView, RegistryAttributeView, RegistryKind, RegistryPermissions,
		RegistryStateView, RegistryStatus,
	},
	Signature,
};
pub use packet::{
	attributes_digest, AttributePairsOf, LookupDigestOf, PacketAttributesOf, PacketDataOf,
	PacketMetadataOf, PacketSnapshotOf, PacketStateOf,
};
use register::{AttributeFlags, AttributeSpec, LookupSpec, RegistryFieldError, RegistryInfo};

pub use pallet::*;
use pallet_entity::EntityLookup;
use pallet_feeless::FeelessAccounts;
use pallet_token::{EventBlock, EventTypeOf, Token};
use sp_io as _;
use sp_runtime::traits::{Hash, UniqueSaturatedInto};
pub use weights::WeightInfo;

/// Convenience type aliases bound to pallet Config.
pub type DataOf<T> = Element<<T as Config>::MaxRawDataLength>;
pub type AttributeSchemaListOf<T> =
	BoundedVec<AttributeSpec, <T as Config>::MaxAdditionalAttributes>;
pub type TokenSpecOf<T> = LookupSpec<<T as Config>::MaxAdditionalAttributes>;
pub type LookupSpecListOf<T> = BoundedVec<
	LookupSpec<<T as Config>::MaxAdditionalAttributes>,
	<T as Config>::MaxAdditionalAttributes,
>;

/// Authorization payload supplied for register authorization requests.
pub type AuthorizationPayloadOf<T> = BoundedVec<u8, <T as Config>::MaxAuthorizationLen>;

/// Authorization details that must accompany every query request.
pub type Authorization<T> =
	ViewAuthorization<<T as frame_system::Config>::AccountId, AuthorizationPayloadOf<T>, Signature>;
pub type AuthorizationOf<T> = Authorization<T>;

/// Registry info type alias for storage.
pub type RegistryInfoOf<T> =
	RegistryInfo<<T as Config>::MaxRawDataLength, <T as Config>::MaxAdditionalAttributes>;

pub trait RegistryView<T: Config> {
	fn registry_info(registry_id: &Ss58Identifier) -> Option<RegistryInfoOf<T>>;
	fn attribute_keys(registry_id: &Ss58Identifier) -> Option<Vec<Vec<u8>>>;
	/// Keys (in order) that are hashed to derive the registry identifier token.
	fn token_specs(registry_id: &Ss58Identifier) -> Option<TokenSpecOf<T>>;
	/// Lookup specifications expressed using the canonical schema.
	fn lookup_specs(registry_id: &Ss58Identifier) -> Option<LookupSpecListOf<T>>;
	fn has_permissions(
		registry_id: &Ss58Identifier,
		delegate: &Ss58Identifier,
		required: RegistryPermissions,
	) -> bool;
	fn packet_metadata(packet: &Ss58Identifier) -> Option<PacketMetadataOf<T>>;
	fn packet_state(packet: &Ss58Identifier, version: Option<u32>) -> Option<PacketSnapshotOf<T>>;
	fn lookup_state(
		registry_id: &Ss58Identifier,
		digest: &LookupDigestOf<T>,
		version: Option<u32>,
	) -> Option<PacketSnapshotOf<T>>;
	fn list_by_token(
		token_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Ss58Identifier>,
		limit: u32,
	) -> (Vec<PacketSnapshotOf<T>>, Option<Ss58Identifier>);
	fn packets_by_lookup_digest(
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<LookupDigestOf<T>>,
		limit: u32,
	) -> (Vec<PacketSnapshotOf<T>>, Option<LookupDigestOf<T>>);
	fn registry_active(registry_id: &Ss58Identifier) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

	#[pallet::config]
	pub trait Config:
		frame_system::Config<AccountId: Into<sp_runtime::AccountId32> + Clone>
	{
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

		/// Max length for authorization challenges.
		#[pallet::constant]
		type MaxAuthorizationLen: Get<u32>;

		/// Maximum blocks a view authorization remains valid.
		#[pallet::constant]
		type MaxAuthorizationTTL: Get<u32>;

		/// Maximum number of packet snapshots returned by query functions.
		#[pallet::constant]
		type MaxPacketListResults: Get<u32>;

		/// Source for feeless account determination.
		type Feeless: FeelessAccounts<Self::AccountId>;

		/// Weight instrumentation.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_); // unit struct

	/// Primary storage mapping registry identifier -> registry info.
	#[pallet::storage]
	pub type Registries<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, RegistryInfoOf<T>, OptionQuery>;

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

	/// Packet token → Version → packet state record.
	#[pallet::storage]
	pub type PacketStates<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Twox64Concat,
		u32,
		PacketStateOf<T>,
		OptionQuery,
	>;

	/// Packet token → metadata for latest state.
	#[pallet::storage]
	pub type Packets<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, PacketMetadataOf<T>, OptionQuery>;

	/// Lookup digest → Registry → lookup anchor.
	#[pallet::storage]
	pub type LookupIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		LookupDigestOf<T>,
		Blake2_128Concat,
		Ss58Identifier,
		packet::LookupAnchor,
		OptionQuery,
	>;

	/// Per-account access counter per registry for query functions.
	#[pallet::storage]
	pub type RegistryQueryCounts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		T::AccountId,
		u64,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry was created.
		RegistryCreated { registry: Ss58Identifier, maintainer: Ss58Identifier },
		/// The registry metadata blob was updated.
		RegistryInfoUpdated { registry: Ss58Identifier, who: Ss58Identifier },
		/// The registry status flag changed.
		RegistryStatusChanged {
			registry: Ss58Identifier,
			who: Option<Ss58Identifier>,
			status: RegistryStatus,
		},
		/// A delegate with permissions was set/updated.
		RegistryDelegateSet {
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
			permissions: RegistryPermissions,
		},
		/// A delegate was removed.
		RegistryDelegateRemoved { registry: Ss58Identifier, delegate: Ss58Identifier },
		/// A packet record was created under a registry.
		PacketCreated { registry: Ss58Identifier, packet: Ss58Identifier, delegate: Ss58Identifier },
		/// A packet record was updated, potentially deriving a new packet token.
		PacketUpdated { registry: Ss58Identifier, packet: Ss58Identifier, delegate: Ss58Identifier },
		/// A packet record was revoked.
		PacketRevoked { registry: Ss58Identifier, packet: Ss58Identifier, delegate: Ss58Identifier },
		/// A revoked packet record was restored.
		PacketRestored {
			registry: Ss58Identifier,
			packet: Ss58Identifier,
			delegate: Ss58Identifier,
		},
		/// A packet record was permanently removed.
		PacketRemoved { registry: Ss58Identifier, packet: Ss58Identifier, delegate: Ss58Identifier },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Registry already exists for derived identifier.
		RegistryAlreadyExists,
		/// Registry not found in storage.
		RegistryNotFound,
		/// Registry is not in active status.
		RegistryInactive,
		/// Registry has been deleted.
		RegistryDeleted,
		/// Token (packet) not found.
		TokenNotFound,
		/// Unable to derive token identifier.
		TokenCreationFailed,
		/// Caller lacks required permissions.
		PermissionDenied,
		/// Attribute key already exists.
		AttributeExists,
		/// Duplicate attribute in payload.
		DuplicateAttribute,
		/// Attribute count exceeds limit.
		TooManyAttributes,
		/// Attribute missing from source list.
		AttributeNotFound,
		/// Attribute key violates requirements.
		InvalidAttributeKey,
		/// Duplicate key in token spec.
		DuplicateTokenField,
		/// Token spec references unknown key.
		UnknownTokenField,
		/// Duplicate key in lookup spec.
		DuplicateLookupField,
		/// Lookup spec references unknown key.
		UnknownLookupField,
		/// Registry requires at least one attribute.
		NoAttributes,
		/// Token spec cannot be empty.
		NoTokenFields,
		/// At least one lookup spec required.
		NoLookupSpecs,
		/// Maintainer cannot be removed.
		CannotRemoveMaintainer,
		/// Attribute not declared in schema.
		UnknownAttribute,
		/// Required attribute missing from payload.
		MissingAttribute,
		/// Attribute value has unexpected type.
		InvalidAttributeType,
		/// Packet already exists with given attributes.
		PacketAlreadyExists,
		/// Packet token not found.
		PacketNotFound,
		/// Packet is already revoked.
		PacketRevoked,
		/// Packet is not revoked.
		PacketNotRevoked,
		/// Packet marked as deleted.
		PacketDeleted,
		/// Lookup digest conflicts with existing entry.
		LookupConflict,
		/// Lookup digest not found.
		LookupNotFound,
		/// Event type cannot be encoded.
		InvalidEventType,
		/// Error posting state event.
		StateUpdateFailed,
		/// Registry must be revoked first.
		RegistryNotRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: Clone + Into<sp_runtime::AccountId32>,
	{
		/// Create a new registry.
		#[pallet::call_index(0)]
		#[pallet::weight({
		let info_size = info.using_encoded(|d| d.len() as u32);
		let attr_size = attributes.iter().fold(0u32, |acc, spec| {
			acc + spec.key.len() as u32 + spec.kind.encode().len() as u32
		});
		let token_key_size = token_spec.total_key_bytes() as u32;
		let lookup_key_size =
			lookup_specs.iter().fold(0u32, |acc, spec| acc + spec.total_key_bytes() as u32);
		T::WeightInfo::create_registry(info_size + attr_size + token_key_size + lookup_key_size)
	})]
		#[pallet::feeless_if(
			|origin: &OriginFor<T>,
			 _info: &DataOf<T>,
			 _kind: &RegistryKind,
			 _attributes: &AttributeSchemaListOf<T>,
			 _token_spec: &TokenSpecOf<T>,
			 _lookup_specs: &LookupSpecListOf<T>|
		 -> bool { Pallet::<T>::is_origin_feeless(origin) }
		)]
		pub fn create_registry(
			origin: OriginFor<T>,
			info: DataOf<T>,
			kind: RegistryKind,
			attributes: AttributeSchemaListOf<T>,
			token_spec: TokenSpecOf<T>,
			lookup_specs: LookupSpecListOf<T>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let maintainer = Self::resolve_entity_token(&signer)?;
			ensure!(!attributes.is_empty(), Error::<T>::NoAttributes);
			ensure!(token_spec.key_count() > 0, Error::<T>::NoTokenFields);

			let mut registry_info = RegistryInfoOf::<T> {
				info: Element::default(),
				maintainer: maintainer.clone(),
				attributes: BoundedVec::default(),
				token_spec: LookupSpec::Combo(BoundedVec::<
					Attribute,
					<T as Config>::MaxAdditionalAttributes,
				>::default()),
				lookup_specs: BoundedVec::default(),
				kind: RegistryKind::Raw,
				status: RegistryStatus::Active,
			};

			registry_info.set_info(info);
			registry_info.set_kind(kind.clone());
			registry_info.set_status(RegistryStatus::Active);
			registry_info.set_maintainer(maintainer.clone());
			registry_info
				.set_attributes(attributes.clone())
				.map_err(Self::map_attribute_error)?;
			registry_info
				.set_token_spec(token_spec.clone())
				.map_err(Self::map_token_field_error)?;
			registry_info
				.set_lookup_specs(lookup_specs.clone())
				.map_err(Self::map_token_field_error_for_lookups)?;

			let digest_material = registry_info.resolve_token_material();
			let digest_seed = (registry_info.info.clone(), digest_material).encode();
			let digest = T::Hashing::hash(&digest_seed);
			let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
			let registry = T::Token::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::TokenCreationFailed)?;

			Registries::<T>::try_mutate_exists(&registry, |slot| -> DispatchResult {
				ensure!(slot.is_none(), Error::<T>::RegistryAlreadyExists);
				*slot = Some(registry_info);
				Ok(())
			})?;

			RegistryDelegates::<T>::insert(
				&registry,
				&maintainer,
				RegistryPermissions::ADMIN | RegistryPermissions::VIEW,
			);
			Self::record_activity(&registry, digest, b"RegistryCreated")?;

			Self::deposit_event(Event::RegistryCreated { registry: registry.clone(), maintainer });
			Ok(())
		}

		/// Grant or update delegate permissions for a registry.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_delegate_permissions(roles.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _delegate: &T::AccountId, _roles: &Vec<RegistryPermissions>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn set_delegate_permissions(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			delegate: T::AccountId,
			roles: Vec<RegistryPermissions>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_entity_token(&signer)?;
			Self::ensure_can_delegate(&registry, &actor)?;

			let delegate_token = Self::resolve_entity_token(&delegate)?;
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

		/// Remove delegate permissions from a registry.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::remove_delegate_permissions())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _delegate: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn remove_delegate_permissions(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_entity_token(&signer)?;
			Self::ensure_can_delegate(&registry, &actor)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_info.maintainer() == &delegate {
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

		/// Update the registry metadata blob.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::update_registry_info(info.using_encoded(|d| d.len() as u32)))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _info: &DataOf<T>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn update_registry_info(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			info: DataOf<T>,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;
			let actor = Self::resolve_entity_token(&signer)?;
			Self::ensure_admin(&registry, &actor)?;

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let registry_info = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				registry_info.set_info(info.clone());
				Ok(())
			})?;

			let digest =
				T::Hashing::hash(&(&registry, &info, b"RegistryInfoUpdated" as &[u8]).encode());
			Self::record_activity(&registry, digest, b"RegistryInfoUpdated")?;
			Self::deposit_event(Event::RegistryInfoUpdated { registry, who: actor });
			Ok(())
		}

		/// Mark an active registry as revoked.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::revoke_registry())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn revoke_registry(origin: OriginFor<T>, registry: Ss58Identifier) -> DispatchResult {
			let actor_token = if ensure_root(origin.clone()).is_ok() {
				None
			} else {
				let signer = ensure_signed(origin)?;
				let actor = Self::resolve_entity_token(&signer)?;
				Self::ensure_admin(&registry, &actor)?;
				Some(actor)
			};

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let info = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				ensure!(!info.is_deleted(), Error::<T>::RegistryDeleted);
				ensure!(info.is_active(), Error::<T>::RegistryInactive);
				info.set_status(RegistryStatus::Revoked);
				Ok(())
			})?;

			let digest = T::Hashing::hash(&(&registry, b"RegistryRevoked" as &[u8]).encode());
			Self::record_activity(&registry, digest, b"RegistryRevoked")?;
			Self::deposit_event(Event::RegistryStatusChanged {
				registry,
				who: actor_token,
				status: RegistryStatus::Revoked,
			});
			Ok(())
		}

		/// Reactivate a revoked registry.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::restore_registry())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn restore_registry(origin: OriginFor<T>, registry: Ss58Identifier) -> DispatchResult {
			let actor_token = if ensure_root(origin.clone()).is_ok() {
				None
			} else {
				let signer = ensure_signed(origin)?;
				let actor = Self::resolve_entity_token(&signer)?;
				Self::ensure_admin(&registry, &actor)?;
				Some(actor)
			};

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let info = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				ensure!(!info.is_deleted(), Error::<T>::RegistryDeleted);
				ensure!(info.is_revoked(), Error::<T>::RegistryNotRevoked);
				info.set_status(RegistryStatus::Active);
				Ok(())
			})?;

			let digest = T::Hashing::hash(&(&registry, b"RegistryRestored" as &[u8]).encode());
			Self::record_activity(&registry, digest, b"RegistryRestored")?;
			Self::deposit_event(Event::RegistryStatusChanged {
				registry,
				who: actor_token,
				status: RegistryStatus::Active,
			});
			Ok(())
		}

		/// Permanently delete a revoked registry.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::delete_registry())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn delete_registry(origin: OriginFor<T>, registry: Ss58Identifier) -> DispatchResult {
			let actor_token = if ensure_root(origin.clone()).is_ok() {
				None
			} else {
				let signer = ensure_signed(origin)?;
				let actor = Self::resolve_entity_token(&signer)?;
				Self::ensure_admin(&registry, &actor)?;
				Some(actor)
			};

			Registries::<T>::try_mutate(&registry, |entry| -> DispatchResult {
				let info = entry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
				ensure!(!info.is_deleted(), Error::<T>::RegistryDeleted);
				ensure!(info.is_revoked(), Error::<T>::RegistryNotRevoked);
				info.set_status(RegistryStatus::Deleted);
				Ok(())
			})?;

			let digest = T::Hashing::hash(&(&registry, b"RegistryDeleted" as &[u8]).encode());
			Self::record_activity(&registry, digest, b"RegistryDeleted")?;
			Self::deposit_event(Event::RegistryStatusChanged {
				registry,
				who: actor_token,
				status: RegistryStatus::Deleted,
			});
			Ok(())
		}

		/// Create a packet entry under a registry.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::create_packet(attributes.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _attributes: &AttributePairsOf<T>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn create_packet(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			attributes: AttributePairsOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegate = Self::resolve_entity_token(&who)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_info.is_deleted(), Error::<T>::RegistryDeleted);
			ensure!(registry_info.is_active(), Error::<T>::RegistryInactive);
			packet::ensure_entry_access::<T>(&registry, &registry_info, &delegate)?;

			let packet_attributes = packet::normalise_attributes::<T>(attributes)?;
			packet::ensure_matches_schema::<T>(&registry_info, &packet_attributes)?;

			let packet_id =
				packet::derive_packet_token::<T>(&registry, &registry_info, &packet_attributes)?;
			ensure!(Packets::<T>::get(&packet_id).is_none(), Error::<T>::PacketAlreadyExists);

			let lookup_entries =
				packet::prepare_lookup_keys::<T>(&registry, &registry_info, &packet_attributes)?;

			for (digest, _) in lookup_entries.iter() {
				if let Some(existing) = LookupIndex::<T>::get(digest, &registry) {
					ensure!(existing.pointer.packet == packet_id, Error::<T>::LookupConflict);
					return Err(Error::<T>::PacketAlreadyExists.into());
				}
			}

			let version: u32 = 1;
			let digest = packet::attributes_digest::<T>(&packet_attributes);
			let pointer =
				PacketPointer { registry: registry.clone(), packet: packet_id.clone(), version };
			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Active,
				version,
				digest: digest.clone(),
				attributes: packet_attributes.clone(),
			};

			PacketStates::<T>::insert(&packet_id, version, state);
			Packets::<T>::insert(
				&packet_id,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Active,
					latest_version: version,
					digest: digest.clone(),
				},
			);

			for (digest, spec) in lookup_entries {
				LookupIndex::<T>::insert(
					&digest,
					&registry,
					packet::LookupAnchor { spec, pointer: pointer.clone() },
				);
			}

			packet::record_packet_event::<T>(&packet_id, b"PacketCreated")?;
			Self::deposit_event(Event::PacketCreated { registry, packet: packet_id, delegate });
			Ok(())
		}

		/// Update an existing packet, bumping the version.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::update_packet(attributes.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _packet: &Ss58Identifier, _attributes: &AttributePairsOf<T>| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn update_packet(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
			attributes: AttributePairsOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegate = Self::resolve_entity_token(&who)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_info.is_deleted(), Error::<T>::RegistryDeleted);
			ensure!(registry_info.is_active(), Error::<T>::RegistryInactive);
			packet::ensure_entry_access::<T>(&registry, &registry_info, &delegate)?;

			let metadata = Packets::<T>::get(&packet).ok_or(Error::<T>::TokenNotFound)?;
			ensure!(metadata.registry == registry, Error::<T>::TokenNotFound);
			ensure!(metadata.status != PacketStatus::Deleted, Error::<T>::PacketDeleted);
			ensure!(metadata.status == PacketStatus::Active, Error::<T>::PacketRevoked);

			let current_version = metadata.latest_version;
			let current_state = PacketStates::<T>::get(&packet, current_version)
				.ok_or(Error::<T>::TokenNotFound)?;

			let update_set = packet::normalise_attributes::<T>(attributes)?;
			let mut merged_attributes = current_state.attributes.clone();
			packet::apply_attribute_updates::<T>(&mut merged_attributes, &update_set)?;
			packet::ensure_matches_schema::<T>(&registry_info, &merged_attributes)?;

			Self::mark_latest_version_revoked(&packet, current_version)?;

			let new_version = current_version.saturating_add(1);
			let pointer = PacketPointer {
				registry: registry.clone(),
				packet: packet.clone(),
				version: new_version,
			};
			Self::refresh_lookup_entries(
				&registry,
				&registry_info,
				Some(&current_state.attributes),
				&merged_attributes,
				&pointer,
			)?;

			let new_digest = packet::attributes_digest::<T>(&merged_attributes);
			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Active,
				version: new_version,
				digest: new_digest.clone(),
				attributes: merged_attributes.clone(),
			};

			PacketStates::<T>::insert(&packet, new_version, state);
			Packets::<T>::insert(
				&packet,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Active,
					latest_version: new_version,
					digest: new_digest.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketUpdated")?;
			Self::deposit_event(Event::PacketUpdated { registry, packet, delegate });
			Ok(())
		}

		/// Revoke an active packet.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::revoke_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _packet: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn revoke_packet(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegate = Self::resolve_entity_token(&who)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_info.is_deleted(), Error::<T>::RegistryDeleted);
			ensure!(registry_info.is_active(), Error::<T>::RegistryInactive);
			packet::ensure_entry_access::<T>(&registry, &registry_info, &delegate)?;

			let metadata = Packets::<T>::get(&packet).ok_or(Error::<T>::TokenNotFound)?;
			ensure!(metadata.registry == registry, Error::<T>::TokenNotFound);
			ensure!(metadata.status != PacketStatus::Deleted, Error::<T>::PacketDeleted);
			ensure!(metadata.status == PacketStatus::Active, Error::<T>::PacketRevoked);

			let current_state = PacketStates::<T>::get(&packet, metadata.latest_version)
				.ok_or(Error::<T>::TokenNotFound)?;
			let new_version = metadata.latest_version.saturating_add(1);
			let pointer = PacketPointer {
				registry: registry.clone(),
				packet: packet.clone(),
				version: new_version,
			};

			Self::mark_latest_version_revoked(&packet, metadata.latest_version)?;
			Self::refresh_lookup_entries(
				&registry,
				&registry_info,
				Some(&current_state.attributes),
				&current_state.attributes,
				&pointer,
			)?;

			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Revoked,
				version: new_version,
				digest: current_state.digest.clone(),
				attributes: current_state.attributes.clone(),
			};

			PacketStates::<T>::insert(&packet, new_version, state);
			Packets::<T>::insert(
				&packet,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Revoked,
					latest_version: new_version,
					digest: current_state.digest.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketRevoked")?;
			Self::deposit_event(Event::PacketRevoked { registry, packet, delegate });
			Ok(())
		}

		/// Restore a revoked packet to active state.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::restore_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _packet: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn restore_packet(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegate = Self::resolve_entity_token(&who)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_info.is_deleted(), Error::<T>::RegistryDeleted);
			ensure!(registry_info.is_active(), Error::<T>::RegistryInactive);
			packet::ensure_entry_access::<T>(&registry, &registry_info, &delegate)?;

			let metadata = Packets::<T>::get(&packet).ok_or(Error::<T>::TokenNotFound)?;
			ensure!(metadata.registry == registry, Error::<T>::TokenNotFound);
			ensure!(metadata.status != PacketStatus::Deleted, Error::<T>::PacketDeleted);
			ensure!(metadata.status == PacketStatus::Revoked, Error::<T>::PacketNotRevoked);

			let current_state = PacketStates::<T>::get(&packet, metadata.latest_version)
				.ok_or(Error::<T>::TokenNotFound)?;
			let new_version = metadata.latest_version.saturating_add(1);
			let pointer = PacketPointer {
				registry: registry.clone(),
				packet: packet.clone(),
				version: new_version,
			};

			Self::mark_latest_version_revoked(&packet, metadata.latest_version)?;
			Self::refresh_lookup_entries(
				&registry,
				&registry_info,
				Some(&current_state.attributes),
				&current_state.attributes,
				&pointer,
			)?;

			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Active,
				version: new_version,
				digest: current_state.digest.clone(),
				attributes: current_state.attributes.clone(),
			};

			PacketStates::<T>::insert(&packet, new_version, state);
			Packets::<T>::insert(
				&packet,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Active,
					latest_version: new_version,
					digest: current_state.digest.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketRestored")?;
			Self::deposit_event(Event::PacketRestored { registry, packet, delegate });
			Ok(())
		}

		/// Mark a revoked packet as permanently removed.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::remove_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _registry: &Ss58Identifier, _packet: &Ss58Identifier| -> bool {
			Pallet::<T>::is_origin_feeless(origin)
		})]
		pub fn remove_packet(
			origin: OriginFor<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let delegate = Self::resolve_entity_token(&who)?;

			let registry_info =
				Registries::<T>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_info.is_deleted(), Error::<T>::RegistryDeleted);
			ensure!(registry_info.is_active(), Error::<T>::RegistryInactive);
			packet::ensure_entry_access::<T>(&registry, &registry_info, &delegate)?;

			let metadata = Packets::<T>::get(&packet).ok_or(Error::<T>::TokenNotFound)?;
			ensure!(metadata.registry == registry, Error::<T>::TokenNotFound);
			ensure!(metadata.status != PacketStatus::Deleted, Error::<T>::PacketDeleted);
			ensure!(metadata.status == PacketStatus::Revoked, Error::<T>::PacketNotRevoked);

			let latest_state = PacketStates::<T>::get(&packet, metadata.latest_version)
				.ok_or(Error::<T>::TokenNotFound)?;

			let new_version = metadata.latest_version.saturating_add(1);
			let pointer = PacketPointer {
				registry: registry.clone(),
				packet: packet.clone(),
				version: new_version,
			};

			Self::mark_latest_version_revoked(&packet, metadata.latest_version)?;
			Self::refresh_lookup_entries(
				&registry,
				&registry_info,
				Some(&latest_state.attributes),
				&latest_state.attributes,
				&pointer,
			)?;

			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Deleted,
				version: new_version,
				digest: latest_state.digest.clone(),
				attributes: latest_state.attributes.clone(),
			};

			PacketStates::<T>::insert(&packet, new_version, state);
			Packets::<T>::insert(
				&packet,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Deleted,
					latest_version: new_version,
					digest: latest_state.digest.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketRemoved")?;
			Self::deposit_event(Event::PacketRemoved { registry, packet, delegate });
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		/// Returns delegate permissions for the supplied registry/delegate pair.
		pub fn delegate_permissions(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
		) -> Option<RegistryPermissions> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let perms = RegistryDelegates::<T>::get(&registry, &delegate)?;
			Some(perms)
		}

		/// Returns the lookup specifications declared for the registry.
		pub fn lookup_specs(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Vec<LookupSpecView>> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let specs = <Self as RegistryView<T>>::lookup_specs(&registry)?;
			let view_specs: Vec<LookupSpecView> = specs
				.into_iter()
				.map(|spec| match spec {
					LookupSpec::Single(attr) => LookupSpecView::Single(attr.into_inner()),
					LookupSpec::Combo(list) => {
						LookupSpecView::Combo(list.into_iter().map(|a| a.into_inner()).collect())
					},
				})
				.collect();
			Some(view_specs)
		}

		/// Returns all attribute keys and their schema types.
		pub fn registry_attributes(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Vec<RegistryAttributeView>> {
			Self::authorize_query(&auth).ok()?;
			let registry_info = Self::get_registry_state_view(&registry).ok()?;
			let entries: Vec<RegistryAttributeView> = registry_info
				.attributes
				.iter()
				.map(|spec| RegistryAttributeView {
					key: spec.key.to_vec(),
					kind: spec.kind,
					optional: spec.flags.is_optional(),
				})
				.collect();
			Some(entries)
		}

		/// Returns the declared schema type of the provided attribute key.
		pub fn registry_attribute(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			key: Vec<u8>,
		) -> Option<(ElementType, bool)> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let key_bounded: Attribute = key.try_into().ok()?;
			let registry_info = Registries::<T>::get(&registry)?;
			let spec = registry_info.attribute_spec(key_bounded.as_slice())?;
			Some((spec.kind, spec.flags.is_optional()))
		}

		/// Returns registry metadata and schema information.
		pub fn registry_details(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<RegistryStateView> {
			Self::authorize_query(&auth).ok()?;
			let info = Self::get_registry_state_view(&registry).ok()?;
			let view = view::build_registry_state_view::<T>(&registry, &info);
			Some(view)
		}

		/// Returns the attribute keys composing the registry token material.
		pub fn registry_token_specs(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Vec<Vec<u8>>> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let spec = <Self as RegistryView<T>>::token_specs(&registry)?;
			let keys: Vec<Vec<u8>> = match spec {
				LookupSpec::Single(attr) => vec![attr.into_inner()],
				LookupSpec::Combo(list) => list.into_iter().map(|a| a.into_inner()).collect(),
			};
			Some(keys)
		}

		/// Does this registry exist (and is auth valid)?
		pub fn registry_exists(auth: AuthorizationOf<T>, registry: Ss58Identifier) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			Registries::<T>::get(&registry).map_or(false, |info| !info.is_deleted())
		}

		/// Current status of the registry: Active/Revoked/Deleted.
		pub fn registry_status(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<RegistryStatus> {
			Self::authorize_query(&auth).ok()?;
			let info = Registries::<T>::get(&registry)?;
			Some(info.status())
		}

		/// All attribute keys defined in the registry schema.
		pub fn registry_attribute_keys(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Vec<Vec<u8>>> {
			Self::authorize_query(&auth).ok()?;
			let info = Self::get_registry_state_view(&registry).ok()?;
			Some(info.attribute_keys())
		}

		/// Is this registry active?
		pub fn registry_is_active(auth: AuthorizationOf<T>, registry: Ss58Identifier) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			Registries::<T>::get(&registry).map_or(false, |info| info.is_active())
		}

		/// Returns true if the registry exists and is marked Revoked.
		pub fn registry_is_revoked(auth: AuthorizationOf<T>, registry: Ss58Identifier) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			let revoked = Registries::<T>::get(&registry).map_or(false, |info| info.is_revoked());
			Self::record_registry_query(&registry, &auth.account);
			revoked
		}

		/// Returns true if the registry exists and is marked Deleted.
		pub fn registry_is_deleted(auth: AuthorizationOf<T>, registry: Ss58Identifier) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			let deleted = Registries::<T>::get(&registry).map_or(false, |info| info.is_deleted());
			Self::record_registry_query(&registry, &auth.account);
			deleted
		}

		/// List all delegates and their permissions for this registry.
		pub fn registry_delegates(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Vec<(Ss58Identifier, RegistryPermissions)>> {
			Self::authorize_query(&auth).ok()?;
			let _info = Registries::<T>::get(&registry)?; // ensure registry exists
			let mut delegates = Vec::new();
			for (delegate, perms) in RegistryDelegates::<T>::iter_prefix(&registry) {
				delegates.push((delegate, perms));
			}
			Self::record_registry_query(&registry, &auth.account);
			Some(delegates)
		}

		/// Check if a given delegate has at least the required permissions.
		pub fn has_registry_permissions(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
			required: RegistryPermissions,
		) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			let perm = <Self as RegistryView<T>>::has_permissions(&registry, &delegate, required);
			Self::record_registry_query(&registry, &auth.account);
			perm
		}

		/// Is this entity a delegate for the registry (i.e., has any permissions)?
		pub fn is_delegate(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			delegate: Ss58Identifier,
		) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			let has_any = RegistryDelegates::<T>::get(&registry, &delegate)
				.map(|perms| !perms.is_empty())
				.unwrap_or(false);
			Self::record_registry_query(&registry, &auth.account);
			has_any
		}

		/// Return the maintainer (owner) token of a registry, if it exists.
		/// None if auth fails or registry not found.
		pub fn registry_maintainer(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Option<Ss58Identifier> {
			Self::authorize_query(&auth).ok()?;
			let info = Registries::<T>::get(&registry)?;
			Some(info.maintainer().clone())
		}

		/// Returns a packet state associated with the given packet identifier for the registry.
		pub fn packet_state(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
			version: Option<u32>,
		) -> Option<PacketStateView> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let snapshot = Self::get_packet_snapshot(&registry, &packet, version).ok()?;
			Some(view::build_packet_state_view::<T>(&packet, &snapshot))
		}

		/// Returns packet metadata only (lightweight).
		pub fn packet_metadata(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> Option<origin_primitives::packet::PacketMetadataView> {
			Self::authorize_query(&auth).ok()?;
			// ensure registry exists & not deleted
			let _info = Self::get_registry_state_view(&registry).ok()?;
			let _snapshot = Self::get_packet_snapshot(&registry, &packet, None).ok()?;
			let metadata = Packets::<T>::get(&packet)?;
			if metadata.registry != registry {
				return None;
			}
			Some(origin_primitives::packet::PacketMetadataView::from(&metadata))
		}

		/// Returns the packet state for the given token (optionally at a specific version).
		pub fn packet_for_token(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			version: Option<u32>,
		) -> Option<PacketStateView> {
			Self::authorize_query(&auth).ok()?;
			let snapshot = Self::packet_state_unchecked(&token, version)?;
			let info = Self::get_registry_state_view(&snapshot.state.registry).ok()?;
			if snapshot.state.status == PacketStatus::Deleted || info.is_deleted() {
				return None;
			}

			Self::record_registry_query(&snapshot.state.registry, &auth.account);
			Some(view::build_packet_state_view::<T>(&token, &snapshot))
		}

		/// Resolves a packet state via a lookup digest.
		pub fn packet_lookup_snapshot(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			digest: LookupDigestOf<T>,
			version: Option<u32>,
		) -> Option<PacketStateView> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			let anchor = LookupIndex::<T>::get(&digest, &registry)?;
			let packet = anchor.pointer.packet.clone();
			let target_version = version.unwrap_or(anchor.pointer.version);
			let snap = Self::get_packet_snapshot(&registry, &packet, Some(target_version)).ok()?;
			Some(view::build_packet_state_view::<T>(&packet, &snap))
		}

		/// Returns packet pointers attached to the given lookup digest.
		///
		/// - `digest`: the exact lookup digest to resolve.
		/// - `offset`: optional starting index (for paging); 0 by default.
		/// - `limit`: max number of results to return; defaults to `MaxPacketListResults`.
		pub fn packets_by_digest(
			auth: AuthorizationOf<T>,
			digest: LookupDigestOf<T>,
			offset: Option<u32>,
			limit: Option<u32>,
		) -> Option<Vec<PacketPointer>> {
			Self::authorize_query(&auth).ok()?;
			let skip = offset.unwrap_or(0) as usize;
			let max = limit.unwrap_or_else(|| T::MaxPacketListResults::get()) as usize;
			let mut points: Vec<PacketPointer> = Vec::new();

			for (idx, (_registry, anchor)) in LookupIndex::<T>::iter_prefix(&digest).enumerate() {
				if idx < skip {
					continue;
				}
				if points.len() >= max {
					break;
				}

				let ptr = &anchor.pointer;
				if let Some(meta) = Packets::<T>::get(&ptr.packet) {
					if meta.status == PacketStatus::Deleted {
						continue;
					}
					if let Some(info) = Registries::<T>::get(&ptr.registry) {
						if info.is_deleted() {
							continue;
						}
					} else {
						continue;
					}
					points.push(ptr.clone());
				}
			}
			Some(points)
		}

		/// List packet snapshots by token prefix with cursor/limit.
		pub fn list_by_token(
			auth: AuthorizationOf<T>,
			token_prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<Ss58Identifier>,
			limit: Option<u32>,
		) -> Option<(Vec<PacketStateView>, Option<Ss58Identifier>)> {
			Self::authorize_query(&auth).ok()?;
			let capped = limit.unwrap_or_else(|| T::MaxPacketListResults::get());
			let (snaps, next) =
				Self::list_by_token_matches(token_prefix.as_slice(), version, cursor, capped);
			let views = snaps
				.into_iter()
				.map(|snap| view::build_packet_state_view::<T>(&snap.state.registry, &snap))
				.collect();
			Some((views, next))
		}

		/// List packet snapshots by lookup digest prefix with cursor/limit.
		pub fn list_by_digest(
			auth: AuthorizationOf<T>,
			digest_prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<LookupDigestOf<T>>,
			limit: Option<u32>,
		) -> Option<(Vec<PacketStateView>, Option<LookupDigestOf<T>>)> {
			Self::authorize_query(&auth).ok()?;
			let capped = limit.unwrap_or_else(|| T::MaxPacketListResults::get());
			let (snaps, next) = Self::packets_by_lookup_digest_matches(
				digest_prefix.as_slice(),
				version,
				cursor,
				capped,
			);
			let views = snaps
				.into_iter()
				.map(|snap| view::build_packet_state_view::<T>(&snap.state.registry, &snap))
				.collect();
			Some((views, next))
		}

		/// View how many times `account` queried this registry.
		pub fn query_count(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			account: T::AccountId,
		) -> Option<u64> {
			Self::authorize_query(&auth).ok()?;
			Self::get_registry_state_view(&registry).ok()?;
			Some(RegistryQueryCounts::<T>::get(&registry, account))
		}

		/// Does a packet exist for this registry + token (and is it not deleted)?
		pub fn packet_exists(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			if let Some(meta) = Packets::<T>::get(&packet) {
				if meta.registry != registry || meta.status == PacketStatus::Deleted {
					return false;
				}
				if let Some(info) = Registries::<T>::get(&registry) {
					!info.is_deleted()
				} else {
					false
				}
			} else {
				false
			}
		}

		/// Get the current status of a packet (Active/Revoked/Deleted).
		pub fn packet_status(
			auth: AuthorizationOf<T>,
			packet: Ss58Identifier,
		) -> Option<PacketStatus> {
			Self::authorize_query(&auth).ok()?;
			let meta = Packets::<T>::get(&packet)?;
			// Hide deleted packets or deleted registries
			let info = Registries::<T>::get(&meta.registry)?;
			if meta.status == PacketStatus::Deleted || info.is_deleted() {
				return None;
			}
			Some(meta.status)
		}

		/// Return the current controller (entity token) for a packet.
		pub fn packet_controller(
			auth: AuthorizationOf<T>,
			packet: Ss58Identifier,
		) -> Option<Ss58Identifier> {
			Self::authorize_query(&auth).ok()?;
			let meta = Packets::<T>::get(&packet)?;
			if meta.status == PacketStatus::Deleted {
				return None;
			}
			let info = Registries::<T>::get(&meta.registry)?;
			if info.is_deleted() {
				return None;
			}
			Some(meta.controller.clone())
		}
	}

	impl<T: Config> Pallet<T> {
		#[inline]
		fn is_origin_feeless(origin: &OriginFor<T>) -> bool {
			origin.caller().as_signed().map(T::Feeless::is_feeless).unwrap_or(false)
		}

		fn get_registry_state_view(
			registry: &Ss58Identifier,
		) -> Result<RegistryInfoOf<T>, AuthorizationError> {
			let info = Registries::<T>::get(registry).ok_or(AuthorizationError::NotFound)?;
			if info.is_deleted() {
				return Err(AuthorizationError::InvalidInput);
			}
			Ok(info)
		}

		fn record_registry_query(registry: &Ss58Identifier, account: &T::AccountId) {
			RegistryQueryCounts::<T>::mutate(registry, account.clone(), |count| {
				*count = count.saturating_add(1);
			});
		}

		fn get_packet_snapshot(
			registry: &Ss58Identifier,
			packet: &Ss58Identifier,
			version: Option<u32>,
		) -> Result<PacketSnapshotOf<T>, AuthorizationError> {
			let snapshot = <Self as RegistryView<T>>::packet_state(packet, version)
				.ok_or(AuthorizationError::NotFound)?;
			if &snapshot.state.registry != registry {
				return Err(AuthorizationError::NotFound);
			}
			if snapshot.state.status == PacketStatus::Deleted {
				return Err(AuthorizationError::InvalidInput);
			}
			Ok(snapshot)
		}

		fn authorize_query(
			auth: &AuthorizationOf<T>,
		) -> Result<Ss58Identifier, AuthorizationError> {
			Self::ensure_authorization_fresh(auth.payload.as_slice())?;
			let token = T::EntityLookup::verify_account_signature(
				&auth.account,
				auth.payload.as_slice(),
				&auth.signature,
			)
			.map_err(|_| AuthorizationError::Unauthorized)?;
			Ok(token)
		}

		fn ensure_authorization_fresh(payload: &[u8]) -> Result<(), AuthorizationError> {
			let issued_at = extract_valid_until(payload).ok_or(AuthorizationError::InvalidInput)?;
			let now: u32 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
			let ttl = T::MaxAuthorizationTTL::get();
			ensure_authorization_ttl(now, issued_at, ttl)
		}

		fn snapshot_for(
			token: &Ss58Identifier,
			registry: &Ss58Identifier,
			version: Option<u32>,
			meta_hint: Option<&PacketMetadataOf<T>>,
		) -> Option<PacketSnapshotOf<T>> {
			let state = if let Some(explicit) = version {
				let state = PacketStates::<T>::get(token, explicit)?;
				if state.registry != *registry {
					return None;
				}
				state
			} else {
				let latest_version = if let Some(meta) = meta_hint {
					if meta.registry != *registry {
						return None;
					}
					meta.latest_version
				} else {
					let meta = Packets::<T>::get(token)?;
					if meta.registry != *registry {
						return None;
					}
					meta.latest_version
				};
				let state = PacketStates::<T>::get(token, latest_version)?;
				if state.registry != *registry {
					return None;
				}
				state
			};
			let registry_info = Registries::<T>::get(registry)?;
			Some(PacketSnapshotOf::<T> { state, registry_status: registry_info.status() })
		}

		fn registry_info_unchecked(registry: &Ss58Identifier) -> Option<RegistryInfoOf<T>> {
			Registries::<T>::get(registry)
		}

		fn attribute_keys_unchecked(registry: &Ss58Identifier) -> Option<Vec<Vec<u8>>> {
			Registries::<T>::get(registry).map(|info| info.attribute_keys())
		}

		fn token_specs_unchecked(registry: &Ss58Identifier) -> Option<TokenSpecOf<T>> {
			Registries::<T>::get(registry).map(|info| info.token_spec.clone())
		}

		fn lookup_specs_unchecked(registry: &Ss58Identifier) -> Option<LookupSpecListOf<T>> {
			Registries::<T>::get(registry).map(|info| info.lookup_specs.clone())
		}

		fn packet_metadata_unchecked(packet: &Ss58Identifier) -> Option<PacketMetadataOf<T>> {
			Packets::<T>::get(packet)
		}

		fn packet_state_unchecked(
			packet: &Ss58Identifier,
			version: Option<u32>,
		) -> Option<PacketSnapshotOf<T>> {
			if let Some(explicit) = version {
				let state = PacketStates::<T>::get(packet, explicit)?;
				Self::snapshot_for(packet, &state.registry, Some(explicit), None)
			} else if let Some(meta) = Packets::<T>::get(packet) {
				Self::snapshot_for(packet, &meta.registry, None, Some(&meta))
			} else {
				None
			}
		}

		fn lookup_state_unchecked(
			registry: &Ss58Identifier,
			digest: &LookupDigestOf<T>,
			version: Option<u32>,
		) -> Option<PacketSnapshotOf<T>> {
			let anchor = LookupIndex::<T>::get(digest, registry)?;
			let target_version = version.unwrap_or(anchor.pointer.version);
			Self::snapshot_for(&anchor.pointer.packet, registry, Some(target_version), None)
		}

		fn registry_active_unchecked(registry: &Ss58Identifier) -> bool {
			Registries::<T>::get(registry).map_or(false, |info| info.is_active())
		}

		fn list_by_token_matches(
			prefix: &[u8],
			version: Option<u32>,
			cursor: Option<Ss58Identifier>,
			limit: u32,
		) -> (Vec<PacketSnapshotOf<T>>, Option<Ss58Identifier>) {
			let mut results = Vec::new();
			let mut past_cursor = cursor.is_none();
			let mut filled = false;
			let mut extra_exists = false;
			let mut last_token: Option<Ss58Identifier> = None;
			for (token, meta) in Packets::<T>::iter() {
				let token_bytes = token.as_ref();
				if !prefix.is_empty() && !token_bytes.starts_with(prefix) {
					continue;
				}
				if !past_cursor {
					if let Some(ref cursor_id) = cursor {
						if token == *cursor_id {
							past_cursor = true;
						}
					}
					continue;
				}
				if filled {
					extra_exists = true;
					break;
				}
				if let Some(snapshot) =
					Self::snapshot_for(&token, &meta.registry, version, Some(&meta))
				{
					results.push(snapshot);
					last_token = Some(token.clone());
					if results.len() as u32 >= limit {
						filled = true;
					}
				}
			}
			let next_cursor = if filled && extra_exists { last_token } else { None };
			(results, next_cursor)
		}

		fn packets_by_lookup_digest_matches(
			prefix: &[u8],
			version: Option<u32>,
			cursor: Option<LookupDigestOf<T>>,
			limit: u32,
		) -> (Vec<PacketSnapshotOf<T>>, Option<LookupDigestOf<T>>) {
			let mut results = Vec::new();
			let mut past_cursor = cursor.is_none();
			let mut filled = false;
			let mut extra_exists = false;
			let mut last_digest: Option<LookupDigestOf<T>> = None;
			for (digest, registry, anchor) in LookupIndex::<T>::iter() {
				let digest_bytes = digest.as_ref();
				if !prefix.is_empty() && !digest_bytes.starts_with(prefix) {
					continue;
				}
				if !past_cursor {
					if let Some(ref cursor_id) = cursor {
						if digest == *cursor_id {
							past_cursor = true;
						}
					}
					continue;
				}
				if filled {
					extra_exists = true;
					break;
				}
				let target_version = version.unwrap_or(anchor.pointer.version);
				if let Some(snapshot) = Self::snapshot_for(
					&anchor.pointer.packet,
					&registry,
					Some(target_version),
					None,
				) {
					results.push(snapshot);
					last_digest = Some(digest.clone());
					if results.len() as u32 >= limit {
						filled = true;
					}
				}
			}
			let next_cursor = if filled && extra_exists { last_digest } else { None };
			(results, next_cursor)
		}

		fn resolve_entity_token(account: &T::AccountId) -> Result<Ss58Identifier, DispatchError> {
			T::EntityLookup::lookup_token_of(account).map_err(|_| Error::<T>::TokenNotFound.into())
		}

		fn ensure_admin(registry: &Ss58Identifier, actor: &Ss58Identifier) -> DispatchResult {
			let registry_info =
				Registries::<T>::get(registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_info.maintainer() == actor {
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
			let registry_info =
				Registries::<T>::get(registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_info.maintainer() == actor {
				return Ok(());
			}
			let perms =
				RegistryDelegates::<T>::get(registry, actor).ok_or(Error::<T>::PermissionDenied)?;
			ensure!(perms.has_delegate(), Error::<T>::PermissionDenied);
			Ok(())
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
				RegistryFieldError::OptionalLookupKey => Error::<T>::InvalidAttributeKey.into(),
				RegistryFieldError::MissingLookupSpecs => Error::<T>::NoLookupSpecs.into(),
			}
		}

		fn map_token_field_error_for_lookups(err: RegistryFieldError) -> DispatchError {
			match err {
				RegistryFieldError::DuplicateKey => Error::<T>::DuplicateLookupField.into(),
				RegistryFieldError::UnknownKey => Error::<T>::UnknownLookupField.into(),
				RegistryFieldError::EmptySpec => Error::<T>::UnknownLookupField.into(),
				RegistryFieldError::DuplicateSpec => Error::<T>::DuplicateLookupField.into(),
				RegistryFieldError::OptionalLookupKey => Error::<T>::InvalidAttributeKey.into(),
				RegistryFieldError::MissingLookupSpecs => Error::<T>::NoLookupSpecs.into(),
			}
		}

		fn mark_latest_version_revoked(
			packet: &Ss58Identifier,
			latest_version: u32,
		) -> DispatchResult {
			if latest_version == 0 {
				return Ok(());
			}
			PacketStates::<T>::try_mutate(packet, latest_version, |maybe_state| -> DispatchResult {
				let state = maybe_state.as_mut().ok_or(Error::<T>::TokenNotFound)?;
				if state.status != PacketStatus::Revoked {
					state.status = PacketStatus::Revoked;
				}
				Ok(())
			})
		}

		fn refresh_lookup_entries(
			registry: &Ss58Identifier,
			registry_info: &RegistryInfoOf<T>,
			previous: Option<&PacketAttributesOf<T>>,
			current: &PacketAttributesOf<T>,
			pointer: &PacketPointer,
		) -> DispatchResult {
			let new_entries = packet::prepare_lookup_keys::<T>(registry, registry_info, current)?;
			let mut old_entries: BTreeMap<_, _> = if let Some(prev) = previous {
				packet::prepare_lookup_keys::<T>(registry, registry_info, prev)?
					.into_iter()
					.collect()
			} else {
				BTreeMap::new()
			};

			for (digest, spec_index) in new_entries {
				if let Some(previous_spec) = old_entries.remove(&digest) {
					ensure!(previous_spec == spec_index, Error::<T>::LookupConflict);
					LookupIndex::<T>::try_mutate(&digest, registry, |slot| -> DispatchResult {
						if let Some(anchor) = slot {
							ensure!(anchor.spec == spec_index, Error::<T>::LookupConflict);
							anchor.pointer = pointer.clone();
						} else {
							*slot = Some(packet::LookupAnchor {
								spec: spec_index,
								pointer: pointer.clone(),
							});
						}
						Ok(())
					})?;
				} else {
					LookupIndex::<T>::try_mutate(&digest, registry, |slot| -> DispatchResult {
						if let Some(anchor) = slot {
							ensure!(
								anchor.pointer.packet == pointer.packet,
								Error::<T>::LookupConflict
							);
							ensure!(anchor.spec == spec_index, Error::<T>::LookupConflict);
							anchor.pointer = pointer.clone();
						} else {
							*slot = Some(packet::LookupAnchor {
								spec: spec_index,
								pointer: pointer.clone(),
							});
						}
						Ok(())
					})?;
				}
			}
			Ok(())
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

	impl<T: Config> RegistryView<T> for Pallet<T> {
		fn registry_info(registry_id: &Ss58Identifier) -> Option<RegistryInfoOf<T>> {
			Self::registry_info_unchecked(registry_id)
		}

		fn attribute_keys(registry_id: &Ss58Identifier) -> Option<Vec<Vec<u8>>> {
			Self::attribute_keys_unchecked(registry_id)
		}

		fn token_specs(registry_id: &Ss58Identifier) -> Option<TokenSpecOf<T>> {
			Self::token_specs_unchecked(registry_id)
		}

		fn lookup_specs(registry_id: &Ss58Identifier) -> Option<LookupSpecListOf<T>> {
			Self::lookup_specs_unchecked(registry_id)
		}

		fn has_permissions(
			registry_id: &Ss58Identifier,
			delegate: &Ss58Identifier,
			required: RegistryPermissions,
		) -> bool {
			if let Some(registry_info) = Registries::<T>::get(registry_id) {
				if registry_info.maintainer() == delegate {
					return true;
				}
			}
			RegistryDelegates::<T>::get(registry_id, delegate).map_or(false, |perms| match required
			{
				RegistryPermissions::ADMIN => perms.has_admin(),
				RegistryPermissions::DELEGATE => perms.has_delegate(),
				RegistryPermissions::ENTRY => perms.has_entry(),
				RegistryPermissions::VIEW => perms.has_view(),
				_ => perms.contains(required),
			})
		}

		fn packet_metadata(packet: &Ss58Identifier) -> Option<PacketMetadataOf<T>> {
			Self::packet_metadata_unchecked(packet)
		}

		fn packet_state(
			packet: &Ss58Identifier,
			version: Option<u32>,
		) -> Option<PacketSnapshotOf<T>> {
			Self::packet_state_unchecked(packet, version)
		}

		fn lookup_state(
			registry_id: &Ss58Identifier,
			digest: &LookupDigestOf<T>,
			version: Option<u32>,
		) -> Option<PacketSnapshotOf<T>> {
			Self::lookup_state_unchecked(registry_id, digest, version)
		}

		fn list_by_token(
			token_prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<Ss58Identifier>,
			limit: u32,
		) -> (Vec<PacketSnapshotOf<T>>, Option<Ss58Identifier>) {
			Self::list_by_token_matches(token_prefix.as_slice(), version, cursor, limit)
		}

		fn packets_by_lookup_digest(
			digest_prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<LookupDigestOf<T>>,
			limit: u32,
		) -> (Vec<PacketSnapshotOf<T>>, Option<LookupDigestOf<T>>) {
			Self::packets_by_lookup_digest_matches(digest_prefix.as_slice(), version, cursor, limit)
		}

		fn registry_active(registry_id: &Ss58Identifier) -> bool {
			Self::registry_active_unchecked(registry_id)
		}
	}
}

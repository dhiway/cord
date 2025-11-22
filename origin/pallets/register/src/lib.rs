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

mod packet;
pub mod register;
pub mod weights;

extern crate alloc;
use alloc::{collections::BTreeMap, vec::Vec};
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
	packet::{PacketPointer, PacketStatus, PacketUpdateError},
	registry::{RegistryKind, RegistryPermissions, RegistryStatus},
	Signature,
};
pub use packet::{
	attributes_digest, AttributePairsOf, LookupDigestOf, PacketAttributesOf, PacketDataOf,
	PacketMetadataOf, PacketSnapshotOf, PacketStateOf, PacketStateView,
};
use register::{
	AttributeFlags, AttributeSpec, LookupSpec, RegistryFieldError, RegistryInfo, RegistryStateView,
};

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
pub type RegistryStateViewOf<T> = RegistryStateView<RegistryInfoOf<T>, LookupSpecListOf<T>>;
pub type PacketStateViewOf<T> = PacketStateView<PacketSnapshotOf<T>>;

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
			let rtoken = T::Token::build(&digest.encode()[..], pallet_name)
				.map_err(|_| Error::<T>::TokenCreationFailed)?;

			Registries::<T>::try_mutate_exists(&rtoken, |slot| -> DispatchResult {
				ensure!(slot.is_none(), Error::<T>::RegistryAlreadyExists);
				*slot = Some(registry_info);
				Ok(())
			})?;

			RegistryDelegates::<T>::insert(
				&rtoken,
				&maintainer,
				RegistryPermissions::ADMIN | RegistryPermissions::VIEW,
			);
			Self::record_activity(&rtoken, digest, b"RegistryCreated")?;

			Self::deposit_event(Event::RegistryCreated { registry: rtoken.clone(), maintainer });
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
		#[pallet::feeless_if(|origin: &OriginFor<T>, _rtoken: &Ss58Identifier, _attributes: &AttributePairsOf<T>| -> bool {
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

			let ptoken =
				packet::derive_packet_token::<T>(&registry, &registry_info, &packet_attributes)?;
			ensure!(Packets::<T>::get(&ptoken).is_none(), Error::<T>::PacketAlreadyExists);

			let lookup_entries =
				packet::prepare_lookup_keys::<T>(&registry, &registry_info, &packet_attributes)?;

			for (digest, _) in lookup_entries.iter() {
				if let Some(existing) = LookupIndex::<T>::get(digest, &registry) {
					ensure!(existing.pointer.packet == ptoken, Error::<T>::LookupConflict);
					return Err(Error::<T>::PacketAlreadyExists.into());
				}
			}

			let version: u32 = 1;
			let attributes_hash = packet::attributes_digest::<T>(&packet_attributes);
			let pointer =
				PacketPointer { registry: registry.clone(), packet: ptoken.clone(), version };
			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Active,
				version,
				attributes_hash: attributes_hash.clone(),
				attributes: packet_attributes.clone(),
			};

			PacketStates::<T>::insert(&ptoken, version, state);
			Packets::<T>::insert(
				&ptoken,
				PacketMetadataOf::<T> {
					registry: registry.clone(),
					controller: delegate.clone(),
					status: PacketStatus::Active,
					latest_version: version,
					attributes_hash: attributes_hash.clone(),
				},
			);

			for (digest, spec) in lookup_entries {
				LookupIndex::<T>::insert(
					&digest,
					&registry,
					packet::LookupAnchor { spec, pointer: pointer.clone() },
				);
			}

			packet::record_packet_event::<T>(&ptoken, b"PacketCreated")?;
			Self::deposit_event(Event::PacketCreated { registry, packet: ptoken, delegate });
			Ok(())
		}

		/// Update an existing packet, bumping the version.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::update_packet(attributes.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _rtoken: &Ss58Identifier, _ptoken: &Ss58Identifier, _attributes: &AttributePairsOf<T>| -> bool {
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

			let new_hash = packet::attributes_digest::<T>(&merged_attributes);
			let state = PacketStateOf::<T> {
				registry: registry.clone(),
				controller: delegate.clone(),
				status: PacketStatus::Active,
				version: new_version,
				attributes_hash: new_hash.clone(),
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
					attributes_hash: new_hash.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketUpdated")?;
			Self::deposit_event(Event::PacketUpdated { registry, packet, delegate });
			Ok(())
		}

		/// Revoke an active packet.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::revoke_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _rtoken: &Ss58Identifier, _ptoken: &Ss58Identifier| -> bool {
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
				attributes_hash: current_state.attributes_hash.clone(),
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
					attributes_hash: current_state.attributes_hash.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketRevoked")?;
			Self::deposit_event(Event::PacketRevoked { registry, packet, delegate });
			Ok(())
		}

		/// Restore a revoked packet to active state.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::restore_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _rtoken: &Ss58Identifier, _ptoken: &Ss58Identifier| -> bool {
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
				attributes_hash: current_state.attributes_hash.clone(),
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
					attributes_hash: current_state.attributes_hash.clone(),
				},
			);

			packet::record_packet_event::<T>(&packet, b"PacketRestored")?;
			Self::deposit_event(Event::PacketRestored { registry, packet, delegate });
			Ok(())
		}

		/// Mark a revoked packet as permanently removed.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::remove_packet())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _rtoken: &Ss58Identifier, _ptoken: &Ss58Identifier| -> bool {
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
				attributes_hash: latest_state.attributes_hash.clone(),
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
					attributes_hash: latest_state.attributes_hash.clone(),
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
		) -> Result<RegistryPermissions, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let perms = RegistryDelegates::<T>::get(&registry, &delegate)
				.ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(perms)
		}

		/// Returns the recorded query count for an account over a registry.
		pub fn query_count(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			account: T::AccountId,
		) -> Result<u64, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let count = RegistryQueryCounts::<T>::get(&registry, account);
			Self::record_registry_query(&registry, &auth.account);
			Ok(count)
		}

		/// Returns the lookup specifications declared for the registry.
		pub fn lookup_specs(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Result<LookupSpecListOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let specs = <Self as RegistryView<T>>::lookup_specs(&registry)
				.ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(specs)
		}

		/// Returns the declared schema type of the provided attribute key.
		pub fn attribute(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			key: Vec<u8>,
		) -> Result<(ElementType, bool), AuthorizationError> {
			Self::authorize_query(&auth)?;
			let key_bounded: Attribute =
				key.try_into().map_err(|_| AuthorizationError::InvalidInput)?;
			let registry_info =
				Registries::<T>::get(&registry).ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			let spec = registry_info
				.attribute_spec(key_bounded.as_slice())
				.ok_or(AuthorizationError::NotFound)?;
			Ok((spec.kind, spec.flags.is_optional()))
		}

		/// Returns all attribute keys and their schema types.
		pub fn attributes(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Result<Vec<(Vec<u8>, ElementType, bool)>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let registry_info =
				Registries::<T>::get(&registry).ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			let entries = registry_info
				.attributes
				.iter()
				.map(|spec| (spec.key.to_vec(), spec.kind, spec.flags.is_optional()))
				.collect();
			Ok(entries)
		}

		/// Returns registry metadata and schema information.
		pub fn details(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Result<RegistryInfoOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let info = <Self as RegistryView<T>>::registry_info(&registry)
				.ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(info)
		}

		/// Returns the attribute keys composing the registry token material.
		pub fn token_specs(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Result<TokenSpecOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let spec = <Self as RegistryView<T>>::token_specs(&registry)
				.ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(spec)
		}
		/// Returns a packet state associated with the given packet identifier for the registry.
		pub fn packet_snapshot(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
			version: Option<u32>,
		) -> Result<PacketStateViewOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			Self::get_registry_state_view(&registry)?;
			let snapshot = Self::get_packet_state_view(&registry, &packet, version)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(PacketStateViewOf::<T> { registry, packet, snapshot })
		}

		/// Returns packet metadata only (lightweight).
		pub fn packet_metadata(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			packet: Ss58Identifier,
		) -> Result<PacketMetadataOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let _info = Self::get_registry_state_view(&registry)?;
			let _snapshot = Self::get_packet_state_view(&registry, &packet, None)?;
			let metadata = Packets::<T>::get(&packet).ok_or(AuthorizationError::NotFound)?;
			if metadata.registry != registry {
				return Err(AuthorizationError::NotFound);
			}
			Self::record_registry_query(&registry, &auth.account);
			Ok(metadata)
		}

		/// Lightweight registry overview (info + lookup specs).
		pub fn overview(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
		) -> Result<RegistryStateViewOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let info = Self::get_registry_state_view(&registry)?;
			let specs = <Self as RegistryView<T>>::lookup_specs(&registry)
				.ok_or(AuthorizationError::NotFound)?;
			Self::record_registry_query(&registry, &auth.account);
			Ok(RegistryStateViewOf::<T> { registry, info, lookup_specs: specs })
		}

		/// Returns a packet snapshot by token without requiring the registry identifier.
		pub fn packet_snapshot_by_token(
			auth: AuthorizationOf<T>,
			token: Ss58Identifier,
			version: Option<u32>,
		) -> Result<Option<PacketStateViewOf<T>>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			let snapshot_opt = Self::packet_state_unchecked(&token, version);

			if let Some(snapshot) = snapshot_opt {
				let info = Self::get_registry_state_view(&snapshot.state.registry)?;
				if snapshot.state.status == PacketStatus::Deleted || info.is_deleted() {
					return Err(AuthorizationError::InvalidInput);
				}
				Self::record_registry_query(&snapshot.state.registry, &auth.account);
				return Ok(Some(PacketStateViewOf::<T> {
					registry: snapshot.state.registry.clone(),
					packet: token,
					snapshot,
				}));
			}

			Ok(None)
		}

		/// Resolves a packet state via a lookup digest.
		pub fn lookup_snapshot(
			auth: AuthorizationOf<T>,
			registry: Ss58Identifier,
			digest: LookupDigestOf<T>,
			version: Option<u32>,
		) -> Result<PacketStateViewOf<T>, AuthorizationError> {
			Self::authorize_query(&auth)?;
			Self::get_registry_state_view(&registry)?;
			let anchor =
				LookupIndex::<T>::get(&digest, &registry).ok_or(AuthorizationError::NotFound)?;
			let packet = anchor.pointer.packet.clone();
			let target_version = version.unwrap_or(anchor.pointer.version);

			let snap = Self::get_packet_state_view(&registry, &packet, Some(target_version))?;
			Self::record_registry_query(&registry, &auth.account);

			Ok(PacketStateViewOf::<T> { registry, packet, snapshot: snap })
		}

		/// Returns packet snapshots matching the provided token prefix (or all when empty).
		pub fn list_by_token(
			auth: AuthorizationOf<T>,
			prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<Ss58Identifier>,
			limit: Option<u32>,
		) -> Result<(Vec<PacketSnapshotOf<T>>, Option<Ss58Identifier>), AuthorizationError> {
			Self::authorize_query(&auth)?;

			let capped = limit.unwrap_or_else(|| T::MaxPacketListResults::get());
			let limit = capped.min(T::MaxPacketListResults::get()).max(1);

			let (snaps, next) =
				<Self as RegistryView<T>>::list_by_token(prefix, version, cursor, limit);

			// Filter out deleted packets and deleted registries.
			let mut filtered = Vec::with_capacity(snaps.len());

			for snap in snaps.into_iter() {
				// Drop deleted packets early.
				if snap.state.status == PacketStatus::Deleted {
					continue;
				}

				let reg = &snap.state.registry;

				// get_registry_state_view already checks that the registry exists
				// and is not deleted. We don't need the value itself here.
				if Self::get_registry_state_view(reg).is_err() {
					continue;
				}

				Self::record_registry_query(reg, &auth.account);
				filtered.push(snap);
			}

			Ok((filtered, next))
		}

		/// Returns packet snapshots for every registry entry matching the digest prefix.
		pub fn list_by_digest(
			auth: AuthorizationOf<T>,
			digest_prefix: Vec<u8>,
			version: Option<u32>,
			cursor: Option<LookupDigestOf<T>>,
			limit: Option<u32>,
		) -> Result<(Vec<PacketSnapshotOf<T>>, Option<LookupDigestOf<T>>), AuthorizationError> {
			Self::authorize_query(&auth)?;

			let capped = limit.unwrap_or_else(|| T::MaxPacketListResults::get());
			let limit = capped.min(T::MaxPacketListResults::get()).max(1);

			let (snaps, next) = <Self as RegistryView<T>>::packets_by_lookup_digest(
				digest_prefix,
				version,
				cursor,
				limit,
			);

			let mut filtered = Vec::with_capacity(snaps.len());

			for snap in snaps.into_iter() {
				// Drop deleted packets.
				if snap.state.status == PacketStatus::Deleted {
					continue;
				}

				let reg = &snap.state.registry;

				// Drop deleted or missing registries.
				if Self::get_registry_state_view(reg).is_err() {
					continue;
				}

				Self::record_registry_query(reg, &auth.account);
				filtered.push(snap);
			}

			Ok((filtered, next))
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

		fn get_packet_state_view(
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
			Some(PacketSnapshotOf::<T>::from_state(&state, registry_info.status()))
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
			rtoken: &Ss58Identifier,
			registry_info: &RegistryInfoOf<T>,
			previous: Option<&PacketAttributesOf<T>>,
			current: &PacketAttributesOf<T>,
			pointer: &PacketPointer,
		) -> DispatchResult {
			let new_entries = packet::prepare_lookup_keys::<T>(rtoken, registry_info, current)?;
			let mut old_entries: BTreeMap<_, _> = if let Some(prev) = previous {
				packet::prepare_lookup_keys::<T>(rtoken, registry_info, prev)?
					.into_iter()
					.collect()
			} else {
				BTreeMap::new()
			};

			for (digest, spec_index) in new_entries {
				if let Some(previous_spec) = old_entries.remove(&digest) {
					ensure!(previous_spec == spec_index, Error::<T>::LookupConflict);
					LookupIndex::<T>::try_mutate(&digest, rtoken, |slot| -> DispatchResult {
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
					LookupIndex::<T>::try_mutate(&digest, rtoken, |slot| -> DispatchResult {
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

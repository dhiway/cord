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

use crate::{AttributeFlags, Config, Error, RegistryInfoOf};
use alloc::{collections::BTreeMap, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::{
	identifier::Ss58Identifier,
	packet::{Attribute, Attributes, AttributesError, Element, ElementType},
};
use core::convert::TryInto;
use frame_support::{
	dispatch::DispatchResult, ensure, traits::Get, BoundedVec, CloneNoBound, EqNoBound,
	PartialEqNoBound, RuntimeDebugNoBound,
};
use pallet_token::{EventBlock, EventTypeOf, Token};
use scale_info::TypeInfo;
use sp_runtime::{traits::Hash, DispatchError, RuntimeDebug};

/// Type alias for packet payload elements bounded by `MaxRawDataLength`.
pub type PacketDataOf<T> = Element<<T as Config>::MaxRawDataLength>;
/// Type alias for packet attribute collections stored on-chain.
pub type PacketAttributesOf<T> =
	Attributes<<T as Config>::MaxRawDataLength, <T as Config>::MaxAdditionalAttributes>;
/// Type alias for bounded packet attribute inputs accepted by extrinsics.
pub type AttributePairsOf<T> =
	BoundedVec<(Attribute, PacketDataOf<T>), <T as Config>::MaxAdditionalAttributes>;

/// Lookup anchor capturing the spec index associated with a digest.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
)]
pub struct LookupAnchor {
	pub spec: u32,
	pub pointer: PacketPointer,
}

/// Packet lifecycle states.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
	Default,
)]
pub enum PacketStatus {
	#[default]
	Active,
	Revoked,
	Deleted,
}

/// Pointer linking an index entry to a specific registry/packet version.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
)]
pub struct PacketPointer {
	pub rtoken: Ss58Identifier,
	pub ptoken: Ss58Identifier,
	pub version: u32,
}

/// Metadata tracked per packet token for quick access to the latest state.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(Hash))]
pub struct PacketMetadata<Hash>
where
	Hash: Clone + PartialEq + Eq + core::fmt::Debug,
{
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub latest_version: u32,
	pub attributes_hash: Hash,
}

/// Packet state persisted for each `(packet token, version)` pair.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes, Hash))]
pub struct PacketState<
	MaxRawDataLength: Get<u32>,
	MaxAdditionalAttributes: Get<u32>,
	Hash: Clone + PartialEq + Eq + core::fmt::Debug,
> {
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub version: u32,
	pub attributes_hash: Hash,
	pub attributes: Attributes<MaxRawDataLength, MaxAdditionalAttributes>,
}

pub fn ensure_entry_access<T: Config>(
	registry: &Ss58Identifier,
	registry_info: &RegistryInfoOf<T>,
	delegate: &Ss58Identifier,
) -> DispatchResult {
	if registry_info.maintainer() == delegate {
		return Ok(());
	}
	let perms = crate::pallet::RegistryDelegates::<T>::get(registry, delegate)
		.ok_or(Error::<T>::PermissionDenied)?;
	ensure!(perms.has_entry(), Error::<T>::PermissionDenied);
	Ok(())
}

pub fn normalise_attributes<T: Config>(
	input: AttributePairsOf<T>,
) -> Result<PacketAttributesOf<T>, DispatchError> {
	let mut pairs: Vec<(Attribute, PacketDataOf<T>)> = Vec::new();
	for (key, value) in input.into_iter() {
		pairs.push((key, value));
	}
	Attributes::try_from(pairs).map_err(|err| map_attribute_error::<T>(err).into())
}

pub fn apply_attribute_updates<T: Config>(
	target: &mut PacketAttributesOf<T>,
	updates: &PacketAttributesOf<T>,
) -> Result<(), DispatchError> {
	for (key, value) in updates.iter() {
		if let Some(existing) = target.get_mut(key.as_slice()) {
			*existing = value.clone();
		} else {
			target
				.try_insert(key.clone(), value.clone())
				.map_err(|err| -> DispatchError { map_attribute_error::<T>(err).into() })?;
		}
	}
	Ok(())
}

pub fn ensure_matches_schema<T: Config>(
	registry: &RegistryInfoOf<T>,
	attributes: &PacketAttributesOf<T>,
) -> Result<(), DispatchError> {
	let mut schema: BTreeMap<Vec<u8>, (ElementType, AttributeFlags)> = BTreeMap::new();
	for spec in registry.attributes.iter() {
		schema.insert(spec.key.to_vec(), (spec.kind, spec.flags));
	}

	for (key, value) in attributes.iter() {
		let (expected_kind, flags) =
			schema.get(key.as_slice()).ok_or(Error::<T>::UnknownAttribute)?;
		let actual_kind = ElementType::from(value);
		if value.is_none() {
			ensure!(flags.is_optional(), Error::<T>::MissingAttribute);
		} else {
			ensure!(actual_kind == *expected_kind, Error::<T>::InvalidAttributeType);
		}
	}

	for (key, (expected_kind, flags)) in schema.iter() {
		if !flags.is_optional() {
			let value = attributes.get(key.as_slice()).ok_or(Error::<T>::MissingAttribute)?;
			let actual_kind = ElementType::from(value);
			ensure!(actual_kind != ElementType::None, Error::<T>::MissingAttribute);
			ensure!(actual_kind == *expected_kind, Error::<T>::InvalidAttributeType);
		}
	}
	Ok(())
}

pub fn derive_packet_token<T: Config>(
	registry: &Ss58Identifier,
	registry_info: &RegistryInfoOf<T>,
	attributes: &PacketAttributesOf<T>,
) -> Result<Ss58Identifier, DispatchError> {
	let mut material: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
	let token_keys = registry_info.token_spec.cloned_keys();
	ensure!(!token_keys.is_empty(), Error::<T>::InvalidAttributeKey);

	for key in token_keys.iter() {
		let value = attributes.get(key.as_slice()).ok_or(Error::<T>::MissingAttribute)?;
		material.push((key.to_vec(), value.encode()));
	}
	material.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));

	let attribute_digest = T::Hashing::hash(&material.encode());
	let digest = T::Hashing::hash(&(attribute_digest, registry).encode());
	let pallet_name = <crate::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();
	let token = T::Token::build(&digest.encode()[..], pallet_name)
		.map_err(|_| Error::<T>::TokenCreationFailed)?;
	Ok(token)
}

pub fn prepare_lookup_keys<T: Config>(
	_registry: &Ss58Identifier,
	registry_info: &RegistryInfoOf<T>,
	attributes: &PacketAttributesOf<T>,
) -> Result<Vec<(<T as frame_system::Config>::Hash, u32)>, DispatchError> {
	let mut lookups: Vec<(T::Hash, u32)> = Vec::new();
	for (index, spec) in registry_info.lookup_specs.iter().enumerate() {
		let mut values: Vec<Vec<u8>> = Vec::new();
		for key in spec.cloned_keys().into_iter() {
			let entry = attributes.get(key.as_slice()).ok_or(Error::<T>::MissingAttribute)?;
			values.push(entry.encode());
		}
		let key_material = (spec.fingerprint(), values).encode();
		let digest = T::Hashing::hash(&key_material);
		lookups.push((digest, index as u32));
	}
	Ok(lookups)
}

pub fn record_packet_event<T: Config>(packet: &Ss58Identifier, event: &[u8]) -> DispatchResult {
	let action: EventTypeOf =
		event.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
	let stamp = EventBlock::current::<T>();
	let digest = T::Hashing::hash(&(packet, action.as_slice(), stamp.height, stamp.index).encode());
	T::Token::state_event(packet, digest, action, stamp)
		.map_err(|_| Error::<T>::StateUpdateFailed)?;
	Ok(())
}

fn map_attribute_error<T: Config>(err: AttributesError) -> Error<T> {
	match err {
		AttributesError::DuplicateKey => Error::<T>::DuplicateAttribute,
		AttributesError::TooManyAttributes => Error::<T>::TooManyAttributes,
		AttributesError::InvalidElement => Error::<T>::InvalidAttributeType,
	}
}

pub fn attributes_digest<T: Config>(
	attributes: &PacketAttributesOf<T>,
) -> <T as frame_system::Config>::Hash {
	let mut normalized: Vec<(Vec<u8>, Vec<u8>)> =
		attributes.iter().map(|(key, value)| (key.to_vec(), value.encode())).collect();
	normalized.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
	T::Hashing::hash(&normalized.encode())
}

pub type LookupDigestOf<T> = <T as frame_system::Config>::Hash;
pub type PacketStateOf<T> = PacketState<
	<T as Config>::MaxRawDataLength,
	<T as Config>::MaxAdditionalAttributes,
	<T as frame_system::Config>::Hash,
>;
pub type PacketMetadataOf<T> = PacketMetadata<<T as frame_system::Config>::Hash>;

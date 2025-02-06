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

//! # CORD Identifiers
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

extern crate alloc;
use alloc::{format, string::String, vec, vec::Vec};
use codec::{Decode, Encode, MaxEncodedLen};
use cord_primitives::Id as NetworkId;
use frame_support::{ensure, pallet_prelude::*, traits::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_core::hexdisplay::HexDisplay;
use sp_runtime::traits::{BlockNumberProvider, UniqueSaturatedInto};

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

const PREFIX: &[u8] = b"CURIV02";
const INDEX: u16 = 64;

pub use crate::pallet::*;

/// EventStamp marks the block and extrinsic where an event occurred.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EventStamp {
	pub height: u32,
	pub index: u32,
}

/// EntryTypeOf is a bounded vector (max 64 bytes) that holds part of an event message,
pub type EntryTypeOf = BoundedVec<u8, ConstU32<64>>;

/// ActivityRecord stores an update entry and the corresponding event stamp.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct ActivityRecord {
	pub entry: EntryTypeOf,
	pub event_stamp: EventStamp,
}

/// Errors for identifier operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierError {
	/// The identifier format is invalid.
	InvalidFormat,
	/// The prefix is invalid or unrecognized.
	InvalidPrefix,
	/// The identifier is not valid.
	InvalidIdentifier,
	/// The checksum validation failed.
	InvalidChecksum,
	/// The identifier length is not valid.
	InvalidIdentifierLength,
	/// The pallet name exceeds the maximum allowed length.
	PalletNameTooLong,
	/// The specified pallet name was not found.
	PalletNotFound,
	/// The specified pallet index is invalid.
	InvalidPalletIndex,
	/// The pallet name format is invalid.
	InvalidPalletNameFormat,
	/// The provided network id does not match the expected value.
	InvalidNetworkId,
	// Max exvents history exceeded
	MaxEventsHistoryExceeded,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Provider for the block number.
		type BlockNumberProvider: BlockNumberProvider;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type PalletIndex<T: Config> =
		StorageMap<_, Blake2_128Concat, BoundedVec<u8, ConstU32<64>>, u16>;

	#[pallet::storage]
	pub type IndexToPallet<T: Config> =
		StorageMap<_, Blake2_128Concat, u16, BoundedVec<u8, ConstU32<64>>>;

	#[pallet::storage]
	pub type GenesisNetworkId<T: Config> = StorageValue<_, NetworkId, ValueQuery>;

	#[pallet::storage]
	pub type ActivityChain<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Twox64Concat,
		u32,
		ActivityRecord,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type ActivityCounter<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, u32, ValueQuery>;
}

impl<T: Config> Pallet<T> {
	pub fn get_or_add_pallet_index(pallet_name: &str) -> Result<u16, IdentifierError> {
		let bounded_name: BoundedVec<u8, ConstU32<64>> = pallet_name
			.as_bytes()
			.to_vec()
			.try_into()
			.map_err(|_| IdentifierError::PalletNameTooLong)?;

		if let Some(index) = PalletIndex::<T>::get(&bounded_name) {
			return Ok(index);
		}

		let current_index = INDEX + PalletIndex::<T>::iter().count() as u16;
		ensure!(current_index <= u16::MAX, IdentifierError::InvalidPalletIndex);

		PalletIndex::<T>::insert(&bounded_name, current_index);
		IndexToPallet::<T>::insert(current_index, bounded_name);

		Ok(current_index)
	}

	pub fn resolve_pallet_name(index: u16) -> Result<String, IdentifierError> {
		IndexToPallet::<T>::get(index).ok_or(IdentifierError::PalletNotFound).and_then(
			|name_bytes| {
				String::from_utf8(name_bytes.into())
					.map_err(|_| IdentifierError::InvalidPalletNameFormat)
			},
		)
	}

	pub fn set_network_id(network_id: NetworkId) {
		GenesisNetworkId::<T>::put(network_id);
	}

	pub fn get_network_id() -> NetworkId {
		GenesisNetworkId::<T>::get()
	}

	/// Record an activity event for the given identifier by appending a new record.
	pub fn record_activity(
		identifier: &Ss58Identifier,
		entry: EntryTypeOf,
		stamp: EventStamp,
	) -> Result<(), IdentifierError> {
		let record = ActivityRecord { entry, event_stamp: stamp };
		let index = ActivityCounter::<T>::get(identifier);
		ActivityChain::<T>::insert(identifier, index, record);
		ActivityCounter::<T>::insert(identifier, index + 1);
		Ok(())
	}
}

#[derive(
	Clone, Eq, PartialEq, Ord, PartialOrd, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo,
)]
pub struct Ss58Identifier(pub(crate) BoundedVec<u8, ConstU32<60>>);

pub trait Identifier {
	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, IdentifierError>;
	fn resolve_identifier(
		identifier: &Ss58Identifier,
	) -> Result<DecodedIdentifier, IdentifierError>;
	fn resolve_pallet(index: u16) -> Result<String, IdentifierError>;
	/// Record an activity event for the given identifier.
	fn record_activity(
		identifier: &Ss58Identifier,
		entry: EntryTypeOf,
		stamp: EventStamp,
	) -> Result<(), IdentifierError>;
}

impl Ss58Identifier {
	fn ss58hash(data: &[u8]) -> Vec<u8> {
		use blake2::{Blake2b512, Digest};

		let mut context = Blake2b512::new();
		context.update(PREFIX);
		context.update(data);
		context.finalize().to_vec()
	}

	pub fn to_encoded<I>(data: I, pid: &u16, nid: &NetworkId) -> Result<Self, IdentifierError>
	where
		I: AsRef<[u8]> + Into<Vec<u8>>,
	{
		let ident_type = Self::fetch_ident(nid.inner() as u16, *pid);
		let ident = Self::compact_encode(ident_type & 0b0011_1111_1111_1111)?;
		let nid_inner = nid.inner();
		let p = Self::compact_encode(*pid & 0b0011_1111_1111_1111)?;

		let mut buffer = Vec::new();
		buffer.extend(ident);
		buffer.extend(data.as_ref());
		buffer.extend(&nid_inner.to_le_bytes());
		buffer.extend(p);

		let checksum = &Self::ss58hash(&buffer)[..2];
		buffer.extend(checksum);

		let encoded = bs58::encode(&buffer).into_string();

		Ok(Self(
			Vec::<u8>::from(encoded)
				.try_into()
				.map_err(|_| IdentifierError::InvalidIdentifier)?,
		))
	}

	pub fn to_decoded(&self) -> Result<DecodedIdentifier, IdentifierError> {
		let decoded =
			bs58::decode(&self.0).into_vec().map_err(|_| IdentifierError::InvalidFormat)?;
		ensure!(
			decoded.len() >= 2 && decoded.len() <= 60,
			IdentifierError::InvalidIdentifierLength
		);

		let (_ident, mut offset) = Self::compact_decode(&decoded)?;

		let payload_end = offset + 32; // 32 bytes payload
		let payload = &decoded[offset..payload_end].to_vec();
		offset = payload_end;

		let nid_offset = offset + 4;
		let network_id = u32::from_le_bytes(
			decoded[offset..nid_offset]
				.try_into()
				.map_err(|_| IdentifierError::InvalidNetworkId)?,
		);
		offset = nid_offset;

		let (pallet_index, _pid_offset) = Self::compact_decode(&decoded[offset..])?;

		let checksum = &decoded[decoded.len() - 2..];
		let expected_checksum = &Self::ss58hash(&decoded[..decoded.len() - 2])[..2];
		ensure!(checksum == expected_checksum, IdentifierError::InvalidChecksum);

		Ok(DecodedIdentifier {
			network: network_id,
			pallet: pallet_index,
			digest: format!("0x{:02x?}", HexDisplay::from(payload)),
		})
	}

	fn compact_decode(data: &[u8]) -> Result<(u16, usize), IdentifierError> {
		match data[0] {
			0..=63 => Ok((data[0] as u16, 1)),
			64..=127 => {
				ensure!(data.len() >= 2, IdentifierError::InvalidPrefix);
				let lower = (data[0] << 2) | (data[1] >> 6);
				let upper = data[1] & 0b0011_1111;
				Ok(((lower as u16) | ((upper as u16) << 8), 2))
			},
			_ => Err(IdentifierError::InvalidPrefix),
		}
	}

	fn compact_encode(value: u16) -> Result<Vec<u8>, IdentifierError> {
		match value {
			0..=63 => Ok(vec![value as u8]),
			64..=16_383 => {
				let first = ((value & 0b0000_0000_1111_1100) as u8) >> 2;
				let second = ((value >> 8) as u8) | ((value & 0b0000_0000_0000_0011) as u8) << 6;
				Ok(vec![first | 0b01000000, second])
			},
			_ => Err(IdentifierError::InvalidPrefix),
		}
	}

	fn fetch_ident(nid: u16, pid: u16) -> u16 {
		let seed = nid as u32 ^ pid as u32;
		let value = seed.wrapping_mul(1664525).wrapping_add(1013904223);

		if nid == 1000 {
			// List of values for nid == 1000
			let options = [10191, 10447, 10959, 10703, 10456, 10200];
			let index = (value % options.len() as u32) as usize;
			options[index]
		} else {
			// List of values for nid != 1000
			let options = [2860, 3893, 3390, 3134, 3646, 3390, 4926, 4670, 3902, 4926];
			let index = (value % options.len() as u32) as usize;
			options[index]
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedIdentifier {
	pub network: u32,
	pub pallet: u16,
	pub digest: String,
}

impl<T: Config> Identifier for Pallet<T> {
	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, IdentifierError> {
		let pallet_index = Self::get_or_add_pallet_index(pallet)?;
		let network_id = Self::get_network_id();

		Ss58Identifier::to_encoded(digest, &pallet_index, &network_id)
	}

	fn resolve_identifier(
		identifier: &Ss58Identifier,
	) -> Result<DecodedIdentifier, IdentifierError> {
		identifier.to_decoded()
	}

	fn resolve_pallet(index: u16) -> Result<String, IdentifierError> {
		Self::resolve_pallet_name(index)
	}

	fn record_activity(
		identifier: &Ss58Identifier,
		entry: EntryTypeOf,
		stamp: EventStamp,
	) -> Result<(), IdentifierError> {
		Self::record_activity(identifier, entry, stamp)
	}
}

impl TryFrom<Vec<u8>> for Ss58Identifier {
	type Error = IdentifierError;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let bounded =
			BoundedVec::try_from(value).map_err(|_| IdentifierError::InvalidIdentifierLength)?;
		Ok(Ss58Identifier(bounded))
	}
}

impl EventStamp {
	/// Returns the current event stamp from the caller’s runtime context.
	pub fn current<T: frame_system::Config>() -> Self {
		Self {
			height: frame_system::Pallet::<T>::current_block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}

/// A trait that can be used to ensure that a registry identifier exists and is active..
pub trait RegistryIdentifierCheck {
	/// Checks that the registry identified by `registry_id` exists and is active.
	fn ensure_active_registry(registry_id: &Ss58Identifier) -> DispatchResult;
}

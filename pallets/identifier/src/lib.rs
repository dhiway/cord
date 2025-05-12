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
use alloc::{string::String, vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::identifier::{DecodedIdentifier, IdentifierError, Ss58Identifier};
use frame_support::{
	dispatch::DispatchResult, ensure, pallet_prelude::*, traits::ConstU32, BoundedVec,
};
use scale_info::TypeInfo;
use sp_runtime::traits::{BlockNumberProvider, UniqueSaturatedInto};

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

/// The starting index for pallets.
const INDEX: u16 = 64;
pub use pallet::*;
pub type HashOf<T> = <T as frame_system::Config>::Hash;

/// EventBlock marks the block and extrinsic where an event occurred.
#[derive(
	Encode, Decode, Debug, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
pub struct EventBlock {
	pub height: u32,
	pub index: u32,
}

/// EntryTypeOf is a bounded vector (max 128 bytes) that holds part of an event message,
pub type EventTypeOf = BoundedVec<u8, ConstU32<128>>;

/// ActivityRecord stores an update entry and the corresponding event stamp.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct StateEvent<Hash> {
	pub event: EventTypeOf,
	pub digest: Hash,
	pub seal: EventBlock,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	// use frame_support::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
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
	pub type NextPalletIndex<T: Config> = StorageValue<_, u16, ValueQuery>;

	#[pallet::storage]
	pub type GenesisNetworkId<T: Config> = StorageValue<_, u16, ValueQuery>;

	#[pallet::storage]
	pub type StateHistory<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Twox64Concat,
		u32,
		StateEvent<HashOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type StateVersion<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An identifier's state was updated.
		State { identifier: Ss58Identifier, version: u32, event: EventTypeOf },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
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
		// State Update Failed
		StateUpdateFailed,
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
		/// The provided digest length is invalid. Expected 32 bytes.
		InvalidDigestLength,
		/// The value is out of the expected range for compact encoding.
		CompactValueOutOfRange,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _config: core::marker::PhantomData<T>,
		pub network_id: u16,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { network_id: 2000, _config: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			if self.network_id < 2000 || self.network_id > 16_383 {
				panic!(
					"Invalid genesis `network_id` = {}: must be between 2000 and 16_383",
					self.network_id
				);
			}
			GenesisNetworkId::<T>::put(self.network_id);
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_or_add_pallet_index(pallet_name: &str) -> Result<u16, Error<T>> {
		let bounded_name: BoundedVec<u8, ConstU32<64>> = pallet_name
			.as_bytes()
			.to_vec()
			.try_into()
			.map_err(|_| Error::<T>::PalletNameTooLong)?;

		if let Some(index) = PalletIndex::<T>::get(&bounded_name) {
			return Ok(index);
		}

		let current_index = INDEX + NextPalletIndex::<T>::get() as u16;
		ensure!(current_index <= u16::MAX, Error::<T>::InvalidPalletIndex);

		PalletIndex::<T>::insert(&bounded_name, current_index);
		IndexToPallet::<T>::insert(current_index, bounded_name);
		NextPalletIndex::<T>::put(current_index.saturating_add(1));

		Ok(current_index)
	}

	pub fn resolve_pallet_name(index: u16) -> Result<String, Error<T>> {
		IndexToPallet::<T>::get(index)
			.ok_or(Error::<T>::PalletNotFound)
			.and_then(|name_bytes| {
				String::from_utf8(name_bytes.into())
					.map_err(|_| Error::<T>::InvalidPalletNameFormat)
			})
	}

	pub fn get_network_id() -> u16 {
		GenesisNetworkId::<T>::get()
	}

	/// Record an activity event for the given identifier by appending a new record.
	pub fn update_identifier_state(
		identifier: &Ss58Identifier,
		digest: HashOf<T>,
		event: EventTypeOf,
		seal: EventBlock,
	) -> DispatchResult {
		let index = StateVersion::<T>::get(identifier);
		let record = StateEvent { event: event.clone(), digest, seal };
		StateHistory::<T>::insert(identifier, index, record);
		StateVersion::<T>::insert(identifier, index.saturating_add(1));

		Self::deposit_event(Event::State { identifier: identifier.clone(), version: index, event });

		Ok(())
	}
}

impl<T: Config> From<IdentifierError> for Error<T> {
	fn from(err: IdentifierError) -> Self {
		match err {
			IdentifierError::InvalidFormat => Self::InvalidFormat,
			IdentifierError::InvalidPrefix => Self::InvalidPrefix,
			IdentifierError::InvalidIdentifier => Self::InvalidIdentifier,
			IdentifierError::InvalidChecksum => Self::InvalidChecksum,
			IdentifierError::InvalidIdentifierLength => Self::InvalidIdentifierLength,
			IdentifierError::CompactValueOutOfRange => Self::CompactValueOutOfRange,
			IdentifierError::InvalidDigestLength => Self::InvalidDigestLength,
		}
	}
}

pub trait Identifier<T: pallet::Config> {
	type Hash: Encode + Decode + DecodeWithMemTracking + Clone + PartialEq + Eq;
	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, pallet::Error<T>>;
	fn resolve_identifier(
		identifier: &Ss58Identifier,
	) -> Result<DecodedIdentifier, pallet::Error<T>>;
	fn resolve_pallet(index: u16) -> Result<String, pallet::Error<T>>;
	/// Record a state trsition event for the given identifier.
	fn state_event(
		identifier: &Ss58Identifier,
		digest: Self::Hash,
		event: EventTypeOf,
		stamp: EventBlock,
	) -> Result<(), pallet::Error<T>>;
}

impl<T: pallet::Config> Identifier<T> for Pallet<T> {
	type Hash = HashOf<T>;
	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, pallet::Error<T>> {
		let pid = Self::get_or_add_pallet_index(pallet)?;
		let nid = Self::get_network_id();
		Ss58Identifier::to_encoded(digest, nid, pid).map_err(Into::into)
	}

	fn resolve_identifier(
		identifier: &Ss58Identifier,
	) -> Result<DecodedIdentifier, pallet::Error<T>> {
		identifier.to_decoded().map_err(Into::into)
	}

	fn resolve_pallet(index: u16) -> Result<String, pallet::Error<T>> {
		Self::resolve_pallet_name(index)
	}

	fn state_event(
		identifier: &Ss58Identifier,
		digest: HashOf<T>,
		event: EventTypeOf,
		stamp: EventBlock,
	) -> Result<(), pallet::Error<T>> {
		Self::update_identifier_state(identifier, digest, event, stamp)
			.map_err(|_| pallet::Error::<T>::StateUpdateFailed)
	}
}

impl EventBlock {
	/// Returns the current event stamp from the caller’s runtime context.
	pub fn current<T: frame_system::Config>() -> Self {
		Self {
			height: frame_system::Pallet::<T>::current_block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}

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

//! # CORD Dato Token (Token)
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};

use cord_primitives::{
	authorization::{
		authorization_signature_hash as primitives_authorization_signature_hash,
		Authorization as CoreAuthorization,
	},
	identifier::{DecodedIdentifier, IdentifierError, Ss58Identifier},
	view::{base64_string, hex_string, maybe_utf8, DevEventBlockView, InfoTokenHistoryEntry},
	view_api::AuthorizationError,
	Signature,
};
use core::{cmp, convert::TryInto};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	traits::{ConstU32, Get},
	BoundedVec,
};
use scale_info::TypeInfo;
use sp_core as _;
use sp_runtime::{
	traits::{BlockNumberProvider, UniqueSaturatedInto, Verify},
	AccountId32,
};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
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

impl EventBlock {
	/// Returns the current event stamp from the caller’s runtime context.
	pub fn current<T: frame_system::Config>() -> Self {
		Self {
			height: frame_system::Pallet::<T>::current_block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}

pub trait Token<T: frame_system::Config> {
	type Hash: Encode + Decode + DecodeWithMemTracking + Clone + PartialEq + Eq;
	type Error;

	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, pallet::Error<T>>;
	fn resolve_token(token: &Ss58Identifier) -> Result<DecodedIdentifier, Self::Error>;
	fn resolve_pallet(index: u16) -> Result<String, Self::Error>;
	fn state_event(
		token: &Ss58Identifier,
		digest: Self::Hash,
		action: EventTypeOf,
		stamp: EventBlock,
	) -> Result<(), Self::Error>;
}

/// EntryTypeOf is a bounded vector (max 128 bytes) that holds part of an event message.
pub type EventTypeOf = BoundedVec<u8, ConstU32<128>>;

/// Maximum payload size for view authorizations.
pub type AuthorizationPayloadOf<T> = BoundedVec<u8, <T as Config>::MaxAuthorizationLen>;

/// Authorization required for read-only token views.
pub type Authorization<T> =
	CoreAuthorization<<T as frame_system::Config>::AccountId, AuthorizationPayloadOf<T>, Signature>;

/// Replay-protection hash for view authorizations.
pub type AuthorizationSignatureHash = [u8; 16];

/// ActivityRecord stores an update entry and the corresponding event stamp.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct StateEvent<Hash> {
	pub action: EventTypeOf,
	pub digest: Hash,
	pub seal: EventBlock,
}

pub type StateEventOf<T> = StateEvent<HashOf<T>>;

pub type TimelineEventsOf<T> = BoundedVec<StateEventOf<T>, <T as Config>::MaxTimelineViewResults>;
pub type TimelineHistoryOf<T> =
	BoundedVec<InfoTokenHistoryEntry, <T as Config>::MaxTimelineViewResults>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Provider for the block number.
		type BlockNumberProvider: BlockNumberProvider;

		/// Maximum view payload length.
		#[pallet::constant]
		type MaxAuthorizationLen: Get<u32>;

		/// Maximum number of history entries returned per view request.
		#[pallet::constant]
		type MaxTimelineViewResults: Get<u32>;

		// Default limit when the caller doesn't provide one
		#[pallet::constant]
		type DefaulTimelineViewResults: Get<u32>;
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
	pub type IsOriginChain<T: Config> = StorageValue<_, bool, ValueQuery>;

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

	#[pallet::storage]
	pub type ViewSignatureUses<T: Config> =
		StorageMap<_, Blake2_128Concat, AuthorizationSignatureHash, (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A token state was updated.
		StateChange { token: Ss58Identifier, state: u32, action: EventTypeOf },
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
		/// The token format is invalid.
		InvalidTokenFormat,
		/// The prefix is invalid or unrecognized.
		InvalidTokenPrefix,
		/// The token is not valid.
		InvalidToken,
		/// The checksum validation failed.
		InvalidTokenChecksum,
		/// The token length is not valid.
		InvalidTokenLength,
		/// The provided digest length is invalid. Expected 32 bytes.
		InvalidDigestLength,
		/// The value is out of the expected range for compact encoding.
		CompactValueOutOfRange,
		/// A compact‐encoded value used the wrong byte‐length form.
		InvalidCompactEncoding,
		/// The origin‐mode flag was not 0 or 1.
		InvalidMode,
		/// View authorization failed verification.
		InvalidAuthorization,
		/// View authorization signature was reused.
		AuthorizationReplay,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub _config: core::marker::PhantomData<T>,
		pub protocol_id: String,
		pub network_id: u16,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { protocol_id: "0rigin".into(), network_id: 1000, _config: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let proto = self.protocol_id.as_str();
			assert!(
				matches!(proto, "c0rd" | "0rigin" | "0rbit"),
				"Invalid protocol_id `{}` — must be `c0rd`, `0rigin`, or `0rbit`",
				proto,
			);

			let is_origin = matches!(proto, "0rigin" | "0rbit");
			IsOriginChain::<T>::put(is_origin);

			let chain_id: u16 = if is_origin {
				assert!(
					(1_000..16_383).contains(&self.network_id),
					"ChainId ({}) must be > 2000 and < 16383 in Origin mode",
					self.network_id
				);
				self.network_id
			} else {
				assert!(
					(100..999).contains(&self.network_id),
					"ChainId ({}) must be ≥ 100 and < 1999 in standalone mode",
					self.network_id
				);
				self.network_id
			};

			GenesisNetworkId::<T>::put(chain_id);
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T>
	where
		AccountId32: From<<T as frame_system::Config>::AccountId>,
		<T as frame_system::Config>::AccountId: Clone,
	{
		/// Returns the pallet index previously assigned to the provided name.
		pub fn pallet_index_of(
			auth: Authorization<T>,
			name: Vec<u8>,
		) -> Result<u16, AuthorizationError> {
			Self::authorize_view(&auth)?;
			let bounded: BoundedVec<u8, ConstU32<64>> =
				name.try_into().map_err(|_| AuthorizationError::InvalidInput)?;
			PalletIndex::<T>::get(&bounded).ok_or(AuthorizationError::NotFound)
		}

		/// Returns the pallet name bytes stored for an index.
		pub fn pallet_name(
			auth: Authorization<T>,
			index: u16,
		) -> Result<Vec<u8>, AuthorizationError> {
			Self::authorize_view(&auth)?;
			IndexToPallet::<T>::get(index)
				.map(Into::into)
				.ok_or(AuthorizationError::NotFound)
		}

		/// Returns the next pallet index counter.
		pub fn next_pallet_index(auth: Authorization<T>) -> Result<u16, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Ok(NextPalletIndex::<T>::get())
		}

		/// Returns the configured genesis network identifier.
		pub fn genesis_network_id(auth: Authorization<T>) -> Result<u16, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Ok(GenesisNetworkId::<T>::get())
		}

		/// Returns whether the chain is running in origin mode.
		pub fn origin_chain_flag(auth: Authorization<T>) -> Result<bool, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Ok(IsOriginChain::<T>::get())
		}

		/// Returns the current state version counter for a token.
		pub fn state_version(
			auth: Authorization<T>,
			token: Ss58Identifier,
		) -> Result<u32, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Ok(StateVersion::<T>::get(&token))
		}

		/// Returns a specific state event for a token and version.
		pub fn state_event_view(
			auth: Authorization<T>,
			token: Ss58Identifier,
			version: u32,
		) -> Result<StateEventOf<T>, AuthorizationError> {
			Self::authorize_view(&auth)?;
			StateHistory::<T>::get(&token, version).ok_or(AuthorizationError::NotFound)
		}

		/// Returns a bounded list of state events mirroring direct storage scans.
		pub fn state_events(
			auth: Authorization<T>,
			token: Ss58Identifier,
			start: Option<u32>,
			limit: u32,
		) -> Result<TimelineEventsOf<T>, AuthorizationError> {
			Self::authorize_view(&auth)?;
			let capped = cmp::min(limit, T::MaxTimelineViewResults::get());
			Ok(Self::timeline_entries(&token, start, capped))
		}

		pub fn timeline(
			auth: Authorization<T>,
			token: Ss58Identifier,
			start: Option<u32>,
			limit: Option<u32>,
		) -> Result<TimelineHistoryOf<T>, AuthorizationError> {
			Self::authorize_view(&auth)?;
			let cap = T::MaxTimelineViewResults::get();
			let def = T::DefaulTimelineViewResults::get();
			let eff = limit.unwrap_or(def).min(cap);

			let events = Self::timeline_entries(&token, start, eff);
			let mut history = TimelineHistoryOf::<T>::default();
			for event in events.into_iter() {
				let _ = history.try_push(Self::info_token_history_entry(event));
			}
			Ok(history)
		}

		pub fn resolve_identifier(
			auth: Authorization<T>,
			token: Ss58Identifier,
		) -> Result<DecodedIdentifier, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Self::resolve_identifier_plain(&token)
		}

		pub fn resolve_pallet(
			auth: Authorization<T>,
			index: u16,
		) -> Result<String, AuthorizationError> {
			Self::authorize_view(&auth)?;
			Self::resolve_pallet_plain(index)
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

		let next_offset = NextPalletIndex::<T>::get();
		let current_index = INDEX.saturating_add(next_offset);
		ensure!(current_index <= u16::MAX, Error::<T>::InvalidPalletIndex);

		PalletIndex::<T>::insert(&bounded_name, current_index);
		IndexToPallet::<T>::insert(current_index, bounded_name);
		NextPalletIndex::<T>::put(next_offset.saturating_add(1));

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

	pub fn is_origin_chain() -> bool {
		IsOriginChain::<T>::get()
	}

	/// Record an activity event for the given token by appending a new record.
	pub fn update_token_state(
		token: &Ss58Identifier,
		digest: HashOf<T>,
		action: EventTypeOf,
		seal: EventBlock,
	) -> DispatchResult {
		let index = StateVersion::<T>::get(&token);
		let record = StateEvent { action: action.clone(), digest, seal };
		StateHistory::<T>::insert(&token, index, record);
		StateVersion::<T>::insert(&token, index.saturating_add(1));

		Self::deposit_event(Event::StateChange { token: token.clone(), state: index, action });

		Ok(())
	}
}

impl<T: Config> From<IdentifierError> for Error<T> {
	fn from(err: IdentifierError) -> Self {
		match err {
			IdentifierError::InvalidFormat => Self::InvalidTokenFormat,
			IdentifierError::InvalidPrefix => Self::InvalidTokenPrefix,
			IdentifierError::InvalidIdentifier => Self::InvalidToken,
			IdentifierError::InvalidChecksum => Self::InvalidTokenChecksum,
			IdentifierError::InvalidIdentifierLength => Self::InvalidTokenLength,
			IdentifierError::CompactValueOutOfRange => Self::CompactValueOutOfRange,
			IdentifierError::InvalidDigestLength => Self::InvalidDigestLength,
			IdentifierError::InvalidCompactEncoding => Self::InvalidCompactEncoding,
			IdentifierError::InvalidMode => Self::InvalidMode,
		}
	}
}

impl<T> Pallet<T>
where
	T: Config,
	AccountId32: From<<T as frame_system::Config>::AccountId>,
	<T as frame_system::Config>::AccountId: Clone,
{
	fn authorization_signature_hash(auth: &Authorization<T>) -> AuthorizationSignatureHash {
		primitives_authorization_signature_hash(
			&auth.account,
			auth.payload.as_slice(),
			&auth.signature,
		)
	}

	fn authorize_view(auth: &Authorization<T>) -> Result<(), AuthorizationError> {
		let signer: AccountId32 = auth.account.clone().into();
		if !auth.signature.verify(auth.payload.as_slice(), &signer) {
			return Err(AuthorizationError::Unauthorized);
		}
		let hash = Self::authorization_signature_hash(auth);
		if ViewSignatureUses::<T>::contains_key(&hash) {
			return Err(AuthorizationError::Unauthorized);
		}
		ViewSignatureUses::<T>::insert(hash, ());
		Ok(())
	}

	fn info_token_event_block(block: &EventBlock) -> DevEventBlockView {
		DevEventBlockView { height: block.height, index: block.index }
	}

	fn info_token_history_entry(entry: StateEventOf<T>) -> InfoTokenHistoryEntry {
		let StateEvent { action, digest, seal } = entry;
		let action_bytes = action.as_slice();
		InfoTokenHistoryEntry {
			action_utf8: maybe_utf8(action_bytes),
			action_hex: hex_string(action_bytes),
			action_base64: base64_string(action_bytes),
			digest_hex: hex_string(digest.as_ref()),
			block: Self::info_token_event_block(&seal),
		}
	}

	pub fn timeline_entries(
		token: &Ss58Identifier,
		start: Option<u32>,
		limit: u32,
	) -> TimelineEventsOf<T> {
		let mut results = TimelineEventsOf::<T>::default();
		let upper = StateVersion::<T>::get(token);
		if upper == 0 {
			return results;
		}
		let start_index = start.unwrap_or(0);
		if start_index >= upper {
			return results;
		}
		let max = cmp::min(limit, T::MaxTimelineViewResults::get());
		let mut index = start_index;
		while index < upper && (results.len() as u32) < max {
			if let Some(event) = StateHistory::<T>::get(token, index) {
				let _ = results.try_push(event);
			}
			index = index.saturating_add(1);
		}
		results
	}

	pub fn timeline_view(
		auth: Authorization<T>,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: u32,
	) -> Result<TimelineEventsOf<T>, AuthorizationError> {
		Self::authorize_view(&auth)?;
		Ok(Self::timeline_entries(&token, start, limit))
	}

	pub fn history_view(
		auth: Authorization<T>,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: u32,
	) -> Result<TimelineEventsOf<T>, AuthorizationError> {
		Self::authorize_view(&auth)?;
		Ok(Self::timeline_entries(&token, start, limit))
	}

	pub fn resolve_identifier_plain(
		token: &Ss58Identifier,
	) -> Result<DecodedIdentifier, AuthorizationError> {
		Self::resolve_token(token).map_err(|_| AuthorizationError::InvalidInput)
	}

	pub fn resolve_identifier_view(
		auth: Authorization<T>,
		token: Ss58Identifier,
	) -> Result<DecodedIdentifier, AuthorizationError> {
		Self::authorize_view(&auth)?;
		Self::resolve_identifier_plain(&token)
	}

	pub fn resolve_pallet_plain(index: u16) -> Result<String, AuthorizationError> {
		match Self::resolve_pallet_name(index) {
			Ok(name) => Ok(name),
			Err(Error::<T>::PalletNotFound) => Err(AuthorizationError::NotFound),
			Err(_) => Err(AuthorizationError::InvalidInput),
		}
	}

	pub fn resolve_pallet_view(
		auth: Authorization<T>,
		index: u16,
	) -> Result<String, AuthorizationError> {
		Self::authorize_view(&auth)?;
		Self::resolve_pallet_plain(index)
	}
}

impl<T: pallet::Config> Token<T> for Pallet<T> {
	type Hash = HashOf<T>;
	type Error = pallet::Error<T>;

	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, Self::Error> {
		let pid = Self::get_or_add_pallet_index(pallet)?;
		let nid = Self::get_network_id();
		let ori = Self::is_origin_chain() as u8;
		Ss58Identifier::to_encoded(digest, nid, pid, ori).map_err(Into::into)
	}

	fn resolve_token(token: &Ss58Identifier) -> Result<DecodedIdentifier, Self::Error> {
		token.to_decoded().map_err(Into::into)
	}

	fn resolve_pallet(index: u16) -> Result<String, Self::Error> {
		Self::resolve_pallet_name(index)
	}

	fn state_event(
		token: &Ss58Identifier,
		digest: Self::Hash,
		event: EventTypeOf,
		stamp: EventBlock,
	) -> Result<(), Self::Error> {
		Self::update_token_state(token, digest, event, stamp)
			.map_err(|_| pallet::Error::<T>::StateUpdateFailed)
	}
}

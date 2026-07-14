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

//! # CORD Data Token (Token)
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};

use core::convert::TryInto;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	traits::{ConstU32, Get},
	BoundedVec,
};
use origin_primitives::{
	authorization::{
		ensure_authorization_ttl, extract_valid_until, Authorization as CoreAuthorization,
		AuthorizationError,
	},
	entity::EventBlockView,
	identifier::{DecodedIdentifier, IdentifierError, Ss58Identifier},
	token::TokenStateEventView,
	Signature,
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

/// Maximum payload size for token authorization payloads.
pub type AuthorizationPayloadOf<T> = BoundedVec<u8, <T as Config>::MaxAuthorizationLen>;

/// Authorization required for read-only token queries.
pub type Authorization<T> =
	CoreAuthorization<<T as frame_system::Config>::AccountId, AuthorizationPayloadOf<T>, Signature>;

/// ActivityRecord stores an update entry and the corresponding event stamp.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug,
)]
pub struct StateEvent<Hash> {
	pub action: EventTypeOf,
	pub digest: Hash,
	pub seal: EventBlock,
}

pub type StateEventOf<T> = StateEvent<HashOf<T>>;

pub type TimelineEventsOf<T> = BoundedVec<StateEventOf<T>, <T as Config>::MaxTimelineViewResults>;

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
		type DefaultTimelineViewResults: Get<u32>;

		/// Maximum number of blocks for which an authorization stays valid.
		#[pallet::constant]
		type MaxAuthorizationTTL: Get<u32>;
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
		pub network_id: u16,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { network_id: 29, _config: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			assert!(
				(1..16_383).contains(&self.network_id),
				"networkId ({}) must be between 1 and 16382 for Origin chains",
				self.network_id
			);

			GenesisNetworkId::<T>::put(self.network_id);
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: Clone + Into<AccountId32>,
		AccountId32: From<T::AccountId>,
	{
		/// Returns the pallet index previously assigned to the provided name.
		pub fn pallet_index_of(auth: Authorization<T>, name: Vec<u8>) -> Option<u16> {
			Self::authorize_query(&auth).ok()?;
			let bounded: BoundedVec<u8, ConstU32<64>> = name.try_into().ok()?;
			PalletIndex::<T>::get(&bounded)
		}

		/// Returns the pallet name string stored for an index. The distinct method name avoids
		/// clashing with generated view-function types.
		pub fn pallet_name_view(auth: Authorization<T>, index: u16) -> Option<String> {
			Self::authorize_query(&auth).ok()?;
			Self::resolve_pallet_plain(index).ok()
		}

		/// Returns the next pallet index counter.
		pub fn next_pallet_index(auth: Authorization<T>) -> Option<u16> {
			Self::authorize_query(&auth).ok()?;
			Some(NextPalletIndex::<T>::get())
		}

		/// Returns the configured genesis network identifier.
		pub fn genesis_network_id(auth: Authorization<T>) -> Option<u16> {
			Self::authorize_query(&auth).ok()?;
			Some(GenesisNetworkId::<T>::get())
		}

		/// Returns the current state version counter for a token.
		pub fn state_version(auth: Authorization<T>, token: Ss58Identifier) -> Option<u32> {
			Self::authorize_query(&auth).ok()?;
			Some(StateVersion::<T>::get(&token))
		}

		/// Returns a specific state event for a token and version.
		pub fn state_event(
			auth: Authorization<T>,
			token: Ss58Identifier,
			version: u32,
		) -> Option<TokenStateEventView<HashOf<T>>> {
			Self::authorize_query(&auth).ok()?;
			let event = StateHistory::<T>::get(&token, version)?;
			Some(Self::state_event_view(&event))
		}

		/// Returns a page of state events for a token, starting from an optional cursor.
		pub fn timeline(
			auth: Authorization<T>,
			token: Ss58Identifier,
			start: Option<u32>,
			limit: Option<u32>,
		) -> Option<(Vec<TokenStateEventView<HashOf<T>>>, Option<u32>)> {
			Self::authorize_query(&auth).ok()?;

			let cap = T::MaxTimelineViewResults::get();
			let def = T::DefaultTimelineViewResults::get();
			let eff = limit.unwrap_or(def).max(1).min(cap);

			let (events, next_cursor) = Self::timeline_entries(&token, start, eff);
			let view_events: Vec<_> = events.iter().map(Self::state_event_view).collect();

			Some((view_events, next_cursor))
		}

		/// Returns the decoded identifier components for the given token.
		pub fn resolve_identifier(
			auth: Authorization<T>,
			token: Ss58Identifier,
		) -> Option<DecodedIdentifier> {
			Self::authorize_query(&auth).ok()?;
			Self::resolve_identifier_plain(&token).ok()
		}

		/// Returns the pallet name for the given index.
		pub fn resolve_pallet(auth: Authorization<T>, index: u16) -> Option<String> {
			Self::authorize_query(&auth).ok()?;
			Self::resolve_pallet_plain(index).ok()
		}

		/// Returns true if the token has at least one recorded state event.
		pub fn has_history(auth: Authorization<T>, token: Ss58Identifier) -> bool {
			if Self::authorize_query(&auth).is_err() {
				return false;
			}
			// StateVersion starts at 0 and increments; >= 1 means we have at least one event
			StateVersion::<T>::get(&token) > 0
		}

		/// Returns the latest state event for a token, if any.
		pub fn latest_state_event(
			auth: Authorization<T>,
			token: Ss58Identifier,
		) -> Option<TokenStateEventView<HashOf<T>>> {
			Self::authorize_query(&auth).ok()?;
			let version = StateVersion::<T>::get(&token);
			if version == 0 {
				return None;
			}
			let last_idx = version.saturating_sub(1);
			let event = StateHistory::<T>::get(&token, last_idx)?;
			Some(Self::state_event_view(&event))
		}

		/// Returns up to `limit` most recent events starting at version 0.
		pub fn recent_timeline(
			auth: Authorization<T>,
			token: Ss58Identifier,
			limit: Option<u32>,
		) -> Option<Vec<TokenStateEventView<HashOf<T>>>> {
			Self::authorize_query(&auth).ok()?;
			let cap = T::MaxTimelineViewResults::get();
			let eff = limit.unwrap_or(cap).max(1).min(cap);
			let (events, _next) = Self::timeline_entries(&token, Some(0), eff);
			let views = events.iter().map(Self::state_event_view).collect();
			Some(views)
		}
	}
}

impl<T: Config> Pallet<T>
where
	T::AccountId: Clone + Into<AccountId32>,
	AccountId32: From<T::AccountId>,
{
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

impl<T: Config> Pallet<T>
where
	T::AccountId: Clone + Into<AccountId32>,
	AccountId32: From<T::AccountId>,
{
	#[inline]
	fn authorize_query(auth: &Authorization<T>) -> Result<(), AuthorizationError> {
		// expiry check
		Self::ensure_authorization_fresh(auth.payload.as_slice())?;

		let signer: AccountId32 = auth.account.clone().into();

		if !auth.signature.verify(auth.payload.as_slice(), &signer) {
			return Err(AuthorizationError::Unauthorized);
		}

		Ok(())
	}

	pub fn timeline_entries(
		token: &Ss58Identifier,
		cursor: Option<u32>,
		limit: u32,
	) -> (TimelineEventsOf<T>, Option<u32>) {
		let mut results = TimelineEventsOf::<T>::default();
		let upper = StateVersion::<T>::get(token);
		if upper == 0 {
			return (results, None);
		}
		let mut index = cursor.unwrap_or(0);
		if index >= upper {
			return (results, None);
		}
		let cap = limit.max(1).min(T::MaxTimelineViewResults::get());
		while index < upper && (results.len() as u32) < cap {
			if let Some(event) = StateHistory::<T>::get(token, index) {
				let _ = results.try_push(event);
			}
			index = index.saturating_add(1);
		}
		let next_cursor = if index < upper { Some(index) } else { None };
		(results, next_cursor)
	}

	fn state_event_view(event: &StateEventOf<T>) -> TokenStateEventView<HashOf<T>> {
		TokenStateEventView {
			action: event.action.to_vec(),
			digest: event.digest.clone(),
			seal: EventBlockView { height: event.seal.height, index: event.seal.index },
		}
	}

	pub fn history(
		auth: Authorization<T>,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: u32,
	) -> Result<TimelineEventsOf<T>, AuthorizationError> {
		Self::authorize_query(&auth)?;
		let capped = limit.max(1).min(T::MaxTimelineViewResults::get());
		Ok(Self::timeline_entries(&token, start, capped).0)
	}

	pub fn resolve_identifier_plain(
		token: &Ss58Identifier,
	) -> Result<DecodedIdentifier, AuthorizationError> {
		Self::resolve_token(token).map_err(|_| AuthorizationError::InvalidInput)
	}

	fn ensure_authorization_fresh(payload: &[u8]) -> Result<(), AuthorizationError> {
		let issued_at = extract_valid_until(payload).ok_or(AuthorizationError::InvalidInput)?;
		let now: u32 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
		let ttl = T::MaxAuthorizationTTL::get();
		ensure_authorization_ttl(now, issued_at, ttl)
	}

	pub fn resolve_identifier_query(
		auth: Authorization<T>,
		token: Ss58Identifier,
	) -> Result<DecodedIdentifier, AuthorizationError> {
		Self::authorize_query(&auth)?;
		Self::resolve_identifier_plain(&token)
	}

	pub fn resolve_pallet_plain(index: u16) -> Result<String, AuthorizationError> {
		match Self::resolve_pallet_name(index) {
			Ok(name) => Ok(name),
			Err(Error::<T>::PalletNotFound) => Err(AuthorizationError::NotFound),
			Err(_) => Err(AuthorizationError::InvalidInput),
		}
	}

	pub fn resolve_pallet_query(
		auth: Authorization<T>,
		index: u16,
	) -> Result<String, AuthorizationError> {
		Self::authorize_query(&auth)?;
		Self::resolve_pallet_plain(index)
	}
}

impl<T: pallet::Config> Token<T> for Pallet<T>
where
	T::AccountId: Clone + Into<AccountId32>,
	AccountId32: From<T::AccountId>,
{
	type Hash = HashOf<T>;
	type Error = pallet::Error<T>;

	fn build(digest: &[u8], pallet: &str) -> Result<Ss58Identifier, Self::Error> {
		let pid = Self::get_or_add_pallet_index(pallet)?;
		let nid = Self::get_network_id();
		let ori: u8 = 1;
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

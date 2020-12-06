// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019  BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

//! DID: Handles decentralized identifiers on chain,
//! adding and removing DIDs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for attestations
#[cfg(test)]
mod tests;

use frame_support::{
	decl_event, decl_module, decl_storage, dispatch::DispatchResult, Parameter, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_runtime::{codec::Codec, traits::Member};
use sp_std::prelude::*;

/// The DID trait
pub trait Trait: frame_system::Trait {
	/// DID specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Public signing key type for DIDs
	type PublicSigningKey: Parameter + Member + Codec + Default;
	/// Public boxing key type for DIDs
	type PublicBoxKey: Parameter + Member + Codec + Default;
}

decl_event!(
	/// Events for DIDs
	pub enum Event<T> where <T as frame_system::Trait>::AccountId {
		/// A did has been created
		DidCreated(AccountId),
		/// A did has been removed
		DidRemoved(AccountId),
	}
);

decl_module! {
	/// The DID runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

		/// Adds a DID on chain, where
		/// origin - the origin of the transaction
		/// sign_key - public signing key of the DID
		/// box_key - public boxing key of the DID
		/// doc_ref - optional reference to the DID document storage
		#[weight = 1]
		pub fn add(origin, sign_key: T::PublicSigningKey, box_key: T::PublicBoxKey, doc_ref: Option<Vec<u8>>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// add DID to the storage
			<DIDs<T>>::insert(sender.clone(), (sign_key, box_key, doc_ref));
			// deposit an event that the DID has been created
			Self::deposit_event(RawEvent::DidCreated(sender));
			Ok(())
		}
		/// Removes a DID from chain storage, where
		/// origin - the origin of the transaction
		#[weight = 1]
		pub fn remove(origin) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// remove DID from storage
			<DIDs<T>>::remove(sender.clone());
			// deposit an event that the DID has been removed
			Self::deposit_event(RawEvent::DidRemoved(sender));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as DID {
		// DID: account-id -> (public-signing-key, public-encryption-key, did-reference?)?
		DIDs get(fn dids):map hasher(opaque_blake2_256) T::AccountId => Option<(T::PublicSigningKey, T::PublicBoxKey, Option<Vec<u8>>)>;
	}
}

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

//! CTYPE: Handles CTYPEs on chain,
//! adding CTYPEs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for CTYPEs
#[cfg(test)]
mod tests;

use frame_support::{
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};

/// The CTYPE trait
pub trait Trait: frame_system::Config + error::Trait {
	/// CTYPE specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for CTYPEs
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash {
		/// A CTYPE has been added
		CTypeCreated(AccountId, Hash),
	}
);

decl_module! {
	/// The CTYPE runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

		/// Adds a CTYPE on chain, where
		/// origin - the origin of the transaction
		/// hash - hash of the CTYPE of the claim
		#[weight = 1]
		pub fn add(origin, hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if CTYPE already exists
			if <CTYPEs<T>>::contains_key(hash) {
				return Self::error(Self::ERROR_CTYPE_ALREADY_EXISTS);
			}

			// add CTYPE to storage
			debug::print!("insert CTYPE");
			<CTYPEs<T>>::insert(hash, sender.clone());
			// deposit event that the CTYPE has been added
			Self::deposit_event(RawEvent::CTypeCreated(sender, hash));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Ctype {
		// CTYPEs: ctype-hash -> account-id?
		pub CTYPEs get(fn ctypes):map hasher(opaque_blake2_256) T::Hash => Option<T::AccountId>;
	}
}

/// Implementation of further module constants and functions for CTYPEs
impl<T: Trait> Module<T> {
	/// Error types for errors in CTYPE module
	pub const ERROR_BASE: u16 = 1000;
	pub const ERROR_CTYPE_NOT_FOUND: error::ErrorType = (Self::ERROR_BASE + 1, "CTYPE not found");
	pub const ERROR_CTYPE_ALREADY_EXISTS: error::ErrorType =
		(Self::ERROR_BASE + 2, "CTYPE already exists");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}

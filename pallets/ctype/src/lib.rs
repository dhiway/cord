// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project


//! #MARK SCHEMA: Handles #MARK SCHEMAs on chain,
//! adding #MARK SCHEMAs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for #MARK SCHEMAs
#[cfg(test)]
mod tests;

use frame_support::{
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};

/// The #MARK SCHEMA trait
pub trait Trait: frame_system::Config + error::Trait {
	/// #MARK SCHEMA specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for #MARK SCHEMAs
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash {
		/// A #MARK SCHEMA has been added
		CTypeCreated(AccountId, Hash),
	}
);

decl_module! {
	/// The #MARK SCHEMA runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

		/// Adds a #MARK SCHEMA on chain, where
		/// origin - the origin of the transaction
		/// hash - hash of the #MARK SCHEMA 
		#[weight = 10]		pub fn add(origin, hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if #MARK SCHEMA already exists
			if <CTYPEs<T>>::contains_key(hash) {
				return Self::error(Self::ERROR_CTYPE_ALREADY_EXISTS);
			}

			// add #MARK SCHEMA to storage
			debug::print!("insert #MARK SCHEMA");
			<CTYPEs<T>>::insert(hash, sender.clone());
			// deposit event that the #MARK SCHEMA has been added
			Self::deposit_event(RawEvent::CTypeCreated(sender, hash));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Ctype {
		// #MARK SCHEMAs: ctype-hash -> account-id?
		pub CTYPEs get(fn ctypes):map hasher(opaque_blake2_256) T::Hash => Option<T::AccountId>;
	}
}

/// Implementation of further module constants and functions for #MARK SCHEMAs
impl<T: Trait> Module<T> {
	/// Error types for errors in #MARK SCHEMA module
	pub const ERROR_BASE: u16 = 1000;
	pub const ERROR_CTYPE_NOT_FOUND: error::ErrorType = (Self::ERROR_BASE + 1, "#MARK SCHEMA not found");
	pub const ERROR_CTYPE_ALREADY_EXISTS: error::ErrorType =
		(Self::ERROR_BASE + 2, "#MARK SCHEMA already exists");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}

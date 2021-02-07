// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for MTYPEs
#[cfg(test)]
mod tests;

use frame_support::{
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};

/// The #MARK TYPE trait
pub trait Trait: frame_system::Config + error::Trait {
	/// #MARK TYPE specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for #MARK TYPEs
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash {
		/// A new #MARK Schema has been anchored
		MTypeAnchored(AccountId, Hash),
	}
);

decl_module! {
	/// The #MARK TYPE runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

		/// Anchors a new #MARK TYPE on chain
		/// origin is the signed sender account, and
		/// hash is the hash of the anchored Type Schema
		#[weight = 1]
		pub fn anchor(origin, hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if #MARK TYPE already exists
			if <MTYPEs<T>>::contains_key(hash) {
				return Self::error(Self::ERROR_MTYPE_ALREADY_EXISTS);
			}

			// Anchors a new #MARK Schema
			debug::print!("insert MTYPE");
			<MTYPEs<T>>::insert(hash, sender.clone());
			// deposit event - #MARK TYPE has been anchored
			Self::deposit_event(RawEvent::MTypeAnchored(sender, hash));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Mtype {
		// MTYPEs: mtype-hash -> account-id?
		pub MTYPEs get(fn mtypes):map hasher(opaque_blake2_256) T::Hash => Option<T::AccountId>;
	}
}

/// Implementation of further module constants and functions for MTYPEs
impl<T: Trait> Module<T> {
	/// Error types for errors in MTYPE module
	pub const ERROR_BASE: u16 = 1000;
	pub const ERROR_MTYPE_NOT_FOUND: error::ErrorType = (Self::ERROR_BASE + 1, "MTYPE not found");
	pub const ERROR_MTYPE_ALREADY_EXISTS: error::ErrorType =
		(Self::ERROR_BASE + 2, "MTYPE already exists");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}
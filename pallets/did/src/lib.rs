// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! DID: Handles decentralized identifiers on chain,
//! adding and removing DIDs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for marks
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::{
	decl_event, decl_module, decl_storage, dispatch::DispatchResult, Parameter, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_runtime::{codec::Codec, traits::Member};
use sp_std::prelude::*;

/// The DID trait
pub trait Config: frame_system::Config {
	/// DID specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// Public signing key type for DIDs
	type PublicSigningKey: Parameter + Member + Codec + Default;
	/// Public boxing key type for DIDs
	type PublicBoxKey: Parameter + Member + Codec + Default;
}

decl_event!(
	/// Events for DIDs
	pub enum Event<T> where <T as frame_system::Config>::AccountId {
		/// A DID has been created
		Anchored(AccountId),
		/// A DID has been removed
		Removed(AccountId),
	}
);

decl_module! {
	/// The DID runtime module
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

		/// Adds a DID Public Key on chain
		/// origin - the origin of the transaction
		/// sign_key - public signing key of the DID
		/// box_key - public boxing key of the DID
		/// doc_ref - optional reference to the DID document storage
		#[weight = 10_000_000]		
		pub fn anchor(origin, sign_key: T::PublicSigningKey, box_key: T::PublicBoxKey, doc_ref: Option<Vec<u8>>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// add DID to the storage
			<DIDs<T>>::insert(sender.clone(), DidRecord::<T> { sign_key, box_key, doc_ref });
			// deposit an event that the DID has been created
			Self::deposit_event(RawEvent::Anchored(sender));
			Ok(())
		}
		/// Removes a DID Public Key from chain storage
		/// origin - the origin of the transaction
		#[weight = 10_000_000]		
		pub fn remove(origin) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// remove DID Public Key from storage
			<DIDs<T>>::remove(sender.clone());
			// deposit an event that the DID Public Key has been removed
			Self::deposit_event(RawEvent::Removed(sender));
			Ok(())
		}
	}
}

#[derive(Encode, Decode)]
pub struct DidRecord<T: Config> {
	// public signing key
	sign_key: T::PublicSigningKey,
	// public encryption key
	box_key: T::PublicBoxKey,
	// did reference
	doc_ref: Option<Vec<u8>>,
}

decl_storage! {
	trait Store for Module<T: Config> as DID {
		// DID: account-id -> (public-signing-key, public-encryption-key, did-reference?)?
		DIDs get(fn dids):map hasher(opaque_blake2_256) T::AccountId => Option<DidRecord<T>>;
	}
}

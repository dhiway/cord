// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Digest: Handles #MARKs presentation digest on chain,
//! adding and revoking #MARKs digest.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for marks
#[cfg(test)]
mod tests;

pub mod benchmarking;

pub mod weights;
use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure, StorageMap};
use frame_system::{self, ensure_signed};
use sp_std::prelude::{Clone, PartialEq};
pub use weights::WeightInfo;

/// The #MARK Digest trait
pub trait Config: frame_system::Config + pallet_mark::Config {
	/// #MARK Digest specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;
}

decl_event!(
	/// Events for Mark Digest
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash {
		/// A new #MARK Digest has been anchored
		Anchored(AccountId, Hash, Hash),
		/// A #MARK Digest has been revoked
		Revoked(AccountId, Hash),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Config> {
		/// The digest has already been anchored.
		AlreadyAnchoredDigest,
		/// The digest does not exist.
		NotFound,
		/// The digest is anchored by another account.
		NotOwner,
		/// The digest is revoked.
		AlreadyRevoked
	}
}

decl_module! {
	/// The #MARK runtime module
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;

		// Initializing errors
		// this includes information about your errors in the node's metadata.
		// it is needed only if you are using errors in your pallet
		type Error = Error<T>;

		/// Anchors a new #MARK Digest on chain
		///, where, origin is the signed sender account,
		/// digest_hash is the hash of the file,
		/// and content_hash is the hash of the #MARK Content.
		#[weight = <T as Config>::WeightInfo::anchor()]
		pub fn anchor(origin, digest_hash: T::Hash, content_hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if the #MARK exists
			let mark = <pallet_mark::Marks<T>>::get(content_hash).ok_or(pallet_mark::Error::<T>::MarkNotFound)?;
			// check for MARK status - revoked?
			ensure!(!mark.revoked, pallet_mark::Error::<T>::AlreadyRevoked);
			// check if the digest already exists
			ensure!(!<Digests<T>>::contains_key(digest_hash), Error::<T>::AlreadyAnchoredDigest);
			// insert #MARK Digest
			<Digests<T>>::insert(digest_hash, Digest {content_hash, marker: sender.clone(), revoked: false});
			// deposit event that mark has beed added
			Self::deposit_event(RawEvent::Anchored(sender, digest_hash, content_hash));
			Ok(())
		}

		/// Revokes a #MARK Digest
		/// where, origin is the signed sender account,
		/// and digest_hash is the hash of the file.
		#[weight = <T as Config>::WeightInfo::revoke()]
		pub fn revoke(origin, digest_hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if it exists
			let Digest {content_hash, marker, revoked, ..} = <Digests<T>>::get(digest_hash).ok_or(Error::<T>::NotFound)?;

			// check if the #MARK Digest has already been revoked
			ensure!(!revoked, Error::<T>::AlreadyRevoked);

			// check the digest has been created by the sender of this transaction
			ensure!(marker.eq(&sender), Error::<T>::NotOwner);
			// revoke #MARK
			<Digests<T>>::insert(digest_hash, Digest {
				content_hash,
				marker,
				revoked: true
			});

			// deposit event that the #MARK has been revoked
			Self::deposit_event(RawEvent::Revoked(sender, digest_hash));
			Ok(())
		}
	}
}

#[derive(Encode, Decode)]
pub struct Digest<T: Config> {
	// hash of the #MARK content
	content_hash: T::Hash,
	// the account which executed the mark
	marker: T::AccountId,
	// revocation status
	revoked: bool,
}

decl_storage! {
	trait Store for Module<T: Config> as Digest {
		/// Digests: digest-hash -> (content-hash, marker-account, revoked)?
		Digests get(fn digests): map hasher(opaque_blake2_256) T::Hash => Option<Digest<T>>;
	}
}

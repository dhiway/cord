// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Mark: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for marks
#[cfg(test)]
mod tests;
mod benchmarking;

use codec::{Decode, Encode};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure, StorageMap,
};
use frame_system::{self, ensure_signed};
use pallet_delegation::Permissions;
use sp_std::prelude::{Clone, PartialEq, Vec};

/// The #MARK trait
pub trait Config: frame_system::Config + pallet_delegation::Config {
	/// #MARK specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for Marks
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash,
			<T as pallet_delegation::Config>::DelegationNodeId {
		/// A new #MARK has been anchored
		Anchored(AccountId, Hash, Hash, Option<DelegationNodeId>),
		/// A #MARK has been revoked
		Revoked(AccountId, Hash),
		/// A #MARK has been restored (previously revoked)
		Restored(AccountId, Hash),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Config> {
		AlreadyAnchored,
		AlreadyRevoked,
		MarkNotFound,
		MTypeMismatch,
		DelegationUnauthorisedToAnchor,
		DelegationRevoked,
		NotDelegatedToMarker,
		UnauthorizedRevocation,
		UnauthorizedRestore,
		MarkStillActive,
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

		/// Anchors a new #MARK on chain
		///, where, originis the signed sender account,
		/// content_hash is the hash of the content stream,
		/// mtype_hash is the hash of the #MARK TYPE,
		/// and delegation_id refers to a #MARK TYPE delegation.
		#[weight = 100_000_000_000]
		pub fn anchor(origin, content_hash: T::Hash, mtype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if the #MARK TYPE exists
			ensure!(<pallet_mtype::MTYPEs<T>>::contains_key(mtype_hash), pallet_mtype::Error::<T>::NotFound);

			// check if the #MARK already exists
			ensure!(!<Marks<T>>::contains_key(content_hash), Error::<T>::AlreadyAnchored);

			if let Some(d) = delegation_id {
				// check if delegation exists
				let delegation = <pallet_delegation::Delegations<T>>::get(d).ok_or(pallet_delegation::Error::<T>::DelegationNotFound)?;
				// check whether delegation has been revoked already
				ensure!(!delegation.revoked, Error::<T>::DelegationRevoked);

				// check whether the owner of the delegation is not the sender of this transaction
				ensure!(delegation.owner.eq(&sender), Error::<T>::NotDelegatedToMarker);
				// check whether the delegation is not set up for attesting claims
				ensure!((delegation.permissions & Permissions::ANCHOR) == Permissions::ANCHOR, Error::<T>::DelegationUnauthorisedToAnchor);

				// check if MTYPE of the delegation is matching the MTYPE of the mark
				let root = <pallet_delegation::Root<T>>::get(delegation.root_id).ok_or(pallet_delegation::Error::<T>::RootNotFound)?;
				ensure!(root.mtype_hash.eq(&mtype_hash), Error::<T>::MTypeMismatch);
			}

			// insert #MARK
			<Marks<T>>::insert(content_hash, Mark {mtype_hash, marker: sender.clone(), delegation_id, revoked: false});

			if let Some(d) = delegation_id {
				// if the #MARK is based on a delegation, store seperately
				let mut delegated_marks = <DelegatedMarks<T>>::get(d);
				delegated_marks.push(content_hash);
				<DelegatedMarks<T>>::insert(d, delegated_marks);
			}

			// deposit event that mark has beed added
			Self::deposit_event(RawEvent::Anchored(sender, content_hash, mtype_hash, delegation_id));
			Ok(())
		}

		/// Revokes a #MARK
		/// where, origin is the signed sender account,
		/// and content_hash is the hash of the anchored #MARK.
		#[weight = 100_000_000_000]
		pub fn revoke(origin, content_hash: T::Hash, max_depth: u32) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if it exists
			let Mark {mtype_hash, marker, delegation_id, revoked, ..} = <Marks<T>>::get(content_hash).ok_or(Error::<T>::MarkNotFound)?;

			// check if the #MARK has already been revoked
			ensure!(!revoked, Error::<T>::AlreadyRevoked);

			// check delegation treee if the sender of the revocation transaction is not the marker
			if !marker.eq(&sender) {
				// check whether the #MARK includes a delegation
				let del_id = delegation_id.ok_or(Error::<T>::UnauthorizedRevocation)?;
				// check whether the sender of the revocation is not a parent in the delegation hierarchy
				ensure!(<pallet_delegation::Module<T>>::is_delegating(&sender, &del_id, max_depth)?, Error::<T>::UnauthorizedRevocation);
			}
			// revoke #MARK
			debug::print!("revoking Attestation");
			<Marks<T>>::insert(content_hash, Mark {
				mtype_hash,
				marker,
				delegation_id,
				revoked: true
			});
			// deposit event that the #MARK has been revoked
			Self::deposit_event(RawEvent::Revoked(sender, content_hash));
			Ok(())
		}

		/// Restores a #MARK
		/// where, origin is the signed sender account,
		/// content_hash is the revoked #MARK.
		#[weight = 10_000_000]
		pub fn restore(origin, content_hash: T::Hash, max_depth: u32) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if it exists
			let Mark {mtype_hash, marker, delegation_id, revoked, ..} = <Marks<T>>::get(content_hash).ok_or(Error::<T>::MarkNotFound)?;

			// check if the #MARK has already been revoked
			ensure!(revoked, Error::<T>::MarkStillActive);

			// check delegation tree if the sender of the restore transaction is not the marker
			if !marker.eq(&sender) {
				// check whether the #MARK includes a delegation
				let del_id = delegation_id.ok_or(Error::<T>::UnauthorizedRestore)?;
				// check whether the sender of the restoration is not a parent in the delegation hierarchy
				ensure!(<pallet_delegation::Module<T>>::is_delegating(&sender, &del_id, max_depth)?, Error::<T>::UnauthorizedRestore);
			}

						// restore #MARK
			debug::print!("restoring #MARK");
			<Marks<T>>::insert(content_hash, Mark {
				mtype_hash,
				marker,
				delegation_id,
				revoked: false
			});

			// deposit event that the #MARK has been restored
			Self::deposit_event(RawEvent::Restored(sender, content_hash));
			Ok(())
		}


	}
}

#[derive(Debug, Encode, Decode, PartialEq)]
pub struct Mark<T: Config> {
	// hash of the MTYPE used for this mark
	pub mtype_hash: T::Hash,
	// the account which executed the mark
	pub marker: T::AccountId,
	// id of the delegation node (if exist)
	pub delegation_id: Option<T::DelegationNodeId>,
	// revocation status
	pub revoked: bool,
}

decl_storage! {
	trait Store for Module<T: Config> as Mark {
		/// Marks: content-hash -> (mtype-hash, marker-account, delegation-id?, revoked)?
		pub Marks get(fn marks): map hasher(opaque_blake2_256) T::Hash => Option<Mark<T>>;
		/// DelegatedMarks: delegation-id -> [content-hash]
		DelegatedMarks get(fn delegated_marks): map hasher(opaque_blake2_256) T::DelegationNodeId => Vec<T::Hash>;
	}
}

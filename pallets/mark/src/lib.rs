// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Mark: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for marks
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, 
	ensure, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_std::prelude::{Clone, PartialEq, Vec};

/// The #MARK trait
pub trait Trait: frame_system::Config + delegation::Trait {
	/// #MARK specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for Marks
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash,
			<T as delegation::Trait>::DelegationNodeId {
		/// A new #MARK has been anchored
		Anchored(AccountId, Hash, Hash, Option<DelegationNodeId>),
		/// A #MARK has been revoked
		Revoked(AccountId, Hash),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		AlreadyAnchored,
		AlreadyRevoked,
		MarkNotFound,
		MTypeMismatch,
		DelegationUnauthorisedToAnchor,
		DelegationRevoked,
		NotDelegatedToMarker,
		UnauthorizedRevocation,
	}
}

decl_module! {
	/// The #MARK runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;

		// Initializing errors
		// this includes information about your errors in the node's metadata.
		// it is needed only if you are using errors in your pallet
		type Error = Error<T>;

		/// Anchors a new #MARK on chain
		///, where, originis the signed sender account,
		/// stream_hash is the hash of the stream,
		/// mtype_hash is the hash of the #MARK TYPE,
		/// and delegation_id refers to a #MARK TYPE delegation.
		#[weight = 10_000_000]
		pub fn anchor(origin, stream_hash: T::Hash, mtype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if the #MARK TYPE exists
			ensure!(<mtype::MTYPEs<T>>::contains_key(mtype_hash), mtype::Error::<T>::NotFound);

			// check if the #MARK already exists
			ensure!(!<Marks<T>>::contains_key(stream_hash), Error::<T>::AlreadyAnchored);

			if let Some(d) = delegation_id {
				// check if delegation exists
				let delegation = <delegation::Delegations<T>>::get(d).ok_or(delegation::Error::<T>::DelegationNotFound)?;
				// check whether delegation has been revoked already
				ensure!(!delegation.revoked, Error::<T>::DelegationRevoked);

				// check whether the owner of the delegation is not the sender of this transaction
				ensure!(delegation.owner.eq(&sender), Error::<T>::NotDelegatedToMarker);
				
				// check whether the delegation is not set up for attesting claims
				ensure!(delegation.permissions == delegation::Permissions::ANCHOR, Error::<T>::DelegationUnauthorisedToAnchor);
				
				// check if MTYPE of the delegation is matching the MTYPE of the mark
				let root = <delegation::Root<T>>::get(delegation.root_id).ok_or(delegation::Error::<T>::RootNotFound)?;
				ensure!(root.mtype_hash.eq(&mtype_hash), Error::<T>::MTypeMismatch);
			}

			// insert #MARK
			debug::print!("insert #MARK");
			<Marks<T>>::insert(stream_hash, Mark {mtype_hash, marker: sender.clone(), delegation_id, revoked: false});

			if let Some(d) = delegation_id {
				// if the #MARK is based on a delegation, store seperately
				let mut delegated_marks = <DelegatedMarks<T>>::get(d);
				delegated_marks.push(stream_hash);
				<DelegatedMarks<T>>::insert(d, delegated_marks);
			}

			// deposit event that mark has beed added
			Self::deposit_event(RawEvent::Anchored(sender, stream_hash, mtype_hash, delegation_id));
			Ok(())
		}

		/// Revokes a #MARK
		/// where, origin is the signed sender account,
		/// and stream_hash is the hash of the anchored #MARK.
		#[weight = 10_000_000]
		pub fn revoke(origin, stream_hash: T::Hash, max_depth: u64) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if it exists
			let Mark {mtype_hash, marker, delegation_id, revoked, ..} = <Marks<T>>::get(stream_hash).ok_or(Error::<T>::MarkNotFound)?;

			// check if the #MARK has already been revoked
			ensure!(!revoked, Error::<T>::AlreadyRevoked);

			// check delegation treee if the sender of the revocation transaction is not the marker
			if !marker.eq(&sender) {
				// check whether the #MARK includes a delegation
				let del_id = delegation_id.ok_or(Error::<T>::UnauthorizedRevocation)?;
				// check whether the sender of the revocation is not a parent in the delegation hierarchy
				ensure!(<delegation::Module<T>>::is_delegating(&sender, &del_id, max_depth)?, Error::<T>::UnauthorizedRevocation);
			}
			
			// revoke #MARK
			debug::print!("revoking #MARK");
			<Marks<T>>::insert(stream_hash, Mark {
				mtype_hash,
				marker,
				delegation_id,
				revoked: true
			});

			// deposit event that the #MARK has been revoked
			Self::deposit_event(RawEvent::Revoked(sender, stream_hash));
			Ok(())
		}
	}
}

#[derive(Encode, Decode)]
pub struct Mark<T: Trait> {
	// hash of the MTYPE used for this mark
	mtype_hash: T::Hash,
	// the account which executed the mark
	marker: T::AccountId,
	// id of the delegation node (if exist)
	delegation_id: Option<T::DelegationNodeId>,
	// revocation status
	revoked: bool,
}

decl_storage! {
	trait Store for Module<T: Trait> as Mark {
		/// Marks: stream-hash -> (mtype-hash, marker-account, delegation-id?, revoked)?
		Marks get(fn marks): map hasher(opaque_blake2_256) T::Hash => Option<Mark<T>>;
		/// DelegatedMarks: delegation-id -> [stream-hash]
		DelegatedMarks get(fn delegated_marks): map hasher(opaque_blake2_256) T::DelegationNodeId => Vec<T::Hash>;
	}
}
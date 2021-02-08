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
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_std::prelude::{Clone, PartialEq, Vec};

/// The #MARK trait
pub trait Trait: frame_system::Config + delegation::Trait + error::Trait {
	/// #MARK specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for Marks
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash,
			<T as delegation::Trait>::DelegationNodeId {
		/// A #MARK has been anchored
		MarkAnchored(AccountId, Hash, Hash, Option<DelegationNodeId>),
		/// A #MARK has been revoked
		MarkRevoked(AccountId, Hash),
	}
);

decl_module! {
	/// The #MARK runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;

		/// Anchors a #MARK on chain
		/// where, originis the signed sender account,
		/// stream_hash is the hash of the stream,
		/// mtype_hash is the hash of the #MARK TYPE,
		/// and delegation_id refers to a #MARK TYPE delegation.
		#[weight = 1]
		pub fn anchor(origin, stream_hash: T::Hash, mtype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if the #MARK TYPE exists
			if !<mtype::MTYPEs<T>>::contains_key(mtype_hash) {
				return Self::error(<mtype::Module<T>>::ERROR_MTYPE_NOT_FOUND);
			}

			if let Some(d) = delegation_id {
				// has a delegation
				// check if delegation exists
				let delegation = <error::Module<T>>::ok_or_deposit_err(
					<delegation::Delegations<T>>::get(d),
					<delegation::Module<T>>::ERROR_DELEGATION_NOT_FOUND
				)?;
				if delegation.revoked {
					// delegation has been revoked
					return Self::error(Self::ERROR_DELEGATION_REVOKED);
				} else if !delegation.owner.eq(&sender) {
					// delegation is not made up for the sender of this transaction
					return Self::error(Self::ERROR_NOT_DELEGATED_TO_ATTESTER);
				} else if (delegation.permissions & delegation::Permissions::ANCHOR) != delegation::Permissions::ANCHOR {
					// delegation is not set up for anchoring mark
					return Self::error(Self::ERROR_DELEGATION_NOT_AUTHORIZED_TO_MARK);
				} else {
					// check if MTYPE of the delegation is matching the MTYPE of the mark
					let root = <error::Module<T>>::ok_or_deposit_err(
						<delegation::Root<T>>::get(delegation.root_id),
						<delegation::Module<T>>::ERROR_ROOT_NOT_FOUND
					)?;
					if !root.mtype_hash.eq(&mtype_hash) {
						return Self::error(Self::ERROR_MTYPE_OF_DELEGATION_NOT_MATCHING);
					}
				}
			}

			// check if the #MARK already exists
			if <Marks<T>>::contains_key(stream_hash) {
				return Self::error(Self::ERROR_ALREADY_MARKED);
			}

			// insert #MARK
			debug::RuntimeLogger::init();
			debug::print!("insert #MARK");
			<Marks<T>>::insert(stream_hash, Mark {mtype_hash, owner: sender.clone(), delegation_id, revoked: false});

			if let Some(d) = delegation_id {
				// if the #MARK is based on a delegation this is stored in a separate map
				let mut delegated_marks = <DelegatedMarks<T>>::get(d);
				delegated_marks.push(stream_hash);
				<DelegatedMarks<T>>::insert(d, delegated_marks);
			}

			// deposit event that mark has beed added
			Self::deposit_event(RawEvent::MarkAnchored(sender, stream_hash,
					mtype_hash, delegation_id));
			Ok(())
		}

		/// Revokes a #MARK
		/// where, origin is the signed sender account,
		/// and stream_hash is the hash of the anchored #MARK.
		#[weight = 1]
		pub fn revoke(origin, stream_hash: T::Hash, max_depth: u64) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if it exists
			let Mark {mtype_hash, owner, delegation_id, revoked, ..} = <error::Module<T>>::ok_or_deposit_err(
				<Marks<T>>::get(stream_hash),
				Self::ERROR_MARKER_NOT_FOUND
			)?;
			// if the sender of the revocation transaction is not the attester, check delegation tree
			if !owner.eq(&sender) {
				match delegation_id {
					Some(delegation_id) => {
						if !<delegation::Module<T>>::is_delegating(&sender, &delegation_id, max_depth)? {
							// the sender of the revocation is not a parent in the delegation hierarchy
							return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_MARK);
						}
					},
					None => {
						// the #MARK is not based on a delegation
						return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_MARK);
					}
				}
			}

			// check if already revoked
			if revoked {
				return Self::error(Self::ERROR_ALREADY_REVOKED);
			}

			// revoke #MARK
			debug::print!("revoking #MARK");
			<Marks<T>>::insert(stream_hash, Mark {
				mtype_hash,
				owner,
				delegation_id,
				revoked: true
			});

			// deposit event that the #MARK has been revoked
			Self::deposit_event(RawEvent::MarkRevoked(sender, stream_hash));
			Ok(())
		}
	}
}

/// Implementation of further module constants and functions for #MARK
impl<T: Trait> Module<T> {
	/// Error types for errors in Mark module
	const ERROR_BASE: u16 = 2000;
	const ERROR_ALREADY_MARKED: error::ErrorType = (Self::ERROR_BASE + 1, "already anchored");
	const ERROR_ALREADY_REVOKED: error::ErrorType = (Self::ERROR_BASE + 2, "already revoked");
	const ERROR_MARKER_NOT_FOUND: error::ErrorType =
		(Self::ERROR_BASE + 3, "mark not found");
	const ERROR_DELEGATION_REVOKED: error::ErrorType = (Self::ERROR_BASE + 4, "delegation revoked");
	const ERROR_NOT_DELEGATED_TO_ATTESTER: error::ErrorType =
		(Self::ERROR_BASE + 5, "not delegated to attester");
	const ERROR_DELEGATION_NOT_AUTHORIZED_TO_MARK: error::ErrorType =
		(Self::ERROR_BASE + 6, "delegation not authorized to attest");
	const ERROR_MTYPE_OF_DELEGATION_NOT_MATCHING: error::ErrorType =
		(Self::ERROR_BASE + 7, "MTYPE of delegation does not match");
	const ERROR_NOT_PERMITTED_TO_REVOKE_MARK: error::ErrorType =
		(Self::ERROR_BASE + 8, "not permitted to revoke mark");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}

#[derive(Encode, Decode)]
pub struct Mark<T: Trait> {
	// hash of the MTYPE used for this mark
	mtype_hash: T::Hash,
	// the account which executed the mark
	owner: T::AccountId,
	// id of the delegation node (if exist)
	delegation_id: Option<T::DelegationNodeId>,
	// revocation status
	revoked: bool,
}

decl_storage! {
	trait Store for Module<T: Trait> as Mark {
		/// Marks: stream-hash -> (mtype-hash, attester-account, delegation-id?, revoked)?
		Marks get(fn marks): map hasher(opaque_blake2_256) T::Hash => Option<Mark<T>>;
		/// DelegatedMarks: delegation-id -> [stream-hash]
		DelegatedMarks get(fn delegated_marks): map hasher(opaque_blake2_256) T::DelegationNodeId => Vec<T::Hash>;
	}
}
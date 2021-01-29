// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Attestation: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for #MARKs
#[cfg(test)]
mod tests;

use mtypes;
use delegation;
use error;
use frame_support::{
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_std::{
	prelude::{Clone, PartialEq, Vec},
	result,
};

/// The #MARK trait
pub trait Trait: frame_system::Config + delegation::Trait + error::Trait {
	/// Attestation specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for #MARKs
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash,
			<T as delegation::Trait>::DelegateId {
		/// A #MARK has been added
		MarkCreated(AccountId, Hash, Hash, Option<DelegateId>),
		/// A #MARK has been revoked
		MarkRevoked(AccountId, Hash),
	}
);

decl_module! {
	/// The #MARK runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;

		/// Adds a #MARK on chain
		/// origin - the origin of the transaction
		/// mark - hash of the attested claim
		/// mtype - hash of the #MARK SCHEMA of the claim
		/// delegate_id - optional id that refers to a delegation this #MARK is based on
		#[weight = 10]
		pub fn add(origin, mark: T::Hash, mtype: T::Hash, delegate_id: Option<T::DelegateId>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if the #MARK SCHEMA exists
			if !<mtypes::MTYPEs<T>>::contains_key(mtype) {
				return Self::error(<mtypes::Module<T>>::ERROR_MTYPE_NOT_FOUND);
			}

			if let Some(d) = delegate_id {
				// has a delegation
				// check if delegation exists
				let delegation = <error::Module<T>>::ok_or_deposit_err(
					<delegation::Delegations<T>>::get(d),
					<delegation::Module<T>>::ERROR_DELEGATION_NOT_FOUND
				)?;
				if delegation.4 {
					// delegation has been revoked
					return Self::error(Self::ERROR_DELEGATION_REVOKED);
				} else if !delegation.2.eq(&sender) {
					// delegation is not made up for the sender of this transaction
					return Self::error(Self::ERROR_NOT_DELEGATED_TO_ATTESTER);
				} else if (delegation.3 & delegation::Permissions::ATTEST) != delegation::Permissions::ATTEST {
					// delegation is not set up for attesting claims
					return Self::error(Self::ERROR_DELEGATION_NOT_AUTHORIZED_TO_ATTEST);
				} else {
					// check if MTYPE of the delegation is matching the SCHEMA of the #MARK
					let root = <error::Module<T>>::ok_or_deposit_err(
						<delegation::Root<T>>::get(delegation.0),
						<delegation::Module<T>>::ERROR_ROOT_NOT_FOUND
					)?;
					if !root.0.eq(&mtype) {
						return Self::error(Self::ERROR_MTYPE_OF_DELEGATION_NOT_MATCHING);
					}
				}
			}

			// check if #MARK already exists
			if <Marks<T>>::contains_key(mark) {
				return Self::error(Self::ERROR_ALREADY_ATTESTED);
			}

			// insert #MARK
			debug::RuntimeLogger::init();
			debug::print!("insert #MARK");
			<Marks<T>>::insert(mark, (mtype, sender.clone(), delegate_id, false));

			if let Some(d) = delegate_id {
				// if #MARK is based on a delegation this is stored in a separate map
				let mut delegated_marks = <DelegatedMarks<T>>::get(d);
				delegated_marks.push(mark);
				<DelegatedMarks<T>>::insert(d, delegated_marks);
			}

			// deposit event that #MARK has beed added
			Self::deposit_event(RawEvent::MarkCreated(sender, mark,
					mtype, delegate_id));
			Ok(())
		}

		/// Revokes a #MARK on chain
		/// origin - the origin of the transaction
		/// content_hash - hash of #MARK
		#[weight = 10]		
		pub fn revoke(origin, mark: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup #MARK & check if the #MARK exists
			let mut existing_mark = <error::Module<T>>::ok_or_deposit_err(
				<Marks<T>>::get(mark),
				Self::ERROR_MARK_NOT_FOUND
			)?;
			// if the sender of the revocation transaction is not the attester, check delegation tree
			if !existing_mark.1.eq(&sender) {
				match existing_mark.2 {
					Some(d) => {
						if !Self::is_delegating(&sender, &d)? {
							// the sender of the revocation is not a parent in the delegation hierarchy
							return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION);
						}
					},
					None => {
						// the #MARK has not been made up based on a delegation
						return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION);
					}
				}
			}

			// check if already revoked
			if existing_mark.3 {
				return Self::error(Self::ERROR_ALREADY_REVOKED);
			}

			// revoke #MARK
			debug::print!("Revoking #MARK");
			existing_mark.3 = true;
			<Marks<T>>::insert(mark, existing_mark);

			// deposit event that the #MARK has been revoked
			Self::deposit_event(RawEvent::MarkRevoked(sender, mark));
			Ok(())
		}
	}
}

/// Implementation of further module constants and functions for #MARKs
impl<T: Trait> Module<T> {
	/// Error types for errors in #MARK module
	const ERROR_BASE: u16 = 2000;
	const ERROR_ALREADY_ATTESTED: error::ErrorType = (Self::ERROR_BASE + 1, "#MARK exists");
	const ERROR_ALREADY_REVOKED: error::ErrorType = (Self::ERROR_BASE + 2, "already revoked");
	const ERROR_MARK_NOT_FOUND: error::ErrorType =
		(Self::ERROR_BASE + 3, "#MARK not found");
	const ERROR_DELEGATION_REVOKED: error::ErrorType = (Self::ERROR_BASE + 4, "delegation revoked");
	const ERROR_NOT_DELEGATED_TO_ATTESTER: error::ErrorType =
		(Self::ERROR_BASE + 5, "not delegated to attester");
	const ERROR_DELEGATION_NOT_AUTHORIZED_TO_ATTEST: error::ErrorType =
		(Self::ERROR_BASE + 6, "delegation not authorized to attest");
	const ERROR_MTYPE_OF_DELEGATION_NOT_MATCHING: error::ErrorType =
		(Self::ERROR_BASE + 7, "#MARK SCHEMA of delegation does not match");
	const ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION: error::ErrorType =
		(Self::ERROR_BASE + 8, "not permitted to revoke #MARK");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}

	/// Check delegation hierarchy using the delegation module
	fn is_delegating(
		account: &T::AccountId,
		delegation: &T::DelegateId,
	) -> result::Result<bool, &'static str> {
		<delegation::Module<T>>::is_delegating(account, delegation)
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Marks {
		/// #MARK: mark -> (mtype, attester-account, delegation-id?, revoked)?
		Marks get(fn marks): map hasher(opaque_blake2_256) T::Hash => Option<(T::Hash, T::AccountId, Option<T::DelegateId>, bool)>;
		/// Delegated#Mark: delegation-id -> [mark]
		DelegatedMarks get(fn delegated_marks): map hasher(opaque_blake2_256) T::DelegateId => Vec<T::Hash>;
	}
}

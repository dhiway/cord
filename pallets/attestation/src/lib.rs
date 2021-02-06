// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Attestation: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for attestations
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::{
	debug, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_std::prelude::{Clone, PartialEq, Vec};

/// The attestation trait
pub trait Trait: frame_system::Config + delegation::Trait + error::Trait {
	/// Attestation specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_event!(
	/// Events for attestations
	pub enum Event<T> where <T as frame_system::Config>::AccountId, <T as frame_system::Config>::Hash,
			<T as delegation::Trait>::DelegationNodeId {
		/// An attestation has been added
		AttestationCreated(AccountId, Hash, Hash, Option<DelegationNodeId>),
		/// An attestation has been revoked
		AttestationRevoked(AccountId, Hash),
	}
);

decl_module! {
	/// The attestation runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;

		/// Adds an attestation on chain, where
		/// origin - the origin of the transaction
		/// claim_hash - hash of the attested claim
		/// ctype_hash - hash of the CTYPE of the claim
		/// delegation_id - optional id that refers to a delegation this attestation is based on
		#[weight = 1]
		pub fn add(origin, claim_hash: T::Hash, ctype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if the CTYPE exists
			if !<ctype::CTYPEs<T>>::contains_key(ctype_hash) {
				return Self::error(<ctype::Module<T>>::ERROR_CTYPE_NOT_FOUND);
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
				} else if (delegation.permissions & delegation::Permissions::ATTEST) != delegation::Permissions::ATTEST {
					// delegation is not set up for attesting claims
					return Self::error(Self::ERROR_DELEGATION_NOT_AUTHORIZED_TO_ATTEST);
				} else {
					// check if CTYPE of the delegation is matching the CTYPE of the attestation
					let root = <error::Module<T>>::ok_or_deposit_err(
						<delegation::Root<T>>::get(delegation.root_id),
						<delegation::Module<T>>::ERROR_ROOT_NOT_FOUND
					)?;
					if !root.ctype_hash.eq(&ctype_hash) {
						return Self::error(Self::ERROR_CTYPE_OF_DELEGATION_NOT_MATCHING);
					}
				}
			}

			// check if attestation already exists
			if <Attestations<T>>::contains_key(claim_hash) {
				return Self::error(Self::ERROR_ALREADY_ATTESTED);
			}

			// insert attestation
			debug::RuntimeLogger::init();
			debug::print!("insert Attestation");
			<Attestations<T>>::insert(claim_hash, Attestation {ctype_hash, owner: sender.clone(), delegation_id, revoked: false});

			if let Some(d) = delegation_id {
				// if attestation is based on a delegation this is stored in a separate map
				let mut delegated_attestations = <DelegatedAttestations<T>>::get(d);
				delegated_attestations.push(claim_hash);
				<DelegatedAttestations<T>>::insert(d, delegated_attestations);
			}

			// deposit event that attestation has beed added
			Self::deposit_event(RawEvent::AttestationCreated(sender, claim_hash,
					ctype_hash, delegation_id));
			Ok(())
		}

		/// Revokes an attestation on chain, where
		/// origin - the origin of the transaction
		/// claim_hash - hash of the attested claim
		#[weight = 1]
		pub fn revoke(origin, claim_hash: T::Hash, max_depth: u64) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// lookup attestation & check if the attestation exists
			let Attestation {ctype_hash, owner, delegation_id, revoked, ..} = <error::Module<T>>::ok_or_deposit_err(
				<Attestations<T>>::get(claim_hash),
				Self::ERROR_ATTESTATION_NOT_FOUND
			)?;
			// if the sender of the revocation transaction is not the attester, check delegation tree
			if !owner.eq(&sender) {
				match delegation_id {
					Some(delegation_id) => {
						if !<delegation::Module<T>>::is_delegating(&sender, &delegation_id, max_depth)? {
							// the sender of the revocation is not a parent in the delegation hierarchy
							return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION);
						}
					},
					None => {
						// the attestation has not been made up based on a delegation
						return Self::error(Self::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION);
					}
				}
			}

			// check if already revoked
			if revoked {
				return Self::error(Self::ERROR_ALREADY_REVOKED);
			}

			// revoke attestation
			debug::print!("revoking Attestation");
			<Attestations<T>>::insert(claim_hash, Attestation {
				ctype_hash,
				owner,
				delegation_id,
				revoked: true
			});

			// deposit event that the attestation has been revoked
			Self::deposit_event(RawEvent::AttestationRevoked(sender, claim_hash));
			Ok(())
		}
	}
}

/// Implementation of further module constants and functions for attestations
impl<T: Trait> Module<T> {
	/// Error types for errors in attestation module
	const ERROR_BASE: u16 = 2000;
	const ERROR_ALREADY_ATTESTED: error::ErrorType = (Self::ERROR_BASE + 1, "already attested");
	const ERROR_ALREADY_REVOKED: error::ErrorType = (Self::ERROR_BASE + 2, "already revoked");
	const ERROR_ATTESTATION_NOT_FOUND: error::ErrorType =
		(Self::ERROR_BASE + 3, "attestation not found");
	const ERROR_DELEGATION_REVOKED: error::ErrorType = (Self::ERROR_BASE + 4, "delegation revoked");
	const ERROR_NOT_DELEGATED_TO_ATTESTER: error::ErrorType =
		(Self::ERROR_BASE + 5, "not delegated to attester");
	const ERROR_DELEGATION_NOT_AUTHORIZED_TO_ATTEST: error::ErrorType =
		(Self::ERROR_BASE + 6, "delegation not authorized to attest");
	const ERROR_CTYPE_OF_DELEGATION_NOT_MATCHING: error::ErrorType =
		(Self::ERROR_BASE + 7, "CTYPE of delegation does not match");
	const ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION: error::ErrorType =
		(Self::ERROR_BASE + 8, "not permitted to revoke attestation");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}

#[derive(Encode, Decode)]
pub struct Attestation<T: Trait> {
	// hash of the CTYPE used for this attestation
	ctype_hash: T::Hash,
	// the account which executed the attestation
	owner: T::AccountId,
	// id of the delegation node (if existent)
	delegation_id: Option<T::DelegationNodeId>,
	// revocation status
	revoked: bool,
}

decl_storage! {
	trait Store for Module<T: Trait> as Attestation {
		/// Attestations: claim-hash -> (ctype-hash, attester-account, delegation-id?, revoked)?
		Attestations get(fn attestations): map hasher(opaque_blake2_256) T::Hash => Option<Attestation<T>>;
		/// DelegatedAttestations: delegation-id -> [claim-hash]
		DelegatedAttestations get(fn delegated_attestations): map hasher(opaque_blake2_256) T::DelegationNodeId => Vec<T::Hash>;
	}
}
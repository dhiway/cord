// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! DID: Handles decentralized identifiers on chain,
//! adding and removing DIDs.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod did_details;
pub mod errors;
pub mod origin;
pub mod url;
pub mod weights;

mod utils;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

pub use did_details::*;
pub use errors::*;
pub use origin::*;
pub use pallet::*;
pub use url::*;

use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	ensure,
	storage::types::StorageMap,
	traits::Get,
	Parameter,
};
use frame_system::ensure_signed;
#[cfg(feature = "runtime-benchmarks")]
use frame_system::RawOrigin;
use sp_std::{boxed::Box, convert::TryFrom, fmt::Debug, prelude::Clone, vec::Vec};

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Reference to a payload of data of variable size.
	pub type Payload = [u8];

	/// Type for a DID key identifier.
	pub type KeyIdOf<T> = <T as frame_system::Config>::Hash;

	/// Type for a DID subject identifier.
	pub type DidIdentifierOf<T> = <T as Config>::DidIdentifier;

	/// Type for a Kilt account identifier.
	pub type AccountIdentifierOf<T> = <T as frame_system::Config>::AccountId;

	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	/// Type for a runtime extrinsic callable under DID-based authorization.
	pub type DidCallableOf<T> = <T as Config>::Call;

	/// Type for origin that supports a DID sender.
	pub type Origin<T> = DidRawOrigin<DidIdentifierOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		type Call: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ DeriveDidCallAuthorizationVerificationKeyRelationship;
		type DidIdentifier: Parameter + Default;
		#[cfg(not(feature = "runtime-benchmarks"))]
		type Origin: From<DidRawOrigin<DidIdentifierOf<Self>>>;
		#[cfg(feature = "runtime-benchmarks")]
		type Origin: From<RawOrigin<DidIdentifierOf<Self>>>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		#[pallet::constant]
		type MaxNewKeyAgreementKeys: Get<u32>;
		#[pallet::constant]
		type MaxVerificationKeysToRevoke: Get<u32>;
		#[pallet::constant]
		type MaxUrlLength: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// DIDs stored on chain.
	///
	/// It maps from a DID identifier to the DID details.
	#[pallet::storage]
	#[pallet::getter(fn get_did)]
	pub type Did<T> = StorageMap<_, Blake2_128Concat, DidIdentifierOf<T>, DidDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new DID has been created.
		/// \[transaction signer, DID identifier\]
		DidCreated(AccountIdentifierOf<T>, DidIdentifierOf<T>),
		/// A DID has been updated.
		/// \[transaction signer, DID identifier\]
		DidUpdated(AccountIdentifierOf<T>, DidIdentifierOf<T>),
		/// A DID has been deleted.
		/// \[transaction signer, DID identifier\]
		DidDeleted(AccountIdentifierOf<T>, DidIdentifierOf<T>),
		/// A DID-authorized call has been successfully executed.
		/// \[DID caller]
		DidCallSuccess(DidIdentifierOf<T>),
		/// A DID-authorized call has failed to execute.
		/// \[DID caller, error]
		DidCallFailure(DidIdentifierOf<T>, DispatchError),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The DID operation signature is not in the format the verification
		/// key expects.
		InvalidSignatureFormat,
		/// The DID operation signature is invalid for the payload and the
		/// verification key provided.
		InvalidSignature,
		/// The DID with the given identifier is already present on chain.
		DidAlreadyPresent,
		/// No DID with the given identifier is present on chain.
		DidNotPresent,
		/// One or more verification keys referenced are not stored in the set
		/// of verification keys.
		VerificationKeyNotPresent,
		/// The DID operation nonce is not equal to the current DID nonce + 1.
		InvalidNonce,
		/// The URL specified is not ASCII-encoded.
		InvalidUrlEncoding,
		/// The URL specified is not properly formatted.
		InvalidUrlScheme,
		/// The maximum supported value for the DID tx counter has been reached.
		/// No more operations with the DID are allowed.
		MaxTxCounterValue,
		/// The user tries to delete a verification key that is currently being
		/// used as an authentication, delegation, or anchor key, and this
		/// is not allowed.
		CurrentlyActiveKey,
		/// The called extrinsic does not support DID authorization.
		UnsupportedDidAuthorizationCall,
		/// A number of new key agreement keys greater than the maximum allowed
		/// has been provided.
		MaxKeyAgreementKeysLimitExceeded,
		/// A number of new verification keys to remove greater than the maximum
		/// allowed has been provided.
		MaxVerificationKeysToRemoveLimitExceeded,
		/// A URL longer than the maximum size allowed has been provided.
		MaxUrlLengthExceeded,
		/// An error that is not supposed to take place, yet it happened.
		InternalError,
	}

	impl<T> From<DidError> for Error<T> {
		fn from(error: DidError) -> Self {
			match error {
				DidError::StorageError(storage_error) => Self::from(storage_error),
				DidError::SignatureError(operation_error) => Self::from(operation_error),
				DidError::UrlError(url_error) => Self::from(url_error),
				DidError::InputError(input_error) => Self::from(input_error),
				DidError::InternalError => Self::InternalError,
			}
		}
	}

	impl<T> From<StorageError> for Error<T> {
		fn from(error: StorageError) -> Self {
			match error {
				StorageError::DidNotPresent => Self::DidNotPresent,
				StorageError::DidAlreadyPresent => Self::DidAlreadyPresent,
				StorageError::DidKeyNotPresent(_) | StorageError::VerificationKeyNotPresent => {
					Self::VerificationKeyNotPresent
				}
				StorageError::MaxTxCounterValue => Self::MaxTxCounterValue,
				StorageError::CurrentlyActiveKey => Self::CurrentlyActiveKey,
			}
		}
	}

	impl<T> From<SignatureError> for Error<T> {
		fn from(error: SignatureError) -> Self {
			match error {
				SignatureError::InvalidSignature => Self::InvalidSignature,
				SignatureError::InvalidSignatureFormat => Self::InvalidSignatureFormat,
				SignatureError::InvalidNonce => Self::InvalidNonce,
			}
		}
	}

	impl<T> From<UrlError> for Error<T> {
		fn from(error: UrlError) -> Self {
			match error {
				UrlError::InvalidUrlEncoding => Self::InvalidUrlEncoding,
				UrlError::InvalidUrlScheme => Self::InvalidUrlScheme,
			}
		}
	}

	impl<T> From<InputError> for Error<T> {
		fn from(error: InputError) -> Self {
			match error {
				InputError::MaxKeyAgreementKeysLimitExceeded => Self::MaxKeyAgreementKeysLimitExceeded,
				InputError::MaxVerificationKeysToRemoveLimitExceeded => Self::MaxVerificationKeysToRemoveLimitExceeded,
				InputError::MaxUrlLengthExceeded => Self::MaxUrlLengthExceeded,
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Store a new DID on chain, after verifying the signature associated
		/// with the creation operation.
		///
		/// * origin: the Substrate account submitting the transaction (which
		///   can be different from the DID subject)
		/// * operation: the creation operation which contains the details of
		///   the new DID
		/// * signature: the signture over the operation that must be signed
		///   with the authentication key provided in the operation
		#[pallet::weight(
			<T as pallet::Config>::WeightInfo::submit_did_create_operation_ed25519_keys(
				operation.new_key_agreement_keys.len() as u32,
				operation.new_endpoint_url.as_ref().map_or(0u32, |url| url.len() as u32)
			)
			.max(<T as pallet::Config>::WeightInfo::submit_did_create_operation_sr25519_keys(
				operation.new_key_agreement_keys.len() as u32,
				operation.new_endpoint_url.as_ref().map_or(0u32, |url| url.len() as u32)
			))
		)]
		pub fn submit_did_create_operation(
			origin: OriginFor<T>,
			operation: DidCreationOperation<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// There has to be no other DID with the same identifier already saved on chain,
			// otherwise generate a DidAlreadyPresent error.
			ensure!(
				!<Did<T>>::contains_key(operation.get_did()),
				<Error<T>>::DidAlreadyPresent
			);
			let did_entry = DidDetails::try_from(operation.clone()).map_err(<Error<T>>::from)?;

			Self::verify_payload_signature_with_did_key_type(
				&operation.encode(),
				&signature,
				&did_entry,
				operation.get_verification_key_relationship(),
			)
			.map_err(<Error<T>>::from)?;

			let did_identifier = operation.get_did();
			log::debug!("Creating DID {:?}", did_identifier);
			<Did<T>>::insert(did_identifier, did_entry);

			Self::deposit_event(Event::DidCreated(sender, did_identifier.clone()));

			Ok(())
		}

		/// Update the information associated with a DID on chain, after
		/// verifying the signature associated with the operation.
		///
		/// * origin: the Substrate account submitting the transaction (which
		///   can be different from the DID subject)
		/// * operation: the update operation which contains the new details of
		///   the existing DID
		/// * signature: the signature over the operation that must be signed
		///   with the authentication key associated with the new DID. Even in
		///   case the authentication key is being updated, the operation must
		///   still be signed with the old one being replaced
		#[pallet::weight(
			<T as pallet::Config>::WeightInfo::submit_did_update_operation_ed25519_keys(
				operation.new_key_agreement_keys.len() as u32,
				operation.public_keys_to_remove.len() as u32,
				operation.new_endpoint_url.as_ref().map_or(0u32, |url| url.len() as u32)
			)
			.max(<T as pallet::Config>::WeightInfo::submit_did_update_operation_sr25519_keys(
				operation.new_key_agreement_keys.len() as u32,
				operation.public_keys_to_remove.len() as u32,
				operation.new_endpoint_url.as_ref().map_or(0u32, |url| url.len() as u32)
			))
		)]
		pub fn submit_did_update_operation(
			origin: OriginFor<T>,
			operation: DidUpdateOperation<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Saved here as it is consumed later when generating the new DidDetails object.
			let did_identifier = operation.get_did().clone();

			let did_details = <Did<T>>::get(&did_identifier).ok_or(<Error<T>>::DidNotPresent)?;

			// Verify the signature and the nonce of the update operation.
			Self::verify_operation_validity_for_did(&operation, &signature, &did_details).map_err(<Error<T>>::from)?;

			// Generate a new DidDetails object by applying the changes in the update
			// operation to the old object (and consuming both).
			let new_did_details = DidDetails::try_from((did_details, operation)).map_err(<Error<T>>::from)?;

			log::debug!("Updating DID {:?}", did_identifier);
			<Did<T>>::insert(&did_identifier, new_did_details);

			Self::deposit_event(Event::DidUpdated(sender, did_identifier));

			Ok(())
		}

		/// Delete all the information associated with a DID from the chain,
		/// after verifying the signature associated with the operation.
		///
		/// * origin: the Substrate account submitting the transaction (which
		///   can be different from the DID subject)
		/// * operation: the deletion operation which includes the DID to
		///   deactivate
		/// * signature: the signature over the operation that must be signed
		///   with the authentication key associated with the DID being deleted
		#[pallet::weight(<T as pallet::Config>::WeightInfo::submit_did_delete_operation())]
		pub fn submit_did_delete_operation(
			origin: OriginFor<T>,
			operation: DidDeletionOperation<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let did_identifier = operation.get_did();

			let did_details = <Did<T>>::get(&did_identifier).ok_or(<Error<T>>::DidNotPresent)?;

			// Verify the signature and the nonce of the delete operation.
			Self::verify_operation_validity_for_did(&operation, &signature, &did_details).map_err(<Error<T>>::from)?;

			log::debug!("Deleting DID {:?}", did_identifier);
			<Did<T>>::remove(&did_identifier);

			Self::deposit_event(Event::DidDeleted(sender, did_identifier.clone()));

			Ok(())
		}

		/// Proxy a [call](Call) to another runtime extrinsic conforming that
		/// supports DID-based authorization.
		///
		/// * origin: the account which will pay the execution fees of the
		///   nested call
		/// * did_call: the extrinsic call to dispatch
		/// * signature: the signature over the encoded extrinsic call that must
		///   be signed by the tx submiter, i.e., the account paying for the
		///   execution fees
		#[allow(clippy::boxed_local)]
		#[pallet::weight(0)]
		pub fn submit_did_call(
			origin: OriginFor<T>,
			did_call: Box<DidAuthorizedCallOperation<T>>,
			signature: DidSignature,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let did_identifier = did_call.did.clone();

			// Compute the right DID verification key to use to verify the operation
			// signature
			let verification_key_relationship = did_call
				.call
				.derive_verification_key_relationship()
				.ok_or(<Error<T>>::UnsupportedDidAuthorizationCall)?;

			// Wrap the operation in the expected structure, specifying the key retrieved
			let wrapped_operation = DidAuthorizedCallOperationWithVerificationRelationship {
				operation: *did_call,
				verification_key_relationship,
			};

			// Verify if the DID exists, if the operation signature is valid, and if so
			// increase the nonce if successful.
			Self::verify_operation_validity_and_increase_did_nonce(&wrapped_operation, &signature)
				.map_err(<Error<T>>::from)?;
			log::debug!("Dispatch call from DID {:?}", did_identifier);

			// Dispatch the referenced [Call] instance and return its result
			let DidAuthorizedCallOperation { did, call, .. } = wrapped_operation.operation;

			#[cfg(not(feature = "runtime-benchmarks"))]
			let result = call.dispatch(DidRawOrigin { id: did }.into());
			#[cfg(feature = "runtime-benchmarks")]
			let result = call.dispatch(RawOrigin::Signed(did).into());

			let dispatch_event = match result {
				Ok(_) => Event::DidCallSuccess(did_identifier),
				Err(err_result) => Event::DidCallFailure(did_identifier, err_result.error),
			};
			Self::deposit_event(dispatch_event);

			result
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Verify the validity (i.e., nonce and signature) of a generic
	/// [DidOperation] and, if valid, update the DID state with the latest
	/// nonce.
	///
	/// * operation: the reference to the operation which validity is to be
	///   verified
	/// * signature: a reference to the signature
	/// * did: the DID identifier to verify the operation signature for
	pub fn verify_operation_validity_and_increase_did_nonce<O: DidOperation<T>>(
		operation: &O,
		signature: &DidSignature,
	) -> Result<(), DidError> {
		let mut did_details =
			<Did<T>>::get(&operation.get_did()).ok_or(DidError::StorageError(StorageError::DidNotPresent))?;

		Self::verify_operation_validity_for_did(operation, signature, &did_details)?;

		// Update tx counter in DID details and save to DID pallet
		did_details.increase_tx_counter().map_err(DidError::StorageError)?;
		<Did<T>>::insert(&operation.get_did(), did_details);

		Ok(())
	}

	// Internally verifies the validity of a DID operation nonce and signature.
	fn verify_operation_validity_for_did<O: DidOperation<T>>(
		operation: &O,
		signature: &DidSignature,
		did_details: &DidDetails<T>,
	) -> Result<(), DidError> {
		Self::verify_operation_counter_for_did(operation, did_details)?;
		Self::verify_payload_signature_with_did_key_type(
			&operation.encode(),
			signature,
			did_details,
			operation.get_verification_key_relationship(),
		)
	}

	// Verify the validity of a DID operation nonce.
	// To be valid, the nonce must be equal to the one currently stored + 1.
	// This is to avoid quickly "consuming" all the possible values for the counter,
	// as that would result in the DID being unusable, since we do not have yet any
	// mechanism in place to wrap the counter value around when the limit is
	// reached.
	fn verify_operation_counter_for_did<O: DidOperation<T>>(
		operation: &O,
		did_details: &DidDetails<T>,
	) -> Result<(), DidError> {
		// Verify that the DID has not reached the maximum tx counter value
		ensure!(
			did_details.get_tx_counter_value() < u64::MAX,
			DidError::StorageError(StorageError::MaxTxCounterValue)
		);

		// Verify that the operation counter is equal to the stored one + 1
		let expected_nonce_value = did_details
			.get_tx_counter_value()
			.checked_add(1)
			.ok_or(DidError::InternalError)?;
		ensure!(
			operation.get_tx_counter() == expected_nonce_value,
			DidError::SignatureError(SignatureError::InvalidNonce)
		);

		Ok(())
	}

	// Verify a generic payload signature using a given DID verification key type.
	pub fn verify_payload_signature_with_did_key_type(
		payload: &Payload,
		signature: &DidSignature,
		did_details: &DidDetails<T>,
		key_type: DidVerificationKeyRelationship,
	) -> Result<(), DidError> {
		// Retrieve the needed verification key from the DID details, or generate an
		// error if there is no key of the type required
		let verification_key = did_details
			.get_verification_key_for_key_type(key_type)
			.ok_or(DidError::StorageError(StorageError::DidKeyNotPresent(key_type)))?;

		// Verify that the signature matches the expected format, otherwise generate
		// an error
		let is_signature_valid = verification_key
			.verify_signature(payload, signature)
			.map_err(|_| DidError::SignatureError(SignatureError::InvalidSignatureFormat))?;

		ensure!(
			is_signature_valid,
			DidError::SignatureError(SignatureError::InvalidSignature)
		);

		Ok(())
	}
}

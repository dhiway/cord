// This file is part of CORD – https://cord.network
// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Based on DID pallet - Copyright (C) 2019-2022 BOTLabs GmbH

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! # DID Pallet
//!
//! Provides W3C-compliant DID functionalities. A DID identifier is derived from
//! a CORD address and must be verifiable, i.e., must be able to generate
//! digital signatures that can be verified starting from a raw payload, its
//! signature, and the signer identifier. Currently, the DID pallet supports the
//! following types of keys: Ed25519, Sr25519, and Ecdsa for signing keys, and
//! X25519 for encryption keys.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Origin`]
//! - [`Pallet`]
//!
//! ### Terminology
//!
//! Each DID identifier is mapped to a set of keys, which in CORD are used for
//! different purposes.
//!
//! - **authentication key**: used to sign and authorise DID-management
//!   operations (e.g., the update of some keys or the deletion of the whole
//!   DID). This is required to always be present as otherwise the DID becomes
//!   unusable since no operation signature can be verified anymore.
//!
//! - **key agreement keys**: used by other parties that want to
//!   interact with the DID subject to perform ECDH and encrypt information
//!   addressed to the DID subject.
//!
//! - **delegation key**: used to sign and authorise the creation of
//!   new delegation nodes on the CORD blockchain. In case no delegation key is
//!   present, the DID subject cannot write new delegations on the CORD
//!   blockchain.
//!
//! - **assertion key**: used to sign and authorise the creation of
//!   new stream tokens (Identifiers) on the CORD blockchain. In case no
//!   assertion key is present, the DID subject cannot write new stream tokens
//!   (Identifiers) on the CORD blockchain.
//!
//! - A set of **public keys**: includes at least the previous keys in addition
//!   to any past assertion keys that has been rotated.
//!
//! - A set of **service endpoints**: pointing to the description of the
//!   services the DID subject exposes. For more information, check the W3C DID
//!   Core specification.
//!
//!
//! ## Assumptions
//!
//! - The maximum number of new key agreement keys that can be specified in a
//!   creation or update operation is bounded by `MaxNewKeyAgreementKeys`.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod default_weights;
pub mod did_details;
pub mod errors;
// pub mod origin;
pub mod service_endpoints;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod mock;
// #[cfg(any(feature = "runtime-benchmarks", test))]
// mod mock_utils;
// #[cfg(test)]
// mod tests;

mod signature;
mod utils;

pub use crate::{
	default_weights::WeightInfo,
	did_details::{DidSignature, DidVerificationKeyRelationship},
	// origin::{DidRawOrigin, EnsureDidOrigin},
	pallet::*,
	signature::DidSignatureVerify,
};

use codec::Encode;
use frame_support::{
	dispatch::DispatchResult, ensure, storage::types::StorageMap, traits::Get, Parameter,
};
use sp_runtime::traits::Zero;
use sp_std::{fmt::Debug, prelude::Clone};

#[cfg(feature = "runtime-benchmarks")]
use frame_system::RawOrigin;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::StorageVersion};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::BadOrigin;

	use crate::{
		did_details::{
			DidCreationDetails, DidDetails, DidEncryptionKey, DidSignature,
			DidVerifiableIdentifier, DidVerificationKey,
		},
		errors::{DidError, InputError, SignatureError, StorageError},
		service_endpoints::{DidEndpoint, ServiceEndpointId},
	};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Reference to a payload of data of variable size.
	pub type Payload = [u8];

	/// Type for a DID key identifier.
	pub type KeyIdOf<T> = <T as frame_system::Config>::Hash;

	/// Type for a call nonce
	pub type CallNonceOf<T> = <T as frame_system::Config>::Hash;

	/// Type for a DID subject identifier.
	pub type DidIdentifierOf<T> = <T as Config>::DidIdentifier;

	/// Type for a CORD account identifier.
	pub type CordAccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = CordAccountIdOf<Self>,
		>;
		/// Type for a DID subject identifier.
		type DidIdentifier: Parameter + DidVerifiableIdentifier + MaxEncodedLen;

		/// Maximum number of total public keys which can be stored per DID key
		/// identifier. This includes the ones currently used for
		/// authentication, key agreement, attestation, and delegation
		#[pallet::constant]
		type MaxPublicKeysPerDid: Get<u32>;

		/// Maximum number of total key agreement keys that can be stored for a
		/// DID identifier.
		#[pallet::constant]
		type MaxKeyAgreementKeys: Get<u32> + Debug + Clone + PartialEq;

		/// The maximum number of services that can be stored under a DID.
		#[pallet::constant]
		type MaxNumberOfServicesPerDid: Get<u32>;

		/// The maximum length of a service ID.
		#[pallet::constant]
		type MaxServiceIdLength: Get<u32>;

		/// The maximum length of a service type description.
		#[pallet::constant]
		type MaxServiceTypeLength: Get<u32>;

		/// The maximum number of a types description for a service endpoint.
		#[pallet::constant]
		type MaxNumberOfTypesPerService: Get<u32>;

		/// The maximum length of a service URL.
		#[pallet::constant]
		type MaxServiceUrlLength: Get<u32>;

		/// The maximum number of a URLs for a service endpoint.
		#[pallet::constant]
		type MaxNumberOfUrlsPerService: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// DIDs stored on chain.
	///
	/// It maps from a DID identifier to the DID details.
	#[pallet::storage]
	#[pallet::getter(fn get_did)]
	pub type Did<T> = StorageMap<_, Blake2_128Concat, DidIdentifierOf<T>, DidDetails<T>>;

	/// Service endpoints associated with DIDs.
	///
	/// It maps from (DID identifier, service ID) to the service details.
	#[pallet::storage]
	#[pallet::getter(fn get_service_endpoints)]
	pub type ServiceEndpoints<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		DidIdentifierOf<T>,
		Blake2_128Concat,
		ServiceEndpointId<T>,
		DidEndpoint<T>,
	>;

	/// Counter of service endpoints for each DID.
	///
	/// It maps from (DID identifier) to a 32-bit counter.
	#[pallet::storage]
	pub(crate) type DidEndpointsCount<T> =
		StorageMap<_, Blake2_128Concat, DidIdentifierOf<T>, u32, ValueQuery>;

	/// The set of DIDs that have been deleted and cannot therefore be created
	/// again for security reasons.
	///
	/// It maps from a DID identifier to a unit tuple, for the sake of tracking
	/// DID identifiers.
	#[pallet::storage]
	#[pallet::getter(fn get_deleted_did)]
	pub(crate) type DidBlacklist<T> = StorageMap<_, Blake2_128Concat, DidIdentifierOf<T>, ()>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new DID has been created.
		/// \[transaction author, DID identifier\]
		Created { author: CordAccountIdOf<T>, identifier: DidIdentifierOf<T> },
		/// A DID has been updated.
		/// \[transaction author, DID identifier\]
		Updated { author: CordAccountIdOf<T>, identifier: DidIdentifierOf<T> },
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
		/// The DID operation nonce is not matching the signature
		InvalidNonce,
		/// The maximum number of key agreements has been reached for the DID
		/// identifier.
		MaxKeyAgreementKeysExceeded,
		/// The maximum number of public keys for this DID key identifier has
		/// been reached.
		MaxPublicKeysPerDidExceeded,
		/// The DID has already been previously deleted.
		DidAlreadyDeleted,
		/// The origin is unable to reserve the deposit and pay the fee.
		UnableToPayFees,
		/// The maximum number of service endpoints for a DID has been exceeded.
		MaxNumberOfServicesPerDidExceeded,
		/// The service endpoint ID exceeded the maximum allowed length.
		MaxServiceIdLengthExceeded,
		/// One of the service endpoint types exceeded the maximum allowed
		/// length.
		MaxServiceTypeLengthExceeded,
		/// The maximum number of types for a service endpoint has been
		/// exceeded.
		MaxNumberOfTypesPerServiceExceeded,
		/// One of the service endpoint URLs exceeded the maximum allowed
		/// length.
		MaxServiceUrlLengthExceeded,
		/// The maximum number of URLs for a service endpoint has been exceeded.
		MaxNumberOfUrlsPerServiceExceeded,
		/// A service with the provided ID is already present for the given DID.
		ServiceAlreadyPresent,
		/// A service with the provided ID is not present under the given DID.
		ServiceNotPresent,
		/// One of the service endpoint details contains non-ASCII characters.
		InvalidServiceEncoding,
		/// The number of service endpoints stored under the DID is larger than
		/// the number of endpoints to delete.
		StoredEndpointsCountTooLarge,
		/// An error that is not supposed to take place, yet it happened.
		InternalError,
	}

	impl<T> From<DidError> for Error<T> {
		fn from(error: DidError) -> Self {
			match error {
				DidError::StorageError(storage_error) => Self::from(storage_error),
				DidError::SignatureError(operation_error) => Self::from(operation_error),
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
				StorageError::DidKeyNotPresent(_) | StorageError::KeyNotPresent => {
					Self::VerificationKeyNotPresent
				},
				StorageError::MaxPublicKeysPerDidExceeded => Self::MaxPublicKeysPerDidExceeded,
				StorageError::MaxKeyAgreementKeysExceeded => Self::MaxKeyAgreementKeysExceeded,
				StorageError::DidAlreadyDeleted => Self::DidAlreadyDeleted,
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

	impl<T> From<InputError> for Error<T> {
		fn from(error: InputError) -> Self {
			match error {
				InputError::MaxIdLengthExceeded => Self::MaxServiceIdLengthExceeded,
				InputError::MaxServicesCountExceeded => Self::MaxNumberOfServicesPerDidExceeded,
				InputError::MaxTypeCountExceeded => Self::MaxNumberOfTypesPerServiceExceeded,
				InputError::MaxTypeLengthExceeded => Self::MaxServiceTypeLengthExceeded,
				InputError::MaxUrlCountExceeded => Self::MaxNumberOfUrlsPerServiceExceeded,
				InputError::MaxUrlLengthExceeded => Self::MaxServiceUrlLengthExceeded,
				InputError::InvalidEncoding => Self::InvalidServiceEncoding,
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Store a new DID on chain, after verifying that the creation
		/// operation has been signed by the CORD account associated with the
		/// identifier of the DID being created and that a DID with the same
		/// identifier has not previously existed on (and then deleted from) the
		/// chain.
		///
		/// There must be no DID information stored on chain under the same DID
		/// identifier.
		///
		/// The new keys added with this operation are stored under the DID
		/// identifier along with the block number in which the operation was
		/// executed.
		///
		/// The dispatch origin can be any CORD account with enough funds to
		/// execute the extrinsic and it does not have to be tied in any way to
		/// the CORD account identifying the DID subject.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did, DidBlacklist
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create_ed25519_key().max(<T as pallet::Config>::WeightInfo::create_sr25519_key()).max(<T as pallet::Config>::WeightInfo::create_ecdsa_key()))]
		pub fn create(
			origin: OriginFor<T>,
			details: DidCreationDetails<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(author == details.submitter, BadOrigin);

			let did_identifier = details.did.clone();

			// Make sure that DIDs cannot be created again after they have been deleted.
			ensure!(
				!DidBlacklist::<T>::contains_key(&did_identifier),
				Error::<T>::DidAlreadyDeleted
			);

			ensure!(!Did::<T>::contains_key(&did_identifier), Error::<T>::DidAlreadyPresent);

			let account_did_auth_key = did_identifier
				.verify_and_recover_signature(&details.encode(), &signature)
				.map_err(Error::<T>::from)?;

			let did_entry = DidDetails::from_creation_details(account_did_auth_key)
				.map_err(Error::<T>::from)?;

			log::debug!("Creating DID {:?}", &did_identifier);

			Did::<T>::insert(&did_identifier, did_entry);

			Self::deposit_event(Event::Created { author, identifier: did_identifier });

			Ok(())
		}

		/// Update the DID authentication key.
		///
		/// If an old key existed, it is deleted from the set of public keys if
		/// it is not used in any other part of the DID. The new key is added to
		/// the set of public keys.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_authentication_key())]
		pub fn set_authentication_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			call_nonce: CallNonceOf<T>,
			new_key: DidVerificationKey,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!(
				"Setting new authentication key {:?} for DID {:?}",
				&new_key,
				&did_identifier
			);

			did_details
				.update_authentication_key(new_key, frame_system::Pallet::<T>::block_number())
				.map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Authentication key set");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });

			Ok(())
		}

		/// Set or update the DID delegation key.
		///
		/// If an old key existed, it is deleted from the set of public keys if
		/// it is not used in any other part of the DID. The new key is added to
		/// the set of public keys.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_delegation_key())]
		pub fn set_delegation_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			call_nonce: CallNonceOf<T>,
			new_key: DidVerificationKey,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!("Setting new delegation key {:?} for DID {:?}", &new_key, &did_identifier);

			did_details
				.update_delegation_key(new_key, frame_system::Pallet::<T>::block_number())
				.map_err(Error::<T>::from)?;

			// *** No Fail beyond this call ***

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Delegation key set");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Remove the DID delegation key.
		///
		/// The old key is deleted from the set of public keys if
		/// it is not used in any other part of the DID.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_delegation_key())]
		pub fn remove_delegation_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!("Removing delegation key for DID {:?}", &did_identifier);
			did_details.remove_delegation_key().map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Delegation key removed");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Set or update the DID assertion key.
		///
		/// The new key is added to the set of public keys.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_attestation_key())]
		pub fn set_assertion_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			call_nonce: CallNonceOf<T>,
			new_key: DidVerificationKey,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!("Setting new assertion key {:?} for DID {:?}", &new_key, &did_identifier);
			did_details
				.update_assertion_key(new_key, frame_system::Pallet::<T>::block_number())
				.map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Assertion key set");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Remove the DID assertion key.
		///
		/// The old key is deleted from the set of public keys if
		/// it is not used in any other part of the DID.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_attestation_key())]
		pub fn remove_assertion_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			key_id: KeyIdOf<T>,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!("Removing assertion key for DID {:?}", &did_identifier);
			did_details.remove_assertion_key(key_id).map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Assertion key removed");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Add a single new key agreement key to the DID.
		///
		/// The new key is added to the set of public keys.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_key_agreement_key())]
		pub fn add_key_agreement_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			new_key: DidEncryptionKey,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!(
				"Adding new key agreement key {:?} for DID {:?}",
				&new_key,
				&did_identifier
			);
			did_details
				.add_key_agreement_key(new_key, frame_system::Pallet::<T>::block_number())
				.map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Key agreement key set");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Remove a DID key agreement key from both its set of key agreement
		/// keys and as well as its public keys.
		///
		/// The dispatch origin must be a DID origin proxied via the
		/// `submit_did_call` extrinsic.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_key_agreement_key())]
		pub fn remove_key_agreement_key(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			key_id: KeyIdOf<T>,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut did_details =
				Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			log::debug!("Removing key agreement key for DID {:?}", &did_identifier);
			did_details.remove_key_agreement_key(key_id).map_err(Error::<T>::from)?;

			Did::<T>::insert(&did_identifier, did_details);
			log::debug!("Key agreement key removed");

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });
			Ok(())
		}

		/// Add a new service endpoint under the given DID.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did, ServiceEndpoints, DidEndpointsCount
		/// - Writes: Did, ServiceEndpoints, DidEndpointsCount
		/// # </weight>
		#[pallet::call_index(8)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_service_endpoint())]
		pub fn add_service_endpoint(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			service_endpoint: DidEndpoint<T>,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let did_details = Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			service_endpoint.validate_against_constraints().map_err(Error::<T>::from)?;

			// Verify that the DID is present.
			ensure!(Did::<T>::get(&did_identifier).is_some(), Error::<T>::DidNotPresent);

			let currently_stored_endpoints_count = DidEndpointsCount::<T>::get(&did_identifier);

			// Verify that there are less than the maximum limit of services stored.
			ensure!(
				currently_stored_endpoints_count < T::MaxNumberOfServicesPerDid::get(),
				Error::<T>::MaxNumberOfServicesPerDidExceeded
			);

			ServiceEndpoints::<T>::try_mutate(
				&did_identifier,
				service_endpoint.id.clone(),
				|existing_service| -> Result<(), Error<T>> {
					ensure!(existing_service.is_none(), Error::<T>::ServiceAlreadyPresent);
					*existing_service = Some(service_endpoint);
					Ok(())
				},
			)?;
			DidEndpointsCount::<T>::insert(
				&did_identifier,
				currently_stored_endpoints_count.saturating_add(1),
			);

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });

			Ok(())
		}

		/// Remove the service with the provided ID from the DID.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], ServiceEndpoints, DidEndpointsCount
		/// - Writes: Did, ServiceEndpoints, DidEndpointsCount
		/// # </weight>
		#[pallet::call_index(9)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_service_endpoint())]
		pub fn remove_service_endpoint(
			origin: OriginFor<T>,
			did_identifier: DidIdentifierOf<T>,
			service_id: ServiceEndpointId<T>,
			call_nonce: CallNonceOf<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let did_details = Did::<T>::get(&did_identifier).ok_or(Error::<T>::DidNotPresent)?;

			Self::verify_signature_with_did_key_type(
				&call_nonce.encode(),
				&signature,
				&did_details,
				DidVerificationKeyRelationship::Authentication,
			)
			.map_err(Error::<T>::from)?;

			ensure!(
				ServiceEndpoints::<T>::take(&did_identifier, &service_id).is_some(),
				Error::<T>::ServiceNotPresent
			);

			// Decrease the endpoints counter or delete the entry if it reaches 0.
			DidEndpointsCount::<T>::mutate_exists(&did_identifier, |existing_endpoint_count| {
				let new_value = existing_endpoint_count.unwrap_or_default().saturating_sub(1);
				if new_value.is_zero() {
					*existing_endpoint_count = None;
				} else {
					*existing_endpoint_count = Some(new_value);
				}
			});

			Self::deposit_event(Event::Updated { author, identifier: did_identifier });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Verify a generic payload signature using a given DID verification
		/// key type.
		pub fn verify_signature_with_did_key_type(
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
			verification_key
				.verify_signature(payload, signature)
				.map_err(DidError::SignatureError)
		}
	}
}

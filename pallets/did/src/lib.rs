// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

//! # DID Pallet
//!
//! Provides W3C-compliant DID functionalities. A DID identifier is derived from
//! a KILT address and must be verifiable, i.e., must be able to generate
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
//! Each DID identifier is mapped to a set of keys, which in KILT are used for
//! different purposes.
//!
//! - One **authentication key**: used to sign and authorise DID-management
//!   operations (e.g., the update of some keys or the deletion of the whole
//!   DID). This is required to always be present as otherwise the DID becomes
//!   unusable since no operation signature can be verified anymore.
//!
//! - Zero or more **key agreement keys**: used by other parties that want to
//!   interact with the DID subject to perform ECDH and encrypt information
//!   addressed to the DID subject.
//!
//! - Zero or one **delegation key**: used to sign and authorise the creation of
//!   new delegation nodes on the KILT blockchain. In case no delegation key is
//!   present, the DID subject cannot write new delegations on the KILT
//!   blockchain. For more info, check the [delegation
//!   pallet](../../delegation/).
//!
//! - Zero or one **attestation key**: used to sign and authorise the creation
//!   of new attested claims on the KILT blockchain. In case no attestation key
//!   is present, the DID subject cannot write new attested claims on the KILT
//!   blockchain. For more info, check the [attestation
//!   pallet](../../attestation/).
//!
//! - A set of **public keys**: includes at least the previous keys in addition
//!   to any past attestation key that has been rotated but not entirely
//!   revoked.
//!
//! - An optional **service endpoints description**: pointing to the description
//!   of the services the DID subject exposes and storing a cryptographic hash
//!   of that information to ensure the integrity of the content.
//!  For more information, check the W3C DID Core specification.
//!
//! - A **transaction counter**: acts as a nonce to avoid replay or signature
//!   forgery attacks. Each time a DID-signed transaction is executed, the
//!   counter is incremented.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `create` - Register a new DID on the KILT blockchain under the given DID
//!   identifier.
//! - `update` - Update any keys or the service endpoints of an existing DID.
//! - `delete` - Delete the specified DID and all related keys from the KILT
//!   blockchain.
//! - `submit_did_call` - Proxy a dispatchable function for an extrinsic that
//!   expects a DID origin. The DID pallet verifies the signature and the nonce
//!   of the wrapping operation and then dispatches the underlying extrinsic
//!   upon successful verification.
//!
//! ## Assumptions
//!
//! - The maximum number of new key agreement keys that can be specified in a
//!   creation or update operation is bounded by `MaxNewKeyAgreementKeys`.
//! - The maximum number of endpoint URLs for a new DID service description is
//!   bounded by `MaxEndpointUrlsCount`.
//! - The maximum length in ASCII characters of any endpoint URL is bounded by
//!   `MaxUrlLength`.
//! - The chain performs basic checks over the endpoint URLs provided in
//!   creation and deletion operations. The SDK will perform more in-depth
//!   validation of the URL string, e.g., by pattern-matching using regexes.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod default_weights;
pub mod did_details;
pub mod errors;
pub mod migrations;
pub mod origin;
pub mod url;

mod utils;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod mock;
// #[cfg(any(feature = "runtime-benchmarks", test))]
// mod mock_utils;
// #[cfg(test)]
// mod tests;

mod deprecated;

pub use crate::{
	default_weights::WeightInfo, did_details::*, errors::*, origin::*, pallet::*, url::*,
};

use codec::Encode;
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	ensure,
	pallet_prelude::Weight,
	storage::types::StorageMap,
	traits::Get,
	Parameter,
};
use frame_system::ensure_signed;
#[cfg(feature = "runtime-benchmarks")]
use frame_system::RawOrigin;
use migrations::*;
use sp_runtime::SaturatedConversion;
use sp_std::{boxed::Box, fmt::Debug, prelude::Clone};

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

	/// Type for a runtime extrinsic callable under DID-based authorisation.
	pub type DidCallableOf<T> = <T as Config>::Call;

	/// Type for origin that supports a DID sender.
	#[pallet::origin]
	pub type Origin<T> = DidRawOrigin<DidIdentifierOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		/// Type for a dispatchable call that can be proxied through the DID
		/// pallet to support DID-based authorisation.
		type Call: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ DeriveDidCallAuthorizationVerificationKeyRelationship;

		/// Type for a DID subject identifier.
		type DidIdentifier: Parameter + Default + DidVerifiableIdentifier;

		#[cfg(not(feature = "runtime-benchmarks"))]
		/// Origin type expected by the proxied dispatchable calls.
		type Origin: From<DidRawOrigin<DidIdentifierOf<Self>>>;

		#[cfg(feature = "runtime-benchmarks")]
		type Origin: From<RawOrigin<DidIdentifierOf<Self>>>;
		type EnsureOrigin: EnsureOrigin<
			Success = DidIdentifierOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// Overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Maximum number of total public keys which can be stored per DID key
		/// identifier. This includes the ones currently used for
		/// authentication, key agreement, attestation, and delegation.
		#[pallet::constant]
		type MaxPublicKeysPerDid: Get<u32>;

		/// Maximum number of key agreement keys that can be added in a creation
		/// operation.
		#[pallet::constant]
		type MaxNewKeyAgreementKeys: Get<u32>;

		/// Maximum number of total key agreement keys that can be stored for a
		/// DID subject.
		///
		/// Should be greater than `MaxNewKeyAgreementKeys`.
		#[pallet::constant]
		type MaxTotalKeyAgreementKeys: Get<u32> + Debug + Clone + PartialEq;

		/// Maximum length in ASCII characters of the endpoint URL specified in
		/// a creation or update operation.
		#[pallet::constant]
		type MaxUrlLength: Get<u32> + Debug + Clone + PartialEq;

		/// Maximum number of URLs that a service endpoint can contain.
		#[pallet::constant]
		type MaxEndpointUrlsCount: Get<u32> + Debug + Clone + PartialEq;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<(), &'static str> {
			migrations::DidStorageMigrator::<T>::pre_migrate()
		}

		fn on_runtime_upgrade() -> Weight {
			migrations::DidStorageMigrator::<T>::migrate()
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade() -> Result<(), &'static str> {
			migrations::DidStorageMigrator::<T>::post_migrate()
		}
	}

	/// DIDs stored on chain.
	///
	/// It maps from a DID identifier to the DID details.
	#[pallet::storage]
	#[pallet::getter(fn get_did)]
	pub type Did<T> = StorageMap<_, Blake2_128Concat, DidIdentifierOf<T>, DidDetails<T>>;

	/// Contains the latest storage version deployed.
	#[pallet::storage]
	#[pallet::getter(fn last_version_migration_used)]
	pub(crate) type StorageVersion<T> = StorageValue<_, DidStorageVersion, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new DID has been created.
		/// \[transaction signer, DID identifier\]
		DidCreated(AccountIdentifierOf<T>, DidIdentifierOf<T>),
		/// A DID has been updated.
		/// \[DID identifier\]
		DidUpdated(DidIdentifierOf<T>),
		/// A DID has been deleted.
		/// \[DID identifier\]
		DidDeleted(DidIdentifierOf<T>),
		/// A DID-authorised call has been executed.
		/// \[DID caller, dispatch result\]
		DidCallDispatched(DidIdentifierOf<T>, DispatchResult),
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
		/// The DID fragment is not present in the DID details.
		DidFragmentNotPresent,
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
		/// used as an authentication, delegation, or attestation key, and this
		/// is not allowed.
		CurrentlyActiveKey,
		/// The called extrinsic does not support DID authorisation.
		UnsupportedDidAuthorizationCall,
		/// A number of new key agreement keys greater than the maximum allowed
		/// has been provided.
		MaxKeyAgreementKeysLimitExceeded,
		/// A number of new verification keys to remove greater than the maximum
		/// allowed has been provided.
		MaxVerificationKeysToRemoveLimitExceeded,
		/// A URL longer than the maximum size allowed has been provided.
		MaxUrlLengthExceeded,
		/// More than the maximum number of URLs have been specified.
		MaxUrlsCountExceeded,
		/// The maximum number of public keys for this DID key identifier has
		/// been reached.
		MaxPublicKeysPerDidExceeded,
		/// The maximum number of key agreements has been reached for the DID
		/// subject.
		MaxTotalKeyAgreementKeysExceeded,
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
				StorageError::DidKeyNotPresent(_) | StorageError::KeyNotPresent => {
					Self::VerificationKeyNotPresent
				}
				StorageError::MaxTxCounterValue => Self::MaxTxCounterValue,
				StorageError::CurrentlyActiveKey => Self::CurrentlyActiveKey,
				StorageError::MaxPublicKeysPerDidExceeded => Self::MaxPublicKeysPerDidExceeded,
				StorageError::MaxTotalKeyAgreementKeysExceeded => {
					Self::MaxTotalKeyAgreementKeysExceeded
				}
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
				InputError::MaxKeyAgreementKeysLimitExceeded => {
					Self::MaxKeyAgreementKeysLimitExceeded
				}
				InputError::MaxVerificationKeysToRemoveLimitExceeded => {
					Self::MaxVerificationKeysToRemoveLimitExceeded
				}
				InputError::MaxUrlLengthExceeded => Self::MaxUrlLengthExceeded,
				InputError::MaxUrlsCountExceeded => Self::MaxUrlsCountExceeded,
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Store a new DID on chain, after verifying that the creation
		/// operation has been signed by the KILT account associated with the
		/// identifier of the DID being created.
		///
		/// There must be no DID information stored on chain under the same DID
		/// identifier.
		///
		/// The new keys added with this operation are stored under the DID
		/// identifier along with the block number in which the operation was
		/// executed.
		///
		/// The dispatch origin can be any KILT account with enough funds to
		/// execute the extrinsic and it does not have to be tied in any way to
		/// the KILT account identifying the DID subject.
		///
		/// Emits `DidCreated`.
		///
		/// # <weight>
		/// - The transaction's complexity is mainly dependent on the number of
		///   new key agreement keys included in the operation as well as the
		///   length of the URL endpoint, if present.
		/// ---------
		/// Weight: O(K) + O(N * E) where K is the number of new key agreement
		/// keys bounded by `MaxNewKeyAgreementKeys`, E the length of the
		/// longest endpoint URL bounded by `MaxUrlLength`, and N the maximum
		/// amount of endpoint URLs, bounded by `MaxEndpointUrlsCount`.
		/// - Reads: [Origin Account], Did
		/// - Writes: Did (with K new key agreement keys and N endpoint URLs)
		/// # </weight>
		#[pallet::weight({
			let new_key_agreement_keys = details.new_key_agreement_keys.len().saturated_into::<u32>();
			let new_urls = details.new_service_endpoints.as_ref().map_or(BoundedVec::default(), |endpoint| endpoint.urls.clone());
			// Max 3, so we can iterate quite easily.
			let max_url_length = new_urls.iter().map(|url| url.len().saturated_into()).max().unwrap_or(0u32);

			let ed25519_weight = <T as pallet::Config>::WeightInfo::create_ed25519_keys(
				new_key_agreement_keys,
				new_urls.len().saturated_into(),
				max_url_length
			);
			let sr25519_weight = <T as pallet::Config>::WeightInfo::create_sr25519_keys(
				new_key_agreement_keys,
				new_urls.len().saturated_into(),
				max_url_length
			);
			let ecdsa_weight = <T as pallet::Config>::WeightInfo::create_ecdsa_keys(
				new_key_agreement_keys,
				new_urls.len().saturated_into(),
				max_url_length
			);

			ed25519_weight.max(sr25519_weight).max(ecdsa_weight)
		})]
		pub fn create(
			origin: OriginFor<T>,
			details: DidCreationDetails<T>,
			signature: DidSignature,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let did_identifier = details.did.clone();

			// There has to be no other DID with the same identifier already saved on chain,
			// otherwise generate a DidAlreadyPresent error.
			ensure!(!<Did<T>>::contains_key(&did_identifier), <Error<T>>::DidAlreadyPresent);

			let account_did_auth_key = did_identifier
				.verify_and_recover_signature(&details.encode(), &signature)
				.map_err(<Error<T>>::from)?;

			let did_entry = DidDetails::from_creation_details(details, account_did_auth_key)
				.map_err(<Error<T>>::from)?;

			log::debug!("Creating DID {:?}", &did_identifier);
			<Did<T>>::insert(&did_identifier, did_entry);

			Self::deposit_event(Event::DidCreated(sender, did_identifier));

			Ok(())
		}

		#[pallet::weight(10)]
		pub fn set_authentication_key(
			origin: OriginFor<T>,
			new_key: DidVerificationKey,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Setting new authentication key {:?} for DID {:?}", &new_key, &did_subject);

			did_details
				.update_authentication_key(new_key, <frame_system::Pallet<T>>::block_number())
				.map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Authentication key set");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn set_delegation_key(
			origin: OriginFor<T>,
			new_key: DidVerificationKey,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Setting new delegation key {:?} for DID {:?}", &new_key, &did_subject);
			did_details
				.update_delegation_key(new_key, <frame_system::Pallet<T>>::block_number())
				.map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Delegation key set");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn remove_delegation_key(origin: OriginFor<T>) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Removing delegation key for DID {:?}", &did_subject);
			did_details.remove_delegation_key().map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Delegation key removed");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn set_attestation_key(
			origin: OriginFor<T>,
			new_key: DidVerificationKey,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Setting new attestation key {:?} for DID {:?}", &new_key, &did_subject);
			did_details
				.update_attestation_key(new_key, <frame_system::Pallet<T>>::block_number())
				.map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Attestation key set");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn remove_attestation_key(origin: OriginFor<T>) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Removing attestation key for DID {:?}", &did_subject);
			did_details.remove_attestation_key().map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Attestation key removed");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn add_key_agreement_key(
			origin: OriginFor<T>,
			new_key: DidEncryptionKey,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Adding new key agreement key {:?} for DID {:?}", &new_key, &did_subject);
			did_details
				.add_key_agreement_key(new_key, <frame_system::Pallet<T>>::block_number())
				.map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Key agreement key set");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn remove_key_agreement_key(
			origin: OriginFor<T>,
			key_id: KeyIdOf<T>,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Removing key agreement key for DID {:?}", &did_subject);
			did_details.remove_key_agreement_key(key_id).map_err(<Error<T>>::from)?;

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Key agreement key removed");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn set_service_endpoints(
			origin: OriginFor<T>,
			service_endpoints: ServiceEndpoints<T>,
		) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!(
				"Adding new service endpoints {:?} for DID {:?}",
				&service_endpoints,
				&did_subject
			);
			service_endpoints.validate_against_config_limits().map_err(<Error<T>>::from)?;

			did_details.service_endpoints = Some(service_endpoints);

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Service endpoints set");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		//TODO: Add comment and weights after benchmarks
		#[pallet::weight(10)]
		pub fn remove_service_endpoints(origin: OriginFor<T>) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;
			let mut did_details = <Did<T>>::get(&did_subject).ok_or(<Error<T>>::DidNotPresent)?;

			log::debug!("Removing service endpoints for DID {:?}", &did_subject);
			ensure!(
				did_details.service_endpoints.take().is_some(),
				<Error<T>>::DidFragmentNotPresent
			);

			<Did<T>>::insert(&did_subject, did_details);
			log::debug!("Service endpoints removed");

			Self::deposit_event(Event::DidUpdated(did_subject));
			Ok(())
		}

		/// Delete a DID from the chain and all information associated with it,
		/// after verifying that the delete operation has been signed by the DID
		/// subject using the authentication key currently stored on chain.
		///
		/// The referenced DID identifier must be present on chain before the
		/// delete operation is evaluated.
		///
		/// As the result of the deletion, all traces of the DID are removed
		/// from the storage, which results in the invalidation of all
		/// attestations issued by the DID subject.
		///
		/// The dispatch origin must be a DID origin proxied via the
		/// `submit_did_call` extrinsic.
		///
		/// Emits `DidDeleted`.
		///
		/// # <weight>
		/// Weight: O(1)
		/// - Reads: [Origin Account], Did
		/// - Kills: Did entry associated to the DID identifier
		/// # </weight>
		#[pallet::weight(<T as pallet::Config>::WeightInfo::delete())]
		pub fn delete(origin: OriginFor<T>) -> DispatchResult {
			let did_subject = T::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				// `take` calls `kill` internally
				<Did<T>>::take(&did_subject).is_some(),
				<Error<T>>::DidNotPresent
			);

			log::debug!("Deleting DID {:?}", did_subject);

			Self::deposit_event(Event::DidDeleted(did_subject));

			Ok(())
		}

		/// Proxy a dispatchable call of another runtime extrinsic that
		/// supports a DID origin.
		///
		/// The referenced DID identifier must be present on chain before the
		/// operation is dispatched.
		///
		/// A call submitted through this extrinsic must be signed with the
		/// right DID key, depending on the call. This information is provided
		/// by the `DidAuthorizedCallOperation` parameter, which specifies the
		/// DID subject acting as the origin of the call, the DID's tx counter
		/// (nonce), the dispatchable to call in case signature verification
		/// succeeds, and the type of DID key to use to verify the operation
		/// signature.
		///
		/// In case the signature is incorrect, the nonce is not valid, or the
		/// required key is not present for the specified DID, the verification
		/// fails and the call is not dispatched. Otherwise, the call is
		/// properly dispatched with a `DidOrigin` origin indicating the DID
		/// subject.
		///
		/// A successful dispatch operation results in the tx counter associated
		/// with the given DID to be incremented, to mitigate replay attacks.
		///
		/// The dispatch origin can be any KILT account with enough funds to
		/// execute the extrinsic and it does not have to be tied in any way to
		/// the KILT account identifying the DID subject.
		///
		/// Emits `DidCallDispatched`.
		///
		/// # <weight>
		/// Weight: O(1) + weight of the dispatched call
		/// - Reads: [Origin Account], Did
		/// - Writes: Did
		/// # </weight>
		#[allow(clippy::boxed_local)]
		#[pallet::weight({
			let di = did_call.call.get_dispatch_info();
			let max_sig_weight = <T as pallet::Config>::WeightInfo::submit_did_call_ed25519_key()
			.max(<T as pallet::Config>::WeightInfo::submit_did_call_sr25519_key())
			.max(<T as pallet::Config>::WeightInfo::submit_did_call_ecdsa_key());

			(max_sig_weight.saturating_add(di.weight), di.class)
		})]
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

			Self::verify_did_operation_signature_and_increase_nonce(&wrapped_operation, &signature)
				.map_err(<Error<T>>::from)?;

			log::debug!("Dispatch call from DID {:?}", did_identifier);

			// Dispatch the referenced [Call] instance and return its result
			let DidAuthorizedCallOperation { did, call, .. } = wrapped_operation.operation;

			#[cfg(not(feature = "runtime-benchmarks"))]
			let result = call.dispatch(DidRawOrigin { id: did }.into());
			#[cfg(feature = "runtime-benchmarks")]
			let result = call.dispatch(RawOrigin::Signed(did).into());

			let dispatch_event_payload = result.map(|_| ()).map_err(|e| e.error);

			Self::deposit_event(Event::DidCallDispatched(did_identifier, dispatch_event_payload));

			result
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Verify the validity (i.e., nonce and signature) of a DID-authorized
	/// operation and, if valid, update the DID state with the latest
	/// nonce.
	///
	/// # <weight>
	/// Weight: O(1)
	/// - Reads: Did
	/// - Writes: Did
	/// # </weight>
	pub fn verify_did_operation_signature_and_increase_nonce(
		operation: &DidAuthorizedCallOperationWithVerificationRelationship<T>,
		signature: &DidSignature,
	) -> Result<(), DidError> {
		let mut did_details = <Did<T>>::get(&operation.did)
			.ok_or(DidError::StorageError(StorageError::DidNotPresent))?;

		Self::validate_counter_value(operation.tx_counter, &did_details)?;
		// Increase the tx counter as soon as it is considered valid, no matter if the
		// signature is valid or not.
		did_details.increase_tx_counter().map_err(DidError::StorageError)?;
		Self::verify_payload_signature_with_did_key_type(
			&operation.encode(),
			signature,
			&did_details,
			operation.verification_key_relationship,
		)?;

		<Did<T>>::insert(&operation.did, did_details);

		Ok(())
	}

	// Verify the validity of a DID-authorized operation nonce.
	// To be valid, the nonce must be equal to the one currently stored + 1.
	// This is to avoid quickly "consuming" all the possible values for the counter,
	// as that would result in the DID being unusable, since we do not have yet any
	// mechanism in place to wrap the counter value around when the limit is
	// reached.
	fn validate_counter_value(counter: u64, did_details: &DidDetails<T>) -> Result<(), DidError> {
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
			counter == expected_nonce_value,
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
		verification_key
			.verify_signature(payload, signature)
			.map_err(DidError::SignatureError)
	}
}

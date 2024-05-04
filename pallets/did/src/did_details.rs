// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use codec::{Decode, Encode, MaxEncodedLen, WrapperTypeEncode};
use frame_support::{
	ensure,
	storage::{bounded_btree_map::BoundedBTreeMap, bounded_btree_set::BoundedBTreeSet},
	traits::Get,
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::RuntimeDebug;

use crate::{
	errors::{self, DidError, SignatureError, StorageError},
	utils, AccountIdOf, Config, DidAuthorizedCallOperationOf, DidCreationDetailsOf, KeyIdOf,
	Payload,
};
// use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_core::{ecdsa, ed25519, sr25519};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiSignature, SaturatedConversion,
};
use sp_std::{convert::TryInto, vec::Vec};

/// Public verification key that a DID can control.
#[derive(
	Clone, Decode, RuntimeDebug, Encode, Eq, Ord, PartialEq, PartialOrd, TypeInfo, MaxEncodedLen,
)]
pub enum DidVerificationKey<AccountId> {
	/// An Ed25519 public key.
	Ed25519(ed25519::Public),
	/// A Sr25519 public key.
	Sr25519(sr25519::Public),
	/// An ECDSA public key.
	Ecdsa(ecdsa::Public),
	/// Account Identifier
	Account(AccountId),
}

impl<AccountId> DidVerificationKey<AccountId> {
	/// Verify a DID signature using one of the DID keys.
	pub fn verify_signature(
		&self,
		payload: &Payload,
		signature: &DidSignature,
	) -> Result<(), SignatureError> {
		match (self, signature) {
			(DidVerificationKey::Ed25519(public_key), DidSignature::Ed25519(sig)) => {
				ensure!(sig.verify(payload, public_key), SignatureError::InvalidData);
				Ok(())
			},
			// Follows same process as above, but using a Sr25519 instead
			(DidVerificationKey::Sr25519(public_key), DidSignature::Sr25519(sig)) => {
				ensure!(sig.verify(payload, public_key), SignatureError::InvalidData);
				Ok(())
			},
			(DidVerificationKey::Ecdsa(public_key), DidSignature::Ecdsa(sig)) => {
				ensure!(sig.verify(payload, public_key), SignatureError::InvalidData);
				Ok(())
			},
			_ => Err(SignatureError::InvalidFormat),
		}
	}
}

impl<AccountId> IdentifyAccount for DidVerificationKey<AccountId>
where
	AccountId: From<[u8; 32]> + AsRef<[u8; 32]>,
{
	type AccountId = AccountId;

	fn into_account(self) -> Self::AccountId {
		let bytes = match self {
			DidVerificationKey::Ed25519(pub_key) => pub_key.0,
			DidVerificationKey::Sr25519(pub_key) => pub_key.0,
			// Hash the Ecdsa key the same way it's done in substrate (the ecdsa key is 33 bytes,
			// one byte too long)
			DidVerificationKey::Ecdsa(pub_key) => sp_io::hashing::blake2_256(pub_key.as_ref()),
			DidVerificationKey::Account(acc_id) => *acc_id.as_ref(),
		};

		bytes.into()
	}
}

impl<AccountId> From<ed25519::Public> for DidVerificationKey<AccountId> {
	fn from(key: ed25519::Public) -> Self {
		DidVerificationKey::Ed25519(key)
	}
}

impl<AccountId> From<sr25519::Public> for DidVerificationKey<AccountId> {
	fn from(key: sr25519::Public) -> Self {
		DidVerificationKey::Sr25519(key)
	}
}

impl<AccountId> From<ecdsa::Public> for DidVerificationKey<AccountId> {
	fn from(key: ecdsa::Public) -> Self {
		DidVerificationKey::Ecdsa(key)
	}
}

/// Types of encryption keys a DID can control.
#[derive(
	Clone,
	Copy,
	Decode,
	RuntimeDebug,
	Encode,
	Eq,
	Ord,
	PartialEq,
	PartialOrd,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum DidEncryptionKey {
	/// An X25519 public key.
	X25519([u8; 32]),
}

/// A general public key under the control of the DID.
#[derive(
	Clone, Decode, RuntimeDebug, Encode, Eq, Ord, PartialEq, PartialOrd, TypeInfo, MaxEncodedLen,
)]
pub enum DidPublicKey<AccountId> {
	/// A verification key, used to generate and verify signatures.
	PublicVerificationKey(DidVerificationKey<AccountId>),
	/// An encryption key, used to encrypt and decrypt payloads.
	PublicEncryptionKey(DidEncryptionKey),
}

impl<AccountId> From<DidVerificationKey<AccountId>> for DidPublicKey<AccountId> {
	fn from(verification_key: DidVerificationKey<AccountId>) -> Self {
		Self::PublicVerificationKey(verification_key)
	}
}

impl<AccountId> From<DidEncryptionKey> for DidPublicKey<AccountId> {
	fn from(encryption_key: DidEncryptionKey) -> Self {
		Self::PublicEncryptionKey(encryption_key)
	}
}

/// Verification methods a verification key can
/// fulfil, according to the [DID specification](https://w3c.github.io/did-spec-registries/#verification-relationships).
#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DidVerificationKeyRelationship {
	/// Key used to authenticate all the DID operations.
	Authentication,
	/// Key used to write and revoke delegations on chain.
	CapabilityDelegation,
	/// Not used for now.
	CapabilityInvocation,
	/// Key used to write and revoke assertions on chain.
	AssertionMethod,
}

/// Types of signatures supported by this pallet.
#[derive(Clone, Decode, RuntimeDebug, Encode, Eq, PartialEq, TypeInfo)]
pub enum DidSignature {
	/// A Ed25519 signature.
	Ed25519(ed25519::Signature),
	/// A Sr25519 signature.
	Sr25519(sr25519::Signature),
	/// An Ecdsa signature.
	Ecdsa(ecdsa::Signature),
}

impl From<ed25519::Signature> for DidSignature {
	fn from(sig: ed25519::Signature) -> Self {
		DidSignature::Ed25519(sig)
	}
}

impl From<sr25519::Signature> for DidSignature {
	fn from(sig: sr25519::Signature) -> Self {
		DidSignature::Sr25519(sig)
	}
}

impl From<ecdsa::Signature> for DidSignature {
	fn from(sig: ecdsa::Signature) -> Self {
		DidSignature::Ecdsa(sig)
	}
}

impl From<MultiSignature> for DidSignature {
	fn from(sig: MultiSignature) -> Self {
		match sig {
			MultiSignature::Ed25519(sig) => Self::Ed25519(sig),
			MultiSignature::Sr25519(sig) => Self::Sr25519(sig),
			MultiSignature::Ecdsa(sig) => Self::Ecdsa(sig),
		}
	}
}

pub trait DidVerifiableIdentifier<AccountId> {
	/// Allows a verifiable identifier to verify a signature it produces and
	/// return the public key
	/// associated with the identifier.
	fn verify_and_recover_signature(
		&self,
		payload: &Payload,
		signature: &DidSignature,
	) -> Result<DidVerificationKey<AccountId>, SignatureError>;
}

impl<I: AsRef<[u8; 32]>, AccountId> DidVerifiableIdentifier<AccountId> for I {
	fn verify_and_recover_signature(
		&self,
		payload: &Payload,
		signature: &DidSignature,
	) -> Result<DidVerificationKey<AccountId>, SignatureError> {
		// So far, either the raw Ed25519/Sr25519 public key or the Blake2-256 hashed
		// ECDSA public key.
		let raw_public_key: &[u8; 32] = self.as_ref();
		match *signature {
			DidSignature::Ed25519(_) => {
				// from_raw simply converts a byte array into a public key with no particular
				// validations
				let ed25519_did_key =
					DidVerificationKey::Ed25519(ed25519::Public::from_raw(*raw_public_key));
				ed25519_did_key.verify_signature(payload, signature).map(|_| ed25519_did_key)
			},
			DidSignature::Sr25519(_) => {
				let sr25519_did_key =
					DidVerificationKey::Sr25519(sr25519::Public::from_raw(*raw_public_key));
				sr25519_did_key.verify_signature(payload, signature).map(|_| sr25519_did_key)
			},
			DidSignature::Ecdsa(ref signature) => {
				let ecdsa_signature: [u8; 65] =
					signature.encode().try_into().map_err(|_| SignatureError::InvalidData)?;
				// ECDSA uses blake2-256 hashing algorithm for signatures, so we hash the given
				// message to recover the public key.
				let hashed_message = sp_io::hashing::blake2_256(payload);
				let recovered_pk: [u8; 33] = sp_io::crypto::secp256k1_ecdsa_recover_compressed(
					&ecdsa_signature,
					&hashed_message,
				)
				.map_err(|_| SignatureError::InvalidData)?;
				let hashed_recovered_pk = sp_io::hashing::blake2_256(&recovered_pk);
				// The hashed recovered public key must be equal to the AccountId32 value, which
				// is the hashed key.
				ensure!(&hashed_recovered_pk == raw_public_key, SignatureError::InvalidData);
				// Safe to reconstruct the public key using the recovered value from
				// secp256k1_ecdsa_recover_compressed
				Ok(DidVerificationKey::from(ecdsa::Public::from(recovered_pk)))
			},
		}
	}
}

/// Details of a public key, which includes the key value and the
/// block number at which it was set.
///
/// It is currently used to keep track of all the past and current
/// assertion keys a DID might control.
#[derive(
	Clone, RuntimeDebug, Decode, Encode, PartialEq, Ord, PartialOrd, Eq, TypeInfo, MaxEncodedLen,
)]
pub struct DidPublicKeyDetails<BlockNumber, AccountId> {
	/// A public key the DID controls.
	pub key: DidPublicKey<AccountId>,
	/// The block number in which the verification key was added to the DID.
	pub block_number: BlockNumber,
}

/// The details associated to a DID identity.
#[derive(Clone, Decode, Encode, PartialEq, TypeInfo, MaxEncodedLen, Debug)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]

pub struct DidDetails<T: Config> {
	/// The ID of the authentication key, used to authenticate DID-related
	/// operations.
	pub authentication_key: KeyIdOf<T>,
	/// The set of the key agreement key IDs, which can be used to encrypt
	/// data addressed to the DID subject.
	pub key_agreement_keys: DidKeyAgreementKeySetOf<T>,
	/// \[OPTIONAL\] The ID of the delegation key, used to verify the
	/// signatures of the delegations created by the DID subject.
	pub delegation_key: Option<KeyIdOf<T>>,
	/// \[OPTIONAL\] The ID of the assertion key, used to verify the
	/// signatures of the assertions created by the DID subject.
	pub assertion_key: Option<KeyIdOf<T>>,
	/// The map of public keys, with the key label as
	/// the key map and the tuple (key, addition_block_number) as the map
	/// value.
	/// The map includes all the keys under the control of the DID subject,
	/// including the ones currently used for authentication, key agreement,
	/// assertion, and delegation. Other than those, the map also contains
	/// the old assertion keys that have been rotated, i.e., they cannot
	/// be used to create new assertions but can still be used to verify
	/// previously issued assertions.
	pub public_keys: DidPublicKeyMapOf<T>,
	/// The counter used to avoid replay attacks, which is checked and
	/// updated upon each DID operation involving with the subject as the
	/// creator.
	pub last_tx_counter: u64,
}

impl<T: Config> DidDetails<T> {
	/// Creates a new instance of DID details with the minimum information,
	/// i.e., an authentication key and the block creation time.
	///
	/// The tx counter is automatically set to 0.
	pub fn new(
		authentication_key: DidVerificationKey<AccountIdOf<T>>,
		block_number: BlockNumberFor<T>,
	) -> Result<Self, StorageError> {
		let mut public_keys = DidPublicKeyMapOf::<T>::default();
		let authentication_key_id =
			utils::calculate_key_id::<T>(&authentication_key.clone().into());
		public_keys
			.try_insert(
				authentication_key_id,
				DidPublicKeyDetails { key: authentication_key.into(), block_number },
			)
			.map_err(|_| StorageError::MaxPublicKeysExceeded)?;

		Ok(Self {
			authentication_key: authentication_key_id,
			key_agreement_keys: DidKeyAgreementKeySetOf::<T>::default(),
			assertion_key: None,
			delegation_key: None,
			public_keys,
			last_tx_counter: 0u64,
		})
	}

	// Creates a new DID entry from some [DidCreationDetails] and a given
	// authentication key.
	pub fn new_with_creation_details(
		details: DidCreationDetailsOf<T>,
		new_auth_key: DidVerificationKey<AccountIdOf<T>>,
	) -> Result<Self, DidError> {
		ensure!(
			details.new_key_agreement_keys.len() <=
				<<T as Config>::MaxNewKeyAgreementKeys>::get().saturated_into::<usize>(),
			errors::InputError::MaxKeyAgreementKeysLimitExceeded
		);

		let current_block_number = frame_system::Pallet::<T>::block_number();

		// Creates a new DID with the given authentication key.
		let mut new_did_details = DidDetails::new(new_auth_key, current_block_number)?;

		new_did_details
			.add_key_agreement_keys(details.clone().new_key_agreement_keys, current_block_number)?;

		if let Some(assertion_key) = details.clone().new_assertion_key {
			new_did_details.update_assertion_key(assertion_key, current_block_number)?;
		}

		if let Some(delegation_key) = details.new_delegation_key {
			new_did_details.update_delegation_key(delegation_key, current_block_number)?;
		}

		Ok(new_did_details)
	}

	/// Update the DID authentication key.
	///
	/// The old key is deleted from the set of public keys if it is
	/// not used in any other part of the DID. The new key is added to the
	/// set of public keys.
	pub fn update_authentication_key(
		&mut self,
		new_authentication_key: DidVerificationKey<AccountIdOf<T>>,
		block_number: BlockNumberFor<T>,
	) -> Result<(), StorageError> {
		let old_authentication_key_id = self.authentication_key;
		let new_authentication_key_id =
			utils::calculate_key_id::<T>(&new_authentication_key.clone().into());
		self.authentication_key = new_authentication_key_id;
		// Remove old key ID from public keys, if not used anymore.
		self.remove_key_if_unused(old_authentication_key_id);
		// Add new key ID to public keys. If a key with the same ID is already present,
		// the result is simply that the block number is updated.
		self.public_keys
			.try_insert(
				new_authentication_key_id,
				DidPublicKeyDetails { key: new_authentication_key.into(), block_number },
			)
			.map_err(|_| StorageError::MaxPublicKeysExceeded)?;
		Ok(())
	}

	/// Add new key agreement keys to the DID.
	///
	/// The new keys are added to the set of public keys.
	pub fn add_key_agreement_keys(
		&mut self,
		new_key_agreement_keys: DidNewKeyAgreementKeySet<T::MaxNewKeyAgreementKeys>,
		block_number: BlockNumberFor<T>,
	) -> Result<(), errors::StorageError> {
		for new_key_agreement_key in new_key_agreement_keys {
			self.add_key_agreement_key(new_key_agreement_key, block_number)?;
		}
		Ok(())
	}

	/// Add a single new key agreement key to the DID.
	///
	/// The new key is added to the set of public keys.
	pub fn add_key_agreement_key(
		&mut self,
		new_key_agreement_key: DidEncryptionKey,
		block_number: BlockNumberFor<T>,
	) -> Result<(), StorageError> {
		let new_key_agreement_id = utils::calculate_key_id::<T>(&new_key_agreement_key.into());
		self.public_keys
			.try_insert(
				new_key_agreement_id,
				DidPublicKeyDetails { key: new_key_agreement_key.into(), block_number },
			)
			.map_err(|_| StorageError::MaxPublicKeysExceeded)?;
		self.key_agreement_keys
			.try_insert(new_key_agreement_id)
			.map_err(|_| StorageError::MaxTotalKeyAgreementKeysExceeded)?;
		Ok(())
	}

	/// Remove a key agreement key from both the set of key agreement keys and
	/// the one of public keys.
	pub fn remove_key_agreement_key(&mut self, key_id: KeyIdOf<T>) -> Result<(), StorageError> {
		ensure!(
			self.key_agreement_keys.remove(&key_id),
			StorageError::NotFound(errors::NotFoundKind::Key(errors::KeyType::KeyAgreement))
		);
		self.remove_key_if_unused(key_id);
		Ok(())
	}

	/// Update the DID assertion key, replacing the old one with the new one.
	///
	/// The old key is deleted from the set of public keys if it is
	/// not used in any other part of the DID. The new key is added to the
	/// set of public keys.
	pub fn update_assertion_key(
		&mut self,
		new_assertion_key: DidVerificationKey<AccountIdOf<T>>,
		block_number: BlockNumberFor<T>,
	) -> Result<(), StorageError> {
		let new_assertion_key_id = utils::calculate_key_id::<T>(&new_assertion_key.clone().into());
		if let Some(old_assertion_key_id) = self.assertion_key.take() {
			self.remove_key_if_unused(old_assertion_key_id);
		}
		self.assertion_key = Some(new_assertion_key_id);
		self.public_keys
			.try_insert(
				new_assertion_key_id,
				DidPublicKeyDetails { key: new_assertion_key.into(), block_number },
			)
			.map_err(|_| StorageError::MaxPublicKeysExceeded)?;
		Ok(())
	}

	/// Remove the DID assertion key.
	///
	/// The old key is deleted from the set of public keys if it is
	/// not used in any other part of the DID. The new key is added to the
	/// set of public keys.
	pub fn remove_assertion_key(&mut self) -> Result<(), StorageError> {
		let old_key_id = self.assertion_key.take().ok_or(errors::StorageError::NotFound(
			errors::NotFoundKind::Key(errors::KeyType::AssertionMethod),
		))?;
		self.remove_key_if_unused(old_key_id);
		Ok(())
	}

	/// Update the DID delegation key, replacing the old one with the new one.
	///
	/// The old key is deleted from the set of public keys if it is
	/// not used in any other part of the DID. The new key is added to the
	/// set of public keys.
	pub fn update_delegation_key(
		&mut self,
		new_delegation_key: DidVerificationKey<AccountIdOf<T>>,
		block_number: BlockNumberFor<T>,
	) -> Result<(), StorageError> {
		let new_delegation_key_id =
			utils::calculate_key_id::<T>(&new_delegation_key.clone().into());
		if let Some(old_delegation_key_id) = self.delegation_key.take() {
			self.remove_key_if_unused(old_delegation_key_id);
		}
		self.delegation_key = Some(new_delegation_key_id);
		self.public_keys
			.try_insert(
				new_delegation_key_id,
				DidPublicKeyDetails { key: new_delegation_key.into(), block_number },
			)
			.map_err(|_| StorageError::MaxPublicKeysExceeded)?;
		Ok(())
	}

	/// Remove the DID delegation key.
	///
	/// The old key is deleted from the set of public keys if it is
	/// not used in any other part of the DID. The new key is added to the
	/// set of public keys.
	pub fn remove_delegation_key(&mut self) -> Result<(), StorageError> {
		let old_key_id = self.delegation_key.take().ok_or(errors::StorageError::NotFound(
			errors::NotFoundKind::Key(errors::KeyType::AssertionMethod),
		))?;
		self.remove_key_if_unused(old_key_id);
		Ok(())
	}

	/// Remove a key from the map of public keys if none of the other keys,
	/// i.e., authentication, key agreement, assertion, or delegation, is
	/// referencing it.
	pub fn remove_key_if_unused(&mut self, key_id: KeyIdOf<T>) {
		if self.authentication_key != key_id &&
			self.assertion_key != Some(key_id) &&
			self.delegation_key != Some(key_id) &&
			!self.key_agreement_keys.contains(&key_id)
		{
			self.public_keys.remove(&key_id);
		}
	}

	/// Returns a reference to a specific verification key given the type of
	/// the key needed.
	pub fn get_verification_key_for_key_type(
		&self,
		key_type: DidVerificationKeyRelationship,
	) -> Option<&DidVerificationKey<AccountIdOf<T>>> {
		let key_id = match key_type {
			DidVerificationKeyRelationship::AssertionMethod => self.assertion_key,
			DidVerificationKeyRelationship::Authentication => Some(self.authentication_key),
			DidVerificationKeyRelationship::CapabilityDelegation => self.delegation_key,
			_ => None,
		}?;
		let key_details = self.public_keys.get(&key_id)?;
		if let DidPublicKey::PublicVerificationKey(key) = &key_details.key {
			Some(key)
		} else {
			// The case of something different than a verification key should never happen.
			None
		}
	}

	/// Increase the tx counter of the DID.
	pub fn increase_tx_counter(&mut self) -> u64 {
		// Since we have transaction mortality now, we can safely wrap nonces around.
		self.last_tx_counter = self.last_tx_counter.wrapping_add(1);
		self.last_tx_counter
	}
}

pub(crate) type DidNewKeyAgreementKeySet<MaxNewKeyAgreementKeys> =
	BoundedBTreeSet<DidEncryptionKey, MaxNewKeyAgreementKeys>;

pub(crate) type DidKeyAgreementKeySetOf<T> =
	BoundedBTreeSet<KeyIdOf<T>, <T as Config>::MaxTotalKeyAgreementKeys>;

pub(crate) type DidPublicKeyMapOf<T> = BoundedBTreeMap<
	KeyIdOf<T>,
	DidPublicKeyDetails<BlockNumberFor<T>, AccountIdOf<T>>,
	<T as Config>::MaxPublicKeysPerDid,
>;

/// The details of a new DID to create.
#[derive(Clone, RuntimeDebug, Decode, Encode, PartialEq, TypeInfo)]
pub struct DidCreationDetails<DidIdentifier, AccountId, MaxNewKeyAgreementKeys, DidEndpoint>
where
	MaxNewKeyAgreementKeys: Get<u32> + Clone,
{
	/// The DID identifier. It has to be unique.
	pub did: DidIdentifier,
	/// The authorised submitter of the creation operation.
	pub submitter: AccountId,
	/// The new key agreement keys.
	pub new_key_agreement_keys: DidNewKeyAgreementKeySet<MaxNewKeyAgreementKeys>,
	/// \[OPTIONAL\] The new assertion key.
	pub new_assertion_key: Option<DidVerificationKey<AccountId>>,
	/// \[OPTIONAL\] The new delegation key.
	pub new_delegation_key: Option<DidVerificationKey<AccountId>>,
	/// The service endpoints details.
	pub new_service_details: Vec<DidEndpoint>,
}

/// Errors that might occur while deriving the authorization verification key
/// relationship.
#[derive(Clone, RuntimeDebug, Decode, Encode, Eq, PartialEq)]
pub enum RelationshipDeriveError {
	/// The call is not callable by a did origin.
	NotCallableByDid,

	/// The parameters of the call where invalid.
	InvalidCallParameter,
}

pub type DeriveDidCallKeyRelationshipResult =
	Result<DidVerificationKeyRelationship, RelationshipDeriveError>;

/// Trait for extrinsic DID-based authorization.
///
/// The trait allows
/// [DidAuthorizedCallOperations](DidAuthorizedCallOperation) wrapping an
/// extrinsic to specify what DID key to use to perform signature validation
/// over the byte-encoded operation. A result of None indicates that the
/// extrinsic does not support DID-based authorization.
pub trait DeriveDidCallAuthorizationVerificationKeyRelationship {
	/// The type of the verification key to be used to validate the
	/// wrapped extrinsic.
	fn derive_verification_key_relationship(&self) -> DeriveDidCallKeyRelationshipResult;

	// Return a call to dispatch in order to test the pallet proxy feature.
	#[cfg(feature = "runtime-benchmarks")]
	fn get_call_for_did_call_benchmark() -> Self;
}

/// A DID operation that wraps other extrinsic calls, allowing those
/// extrinsic to have a DID origin and perform DID-based authorization upon
/// their invocation.
#[derive(Clone, RuntimeDebug, Decode, Encode, PartialEq, TypeInfo)]

pub struct DidAuthorizedCallOperation<DidIdentifier, DidCallable, BlockNumber, AccountId, TxCounter>
{
	/// The DID identifier.
	pub did: DidIdentifier,
	/// The DID tx counter.
	pub tx_counter: TxCounter,
	/// The extrinsic call to authorize with the DID.
	pub call: DidCallable,
	/// The block number at which the operation was created.
	pub block_number: BlockNumber,
	/// The account which is authorized to submit the did call.
	pub submitter: AccountId,
}

/// Wrapper around a [DidAuthorizedCallOperation].
///
/// It contains additional information about the type of DID key to used for
/// authorization.
#[derive(Clone, RuntimeDebug, PartialEq, TypeInfo)]

pub struct DidAuthorizedCallOperationWithVerificationRelationship<T: Config> {
	/// The wrapped [DidAuthorizedCallOperation].
	pub operation: DidAuthorizedCallOperationOf<T>,
	/// The type of DID key to use for authorization.
	pub verification_key_relationship: DidVerificationKeyRelationship,
}

impl<T: Config> core::ops::Deref for DidAuthorizedCallOperationWithVerificationRelationship<T> {
	type Target = DidAuthorizedCallOperationOf<T>;

	fn deref(&self) -> &Self::Target {
		&self.operation
	}
}

// Opaque implementation.
// [DidAuthorizedCallOperationWithVerificationRelationship] encodes to
// [DidAuthorizedCallOperation].
impl<T: Config> WrapperTypeEncode for DidAuthorizedCallOperationWithVerificationRelationship<T> {}

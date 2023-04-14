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

use scale_info::TypeInfo;

use crate::did_details::DidVerificationKeyRelationship;

/// All the errors that can be generated when validating a DID operation.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum DidError {
	/// See [Storage].
	Storage(StorageError),
	/// See [Signature].
	Signature(SignatureError),
	/// See [Input].
	Input(InputError),
	/// An error that is not supposed to take place, yet it happened.
	Internal,
}

impl From<StorageError> for DidError {
	fn from(err: StorageError) -> Self {
		DidError::Storage(err)
	}
}

impl From<InputError> for DidError {
	fn from(err: InputError) -> Self {
		DidError::Input(err)
	}
}

/// Error involving the pallet's storage.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum StorageError {
	/// The DID being created is already present on chain.
	AlreadyExists,
	/// The expected DID cannot be found on chain.
	NotFound(NotFoundKind),
	/// The maximum number of public keys for this DID key identifier has
	/// been reached.
	MaxPublicKeysExceeded,
	/// The maximum number of key agreements has been reached for the DID
	/// subject.
	MaxTotalKeyAgreementKeysExceeded,
	/// The DID has already been previously deleted.
	AlreadyDeleted,
}

/// Error involving the pallet's storage.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum NotFoundKind {
	/// The expected DID cannot be found on chain.
	Did,
	/// At least one key referenced is not stored under the given DID.
	Key(KeyType),
}

/// Enum describing the different did key types.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum KeyType {
	/// Authentication key type.
	/// This key is used to authenticate the DID subject.
	Authentication,
	/// Key agreement key type.
	/// This key is used to encrypt messages to the DID subject.
	/// It can be used to derive shared secrets.
	KeyAgreement,
	/// Assertion method key type.
	/// This key is used to assert statements on behalf of the DID subject.
	/// It is generally used to attest things.
	AssertionMethod,
	/// Delegation key type.
	/// This key is used to delegate the DID subject's capabilities.
	Delegation,
}

impl From<DidVerificationKeyRelationship> for KeyType {
	fn from(key_type: DidVerificationKeyRelationship) -> Self {
		match key_type {
			DidVerificationKeyRelationship::Authentication => KeyType::Authentication,
			DidVerificationKeyRelationship::AssertionMethod => KeyType::AssertionMethod,
			DidVerificationKeyRelationship::CapabilityDelegation |
			DidVerificationKeyRelationship::CapabilityInvocation => KeyType::Delegation,
		}
	}
}

/// Error generated when validating a DID operation.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum SignatureError {
	/// The signature is not in the format the verification key expects.
	InvalidFormat,
	/// The signature is invalid for the payload and the verification key
	/// provided.
	InvalidData,
	/// The operation nonce is not equal to the current DID nonce + 1.
	InvalidNonce,
	/// The provided operation block number is not valid.
	TransactionExpired,
}

/// Error generated when some extrinsic input does not respect the pallet's
/// constraints.
#[derive(Debug, Eq, PartialEq, TypeInfo)]
pub enum InputError {
	/// A number of new key agreement keys greater than the maximum allowed has
	/// been provided.
	MaxKeyAgreementKeysLimitExceeded,
	/// The maximum number of service endpoints for a DID has been exceeded.
	MaxServicesCountExceeded,
	/// The maximum number of URLs for a service endpoint has been exceeded.
	MaxUrlCountExceeded,
	/// The maximum number of types for a service endpoint has been exceeded.
	MaxTypeCountExceeded,
	/// The service endpoint ID exceeded the maximum allowed length.
	MaxIdLengthExceeded,
	/// One of the service endpoint URLs exceeded the maximum allowed length.
	MaxUrlLengthExceeded,
	/// One of the service endpoint types exceeded the maximum allowed length.
	MaxTypeLengthExceeded,
	/// One of the service endpoint details contains non-ASCII characters.
	InvalidEncoding,
}

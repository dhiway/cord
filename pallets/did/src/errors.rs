use crate::*;

/// All the errors that can be generated when validating a DID operation.
#[derive(Debug, Eq, PartialEq)]
pub enum DidError {
	/// See [StorageError].
	StorageError(StorageError),
	/// See [SignatureError].
	SignatureError(SignatureError),
	/// See [UrlError].
	UrlError(UrlError),
	/// See [InputError].
	InputError(InputError),
	/// An error that is not supposed to take place, yet it happened.
	InternalError,
}

/// Error involving the pallet's storage.
#[derive(Debug, Eq, PartialEq)]
pub enum StorageError {
	/// The DID being created is already present on chain.
	DidAlreadyPresent,
	/// The expected DID cannot be found on chain.
	DidNotPresent,
	/// The given DID does not contain the right key to verify the signature
	/// of a DID operation.
	DidKeyNotPresent(DidVerificationKeyRelationship),
	/// At least one verification key referenced is not stored in the set
	/// of verification keys.
	VerificationKeyNotPresent,
	/// The user tries to delete a verification key that is currently being
	/// used to authorize operations, and this is not allowed.
	CurrentlyActiveKey,
	/// The maximum supported value for the DID tx counter has been reached.
	/// No more operations with the DID are allowed.
	MaxTxCounterValue,
}

/// Error generated when validating a DID operation.
#[derive(Debug, Eq, PartialEq)]
pub enum SignatureError {
	/// The signature is not in the format the verification key expects.
	InvalidSignatureFormat,
	/// The signature is invalid for the payload and the verification key
	/// provided.
	InvalidSignature,
	/// The operation nonce is not equal to the current DID nonce + 1.
	InvalidNonce,
}

pub enum KeyError {
	/// The verification key provided does not match any supported key.
	InvalidVerificationKeyFormat,
	/// The encryption key provided does not match any supported key.
	InvalidEncryptionKeyFormat,
}

/// Error generated when validating a byte-encoded endpoint URL.
#[derive(Debug, Eq, PartialEq)]
pub enum UrlError {
	/// The URL specified is not ASCII-encoded.
	InvalidUrlEncoding,
	/// The URL specified is not properly formatted.
	InvalidUrlScheme,
}

/// Error generated when some extrinsic input does not respect the pallet's
/// constraints.
#[derive(Debug, Eq, PartialEq)]
pub enum InputError {
	/// A number of new key agreement keys greater than the maximum allowed has
	/// been provided.
	MaxKeyAgreementKeysLimitExceeded,
	/// A number of new verification keys to remove greater than the maximum
	/// allowed has been provided.
	MaxVerificationKeysToRemoveLimitExceeded,
	/// A URL longer than the maximum size allowed has been provided.
	MaxUrlLengthExceeded,
}

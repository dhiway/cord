// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

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

use frame_support::weights::Weight;
use origin_primitives::{AccountId, Signature};
use scale_info::TypeInfo;
use sp_runtime::{traits::Verify, AccountId32};

/// Result type returned by signature verification routines used across CORD pallets.
pub type SignatureVerificationResult = Result<(), SignatureVerificationError>;

/// Errors that may occur when attempting to validate a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TypeInfo)]
pub enum SignatureVerificationError {
	/// The signer information required to validate the signature is missing.
	SignerInformationNotPresent,
	/// The provided signature does not match the payload and signer.
	SignatureInvalid,
}

/// Trait describing a signature verification strategy.
pub trait VerifySignature {
	/// The identifier used to look up signer metadata (e.g. account identifier).
	type SignerId;
	/// The payload that has been signed.
	type Payload;
	/// The signature over the payload.
	type Signature;

	/// Verify that `signature` was produced by `signer` over `payload`.
	fn verify(
		signer: &Self::SignerId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> SignatureVerificationResult;

	/// Weight charged when verifying a signature for a payload with the provided length.
	fn weight(payload_byte_length: usize) -> Weight;
}

/// Verify a Substrate multi-signature for the provided account and payload bytes.
pub fn verify_multisignature<Account>(
	account: &Account,
	payload: &[u8],
	signature: &Signature,
) -> SignatureVerificationResult
where
	Account: Clone + Into<AccountId32>,
{
	let signer: AccountId = account.clone().into();
	if signature.verify(payload, &signer) {
		SignatureVerificationResult::Ok(())
	} else {
		SignatureVerificationResult::Err(SignatureVerificationError::SignatureInvalid)
	}
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
use core::marker::PhantomData;

/// A helper verifier that succeeds when the `(signer, payload)` pair exactly matches the signature.
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub struct EqualVerify<A, P>(PhantomData<(A, P)>);

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl<Account, Payload> VerifySignature for EqualVerify<Account, Payload>
where
	Account: PartialEq,
	Payload: PartialEq,
{
	type SignerId = Account;
	type Payload = Payload;
	type Signature = (Account, Payload);

	fn verify(
		signer: &Self::SignerId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> SignatureVerificationResult {
		if (signer, payload) == (&signature.0, &signature.1) {
			SignatureVerificationResult::Ok(())
		} else {
			SignatureVerificationResult::Err(SignatureVerificationError::SignatureInvalid)
		}
	}

	fn weight(_: usize) -> Weight {
		Weight::zero()
	}
}

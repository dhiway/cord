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

// use frame_support::dispatch::Weight;
use scale_info::TypeInfo;
use sp_weights::Weight;

#[cfg(any(test, feature = "mock", feature = "runtime-benchmarks"))]
use sp_std::marker::PhantomData;

/// The Result of the signature verification.
pub type SignatureVerificationResult = Result<(), SignatureVerificationError>;

/// The Errors that can occur during signature verification.
#[derive(Debug, Clone, Copy, TypeInfo)]
pub enum SignatureVerificationError {
	/// The signers information is not present on chain.
	SignerInformationNotPresent,
	/// The signature is not valid for the given payload.
	SignatureInvalid,
}

/// A signature verification implementation.
pub trait VerifySignature {
	/// The identifier of the signer.
	type SignerId;
	/// The type of the payload that can be verified with the implementation.
	type Payload;
	/// The type of the signature that is expected by the implementation.
	type Signature;

	/// Verifies that the signature matches the payload and has been generated
	/// by the signer.
	fn verify(
		signer: &Self::SignerId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> SignatureVerificationResult;

	/// The weight if the signature verification.
	fn weight(payload_byte_length: usize) -> Weight;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AlwaysVerify<A, P, S>(PhantomData<(A, P, S)>);
#[cfg(feature = "runtime-benchmarks")]
impl<Account, Payload, Signature: Default> VerifySignature
	for AlwaysVerify<Account, Payload, Signature>
{
	type SignerId = Account;

	type Payload = Payload;

	type Signature = Signature;

	fn verify(
		_delegate: &Self::SignerId,
		_payload: &Self::Payload,
		_signature: &Self::Signature,
	) -> SignatureVerificationResult {
		SignatureVerificationResult::Ok(())
	}

	fn weight(_: usize) -> Weight {
		Weight::zero()
	}
}

#[cfg(any(test, feature = "mock", feature = "runtime-benchmarks"))]
pub struct EqualVerify<A, B>(PhantomData<(A, B)>);
#[cfg(any(test, feature = "mock", feature = "runtime-benchmarks"))]
impl<Account, Payload> VerifySignature for EqualVerify<Account, Payload>
where
	Account: PartialEq,
	Payload: PartialEq,
{
	type SignerId = Account;

	type Payload = Payload;

	type Signature = (Account, Payload);

	fn verify(
		delegate: &Self::SignerId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> SignatureVerificationResult {
		if (delegate, payload) == (&signature.0, &signature.1) {
			SignatureVerificationResult::Ok(())
		} else {
			SignatureVerificationResult::Err(SignatureVerificationError::SignatureInvalid)
		}
	}

	fn weight(_: usize) -> Weight {
		Weight::zero()
	}
}

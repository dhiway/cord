// This file is part of CORD – https://cord.network
// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

use cord_utilities::signature::{
	SignatureVerificationError, SignatureVerificationResult, VerifySignature,
};
use frame_support::dispatch;
use sp_runtime::SaturatedConversion;
use sp_std::{marker::PhantomData, vec::Vec};

use crate::{
	did_details::{DidSignature, DidVerificationKeyRelationship},
	errors::DidError,
	Config, Did, Pallet, WeightInfo,
};

pub struct DidSignatureVerify<T>(PhantomData<T>);
impl<T: Config> VerifySignature for DidSignatureVerify<T> {
	type SignerId = <T as Config>::DidIdentifier;
	type Payload = Vec<u8>;
	type Signature = DidSignature;
	type KeyType = DidVerificationKeyRelationship;

	fn verify(
		delegate: &Self::SignerId,
		payload: &Self::Payload,
		signature: &Self::Signature,
		key_type: &Self::KeyType,
	) -> SignatureVerificationResult {
		let delegate_details = Did::<T>::get(delegate)
			.ok_or(SignatureVerificationError::SignerInformationNotPresent)?;
		match key_type {
			DidVerificationKeyRelationship::Authentication => {
				Pallet::verify_signature_with_did_key_type(
					payload,
					signature,
					&delegate_details,
					DidVerificationKeyRelationship::Authentication,
				)
				.map_err(|err| match err {
					DidError::SignatureError(_) => SignatureVerificationError::SignatureInvalid,
					_ => SignatureVerificationError::SignerInformationNotPresent,
				})
			},
			DidVerificationKeyRelationship::AssertionMethod => {
				Pallet::verify_signature_with_did_key_type(
					payload,
					signature,
					&delegate_details,
					DidVerificationKeyRelationship::AssertionMethod,
				)
				.map_err(|err| match err {
					DidError::SignatureError(_) => SignatureVerificationError::SignatureInvalid,
					_ => SignatureVerificationError::SignerInformationNotPresent,
				})
			},
			DidVerificationKeyRelationship::CapabilityDelegation => {
				Pallet::verify_signature_with_did_key_type(
					payload,
					signature,
					&delegate_details,
					DidVerificationKeyRelationship::CapabilityDelegation,
				)
				.map_err(|err| match err {
					DidError::SignatureError(_) => SignatureVerificationError::SignatureInvalid,
					_ => SignatureVerificationError::SignerInformationNotPresent,
				})
			},
			_ => Err(SignatureVerificationError::InvalidSignerKey),
		}
	}

	fn weight(payload_byte_length: usize) -> dispatch::Weight {
		<T as Config>::WeightInfo::signature_verification_sr25519(
			payload_byte_length.saturated_into(),
		)
		.max(<T as Config>::WeightInfo::signature_verification_ed25519(
			payload_byte_length.saturated_into(),
		))
		.max(<T as Config>::WeightInfo::signature_verification_ecdsa(
			payload_byte_length.saturated_into(),
		))
	}
}

// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Transaction extension for members.
//!
//! Provides a single variant:
//! - `SelfInclude(signature)`: Validates that the member has waited long enough in the onboarding
//!   queue and authenticates the call by verifying the member's signature, ensuring this action is
//!   an explicit user action.

use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::fmt;
use frame_support::{
	ensure, pallet_prelude::TransactionSource, traits::IsSubType, weights::Weight, CloneNoBound,
	DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Implication, TransactionExtension, ValidateResult},
	transaction_validity::{InvalidTransaction, TransactionValidityError, ValidTransaction},
};
use verifiable::GenerateVerifiable;

/// Custom invalidity reasons for the `AsMember` transaction extension.
#[repr(u8)]
pub enum CustomValidity {
	/// The call is not a `self_include` call.
	NotSelfIncludeCall = 200,
	/// The collection does not exist.
	CollectionNotFound = 201,
	/// Self-inclusion is not enabled for this collection.
	SelfInclusionNotEnabled = 202,
	/// The rings are currently mutating and not in append-only mode.
	RingsMutating = 203,
	/// The member was not found in the collection.
	MemberNotFound = 204,
	/// The member is not in the onboarding queue.
	NotOnboarding = 205,
	/// The member has not waited long enough in the onboarding queue.
	SelfInclusionTooEarly = 206,
}

impl From<CustomValidity> for TransactionValidityError {
	fn from(e: CustomValidity) -> Self {
		InvalidTransaction::Custom(e as u8).into()
	}
}

/// Initialization information about the extension.
#[derive(
	Encode, Decode, TypeInfo, EqNoBound, CloneNoBound, PartialEqNoBound, DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub enum AsMemberInfo<T: Config + Send + Sync> {
	/// Self-include a member from the onboarding queue. The signature is over the
	/// transaction's inherited implication, proving ownership of the member key.
	SelfInclude(SignatureOf<T>),
}

/// Transaction extension for self-inclusion of queued members.
#[derive(
	Encode,
	Decode,
	TypeInfo,
	EqNoBound,
	CloneNoBound,
	PartialEqNoBound,
	DefaultNoBound,
	DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub struct AsMember<T: Config + Send + Sync>(pub Option<AsMemberInfo<T>>);

impl<T: Config + Send + Sync> fmt::Debug for AsMember<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "AsMember")
	}
}

impl<T: Config + Send + Sync> AsMember<T> {
	pub fn new(info: Option<AsMemberInfo<T>>) -> Self {
		Self(info)
	}
}

impl<T: Config + Send + Sync> TransactionExtension<<T as frame_system::Config>::RuntimeCall>
	for AsMember<T>
{
	const IDENTIFIER: &'static str = "AsMember";
	type Implicit = ();
	type Val = ();
	type Pre = ();

	fn weight(&self, _call: &<T as frame_system::Config>::RuntimeCall) -> Weight {
		match &self.0 {
			None => Weight::zero(),
			Some(AsMemberInfo::SelfInclude(_)) => T::WeightInfo::validate_self_include(),
		}
	}

	fn validate(
		&self,
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		inherited_implication: &impl Implication,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, <T as frame_system::Config>::RuntimeCall> {
		let Some(AsMemberInfo::SelfInclude(signature)) = &self.0 else {
			return Ok((ValidTransaction::default(), (), origin));
		};

		// The origin must not have already been authorized by another extension.
		ensure!(
			matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
			InvalidTransaction::BadSigner
		);

		// Extract call parameters.
		let Some(Call::<T>::self_include { identifier, member, call_valid_at }) =
			call.is_sub_type()
		else {
			return Err(CustomValidity::NotSelfIncludeCall.into());
		};

		// Validate call_valid_at is within the time tolerance window.
		let now = T::Clock::now().as_secs();
		let window = Pallet::<T>::self_inclusion_time_tolerance();
		if now < *call_valid_at {
			return Err(InvalidTransaction::Future.into());
		}
		if now > call_valid_at.saturating_add(window) {
			return Err(InvalidTransaction::Stale.into());
		}

		// Verify collection exists and has self-inclusion enabled.
		let collection_info =
			Collections::<T>::get(identifier).ok_or(CustomValidity::CollectionNotFound)?;
		let delay = collection_info
			.self_inclusion_delay
			.ok_or(CustomValidity::SelfInclusionNotEnabled)?;

		// Reject early if rings are mutating.
		ensure!(RingsState::<T>::get(identifier).append_only(), CustomValidity::RingsMutating);

		// Verify the member is in the onboarding queue and has waited long enough.
		let position =
			Members::<T>::get(identifier, member).ok_or(CustomValidity::MemberNotFound)?;
		let queued_at = match position {
			RingPosition::Onboarding { queued_at, .. } => queued_at,
			_ => return Err(CustomValidity::NotOnboarding.into()),
		};
		if now < queued_at.saturating_add(delay) {
			return Err(CustomValidity::SelfInclusionTooEarly.into());
		}

		// Verify the signature over the inherited implication.
		let msg = inherited_implication.using_encoded(sp_io::hashing::blake2_256);
		if !T::Crypto::verify_signature(signature, &msg[..], member) {
			return Err(InvalidTransaction::BadProof.into());
		}

		let provides =
			sp_io::hashing::twox_64(&("self-include", &identifier, &member).encode()[..]);
		let valid_transaction =
			ValidTransaction::with_tag_prefix("Members").and_provides(provides).build()?;

		// Set the origin to SelfInclude with the authenticated member key.
		let local_origin = pallet::Origin::<T>::SelfInclude(member.clone());
		let mut origin = origin;
		origin.set_caller_from(local_origin);

		Ok((valid_transaction, (), origin))
	}

	fn prepare(
		self,
		_val: Self::Val,
		_origin: &<T as frame_system::Config>::RuntimeOrigin,
		_call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		Ok(())
	}
}

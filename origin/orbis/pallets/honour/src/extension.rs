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

//! Transaction extension for `indiv-pallet-honour`.

use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::fmt;
use frame_support::{
	ensure,
	pallet_prelude::Weight,
	traits::{IsSubType, OriginTrait, UnixTime},
	CloneNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use indiv_support::traits::{RevisionIndex, RingIndex};
use scale_info::TypeInfo;
use sp_core::Get;
use sp_runtime::{
	traits::{DispatchInfoOf, TransactionExtension, ValidateResult},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
	},
};

const VALIDATION_TAG: &str = "pallet-honour:bestow";

type RuntimeCallOf<T> = <T as frame_system::Config>::RuntimeCall;

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo, DecodeWithMemTracking, Debug)]
#[scale_info(skip_type_params(T))]
pub struct VoterAuthData<T: Config> {
	/// Signed account that co-authorizes this anonymous ring vote.
	pub account: T::AccountId,
	pub proof: RingProofOf<T>,
	/// Ring to validate the `proof` against.
	pub ring_index: RingIndex,
	/// The ring revision the `proof` was constructed against.
	pub revision: RevisionIndex,
}

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
pub struct VoterAuth<T: Config>(Option<VoterAuthData<T>>);

impl<T: Config> VoterAuth<T> {
	pub fn new(data: Option<VoterAuthData<T>>) -> Self {
		Self(data)
	}
}

impl<T: Config> fmt::Debug for VoterAuth<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "VoterAuth")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> TransactionExtension<RuntimeCallOf<T>> for VoterAuth<T> {
	const IDENTIFIER: &'static str = "HonourAuth";
	type Implicit = ();

	type Val = ();
	type Pre = ();

	fn weight(&self, _call: &RuntimeCallOf<T>) -> Weight {
		match self.0 {
			Some(_) => T::WeightInfo::extension_validate(),
			None => Weight::zero(),
		}
	}

	fn validate(
		&self,
		origin: T::RuntimeOrigin,
		call: &RuntimeCallOf<T>,
		_info: &DispatchInfoOf<RuntimeCallOf<T>>,
		_len: usize,
		_self_implicit: Self::Implicit,
		inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, RuntimeCallOf<T>> {
		let Some(VoterAuthData { account, proof, ring_index, revision }) = &self.0 else {
			return Ok((ValidTransaction::default(), (), origin));
		};

		// Normal and Meta transactions must both be signed. VerifySignature establishes the inner
		// signed origin for Meta before this extension; an arbitrary or mismatched signer cannot
		// borrow an anonymous proof.
		let Some(frame_system::RawOrigin::Signed(who)) = origin.as_system_ref() else {
			return Err(InvalidTransaction::BadSigner.into());
		};
		ensure!(who == account, InvalidTransaction::BadSigner);

		let Some(Call::<T>::bestow { vote, call_valid_from }) = call.is_sub_type() else {
			return Err(InvalidTransaction::Call.into());
		};

		let now = T::Clock::now().as_secs();

		if *call_valid_from > now {
			return Err(InvalidTransaction::Future.into());
		}
		let call_valid_to = call_valid_from.saturating_add(T::CallMortality::get());
		if call_valid_to <= now {
			return Err(InvalidTransaction::Stale.into());
		}

		let message = (inherited_implication, account).using_encoded(sp_io::hashing::blake2_256);

		let aliases =
			Pallet::<T>::validate_vote_proof(vote, &message, proof, *ring_index, *revision)
				.map_err(|_| InvalidTransaction::BadProof)?;

		if Pallet::<T>::is_point_frozen(&aliases.point_alias, now) {
			return Err(InvalidTransaction::Future.into());
		}

		if Votes::<T>::contains_key(aliases.subject_alias) {
			return Err(InvalidTransaction::Stale.into());
		}

		let validity = ValidTransaction::with_tag_prefix(VALIDATION_TAG)
			.and_provides(aliases.point_alias)
			.into();

		let local_origin = Origin::Voter { aliases };
		let mut origin = origin;
		origin.set_caller_from(local_origin);

		Ok((validity, (), origin))
	}

	fn prepare(
		self,
		_val: Self::Val,
		_origin: &T::RuntimeOrigin,
		_call: &RuntimeCallOf<T>,
		_info: &DispatchInfoOf<RuntimeCallOf<T>>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		Ok(())
	}
}

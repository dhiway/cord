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

//! Transaction extensions for indiv-pallet-people-lite.
//!
//! Warning: The transaction extension `PeopleLiteAuth` does not provide spam protection for the
//! origins `LitePerson` and `LiteAlias`. This means a user can spam valid but potentially failing
//! calls without restriction. Another transaction extension must handle spam protection. Using
//! `pallet-origin-restriction` is advised.

use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{
	ensure, pallet_prelude::Weight, traits::OriginTrait, CloneNoBound, DebugNoBound, EqNoBound,
	PartialEqNoBound,
};
use frame_system::{CheckNonce, ValidNonceInfo};
use indiv_support::traits::{Context, MembershipProver, RevisedContextualAlias, RingIndex};
use scale_info::TypeInfo;
use sp_io::hashing::{blake2_256, twox_64};
use sp_runtime::{
	traits::{DispatchInfoOf, TransactionExtension, ValidateResult},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
	},
	Saturating,
};

/// Custom invalidity for invalid transactions in the `PeopleLiteAuth` transaction extension.
#[repr(u8)]
pub enum CustomError {
	/// The signed origin is not a registered lite person.
	NotLitePerson = 171,
	/// The stored alias binding is stale and must be revised with a fresh proof.
	StaleAlias = 172,
	/// The supplied proof did not validate against the lite collection.
	InvalidProof = 173,
	/// The supplied proof did not resolve to the stored contextual alias.
	AliasMismatch = 174,
	/// The signed account has no alias binding.
	NoAliasBinding = 175,
}

impl From<CustomError> for TransactionValidityError {
	fn from(e: CustomError) -> Self {
		InvalidTransaction::Custom(e as u8).into()
	}
}

/// A type alias to access system runtime call.
type RuntimeCallOf<T> = <T as frame_system::Config>::RuntimeCall;

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo, DecodeWithMemTracking, Debug)]
pub enum PeopleLiteAuthData<Nonce, Proof> {
	/// Authenticate using the canonical lite account.
	AsLitePerson(Nonce),
	/// Authenticate using an already-established alias account.
	AsLiteAliasWithAccount(Nonce),
	/// Establish an alias <-> account mapping using a proof and no signed identity.
	AsLiteAliasWithProof(Proof, RingIndex, Context),
	/// Refresh a stale alias binding with a fresh proof while authenticating with the old account.
	AsLiteAliasWithAccountRevised(Nonce, Proof, RingIndex, Context),
}
#[allow(type_alias_bounds)]
pub type PeopleLiteAuthDataOf<T: Config> = PeopleLiteAuthData<T::Nonce, ProofOf<T>>;

#[derive(
	CloneNoBound,
	EqNoBound,
	PartialEqNoBound,
	Encode,
	Decode,
	TypeInfo,
	DecodeWithMemTracking,
	DebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct PeopleLiteAuth<T: Config>(Option<PeopleLiteAuthDataOf<T>>);

impl<T: Config> PeopleLiteAuth<T> {
	/// Creates a new `PeopleLiteAuth` transaction extension.
	pub fn new(data: Option<PeopleLiteAuthDataOf<T>>) -> Self {
		Self(data)
	}
}

/// The value passed from validate to prepare in the [`PeopleLiteAuth`] transaction extension.
pub enum PeopleLiteAuthVal<AccountId, Nonce> {
	AsLitePerson(AccountId, Nonce),
	AsLiteAliasWithAccount(AccountId, Nonce),
	AsLiteAliasWithProof,
	AsLiteAliasWithAccountRevised(AccountId, Nonce, RevisedContextualAlias),
	None,
}

impl<T: Config> TransactionExtension<RuntimeCallOf<T>> for PeopleLiteAuth<T> {
	const IDENTIFIER: &'static str = "PeopleLiteAuth";
	type Implicit = ();

	type Val = PeopleLiteAuthVal<T::AccountId, T::Nonce>;
	type Pre = ();

	fn weight(&self, _call: &RuntimeCallOf<T>) -> Weight {
		match self.0 {
			Some(PeopleLiteAuthData::AsLitePerson(..)) =>
				<T as Config>::WeightInfo::as_lite_person_tx_ext(),
			Some(PeopleLiteAuthData::AsLiteAliasWithAccount(..)) =>
				<T as Config>::WeightInfo::as_lite_alias_with_account_tx_ext(),
			Some(PeopleLiteAuthData::AsLiteAliasWithProof(..)) =>
				<T as Config>::WeightInfo::as_lite_alias_with_proof_tx_ext(),
			Some(PeopleLiteAuthData::AsLiteAliasWithAccountRevised(..)) =>
				<T as Config>::WeightInfo::as_lite_alias_with_account_revised_tx_ext(),
			None => Weight::zero(),
		}
	}

	fn validate(
		&self,
		mut origin: T::RuntimeOrigin,
		call: &RuntimeCallOf<T>,
		_info: &DispatchInfoOf<RuntimeCallOf<T>>,
		_len: usize,
		_self_implicit: Self::Implicit,
		inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, RuntimeCallOf<T>> {
		match &self.0 {
			Some(PeopleLiteAuthData::AsLitePerson(nonce)) => {
				// Origin must be a signed origin.
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref().cloned()
				else {
					return Err(InvalidTransaction::BadSigner.into());
				};

				ensure!(LitePeople::<T>::contains_key(&who), CustomError::NotLitePerson);

				// Validate the nonce.
				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				origin.set_caller_from(Origin::LitePerson(who.clone()));
				Ok((validity, PeopleLiteAuthVal::AsLitePerson(who, *nonce), origin))
			},
			Some(PeopleLiteAuthData::AsLiteAliasWithAccount(nonce)) => {
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref().cloned()
				else {
					return Err(InvalidTransaction::BadSigner.into());
				};

				let rev_ca = AccountToAlias::<T>::get(&who).ok_or(CustomError::NoAliasBinding)?;
				let ring_revision =
					T::MemberService::ring_revision(LITE_PEOPLE_MEMBER_IDENTIFIER, rev_ca.ring)
						.ok_or(CustomError::StaleAlias)?;
				if ring_revision != rev_ca.revision {
					return Err(CustomError::StaleAlias.into());
				}

				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				origin.set_caller_from(Origin::LiteAlias(rev_ca));
				Ok((validity, PeopleLiteAuthVal::AsLiteAliasWithAccount(who, *nonce), origin))
			},
			Some(PeopleLiteAuthData::AsLiteAliasWithProof(proof, ring_index, context)) => {
				ensure!(
					matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
					InvalidTransaction::BadSigner
				);

				let Some(Call::<T>::set_alias_account { account, valid_at_block }) =
					call.is_sub_type()
				else {
					return Err(InvalidTransaction::Call.into());
				};

				ensure!(context == LITE_PEOPLE_AUTH_CONTEXT, InvalidTransaction::Call);

				let current_block = frame_system::Pallet::<T>::block_number();
				if current_block < *valid_at_block {
					return Err(InvalidTransaction::Future.into());
				}
				let block_tolerance = Pallet::<T>::account_setup_block_tolerance();
				if current_block > valid_at_block.saturating_add(block_tolerance) {
					return Err(InvalidTransaction::Stale.into());
				}

				let msg = inherited_implication.using_encoded(blake2_256);
				let validated_rev_ca = T::MemberService::verify_membership(
					LITE_PEOPLE_MEMBER_IDENTIFIER,
					proof,
					*ring_index,
					*context,
					&msg[..],
				)
				.map_err(|_| InvalidTransaction::BadProof)?;

				if AccountToAlias::<T>::get(account)
					.is_some_and(|stored_rev_ca| stored_rev_ca == validated_rev_ca)
				{
					return Err(InvalidTransaction::Stale.into());
				}

				let provides = twox_64(&("setup", &validated_rev_ca, &account).encode()[..]);
				let valid_transaction =
					ValidTransaction::with_tag_prefix("PLite:Alias").and_provides(provides).into();

				origin.set_caller_from(Origin::LiteAlias(validated_rev_ca));
				Ok((valid_transaction, PeopleLiteAuthVal::AsLiteAliasWithProof, origin))
			},
			Some(PeopleLiteAuthData::AsLiteAliasWithAccountRevised(
				nonce,
				proof,
				ring_index,
				context,
			)) => {
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref().cloned()
				else {
					return Err(InvalidTransaction::BadSigner.into());
				};

				let old_rev_ca =
					AccountToAlias::<T>::get(&who).ok_or(CustomError::NoAliasBinding)?;
				ensure!(context == LITE_PEOPLE_AUTH_CONTEXT, InvalidTransaction::Call);

				let msg = (inherited_implication, "revise", &who, nonce).using_encoded(blake2_256);
				let validated_rev_ca = T::MemberService::verify_membership(
					LITE_PEOPLE_MEMBER_IDENTIFIER,
					proof,
					*ring_index,
					*context,
					&msg[..],
				)
				.map_err(|_| CustomError::InvalidProof)?;
				if validated_rev_ca.ca.alias != old_rev_ca.ca.alias ||
					validated_rev_ca.ca.context != old_rev_ca.ca.context
				{
					return Err(CustomError::AliasMismatch.into());
				}

				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				origin.set_caller_from(Origin::LiteAlias(validated_rev_ca.clone()));
				Ok((
					validity,
					PeopleLiteAuthVal::AsLiteAliasWithAccountRevised(who, *nonce, validated_rev_ca),
					origin,
				))
			},
			None => Ok((ValidTransaction::default(), PeopleLiteAuthVal::None, origin)),
		}
	}

	fn prepare(
		self,
		val: Self::Val,
		_origin: &T::RuntimeOrigin,
		_call: &RuntimeCallOf<T>,
		_info: &DispatchInfoOf<RuntimeCallOf<T>>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		match val {
			PeopleLiteAuthVal::AsLitePerson(account, nonce) |
			PeopleLiteAuthVal::AsLiteAliasWithAccount(account, nonce) => {
				CheckNonce::<T>::prepare_nonce_for_account(&account, nonce)?;
				Ok(())
			},
			PeopleLiteAuthVal::AsLiteAliasWithProof => Ok(()),
			PeopleLiteAuthVal::AsLiteAliasWithAccountRevised(account, nonce, rev_ca) => {
				AccountToAlias::<T>::insert(&account, &rev_ca);
				CheckNonce::<T>::prepare_nonce_for_account(&account, nonce)?;
				Ok(())
			},
			PeopleLiteAuthVal::None => Ok(()),
		}
	}
}

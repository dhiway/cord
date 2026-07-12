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

//! People transaction extensions.
//!
//! Warning: The transaction extension [`AsPersonInfo`] does not provide spam protection for the
//! origins `PersonalIdentity` and `PersonalAlias` in contexts within `AccountContexts`. This means
//! a user can spam valid but potentially failing calls without restriction. Another transaction
//! extension must handle spam protection. Using `pallet-origin-restriction` is advised.

use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::fmt;
use frame_support::{
	ensure, pallet_prelude::TransactionSource, traits::Contains, weights::Weight, CloneNoBound,
	DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::{CheckNonce, ValidNonceInfo};
use indiv_support::traits::{Context, MembershipProver, RevisedContextualAlias};
use scale_info::TypeInfo;
use sp_io::hashing::twox_64;
use sp_runtime::{
	traits::{DispatchInfoOf, TransactionExtension, ValidateResult},
	transaction_validity::{InvalidTransaction, TransactionValidityError, ValidTransaction},
	Saturating,
};

/// Information required to transform an origin into a personal alias or personal identity.
#[derive(
	Encode, Decode, TypeInfo, EqNoBound, CloneNoBound, PartialEqNoBound, DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub enum AsPersonInfo<T: Config + Send + Sync> {
	/// The signed origin will be transformed using account to alias.
	AsPersonalAliasWithAccount(T::Nonce),
	/// The none origin will be transformed using proof.
	///
	/// This can only dispatch the call `set_alias_account`.
	/// The supplied context must be one of `T::AccountContexts`, otherwise validation fails with
	/// `InvalidTransaction::Call`.
	///
	/// Replay is only protected against resetting the same account during the tolerance period
	/// after `call_valid_at` parameter.
	/// If 2 transaction that set 2 different account are sent for an overlapping validity period,
	/// then those 2 transactions can be replayed indefinitely for the duration of the overlapping
	/// period.
	AsPersonalAliasWithProof(ProofOf<T>, RingIndex, Context),
	/// The none origin will be transformed using signature.
	///
	/// This can only dispatch the call `set_personal_id_account`.
	///
	/// Replay is only protected against resetting the same account during the tolerance period
	/// after `call_valid_at` parameter.
	/// If 2 transaction that set 2 different account are sent for an overlapping validity period,
	/// then those 2 transactions can be replayed indefinitely for the duration of the overlapping
	/// period.
	AsPersonalIdentityWithProof(SignatureOf<T>, PersonalId),
	/// The signed origin will be transformed using account to personal id.
	AsPersonalIdentityWithAccount(T::Nonce),
	/// The signed origin will be transformed using account to alias and the revision associated
	/// with the alias will be updated to match the current ring of the person.
	///
	/// There must already be an alias <-> account pairing, although it does not have to
	/// necessarily be out of date. If no alias <-> account pairing is present, the
	/// `AsPersonalAliasWithProof` variant should be used along with a `set_alias_account` call.
	///
	/// The nonce must be the nonce of the account of the signer.
	/// The proof must be created using the same key and context of the alias which is associated
	/// with the account, but must take into account any changes related to the ring and its
	/// revision. The proof message is created by hashing with blake2 256 the encoded tuple of:
	/// * the inherited_implication,
	/// * the string "revise" (so length in compact encoding is prepend),
	/// * the alias account
	/// * and the account nonce.
	///
	/// The ring index must be the index of the ring to which the person is assigned at the time of
	/// dispatch.
	/// The context must be the same context that was used when establishing the alias <-> account
	/// pairing.
	AsPersonalAliasWithAccountRevised(T::Nonce, ProofOf<T>, RingIndex, Context),
}

/// Transaction extension to transform an origin into a personal alias or personal identity.
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
pub struct AsPerson<T: Config + Send + Sync>(Option<AsPersonInfo<T>>);

impl<T: Config + Send + Sync> fmt::Debug for AsPerson<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "AsPerson")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> AsPerson<T> {
	pub fn new(explicit: Option<AsPersonInfo<T>>) -> Self {
		Self(explicit)
	}
}

/// Info returned by validate to prepare in the [`AsPerson`] transaction extension.
pub enum Val<T: Config + Send + Sync> {
	NotUsing,
	UsingProof,
	UsingAccount(T::AccountId, T::Nonce),
	UsingAccountWithRevisionUpdate(T::AccountId, T::Nonce, RevisedContextualAlias),
}

impl<T: Config + Send + Sync> TransactionExtension<<T as frame_system::Config>::RuntimeCall>
	for AsPerson<T>
{
	const IDENTIFIER: &'static str = "AsPerson";
	type Implicit = ();

	type Val = Val<T>;
	type Pre = ();

	fn weight(&self, _call: &<T as frame_system::Config>::RuntimeCall) -> Weight {
		match self.0 {
			// Extension is passthrough
			None => Weight::zero(),
			// Alias with existing account
			Some(AsPersonInfo::AsPersonalAliasWithAccount(_)) =>
				T::WeightInfo::as_person_alias_with_account(),
			// Alias with proof
			Some(AsPersonInfo::AsPersonalAliasWithProof(_, _, _)) =>
				T::WeightInfo::as_person_alias_with_proof(),
			// Personal Identity with proof
			Some(AsPersonInfo::AsPersonalIdentityWithProof(_, _)) =>
				T::WeightInfo::as_person_identity_with_proof(),
			// Personal Identity with existing account
			Some(AsPersonInfo::AsPersonalIdentityWithAccount(_)) =>
				T::WeightInfo::as_person_identity_with_account(),
			// Alias with account revised at the same time
			Some(AsPersonInfo::AsPersonalAliasWithAccountRevised(..)) =>
				T::WeightInfo::as_person_alias_with_account_revised(),
		}
	}

	fn validate(
		&self,
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, <T as frame_system::Config>::RuntimeCall> {
		match &self.0 {
			Some(AsPersonInfo::AsPersonalAliasWithAccount(nonce)) => {
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref() else {
					return Err(InvalidTransaction::BadSigner.into());
				};
				let who = who.clone();

				let rev_ca = AccountToAlias::<T>::get(&who).ok_or(InvalidTransaction::BadSigner)?;
				ensure!(
					T::AccountContexts::contains(&rev_ca.ca.context),
					InvalidTransaction::BadSigner,
				);
				let ring_revision =
					T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, rev_ca.ring)
						.ok_or(InvalidTransaction::BadSigner)?;
				ensure!(ring_revision == rev_ca.revision, InvalidTransaction::BadSigner);

				let local_origin = Origin::PersonalAlias(rev_ca);
				let mut origin = origin;
				origin.set_caller_from(local_origin);

				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				Ok((validity, Val::UsingAccount(who, *nonce), origin))
			},
			Some(AsPersonInfo::AsPersonalAliasWithAccountRevised(
				nonce,
				proof,
				ring_index,
				context,
			)) => {
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref() else {
					return Err(InvalidTransaction::BadSigner.into());
				};
				let who = who.clone();

				// Get the old entry in `AccountToAlias`, it will be removed in `prepare` if the
				// checks are successful.
				let old_rev_ca =
					AccountToAlias::<T>::get(&who).ok_or(InvalidTransaction::BadSigner)?;

				let msg = (inherited_implication, "revise", &who, nonce)
					.using_encoded(sp_io::hashing::blake2_256);

				let validated_rev_ca = T::MemberService::verify_membership(
					PEOPLE_MEMBER_IDENTIFIER,
					proof,
					*ring_index,
					*context,
					&msg[..],
				)
				.map_err(|_| InvalidTransaction::BadProof)?;
				ensure!(
					validated_rev_ca.ca.alias == old_rev_ca.ca.alias,
					InvalidTransaction::BadSigner
				);
				ensure!(*context == old_rev_ca.ca.context, InvalidTransaction::BadSigner);
				ensure!(T::AccountContexts::contains(context), InvalidTransaction::BadSigner);

				let local_origin = Origin::PersonalAlias(validated_rev_ca.clone());
				let mut origin = origin;
				origin.set_caller_from(local_origin);

				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				Ok((
					validity,
					Val::UsingAccountWithRevisionUpdate(who, *nonce, validated_rev_ca),
					origin,
				))
			},
			Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce)) => {
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref() else {
					return Err(InvalidTransaction::BadSigner.into());
				};
				let who = who.clone();

				let id =
					AccountToPersonalId::<T>::get(&who).ok_or(InvalidTransaction::BadSigner)?;
				let local_origin = Origin::PersonalIdentity(id);
				let mut origin = origin;
				origin.set_caller_from(local_origin);

				let ValidNonceInfo { requires, provides } =
					CheckNonce::<T>::validate_nonce_for_account(&who, *nonce)?;
				let validity = ValidTransaction { requires, provides, ..Default::default() };

				Ok((validity, Val::UsingAccount(who, *nonce), origin))
			},
			Some(AsPersonInfo::AsPersonalAliasWithProof(proof, ring_index, context)) => {
				ensure!(
					matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
					InvalidTransaction::BadSigner
				);

				let Some(Call::<T>::set_alias_account { account, call_valid_at }) =
					call.is_sub_type()
				else {
					return Err(InvalidTransaction::Call.into());
				};

				ensure!(T::AccountContexts::contains(context), InvalidTransaction::Call);

				let now = frame_system::Pallet::<T>::block_number();
				if now < *call_valid_at {
					return Err(InvalidTransaction::Future.into());
				}
				let time_tolerance = Pallet::<T>::account_setup_time_tolerance();
				if now > call_valid_at.saturating_add(time_tolerance) {
					return Err(InvalidTransaction::Stale.into());
				}

				let msg = inherited_implication.using_encoded(sp_io::hashing::blake2_256);

				let validated_rev_ca = T::MemberService::verify_membership(
					PEOPLE_MEMBER_IDENTIFIER,
					proof,
					*ring_index,
					*context,
					&msg[..],
				)
				.map_err(|_| InvalidTransaction::BadProof)?;

				// This protects again replay attack.
				if AccountToAlias::<T>::get(account)
					.is_some_and(|stored_rev_ca| stored_rev_ca == validated_rev_ca)
				{
					return Err(InvalidTransaction::Stale.into());
				}

				// The extrinsic provides the setup of the account for the alias.
				let provides = twox_64(&("setup", &validated_rev_ca, &account).encode()[..]);
				let valid_transaction =
					ValidTransaction::with_tag_prefix("Ppl:Alias").and_provides(provides).into();

				// We transmute the origin.
				let local_origin = Origin::PersonalAlias(validated_rev_ca);
				let mut origin = origin;
				origin.set_caller_from(local_origin);

				Ok((valid_transaction, Val::UsingProof, origin))
			},
			Some(AsPersonInfo::AsPersonalIdentityWithProof(signature, index)) => {
				ensure!(
					matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
					InvalidTransaction::BadSigner
				);

				let Some(Call::<T>::set_personal_id_account { account, call_valid_at }) =
					call.is_sub_type()
				else {
					return Err(InvalidTransaction::Call.into());
				};

				let now = frame_system::Pallet::<T>::block_number();
				if now < *call_valid_at {
					return Err(InvalidTransaction::Future.into());
				}
				let time_tolerance = Pallet::<T>::account_setup_time_tolerance();
				if now > call_valid_at.saturating_add(time_tolerance) {
					return Err(InvalidTransaction::Stale.into());
				}

				let key = People::<T>::get(index)
					.map(|record| record.key)
					.ok_or(InvalidTransaction::BadSigner)?;

				let msg = inherited_implication.using_encoded(sp_io::hashing::blake2_256);

				if !CryptoOf::<T>::verify_signature(signature, &msg[..], &key) {
					return Err(InvalidTransaction::BadProof.into());
				}

				// This protects again replay attack.
				if People::<T>::get(index).is_some_and(|record| {
					record.account.is_some_and(|stored_account| stored_account == *account)
				}) {
					return Err(InvalidTransaction::Stale.into());
				}

				// The extrinsic provides the setup of the account for the personal id.
				let provides = twox_64(&("setup", index, &account).encode()[..]);
				let valid_transaction =
					ValidTransaction::with_tag_prefix("Ppl:Id").and_provides(provides).into();

				// We transmute the origin.
				let local_origin = Origin::PersonalIdentity(*index);
				let mut origin = origin;
				origin.set_caller_from(local_origin);

				Ok((valid_transaction, Val::UsingProof, origin))
			},
			None => Ok((ValidTransaction::default(), Val::NotUsing, origin)),
		}
	}

	fn prepare(
		self,
		val: Self::Val,
		_origin: &<T as frame_system::Config>::RuntimeOrigin,
		_call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		match val {
			Val::UsingAccount(who, nonce) =>
				CheckNonce::<T>::prepare_nonce_for_account(&who, nonce)?,
			Val::UsingAccountWithRevisionUpdate(who, nonce, rev_ca) => {
				// `AliasToAccount` doesn't need any changes because the key is an unrevised
				// contextual alias, which remains constant in this operation, as checked above.
				// Replace the old entry in `AccountToAlias` with the new, updated revised alias.
				AccountToAlias::<T>::insert(&who, &rev_ca);
				CheckNonce::<T>::prepare_nonce_for_account(&who, nonce)?
			},
			Val::NotUsing | Val::UsingProof => (),
		}

		Ok(())
	}
}

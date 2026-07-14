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

//! Transaction extensions for indiv-pallet-score.

use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::fmt;
use frame_support::pallet_prelude::Weight;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, TransactionExtension, ValidateResult},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
	},
};

/// A type alias to access system runtime call.
type RuntimeCallOf<T> = <T as frame_system::Config>::RuntimeCall;

/// The data for the transaction extension [`ScoreAsParticipant`].
///
/// Use state to transmute to `AccountParticipant` origin.
#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T))]
pub struct ScoreAsParticipantData<Nonce> {
	/// The nonce of the account.
	pub nonce: Nonce,
}

/// Transaction extension to validate and transmute to `AccountParticipant`.
///
/// If `None` the extension is not used, if `Some`, the extension is used, will validate and
/// transmute to `AccountParticipant` origin.
///
/// **Warning**: This extension is not used alongside another extension that restrict the origin
/// [`Origin::AccountParticipant`](crate::Origin::AccountParticipant) to prevent spam.
/// It is recommended to use `indiv-pallet-origin-restriction`.
#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T))]
pub struct ScoreAsParticipant<T: Config>(Option<ScoreAsParticipantData<T::Nonce>>);

impl<T: Config> fmt::Debug for ScoreAsParticipant<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "ScoreAsParticipant")
	}
}

impl<T: Config> ScoreAsParticipant<T> {
	/// Creates a new `ScoreAsParticipant` transaction extension.
	pub fn new(data: Option<ScoreAsParticipantData<T::Nonce>>) -> Self {
		Self(data)
	}
}

/// The value passed from validate to prepare in the [`ScoreAsParticipant`] transaction extension.
pub enum ScoreAsParticipantVal<AccountId> {
	UseAsParticipant(AccountId),
	None,
}

impl<T: Config> TransactionExtension<RuntimeCallOf<T>> for ScoreAsParticipant<T> {
	const IDENTIFIER: &'static str = "ScoreAsParticipant";
	type Implicit = ();

	type Val = ScoreAsParticipantVal<T::AccountId>;
	type Pre = ();

	fn weight(&self, _call: &RuntimeCallOf<T>) -> Weight {
		match self.0 {
			Some(_) => <T as Config>::WeightInfo::as_participant_tx_ext(),
			None => Weight::zero(),
		}
	}

	fn validate(
		&self,
		origin: T::RuntimeOrigin,
		_call: &RuntimeCallOf<T>,
		_info: &DispatchInfoOf<RuntimeCallOf<T>>,
		_len: usize,
		_self_implicit: Self::Implicit,
		_inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, RuntimeCallOf<T>> {
		match self.0 {
			Some(ScoreAsParticipantData { nonce }) => {
				// Origin must be a signed origin.
				let Some(frame_system::Origin::<T>::Signed(who)) = origin.as_system_ref() else {
					return Err(InvalidTransaction::Call.into());
				};

				// We ensure it is an active participant.
				Pallet::<T>::ensure_active_participant(&AccountOrPerson::Account(who.clone()))
					.map_err(|_| InvalidTransaction::Call)?;

				// This policy nonce must exactly match the account nonce. The standard
				// account-aware CheckNonce owns pool tags and the single increment; accepting
				// dependency-style future nonces here would allow a transaction to satisfy its
				// own policy requirement.
				let current = frame_system::Pallet::<T>::account_nonce(who);
				if nonce < current {
					return Err(InvalidTransaction::Stale.into());
				}
				if nonce > current {
					return Err(InvalidTransaction::Future.into());
				}
				let validity = ValidTransaction::default();

				Ok((
					validity,
					ScoreAsParticipantVal::UseAsParticipant(who.clone()),
					Origin::AccountParticipant(who.clone()).into(),
				))
			},
			None => Ok((ValidTransaction::default(), ScoreAsParticipantVal::None, origin)),
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
		// The account-aware standard CheckNonce in both direct and Meta pipelines owns the single
		// nonce increment. Score validates its explicit nonce above, but must not increment twice.
		match val {
			ScoreAsParticipantVal::UseAsParticipant(_) | ScoreAsParticipantVal::None => Ok(()),
		}
	}
}

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

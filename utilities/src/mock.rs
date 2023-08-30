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

//! This module contains utilities for testing.
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::AccountId32;

/// This pallet only contains an origin which supports separated sender and
/// subject.
///
/// WARNING: This is only used for testing!
#[frame_support::pallet]
#[allow(dead_code)]
pub mod mock_origin {
	use sp_std::marker::PhantomData;

	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{traits::EnsureOrigin, Parameter};
	use scale_info::TypeInfo;
	use sp_runtime::AccountId32;

	use crate::traits::CallSources;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeOrigin: From<Origin<Self>>;
		type AccountId: Parameter;
		type SubjectId: Parameter;
	}

	/// A dummy pallet for adding an origin to the runtime that contains
	/// separate sender and subject accounts.
	///
	/// WARNING: This is only used for testing!
	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// An origin that is split into sender and subject.
	///
	/// WARNING: This is only used for testing!
	#[pallet::origin]
	pub type Origin<T> = DoubleOrigin<<T as Config>::AccountId, <T as Config>::SubjectId>;

	/// An origin that is split into sender and subject.
	///
	/// WARNING: This is only used for testing!
	#[derive(Debug, Clone, PartialEq, Eq, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub struct DoubleOrigin<AccountId, SubjectId>(pub AccountId, pub SubjectId);

	impl<AccountId: Clone, SubjectId: Clone> CallSources<AccountId, SubjectId>
		for DoubleOrigin<AccountId, SubjectId>
	{
		fn sender(&self) -> AccountId {
			self.0.clone()
		}

		fn subject(&self) -> SubjectId {
			self.1.clone()
		}
	}

	/// Ensure that the call was made using the split origin.
	///
	/// WARNING: This is only used for testing!
	pub struct EnsureDoubleOrigin<AccountId, SubjectId>(PhantomData<(AccountId, SubjectId)>);

	impl<OuterOrigin, AccountId, SubjectId> EnsureOrigin<OuterOrigin>
		for EnsureDoubleOrigin<AccountId, SubjectId>
	where
		OuterOrigin: Into<Result<DoubleOrigin<AccountId, SubjectId>, OuterOrigin>>
			+ From<DoubleOrigin<AccountId, SubjectId>>,
		AccountId: From<AccountId32>,
		SubjectId: From<AccountId32>,
	{
		type Success = DoubleOrigin<AccountId, SubjectId>;

		fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
			o.into()
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin() -> Result<OuterOrigin, ()> {
			const TEST_ACC: AccountId32 = AccountId32::new([0u8; 32]);

			Ok(OuterOrigin::from(DoubleOrigin(TEST_ACC.clone().into(), TEST_ACC.into())))
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<OuterOrigin, AccountId, SubjectId>
		crate::traits::GenerateBenchmarkOrigin<OuterOrigin, AccountId, SubjectId>
		for EnsureDoubleOrigin<AccountId, SubjectId>
	where
		OuterOrigin: Into<Result<DoubleOrigin<AccountId, SubjectId>, OuterOrigin>>
			+ From<DoubleOrigin<AccountId, SubjectId>>,
	{
		fn generate_origin(sender: AccountId, subject: SubjectId) -> OuterOrigin {
			OuterOrigin::from(DoubleOrigin(sender, subject))
		}
	}
}

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct SubjectId(pub AccountId32);

impl From<AccountId32> for SubjectId {
	fn from(acc: AccountId32) -> Self {
		SubjectId(acc)
	}
}

impl From<sr25519::Public> for SubjectId {
	fn from(acc: sr25519::Public) -> Self {
		SubjectId(acc.into())
	}
}

impl AsRef<[u8]> for SubjectId {
	fn as_ref(&self) -> &[u8] {
		self.0.as_ref()
	}
}

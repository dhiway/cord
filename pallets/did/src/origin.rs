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

use codec::{Decode, Encode, MaxEncodedLen};
use cord_utilities::traits::CallSources;
use frame_support::traits::EnsureOrigin;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::marker::PhantomData;

/// Origin for modules that support DID-based authorization.
#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DidRawOrigin<DidIdentifier, AccountId> {
	pub id: DidIdentifier,
	pub submitter: AccountId,
}

impl<DidIdentifier, AccountId> DidRawOrigin<DidIdentifier, AccountId> {
	pub fn new(id: DidIdentifier, submitter: AccountId) -> Self {
		Self { id, submitter }
	}
}

pub struct EnsureDidOrigin<DidIdentifier, AccountId>(PhantomData<(DidIdentifier, AccountId)>);

impl<OuterOrigin, DidIdentifier, AccountId> EnsureOrigin<OuterOrigin>
	for EnsureDidOrigin<DidIdentifier, AccountId>
where
	OuterOrigin: Into<Result<DidRawOrigin<DidIdentifier, AccountId>, OuterOrigin>>
		+ From<DidRawOrigin<DidIdentifier, AccountId>>,
	DidIdentifier: From<AccountId>,
	AccountId: Clone + Decode,
{
	type Success = DidRawOrigin<DidIdentifier, AccountId>;

	fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
		o.into()
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<OuterOrigin, ()> {
		let zero_account_id =
			AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
				.expect("infinite length input; no invalid inputs for type; qed");

		Ok(OuterOrigin::from(DidRawOrigin {
			id: zero_account_id.clone().into(),
			submitter: zero_account_id,
		}))
	}
}

impl<DidIdentifier: Clone, AccountId: Clone> CallSources<AccountId, DidIdentifier>
	for DidRawOrigin<DidIdentifier, AccountId>
{
	fn sender(&self) -> AccountId {
		self.submitter.clone()
	}

	fn subject(&self) -> DidIdentifier {
		self.id.clone()
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<OuterOrigin, AccountId, DidIdentifier>
	cord_utilities::traits::GenerateBenchmarkOrigin<OuterOrigin, AccountId, DidIdentifier>
	for EnsureDidOrigin<DidIdentifier, AccountId>
where
	OuterOrigin: Into<Result<DidRawOrigin<DidIdentifier, AccountId>, OuterOrigin>>
		+ From<DidRawOrigin<DidIdentifier, AccountId>>,
{
	fn generate_origin(sender: AccountId, subject: DidIdentifier) -> OuterOrigin {
		OuterOrigin::from(DidRawOrigin { id: subject, submitter: sender })
	}
}

#[cfg(all(test, feature = "runtime-benchmarks"))]
mod tests {
	use super::EnsureDidOrigin;

	#[test]
	pub fn successful_origin() {
		use crate::mock::Test;
		use frame_support::{assert_ok, traits::EnsureOrigin};

		let origin: <Test as frame_system::Config>::RuntimeOrigin =
			EnsureDidOrigin::try_successful_origin()
				.expect("Successful origin creation should not fail.");
		assert_ok!(EnsureDidOrigin::try_origin(origin));
	}
}

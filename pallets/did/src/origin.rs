// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use frame_support::{
	codec::{Decode, Encode},
	traits::EnsureOrigin,
};
use sp_runtime::RuntimeDebug;
use sp_std::marker::PhantomData;

use crate::*;

/// Origin for modules that support DID-based authorization.
#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug)]
pub struct DidRawOrigin<DidIdentifier> {
	pub id: DidIdentifier,
}

pub struct EnsureDidOrigin<DidIdentifier>(PhantomData<DidIdentifier>);

impl<OuterOrigin, DidIdentifier> EnsureOrigin<OuterOrigin> for EnsureDidOrigin<DidIdentifier>
where
	OuterOrigin: Into<Result<DidRawOrigin<DidIdentifier>, OuterOrigin>> + From<DidRawOrigin<DidIdentifier>>,
	DidIdentifier: Default,
{
	type Success = DidIdentifier;

	fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
		o.into().map(|o| o.id)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> OuterOrigin {
		OuterOrigin::from(DidRawOrigin { id: Default::default() })
	}
}

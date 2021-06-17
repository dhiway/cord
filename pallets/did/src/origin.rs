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

use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an attester.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MarkDetails<T: Config> {
	/// The hash of the MTYPE used for this mark.
	pub mtype_hash: MtypeHashOf<T>,
	/// The ID of the marker.
	pub marker: MarkerOf<T>,
	/// \[OPTIONAL\] The ID of the delegation node used to authorize the
	/// marker.
	pub delegation_id: Option<DelegationNodeIdOf<T>>,
	/// The flag indicating whether the mark has been revoked or not.
	pub revoked: bool,
}

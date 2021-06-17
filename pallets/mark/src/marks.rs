use codec::{Decode, Encode};

use crate::*;

/// An on-chain attestation written by an attester.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MarkDetails<T: Config> {
	/// The hash of the MTYPE used for this attestation.
	pub mtype_hash: MtypeHashOf<T>,
	/// The ID of the marker.
	pub marker: MarkerOf<T>,
	/// \[OPTIONAL\] The ID of the delegation node used to authorize the
	/// attester.
	pub delegation_id: Option<DelegationNodeIdOf<T>>,
	/// The flag indicating whether the attestation has been revoked or not.
	pub revoked: bool,
}

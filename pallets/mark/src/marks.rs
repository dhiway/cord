use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MarkDetails<T: Config> {
	/// The hash of the MTYPE used for this mark.
	pub mtype_hash: MtypeHashOf<T>,
	/// The ID of the issuer.
	pub issuer: IssuerOf<T>,
	/// StreamId type for CID
	pub streamid: Vec<u8>,
	/// StreamId type for CID
	pub parentid: Option<Vec<u8>>,
	/// \[OPTIONAL\] The ID of the delegation node used to authorize the
	/// issuer.
	pub delegation_id: Option<DelegationNodeIdOf<T>>,
	/// The flag indicating whether the mark has been revoked or not.
	pub revoked: bool,
}

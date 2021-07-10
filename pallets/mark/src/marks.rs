use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MarkDetails<T: Config> {
	/// The hash of the MTYPE used for this mark.
	pub mtype_hash: MtypeHashOf<T>,
	/// The ID of the issuer.
	pub issuer: MarkIssuerOf<T>,
	/// Stream CID
	pub stream_cid: CidOf,
	/// \[OPTIONAL\] Parent CID of the Stream
	pub parent_cid: Option<CidOf>,
	/// The digest hash of the mark presentation.
	pub digest_hash: Option<DigestHashOf<T>>,
	/// Mark block number
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating whether the mark has been revoked or not.
	pub revoked: bool,
}

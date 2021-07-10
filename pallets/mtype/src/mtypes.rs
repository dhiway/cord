use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MTypeDetails<T: Config> {
	/// The ID of the issuer.
	pub owner: MtypeOwnerOf<T>,
	/// Stream CID
	pub stream_cid: CidOf,
	/// Mark block number
	pub block_number: BlockNumberOf<T>,
}

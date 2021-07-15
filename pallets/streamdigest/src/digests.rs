use codec::{Decode, Encode};

use crate::*;

/// An on-chain digest of a stream written by the creator.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct DigestDetails<T: Config> {
	/// The ID of the creator.
	pub creator: DigestCreatorOf<T>,
	/// Hash of the stream linked
	pub stream_hash: StreamHashOf<T>,
	/// Digest transaction block number
	pub block_number: BlockNumberOf<T>,
	/// \[OPTIONAL\] CID of the digest
	pub digest_cid: Option<DigestCidOf>,
}

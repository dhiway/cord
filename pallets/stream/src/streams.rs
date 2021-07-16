use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamDetails<T: Config> {
	/// The hash of the schema used for this stream.
	pub schema_hash: SchemaHashOf<T>,
	/// The ID of the issuer.
	pub creator: StreamCreatorOf<T>,
	/// CID of the stream
	pub stream_cid: StreamCidOf,
	/// Hash of the linked journal stream
	pub journal_stream_hash: JournalStreamHashOf<T>,
	/// Stream transaction block
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating whether the cred has been revoked or not.
	pub revoked: bool,
}

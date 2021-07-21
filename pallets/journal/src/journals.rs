use codec::{Decode, Encode};

use crate::*;

/// An on-chain record of journal stream.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct JournalStreamDetails<T: Config> {
	/// The hash of the schema used for this journal stream.
	pub schema_hash: SchemaHashOf<T>,
	/// The ID of the journel stream creator.
	pub creator: JournalStreamCreatorOf<T>,
	/// Journal stream CID
	pub stream_cid: CidOf,
	/// \[OPTIONAL\] Parent CID of the journal stream
	pub parent_cid: Option<CidOf>,
	/// Journal stream transaction block
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating whether the journal stream has been revoked or not.
	pub revoked: bool,
}

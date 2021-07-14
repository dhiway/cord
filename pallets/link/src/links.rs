use codec::{Decode, Encode};

use crate::*;

/// An on-chain mark written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamLinkDetails<T: Config> {
	/// The hash of the schema used for this stream.
	pub schema_hash: SchemaHashOf<T>,
	/// The ID of the issuer.
	pub owner: LinkOwnerOf<T>,
	/// CID of the Link Transaction
	pub link_cid: LinkCidOf,
	/// Hash of the credential this stream is linked to
	pub cred_hash: CredHashOf<T>,
	/// Stream link block number
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating whether the linked transation
	/// has been revoked or not.
	pub revoked: bool,
}

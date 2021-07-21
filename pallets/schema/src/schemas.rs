use codec::{Decode, Encode};

use crate::*;

/// An on-chain schema input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaInput<T: Config> {
	/// Schema CID
	pub schema_id: SchemaIdOf<T>,
	/// Schema CID
	pub schema_cid: CidOf,
	/// Space ID
	pub space_id: SpaceIdOf<T>,
}

/// An on-chain schema written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaDetails<T: Config> {
	/// Schema CID
	pub schema_id: SchemaIdOf<T>,
	/// The identity of the owner.
	pub controller: SchemaControllerOf<T>,
	/// Schema CID
	pub schema_cid: CidOf,
	/// \[OPTIONAL\] Parent CID of the schema
	pub parent_cid: Option<CidOf>,
	/// Space ID
	pub space_id: SpaceIdOf<T>,
	/// Schema block number
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating whether the schema has been revoked or not.
	pub revoked: bool,
}

/// An on-chain schema input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaIdLinks<T: Config> {
	/// Schema CID
	pub schema_hash: SchemaHashOf<T>,
	/// Schema block number
	pub block_number: BlockNumberOf<T>,
	/// Transaction Type
	pub activity: ActivityOf,
}

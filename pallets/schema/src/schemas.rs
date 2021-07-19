use codec::{Decode, Encode};

use crate::*;

/// An on-chain schema input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaInput<T: Config> {
	/// Schema CID
	pub schema_id: SchemaIdOf<T>,
	/// Schema CID
	pub schema_cid: SchemaCidOf,
}

/// An on-chain schema written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaDetails<T: Config> {
	/// Schema CID
	pub schema_id: SchemaIdOf<T>,
	/// The identity of the owner.
	pub owner: SchemaOwnerOf<T>,
	/// Schema CID
	pub schema_cid: SchemaCidOf,
	/// \[OPTIONAL\] Parent CID of the schema
	pub parent_cid: Option<SchemaCidOf>,
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
	pub trans_type: SchemaTransOf,
}

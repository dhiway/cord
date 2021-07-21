use codec::{Decode, Encode};

use crate::*;

/// An on-chain entity input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct EntityInput<T: Config> {
	/// Entity ID
	pub entity_id: EntityIdOf<T>,
	/// Entity CID
	pub entity_cid: CidOf,
}

/// An on-chain schema written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct EntityDetails<T: Config> {
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// Entity CID
	pub entity_cid: CidOf,
	/// \[OPTIONAL\] Parent CID of the entity
	pub parent_cid: Option<CidOf>,
	/// Transaction block number
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating the verification status of the entity.
	pub verified: bool,
	/// The flag indicating the status of the entity account.
	pub active: bool,
}

/// An on-chain space input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceInput<T: Config> {
	/// Entity ID
	pub space_id: SpaceIdOf<T>,
	/// Entity CID
	pub space_cid: CidOf,
	/// Entity CID
	pub entity_id: EntityIdOf<T>,
}

/// An on-chain schema written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
	/// Entity CID
	pub space_id: SpaceIdOf<T>,
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// Entity ID
	pub entity_id: EntityIdOf<T>,
	/// Entity CID
	pub space_cid: CidOf,
	/// \[OPTIONAL\] Parent CID of the entity
	pub parent_cid: Option<CidOf>,
	/// Transaction block number
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating the status of the entity account.
	pub active: bool,
}

/// An on-chain schema input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct EntitySpaceDetails<T: Config> {
	/// Space transaction hash
	pub space_hash: HashOf<T>,
	/// Space ID
	pub space_id: SpaceIdOf<T>,
	/// Space Transaction block number
	pub block_number: BlockNumberOf<T>,
	/// Space Transaction Type
	pub activity: ActivityOf,
}

/// An on-chain schema input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct ActivityDetails<T: Config> {
	/// Schema CID
	pub entity_cid: CidOf,
	/// Schema block number
	pub block_number: BlockNumberOf<T>,
	/// Transaction Type
	pub activity: ActivityOf,
}

// impl Default for ActivityDetails<T> {}

// impl Default for ActivityDetails {
// 	fn default() -> Self {
//         entity_cid: None,
//         block_number: None,
//         activity: None
// 	}
// }

// /// An on-chain schema input.
// #[derive(Clone, Debug, Encode, Decode, PartialEq)]
// pub struct SchemaIdLinks<T: Config> {
// 	/// Schema CID
// 	pub schema_hash: SchemaHashOf<T>,
// 	/// Schema block number
// 	pub block_number: BlockNumberOf<T>,
// 	/// Transaction Type
// 	pub trans_type: SchemaTransOf,
// }

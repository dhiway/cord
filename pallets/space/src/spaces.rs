use codec::{Decode, Encode};

use crate::*;

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

/// An on-chain space transaction written by the controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
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

/// An on-chain space activity details mapped to space id.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct ActivityDetails<T: Config> {
	/// Schema CID
	pub space_cid: CidOf,
	/// Schema block number
	pub block_number: BlockNumberOf<T>,
	/// Transaction Type
	pub activity: ActivityOf,
}

/// An on-chain space links mapped to entity id.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct EntitySpaceLinkDetails<T: Config> {
	/// Space ID
	pub space_id: SpaceIdOf<T>,
	/// Space Transaction block number
	pub block_number: BlockNumberOf<T>,
	/// Space Transaction Type
	pub activity: ActivityOf,
}

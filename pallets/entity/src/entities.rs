use codec::{Decode, Encode};

use crate::*;

/// An on-chain entity written by a controller.
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

/// An on-chain entity activity details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct ActivityDetails<T: Config> {
	/// Schema CID
	pub entity_cid: CidOf,
	/// Schema block number
	pub block_number: BlockNumberOf<T>,
	/// Transaction Type
	pub activity: ActivityOf,
}

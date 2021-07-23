use codec::{Decode, Encode};

use crate::*;

/// An on-chain space transaction written by the controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// The transaction hash.
	pub tx_hash: SpaceHashOf<T>,
	/// \[OPTIONAL\] CID data
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] Previos CID
	pub ptx_cid: Option<CidOf>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

// An on-chain entity activity details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct ActionDetails<T: Config> {
	/// The transaction hash.
	pub tx_hash: EntityIdOf<T>,
	/// Schema CID
	pub tx_cid: Option<CidOf>,
	/// Schema block number
	pub block: BlockNumberOf<T>,
	/// Transaction Type
	pub action: ActionOf,
}

use codec::{Decode, Encode};

use crate::*;

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxInput<T: Config> {
	/// \[OPTIONAL\] CID data
	pub tx_type: TypeOf,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] CID data
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] CID data
	pub tx_link: Option<IdOf<T>>,
	/// \[OPTIONAL\] CID data
	pub tx_req: TypeOf,
}

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxDetails<T: Config> {
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] CID data
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] Previos CID
	pub ptx_cid: Option<CidOf>,
	/// \[OPTIONAL\] CID data
	pub tx_link: Option<IdOf<T>>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

/// An on-chain entity activity details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxCommits<T: Config> {
	/// \[OPTIONAL\] CID data
	pub tx_type: TypeOf,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Schema CID
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] CID data
	pub tx_link: Option<IdOf<T>>,
	/// Schema block number
	pub block: BlockNumberOf<T>,
	/// Transaction Type
	pub commit: RequestOf,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub enum RequestOf {
	Create,
	Update,
	Status,
	Verify,
}

// #[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
// #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub enum TypeOf {
	Entity,
	Space,
}

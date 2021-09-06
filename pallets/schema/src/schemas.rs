use crate::*;
use codec::{Decode, Encode};

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxDetails<T: Config> {
	/// Transaction identifier.
	pub tx_hash: HashOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// Space link ID.
	pub tx_link: IdOf<T>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
}

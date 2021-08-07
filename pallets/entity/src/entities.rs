use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct EntityDetails<T: Config> {
	/// Transaction identifier.
	pub tx_hash: HashOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> EntityDetails<T> {
	pub fn check_cid(incoming: &CidOf) -> bool {
		let cid_base = str::from_utf8(incoming).unwrap();
		if cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)) {
			true
		} else {
			false
		}
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxCommits<T: Config> {
	/// Transaction type.
	pub tx_type: TypeOf,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Transaction CID
	pub tx_cid: CidOf,
	/// \[OPTIONAL\] Transaction link
	pub tx_link: Option<IdOf<T>>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// Transaction request type
	pub commit: RequestOf,
}

impl<T: Config> TxCommits<T> {
	pub fn store_commit_tx(tx_id: &IdOf<T>, tx_commit: TxCommits<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(tx_id).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(tx_id, commit);
		Ok(())
	}

	pub fn update_commit_tx(tx_id: &IdOf<T>, tx_commit: TxCommits<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(tx_id).unwrap();
		commit.push(tx_commit);
		<Commits<T>>::insert(tx_id, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum RequestOf {
	Create,
	Update,
	Status,
	Verify,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum TypeOf {
	Entity,
	Space,
	Schema,
	Journal,
	Mark,
	Link,
}

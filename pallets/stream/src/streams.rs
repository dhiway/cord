use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain stream transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamDetails<T: Config> {
	/// Stream tx hash.
	pub tx_hash: HashOf<T>,
	/// Stream tx Store Id.
	pub tx_sid: Option<SidOf>,
	/// Stream parent Store Id.
	pub ptx_sid: Option<SidOf>,
	/// Schema tx Link
	pub tx_schema: Option<IdOf<T>>,
	/// Stream tx Link
	pub tx_link: Option<IdOf<T>>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the journal entry.
	pub revoked: StatusOf,
}

impl<T: Config> StreamDetails<T> {
	pub fn check_sid(incoming: &SidOf) -> bool {
		let sid_base = str::from_utf8(incoming).unwrap();
		if sid_base.len() <= 64 && (utils::is_base_32(sid_base) || utils::is_base_58(sid_base)) {
			true
		} else {
			false
		}
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamLink<T: Config> {
	/// The stream Id to link.
	pub tx_id: IdOf<T>,
	/// The identity of the stream controller.
	pub controller: CordAccountOf<T>,
}

impl<T: Config> StreamLink<T> {
	pub fn link_tx(tx_stream: &IdOf<T>, tx_link: StreamLink<T>) -> DispatchResult {
		let mut link = <Links<T>>::get(tx_stream).unwrap_or_default();
		link.push(tx_link);
		<Links<T>>::insert(tx_stream, link);
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamCommit<T: Config> {
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Transaction Store Id
	pub tx_sid: Option<SidOf>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// Transaction request type
	pub commit: StreamCommitOf,
}

impl<T: Config> StreamCommit<T> {
	pub fn store_tx(tx_id: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(tx_id).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(tx_id, commit);
		Ok(())
	}

	pub fn update_tx(tx_id: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(tx_id).unwrap();
		commit.push(tx_commit);
		<Commits<T>>::insert(tx_id, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum StreamCommitOf {
	Genesis,
	Update,
	StatusChange,
}

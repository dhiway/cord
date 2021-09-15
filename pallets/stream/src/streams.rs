use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain stream transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamDetails<T: Config> {
	/// Stream tx hash.
	pub hash: HashOf<T>,
	/// Stream tx Store Id.
	pub cid: Option<IdentifierOf>,
	/// Stream parent Store Id.
	pub parent_cid: Option<IdentifierOf>,
	/// Schema tx Link
	pub schema: Option<IdOf<T>>,
	/// Stream tx Link
	pub link: Option<IdOf<T>>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the journal entry.
	pub revoked: StatusOf,
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamLink<T: Config> {
	/// The stream Id to link.
	pub identifier: IdOf<T>,
	/// The identity of the stream controller.
	pub controller: CordAccountOf<T>,
}

impl<T: Config> StreamLink<T> {
	pub fn link_tx(stream: &IdOf<T>, link: StreamLink<T>) -> DispatchResult {
		let mut links = <Links<T>>::get(stream).unwrap_or_default();
		links.push(link);
		<Links<T>>::insert(stream, links);
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct StreamCommit<T: Config> {
	/// The transaction hash.
	pub hash: HashOf<T>,
	/// Transaction Store Id
	pub cid: Option<IdentifierOf>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// Transaction request type
	pub commit: StreamCommitOf,
}

impl<T: Config> StreamCommit<T> {
	pub fn store_tx(identifier: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}

	pub fn update_tx(identifier: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum StreamCommitOf {
	Genesis,
	Update,
	StatusChange,
}

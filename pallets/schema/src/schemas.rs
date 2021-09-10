use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain schema details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaDetails<T: Config> {
	/// Schema identifier.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] Schema CID.
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] Schema parent CID.
	pub ptx_cid: Option<CidOf>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating schema type.
	pub permissioned: bool,
	/// The flag indicating the status of the schema.
	pub revoked: bool,
}

impl<T: Config> SchemaDetails<T> {
	pub fn check_cid(incoming: &CidOf) -> bool {
		let cid_base = str::from_utf8(incoming).unwrap();
		if cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)) {
			true
		} else {
			false
		}
	}

	pub fn schema_status(tx_schema: IdOf<T>, controller: CordAccountOf<T>) -> Result<(), Error<T>> {
		let schema_details = <Schemas<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(schema_details.revoked, Error::<T>::SchemaNotActive);
		if schema_details.permissioned {
			let delegates = <Delegations<T>>::take(&tx_schema);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == controller) == Some(&controller)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaCommit<T: Config> {
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Transaction CID
	pub tx_cid: Option<CidOf>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// Transaction request type
	pub commit: SchemaCommitOf,
}

impl<T: Config> SchemaCommit<T> {
	pub fn store_tx(tx_id: &IdOf<T>, tx_commit: SchemaCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(tx_id).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(tx_id, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum SchemaCommitOf {
	Genesis,
	Update,
	Delegate,
	Permission,
	Status,
}

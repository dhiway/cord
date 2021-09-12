use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain schema details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaDetails<T: Config> {
	/// Schema identifier.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] Storage ID (base32/52).
	pub tx_sid: Option<SidOf>,
	/// \[OPTIONAL\] Previous Storage ID (base32/52).
	pub ptx_sid: Option<SidOf>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating schema type.
	pub permissioned: StatusOf,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
}

impl<T: Config> SchemaDetails<T> {
	pub fn check_sid(incoming: &SidOf) -> bool {
		let sid_base = str::from_utf8(incoming).unwrap();
		if sid_base.len() <= 64 && (utils::is_base_32(sid_base) || utils::is_base_58(sid_base)) {
			true
		} else {
			false
		}
	}

	pub fn schema_status(tx_schema: IdOf<T>, controller: CordAccountOf<T>) -> Result<(), Error<T>> {
		let schema_details = <Schemas<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
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
	/// schema hash.
	pub tx_hash: HashOf<T>,
	/// schema storage ID
	pub tx_sid: Option<SidOf>,
	/// schema tx block number
	pub block: BlockNumberOf<T>,
	/// schema tx request type
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
	RevokeDelegation,
	Permission,
	StatusChange,
}

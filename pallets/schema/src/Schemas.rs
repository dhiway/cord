use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SchemaDetails<T: Config> {
	/// Transaction identifier.
	pub tx_hash: HashOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// \[OPTIONAL\] Transaction link ID.
	pub tx_link: IdOf<T>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> SchemaDetails<T> {
	pub fn schema_status(tx_schema: IdOf<T>, tx_link: IdOf<T>) -> DispatchResult {
		//check schema status
		let tx_schema_details = <Schemas<T>>::get(tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(tx_schema_details.tx_link == tx_link, Error::<T>::UnauthorizedOperation);

		Ok(())
	}
}

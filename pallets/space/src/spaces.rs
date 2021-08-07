use crate::*;
use codec::{Decode, Encode};
// use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
	/// Transaction identifier.
	pub tx_hash: HashOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// Transaction link ID.
	pub tx_link: IdOf<T>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> SpaceDetails<T> {
	pub fn space_status(tx_id: IdOf<T>) -> Result<IdOf<T>, Error<T>> {
		let tx_space_details = <Spaces<T>>::get(&tx_id).ok_or(Error::<T>::SpaceNotFound)?;
		ensure!(tx_space_details.active, Error::<T>::SpaceNotActive);

		Ok(tx_space_details.tx_link)
	}
}

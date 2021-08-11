use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct MarkDetails<T: Config> {
	/// Transaction identifier.
	pub tx_hash: HashOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// Space Link
	pub tx_link: IdOf<T>,
	/// The identity of the controller.
	pub controller: CordAccountOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the journal entry.
	pub active: bool,
}

impl<T: Config> MarkDetails<T> {
	pub fn mark_status(tx_id: IdOf<T>) -> Result<IdOf<T>, Error<T>> {
		let tx_mark_details = <Marks<T>>::get(tx_id).ok_or(Error::<T>::MarkNotFound)?;
		ensure!(tx_mark_details.active, Error::<T>::MarkNotActive);

		Ok(tx_mark_details.tx_link)
	}
	pub fn store_link_tx(tx_mark: &IdOf<T>, tx_link: &IdOf<T>) -> DispatchResult {
		let mut link = <MarkLinks<T>>::get(tx_mark).unwrap_or_default();
		link.push(*tx_link);
		<MarkLinks<T>>::insert(tx_mark, link);
		Ok(())
	}
}

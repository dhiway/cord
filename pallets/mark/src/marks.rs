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
	pub fn mark_status(tx_link: IdOf<T>, controller: CordAccountOf<T>) -> DispatchResult {
		let tx_mark_details = <Marks<T>>::get(tx_link).ok_or(Error::<T>::MarkNotFound)?;
		ensure!(tx_mark_details.active, Error::<T>::MarkNotActive);

		let _journal_link_status = pallet_journal::JournalDetails::<T>::journal_status(
			tx_mark_details.tx_link,
			controller,
		);
		Ok(())
	}
}

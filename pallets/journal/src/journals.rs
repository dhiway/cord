use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct JournalDetails<T: Config> {
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

impl<T: Config> JournalDetails<T> {
	pub fn journal_status(tx_link: IdOf<T>) -> Result<IdOf<T>, Error<T>> {
		let tx_journal_details = <Journals<T>>::get(tx_link).ok_or(Error::<T>::JournalNotFound)?;
		ensure!(tx_journal_details.active, Error::<T>::JournalNotActive);

		Ok(tx_journal_details.tx_link)
	}

	pub fn store_link_tx(tx_journal: &IdOf<T>, tx_mark: &IdOf<T>) -> DispatchResult {
		let mut link = <Jlinks<T>>::get(tx_journal).unwrap_or_default();
		link.push(*tx_mark);
		<Jlinks<T>>::insert(tx_journal, link);
		Ok(())
	}
}

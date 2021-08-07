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
	pub fn journal_status(tx_link: IdOf<T>, controller: CordAccountOf<T>) -> DispatchResult {
		let tx_journal_details = <Journals<T>>::get(tx_link).ok_or(Error::<T>::JournalNotFound)?;
		ensure!(tx_journal_details.active, Error::<T>::JournalNotActive);

		let _space_link_status =
			pallet_space::SpaceDetails::<T>::space_status(tx_journal_details.tx_link, controller);
		Ok(())
	}
}

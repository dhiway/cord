use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
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

impl<T: Config> SpaceDetails<T> {
	pub fn space_status(tx_link: IdOf<T>, controller: CordAccountOf<T>) -> DispatchResult {
		let tx_space_details = <Spaces<T>>::get(&tx_link).ok_or(Error::<T>::SpaceNotFound)?;
		ensure!(tx_space_details.active, Error::<T>::SpaceNotActive);
		//check entity status
		let tx_entity_details = <pallet_entity::Entities<T>>::get(&tx_space_details.tx_link)
			.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
		ensure!(tx_entity_details.active, pallet_entity::Error::<T>::EntityNotActive);
		ensure!(
			tx_entity_details.controller == controller,
			pallet_entity::Error::<T>::UnauthorizedOperation
		);
		Ok(())
	}
}

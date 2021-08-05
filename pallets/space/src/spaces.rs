use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SpaceDetails<T: Config> {
	/// Transaction identifier.
	pub tx_id: IdOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
	/// \[OPTIONAL\] Transaction link ID.
	pub tx_link: IdOf<T>,
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> SpaceDetails<T> {
	pub fn store_commit_tx(
		tx_hash: HashOf<T>,
		tx_details: &SpaceDetails<T>,
		tx_request: RequestOf,
	) -> DispatchResult {
		let mut commit = <pallet_entity::Commits<T>>::get(tx_details.tx_id).unwrap_or_default();
		commit.push(pallet_entity::TxCommits {
			tx_type: TypeOf::Space,
			tx_hash,
			tx_cid: tx_details.tx_cid.clone(),
			tx_link: Some(tx_details.tx_link.clone()),
			block: tx_details.block,
			commit: tx_request,
		});
		<pallet_entity::Commits<T>>::insert(tx_details.tx_id, commit);
		Ok(())
	}

	pub fn update_commit_tx(
		tx_hash: HashOf<T>,
		tx_details: &SpaceDetails<T>,
		tx_request: RequestOf,
	) -> DispatchResult {
		let mut commit = <pallet_entity::Commits<T>>::get(tx_details.tx_id).unwrap();
		commit.push(pallet_entity::TxCommits {
			tx_type: TypeOf::Space,
			tx_hash,
			tx_cid: tx_details.tx_cid.clone(),
			tx_link: Some(tx_details.tx_link.clone()),
			block: tx_details.block,
			commit: tx_request,
		});
		<pallet_entity::Commits<T>>::insert(tx_details.tx_id, commit);
		Ok(())
	}
	// 	pub fn check_cid(incoming: &CidOf) -> bool {
	// 		let cid_base = str::from_utf8(incoming).unwrap();
	// 		if cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)) {
	// 			true
	// 		} else {
	// 			false
	// 		}
	// 	}
}

// /// An on-chain commit details.
// #[derive(Clone, Debug, Encode, Decode, PartialEq)]
// pub struct TxCommits<T: Config> {
// 	/// The transaction hash.
// 	pub tx_hash: HashOf<T>,
// 	/// Transaction CID
// 	pub tx_cid: CidOf,
// 	/// Transaction block number
// 	pub block: BlockNumberOf<T>,
// 	/// Transaction request type
// 	pub commit: RequestOf,
// }

// impl<T: Config> TxCommits<T> {

// }

// #[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
// pub enum RequestOf {
// 	Create,
// 	Update,
// 	Status,
// 	Verify,
// }

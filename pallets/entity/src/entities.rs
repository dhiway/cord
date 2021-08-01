use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain transaction input.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxInput<T: Config> {
	/// Transaction type.
	pub tx_type: TypeOf,
	/// Transaction identifier.
	pub tx_id: IdOf<T>,
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// \[OPTIONAL\] Transaction schema.
	pub tx_schema: Option<IdOf<T>>,
	/// \[OPTIONAL\] Transaction link.
	pub tx_link: Option<IdOf<T>>,
}

impl<T: Config> TxInput<T> {
	pub fn is_valid(tx_details: &TxInput<T>) -> bool {
		match tx_details.tx_type {
			TypeOf::Entity => true,
			TypeOf::Space | TypeOf::Schema => {
				if tx_details.tx_link.is_some() {
					true
				} else {
					false
				}
			}
			TypeOf::Journal | TypeOf::Stream | TypeOf::Link => {
				if tx_details.tx_link.is_some() && tx_details.tx_schema.is_some() {
					true
				} else {
					false
				}
			}
		}
	}

	pub fn check_cid(incoming: &CidOf) -> bool {
		let cid_base = str::from_utf8(incoming).unwrap();
		if cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)) {
			true
		} else {
			false
		}
	}
}

/// An on-chain transaction storage details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxStorageOf {
	/// Transaction CID.
	pub tx_cid: CidOf,
	/// Transaction parent CID.
	pub ptx_cid: Option<CidOf>,
}

/// An on-chain transaction details written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxDetails<T: Config> {
	/// Transaction type.
	pub tx_type: TypeOf,
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// Transaction identifier.
	pub tx_id: IdOf<T>,
	/// Transaction storage details.
	pub tx_storage: TxStorageOf,
	/// \[OPTIONAL\] Transaction schema.
	pub tx_schema: Option<IdOf<T>>,
	/// \[OPTIONAL\] CID data
	pub tx_link: Option<IdOf<T>>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> TxDetails<T> {
	pub fn validate_tx(
		tx_input: TxInput<T>,
		controller: ControllerOf<T>,
	) -> Result<TxDetails<T>, Error<T>> {
		if let Some(schema_id) = &tx_input.tx_schema {
			ensure!(<Schemas<T>>::contains_key(schema_id), Error::<T>::SchemaNotFound);
			let hash = <TransactionIds<T>>::get(schema_id).ok_or(Error::<T>::SchemaIdNotFound)?;
			let schema = <Transactions<T>>::get(hash).ok_or(Error::<T>::SchemaHashNotFound)?;
			ensure!(schema.active, Error::<T>::SchemaNotActive);
			ensure!(schema.controller == controller, Error::<T>::UnauthorizedOperation);
			// ensure!(schema.tx_link == tx_input.tx_link, Error::<T>::SchemaLinkMisMatch);
		}

		if let Some(link_id) = &tx_input.tx_link {
			let hash = <TransactionIds<T>>::get(link_id).ok_or(Error::<T>::LinkNotFound)?;
			let link = <Transactions<T>>::get(hash).ok_or(Error::<T>::LinkNotFound)?;
			ensure!(link.active, Error::<T>::LinkNotActive);
			ensure!(link.controller == controller, Error::<T>::UnauthorizedOperation);
		}

		let block_number = <frame_system::Pallet<T>>::block_number();

		Ok(TxDetails {
			tx_type: tx_input.tx_type,
			controller,
			tx_id: tx_input.tx_id,
			tx_storage: TxStorageOf { tx_cid: tx_input.tx_cid, ptx_cid: None },
			tx_schema: tx_input.tx_schema,
			tx_link: tx_input.tx_link,
			block: block_number,
			active: true,
		})
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxCommits<T: Config> {
	/// Transaction type.
	pub tx_type: TypeOf,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Transaction CID
	pub tx_cid: CidOf,
	/// \[OPTIONAL\] Transaction link
	pub tx_link: Option<IdOf<T>>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// Transaction request type
	pub commit: RequestOf,
}

impl<T: Config> TxCommits<T> {
	pub fn store_tx(
		tx_hash: HashOf<T>,
		tx_details: &TxDetails<T>,
		tx_request: RequestOf,
	) -> DispatchResult {
		match tx_details.tx_type {
			TypeOf::Entity => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(<Entities<T>>::contains_key(tx_link), Error::<T>::EntityLinkNotFound);
				}
				let mut commit = <Entities<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Entities<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Space => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(
						(<Entities<T>>::contains_key(tx_link)
							|| <Spaces<T>>::contains_key(tx_link)),
						Error::<T>::InvalidSpaceLink
					);
				}
				let mut commit = <Spaces<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Spaces<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Schema => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(
						<TransactionIds<T>>::contains_key(tx_link),
						Error::<T>::SchemaLinkNotFound
					);
				}
				let mut commit = <Schemas<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Schemas<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Journal => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(<Spaces<T>>::contains_key(tx_link), Error::<T>::SpaceLinkNotFound);
				}
				let mut commit = <Journals<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Journals<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Stream => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(<Journals<T>>::contains_key(tx_link), Error::<T>::JournalLinkNotFound);
				}
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Streams<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Link => {
				if let Some(ref tx_link) = tx_details.tx_link {
					ensure!(<Streams<T>>::contains_key(tx_link), Error::<T>::StreamLinkNotFound);
				}
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Links<T>>::insert(tx_details.tx_id, commit);
			}
		}
		let mut commit = <Commits<T>>::get(tx_details.tx_id).unwrap_or_default();
		commit.push(TxCommits {
			tx_type: tx_details.tx_type.clone(),
			tx_hash,
			tx_cid: tx_details.tx_storage.tx_cid.clone(),
			tx_link: tx_details.tx_link,
			block: tx_details.block,
			commit: tx_request,
		});
		<Commits<T>>::insert(tx_details.tx_id, commit);
		Ok(())
	}

	pub fn update_tx(
		tx_hash: HashOf<T>,
		tx_details: &TxDetails<T>,
		tx_request: RequestOf,
	) -> DispatchResult {
		match tx_details.tx_type {
			TypeOf::Entity => {
				let mut commit = <Entities<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Entities<T>>::insert(&tx_details.tx_id, commit);
				<VerifiedEntities<T>>::insert(&tx_details.tx_id, false);
			}
			TypeOf::Space => {
				let mut commit = <Spaces<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Spaces<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Schema => {
				let mut commit = <Schemas<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Schemas<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Journal => {
				let mut commit = <Journals<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Journals<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Stream => {
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Streams<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Link => {
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap();
				commit.push(tx_hash);
				<Links<T>>::insert(tx_details.tx_id, commit);
			}
		}
		let mut commit = <Commits<T>>::get(tx_details.tx_id).unwrap();
		commit.push(TxCommits {
			tx_type: tx_details.tx_type.clone(),
			tx_hash,
			tx_cid: tx_details.tx_storage.tx_cid.clone(),
			tx_link: tx_details.tx_link,
			block: tx_details.block,
			commit: tx_request,
		});
		<Commits<T>>::insert(tx_details.tx_id, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum RequestOf {
	Create,
	Update,
	Status,
	Verify,
}

// impl RequestOf {
// 	pub fn is_valid(request: &RequestOf) -> bool {
// 		matches!(
// 			request,
// 			RequestOf::Create | RequestOf::Update | RequestOf::Status | RequestOf::Verify
// 		)
// 	}
// }

// impl Default for RequestOf {
// 	fn default() -> Self {
// 		RequestOf::Create
// 	}
// }

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum TypeOf {
	Entity,
	Space,
	Schema,
	Journal,
	Stream,
	Link,
}

// impl Default for TypeOf {
// 	fn default() -> Self {
// 		TypeOf::Stream
// 	}
// }

impl TypeOf {
	pub fn is_valid(tx_type: &TypeOf) -> bool {
		// let mut boolean = false;
		matches!(
			tx_type,
			TypeOf::Entity
				| TypeOf::Space | TypeOf::Schema
				| TypeOf::Journal
				| TypeOf::Stream | TypeOf::Link
		)
	}
}

// /// An on-chain entity written by a controller.
// #[derive(Clone, Debug, Encode, Decode, PartialEq)]
// pub struct TxVerifiedOf<T: Config> {
// 	/// Entity Hash.
// 	pub tx_hash: HashOf<T>,
// 	/// Entity Verification status.
// 	pub tx_verified: bool,
// }

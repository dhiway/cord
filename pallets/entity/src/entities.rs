use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxInput<T: Config> {
	/// \[OPTIONAL\] CID data
	pub tx_req: RequestOf,
	/// \[OPTIONAL\] CID data
	pub tx_type: TypeOf,
	/// Transaction hash.
	pub tx_id: IdOf<T>,
	/// \[OPTIONAL\] CID data
	pub tx_cid: Option<CidOf>,
	/// Transaction schema.
	pub tx_schema_id: Option<IdOf<T>>,
	/// Transaction schema hash.
	pub tx_schema_hash: Option<HashOf<T>>,
	/// \[OPTIONAL\] CID data
	pub tx_link_id: Option<IdOf<T>>,
	/// \[OPTIONAL\] CID data
	pub tx_link_hash: Option<HashOf<T>>,
}

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxLinkOf<T: Config> {
	/// Transaction schema.
	pub tx_id: Option<IdOf<T>>,
	/// Transaction hash.
	pub tx_hash: Option<HashOf<T>>,
	/// Transaction schema.
	pub tx_type: Option<TypeOf>,
}

impl<T: Config> Default for TxLinkOf<T> {
	fn default() -> Self {
		TxLinkOf { tx_id: None, tx_hash: None, tx_type: None }
	}
}

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxStorageOf {
	/// Transaction schema.
	pub tx_cid: Option<CidOf>,
	/// Transaction schema.
	pub ptx_cid: Option<CidOf>,
}

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxVerifiedOf<T: Config> {
	/// Transaction schema.
	pub tx_hash: HashOf<T>,
	/// Transaction schema.
	pub tx_verified: bool,
}

/// An on-chain entity written by a controller.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxDetails<T: Config> {
	/// \[OPTIONAL\] CID data
	pub tx_type: TypeOf,
	/// The identity of the controller.
	pub controller: ControllerOf<T>,
	/// The transaction hash.
	pub tx_id: IdOf<T>,
	/// \[OPTIONAL\] Storage data
	pub tx_storage: TxStorageOf,
	/// Transaction schema.
	pub tx_schema: TxLinkOf<T>,
	/// \[OPTIONAL\] CID data
	pub tx_link: TxLinkOf<T>,
	/// Transaction block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the account.
	pub active: bool,
}

impl<T: Config> TxDetails<T> {
	pub fn check_cid(incoming: &CidOf) -> bool {
		let cid_base = str::from_utf8(&incoming).unwrap();
		if cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)) {
			true
		} else {
			false
		}
	}

	fn check_tx_schema(
		hash: HashOf<T>,
		controller: ControllerOf<T>,
	) -> Result<TxLinkOf<T>, Error<T>> {
		let schema = <Transactions<T>>::get(hash).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(schema.active, Error::<T>::SchemaNotActive);
		ensure!(schema.controller == controller, Error::<T>::UnauthorizedOperation);

		Ok(TxLinkOf {
			tx_id: schema.tx_schema.tx_id,
			tx_hash: schema.tx_schema.tx_hash,
			tx_type: schema.tx_schema.tx_type,
		})
	}

	fn check_tx_link(
		hash: HashOf<T>,
		controller: ControllerOf<T>,
	) -> Result<TxLinkOf<T>, Error<T>> {
		let link = <Transactions<T>>::get(hash).ok_or(Error::<T>::LinkNotFound)?;
		ensure!(link.active, Error::<T>::LinkNotActive);
		ensure!(link.controller == controller, Error::<T>::UnauthorizedOperation);

		Ok(TxLinkOf {
			tx_id: link.tx_link.tx_id,
			tx_hash: link.tx_link.tx_hash,
			tx_type: link.tx_link.tx_type,
		})
	}

	pub fn validate_tx(
		tx_input: TxInput<T>,
		controller: ControllerOf<T>,
	) -> Result<TxDetails<T>, Error<T>> {
		let mut tx_valid_schema = TxLinkOf::default();
		let mut tx_valid_link = TxLinkOf::default();

		if let Some(ref schema_id) = tx_input.tx_schema_id {
			let hash = <TransactionIds<T>>::get(schema_id).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(<Schemas<T>>::contains_key(hash), Error::<T>::SchemaNotFound);
			tx_valid_schema = Self::check_tx_schema(hash, controller.clone())?;
		}
		if let Some(schema_hash) = tx_input.tx_schema_hash {
			ensure!(<Schemas<T>>::contains_key(schema_hash), Error::<T>::SchemaNotFound);
			tx_valid_schema = Self::check_tx_schema(schema_hash, controller.clone())?;
		}

		if let Some(ref link_id) = tx_input.tx_link_id {
			let hash = <TransactionIds<T>>::get(link_id).ok_or(Error::<T>::LinkNotFound)?;
			tx_valid_link = Self::check_tx_link(hash, controller.clone())?;
		}
		if let Some(link_hash) = tx_input.tx_link_hash {
			tx_valid_link = Self::check_tx_link(link_hash, controller.clone())?;
		}
		let block_number = <frame_system::Pallet<T>>::block_number();

		Ok(TxDetails {
			tx_type: tx_input.tx_type,
			controller,
			tx_id: tx_input.tx_id,
			tx_storage: TxStorageOf { tx_cid: tx_input.tx_cid, ptx_cid: None },
			tx_schema: tx_valid_schema,
			tx_link: tx_valid_link,
			block: block_number,
			active: true,
		})
	}

	pub fn map_update_tx(
		tx_input: &TxInput<T>,
		tx_update: &TxDetails<T>,
	) -> Result<TxDetails<T>, Error<T>> {
		let mut tx_update_details = tx_update.clone();

		if let Some(tx_cid) = &tx_input.tx_cid {
			ensure!(Self::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);
			tx_update_details.tx_storage.ptx_cid = tx_update_details.tx_storage.tx_cid;
			tx_update_details.tx_storage.tx_cid = Some(tx_cid.to_vec());
		}

		if let Some(tx_schema_id) = tx_input.tx_schema_id {
			ensure!(<Schemas<T>>::contains_key(tx_schema_id), Error::<T>::SchemaNotFound);
			let tx_schema_update = Self::check_tx_schema(
				tx_update_details.tx_schema.tx_hash.unwrap(),
				tx_update_details.controller.clone(),
			)?;
			tx_update_details.tx_schema = tx_schema_update;
		}

		if let Some(tx_schema_hash) = tx_input.tx_schema_hash {
			ensure!(
				<Schemas<T>>::contains_key(tx_update_details.tx_schema.tx_id.unwrap()),
				Error::<T>::SchemaNotFound
			);
			let tx_schema_update =
				Self::check_tx_schema(tx_schema_hash, tx_update_details.controller.clone())?;
			tx_update_details.tx_schema = tx_schema_update;
		}

		if let Some(tx_link_id) = tx_input.tx_link_id {
			ensure!(<TransactionIds<T>>::contains_key(tx_link_id), Error::<T>::LinkNotFound);
			let tx_link_update = Self::check_tx_schema(
				tx_update_details.tx_link.tx_hash.unwrap(),
				tx_update_details.controller.clone(),
			)?;
			tx_update_details.tx_link = tx_link_update;
		}

		if let Some(tx_link_hash) = tx_input.tx_link_hash {
			ensure!(
				<TransactionIds<T>>::contains_key(tx_update_details.tx_link.tx_hash.unwrap()),
				Error::<T>::LinkNotFound
			);
			let tx_link_update =
				Self::check_tx_schema(tx_link_hash, tx_update_details.controller.clone())?;
			tx_update_details.tx_link = tx_link_update;
		}

		Ok(tx_update_details)
	}
}

/// An on-chain entity activity details.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct TxCommits<T: Config> {
	/// \[OPTIONAL\] CID data
	pub tx_type: TypeOf,
	/// The transaction hash.
	pub tx_hash: HashOf<T>,
	/// Schema CID
	pub tx_cid: Option<CidOf>,
	/// \[OPTIONAL\] CID data
	pub tx_link: Option<HashOf<T>>,
	/// Schema block number
	pub block: BlockNumberOf<T>,
	/// Transaction Type
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
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Entities<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				let mut commit = <Entities<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Entities<T>>::insert(tx_details.tx_id, commit);
			}
			TypeOf::Space => {
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(
						(<Entities<T>>::contains_key(tx_link)
							|| <Spaces<T>>::contains_key(tx_link)),
						Error::<T>::LinkNotFound
					);
				}
				let mut commit = <Spaces<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Spaces<T>>::insert(tx_details.tx_id, commit);
				// <SpaceHashes<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Schema(SchemaOf::Journal) => {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Journal)),
					Error::<T>::LinkNotFound
				);
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Spaces<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				let mut commit = <Schemas<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Schemas<T>>::insert(tx_details.tx_id, commit);
				// <SchemaIds<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Schema(SchemaOf::Stream) => {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Stream)),
					Error::<T>::LinkNotFound
				);
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Journals<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				let mut commit = <Schemas<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Schemas<T>>::insert(tx_details.tx_id, commit);
				// <SchemaIds<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Schema(SchemaOf::Link) => {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Stream)),
					Error::<T>::LinkNotFound
				);
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Streams<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				let mut commit = <Schemas<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Schemas<T>>::insert(tx_details.tx_id, commit);
				// <SchemaIds<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Journal => {
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Spaces<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				// if let Some(ref tx_schema) = tx_details.tx_schema.tx_hash {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Journal)),
					Error::<T>::LinkNotFound
				);
				// }
				let mut commit = <Journals<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Journals<T>>::insert(tx_details.tx_id, commit);
				// <JournalHashes<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Stream => {
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Journals<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				// if let Some(ref tx_schema) = tx_details.tx_schema.tx_hash {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Stream)),
					Error::<T>::LinkNotFound
				);
				// }
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Streams<T>>::insert(tx_details.tx_id, commit);
				// <StreamHashes<T>>::insert(tx_hash, tx_details.tx_id);
			}
			TypeOf::Link => {
				if let Some(ref tx_link) = tx_details.tx_link.tx_id {
					ensure!(<Streams<T>>::contains_key(tx_link), Error::<T>::LinkNotFound);
				}
				// if let Some(ref tx_schema) = tx_details.tx_schema.tx_hash {
				ensure!(
					tx_details.tx_schema.tx_type == Some(TypeOf::Schema(SchemaOf::Link)),
					Error::<T>::LinkNotFound
				);
				// }
				let mut commit = <Streams<T>>::get(tx_details.tx_id).unwrap_or_default();
				commit.push(tx_hash);
				<Links<T>>::insert(tx_details.tx_id, commit);
				// <LinkHashes<T>>::insert(tx_hash, tx_details.tx_id);
			}
		}
		let mut commit = <Commits<T>>::get(tx_details.tx_id).unwrap_or_default();
		commit.push(TxCommits {
			tx_type: tx_details.tx_type.clone(),
			tx_hash,
			tx_cid: tx_details.tx_storage.tx_cid.clone(),
			tx_link: tx_details.tx_link.tx_hash,
			block: tx_details.block,
			commit: tx_request,
		});
		<Commits<T>>::insert(tx_details.tx_id, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub enum RequestOf {
	Create,
	Update,
	Status,
	Verify,
}

impl Default for RequestOf {
	fn default() -> Self {
		RequestOf::Create
	}
}

// #[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
// #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub enum TypeOf {
	Entity,
	Space,
	Schema(SchemaOf),
	Journal,
	Stream,
	Link,
}

impl Default for TypeOf {
	fn default() -> Self {
		TypeOf::Stream
	}
}

impl TypeOf {
	pub fn is_valid(incoming: &TypeOf) -> bool {
		if incoming == &TypeOf::Entity
			|| incoming == &TypeOf::Space
			|| incoming == &TypeOf::Schema(SchemaOf::Journal)
			|| incoming == &TypeOf::Schema(SchemaOf::Stream)
			|| incoming == &TypeOf::Schema(SchemaOf::Link)
			|| incoming == &TypeOf::Journal
			|| incoming == &TypeOf::Stream
			|| incoming == &TypeOf::Link
		{
			true
		} else {
			false
		}
	}
}

// #[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
// #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub enum SchemaOf {
	Journal,
	Stream,
	Link,
}

impl Default for SchemaOf {
	fn default() -> Self {
		SchemaOf::Stream
	}
}

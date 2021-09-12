// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use cord_primitives::{SidOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod streams;
pub mod weights;

pub use crate::streams::*;
pub mod utils;

use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of an entity.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
	/// Hash of the transaction.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a entity controller.
	pub type CordAccountOf<T> = pallet_schema::CordAccountOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// streams stored on chain.
	/// It maps from stream Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, StreamDetails<T>>;

	/// stream commit details stored on chain.
	/// It maps from a stream Id to a vector of commit details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<StreamCommit<T>>>;

	/// stream links stored on chain.
	/// It maps from a stream Id to a vector of links.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<StreamLink<T>>>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id.
	#[pallet::storage]
	#[pallet::getter(fn hashes)]
	pub type Hashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, controller\]
		TxAdd(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entityhas been created.
		/// \[entity identifier, controller\]
		TxUpdate(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		TxStatus(IdOf<T>, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Hash and ID are the same
		SameIdAndHash,
		/// Transaction idenfier is not unique
		StreamAlreadyExists,
		/// Transaction idenfier not found
		StreamNotFound,
		/// Transaction idenfier marked inactive
		StreamRevoked,
		/// Invalid SID encoding.
		InvalidStoreIdEncoding,
		/// SID already anchored
		StoreIdAlreadyMapped,
		/// no status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		/// Stream link does not exist
		StreamLinkNotFound,
		/// Linked stream is revoked
		StreamLinkRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its controller.
		///
		/// * origin: the identifier of the stream controller
		/// * tx_id: unique identifier of the incoming stream.
		/// * tx_hash: hash of the incoming stream.
		/// * tx_sid: SID of the incoming  stream.
		/// * tx_schema: stream schema.
		/// * tx_link: stream link.
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_sid: Option<SidOf>,
			tx_schema: Option<IdOf<T>>,
			tx_link: Option<IdOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != tx_id, Error::<T>::SameIdAndHash);
			//check store Id encoding
			if let Some(ref tx_sid) = tx_sid {
				ensure!(StreamDetails::<T>::check_sid(&tx_sid), Error::<T>::InvalidStoreIdEncoding);
			}
			ensure!(!<Streams<T>>::contains_key(&tx_id), Error::<T>::StreamAlreadyExists);
			//check stream schema status
			if let Some(tx_schema) = tx_schema {
				pallet_schema::SchemaDetails::<T>::schema_status(tx_schema, controller.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}
			//check link status
			if let Some(ref tx_link) = tx_link {
				let link = <Streams<T>>::get(&tx_link).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!link.revoked, Error::<T>::StreamLinkRevoked);
				StreamLink::<T>::link_tx(
					&tx_link,
					StreamLink { tx_id: tx_id.clone(), controller: controller.clone() },
				)?;
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&tx_id,
				StreamCommit {
					tx_hash: tx_hash.clone(),
					tx_sid: tx_sid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::Genesis,
				},
			)?;

			<Hashes<T>>::insert(&tx_hash, &tx_id);

			<Streams<T>>::insert(
				&tx_id,
				StreamDetails {
					tx_hash: tx_hash.clone(),
					tx_sid,
					ptx_sid: None,
					tx_schema,
					tx_link,
					controller: controller.clone(),
					block: block_number,
					revoked: false,
				},
			);
			Self::deposit_event(Event::TxAdd(tx_id, tx_hash, controller));

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identifier of the stream controller
		/// * tx_id: unique identifier of the incoming stream.
		/// * tx_hash: hash of the incoming stream.
		/// * tx_sid: storage Id of the incoming stream.
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_sid: Option<SidOf>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != tx_id, Error::<T>::SameIdAndHash);

			let tx_prev = <Streams<T>>::get(&tx_id).ok_or(Error::<T>::StreamNotFound)?;
			//check sid encoding
			if let Some(ref tx_sid) = tx_sid {
				ensure!(
					tx_sid != tx_prev.tx_sid.as_ref().unwrap(),
					Error::<T>::StoreIdAlreadyMapped
				);
				ensure!(StreamDetails::<T>::check_sid(&tx_sid), Error::<T>::InvalidStoreIdEncoding);
			}
			ensure!(!tx_prev.revoked, Error::<T>::StreamRevoked);
			ensure!(tx_prev.controller == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&tx_id,
				StreamCommit {
					tx_hash: tx_hash.clone(),
					tx_sid: tx_sid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::Update,
				},
			)?;

			<Hashes<T>>::insert(&tx_hash, &tx_id);

			<Streams<T>>::insert(
				&tx_id,
				StreamDetails {
					tx_hash: tx_hash.clone(),
					tx_sid,
					ptx_sid: tx_prev.tx_sid,
					controller: updater.clone(),
					block: block_number,
					..tx_prev
				},
			);

			Self::deposit_event(Event::TxUpdate(tx_id, tx_hash, updater));

			Ok(())
		}
		/// Update the status of the stream
		///
		/// * origin: the identifier of the stream controller
		/// * tx_id: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(0)]
		pub fn set_status(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let tx_status = <Streams<T>>::get(&tx_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(tx_status.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&tx_id,
				StreamCommit {
					tx_hash: tx_status.tx_hash.clone(),
					tx_sid: tx_status.tx_sid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::StatusChange,
				},
			)?;

			<Streams<T>>::insert(
				&tx_id,
				StreamDetails { block: block_number, revoked: status, ..tx_status },
			);

			Self::deposit_event(Event::TxStatus(tx_id, updater));

			Ok(())
		}
	}
}

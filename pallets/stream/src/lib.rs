// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use cord_primitives::{IdentifierOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod streams;
pub mod weights;

pub use crate::streams::*;

use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of a Stream.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
	/// Hash of the Stream.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the controller.
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
	// #[pallet::generate_storage_info]
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
	/// It maps from a stream Id to a vector of stream links.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<StreamLink<T>>>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn hashes)]
	pub type Hashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream identifier, controller\]
		TxAdd(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A stream has been updated.
		/// \[stream identifier, controller\]
		TxUpdate(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A stream status has been changed.
		/// \[stream identifier\]
		TxStatus(IdOf<T>, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Hash and Identifier are the same
		SameIdentifierAndHash,
		/// Stream idenfier is not unique
		StreamAlreadyAnchored,
		/// Stream idenfier not found
		StreamNotFound,
		/// Stream idenfier marked inactive
		StreamRevoked,
		/// Invalid CID encoding.
		// InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyAnchored,
		/// No stream status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Stream link does not exist
		StreamLinkNotFound,
		/// Stream Link is revoked
		StreamLinkRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its controller.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * hash: hash of the incoming stream.
		/// * cid: CID of the incoming  stream.
		/// * schema: stream schema.
		/// * link: stream link.
		#[pallet::weight(682_886_000 + T::DbWeight::get().reads_writes(3, 4))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			creator: CordAccountOf<T>,
			hash: HashOf<T>,
			cid: Option<IdentifierOf>,
			schema: Option<IdOf<T>>,
			link: Option<IdOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(hash != identifier, Error::<T>::SameIdentifierAndHash);
			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);
			if let Some(ref schema) = schema {
				pallet_schema::SchemaDetails::<T>::schema_status(schema, creator.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}
			if let Some(ref link) = link {
				let links = <Streams<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!links.revoked, Error::<T>::StreamLinkRevoked);
				StreamLink::<T>::link_tx(
					&link,
					StreamLink { identifier: identifier.clone(), creator: creator.clone() },
				)?;
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit {
					hash: hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::Genesis,
				},
			)?;

			<Hashes<T>>::insert(&hash, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamDetails {
					hash: hash.clone(),
					cid,
					parent_cid: None,
					schema,
					link,
					creator: creator.clone(),
					block: block_number,
					revoked: false,
				},
			);
			Self::deposit_event(Event::TxAdd(identifier, hash, creator));

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * hash: hash of the incoming stream.
		/// * cid: storage Id of the incoming stream.
		#[pallet::weight(471_532_000 + T::DbWeight::get().reads_writes(1, 3))]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			hash: HashOf<T>,
			cid: Option<IdentifierOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(hash != identifier, Error::<T>::SameIdentifierAndHash);

			let tx_prev = <Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			if let Some(ref cid) = cid {
				ensure!(cid != tx_prev.cid.as_ref().unwrap(), Error::<T>::CidAlreadyAnchored);
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(!tx_prev.revoked, Error::<T>::StreamRevoked);
			ensure!(tx_prev.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit {
					hash: hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::Update,
				},
			)?;

			<Hashes<T>>::insert(&hash, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamDetails {
					hash: hash.clone(),
					cid,
					parent_cid: tx_prev.cid,
					creator: updater.clone(),
					block: block_number,
					..tx_prev
				},
			);

			Self::deposit_event(Event::TxUpdate(identifier, hash, updater));

			Ok(())
		}
		/// Update the status of the stream
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(121_825_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn set_status(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let tx_status = <Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(tx_status.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit {
					hash: tx_status.hash.clone(),
					cid: tx_status.cid.clone(),
					block: block_number.clone(),
					commit: StreamCommitOf::StatusChange,
				},
			)?;

			<Streams<T>>::insert(
				&identifier,
				StreamDetails { block: block_number, revoked: status, ..tx_status },
			);

			Self::deposit_event(Event::TxStatus(identifier, updater));

			Ok(())
		}
	}
}

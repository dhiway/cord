// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
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

use cord_primitives::{CidOf, IdentifierOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str};

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

	/// Hash of the Stream.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the controller.
	pub type CordAccountOf<T> = pallet_schema::CordAccountOf<T>;
	// stream identifier prefix.
	pub const STREAM_IDENTIFIER_PREFIX: u16 = 4042;

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
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// streams stored on chain.
	/// It maps from stream Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, StreamDetails<T>>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn streamid)]
	pub type StreamId<T> = StorageMap<_, Blake2_128Concat, IdentifierOf, HashOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream identifier, controller\]
		Anchor(IdentifierOf, HashOf<T>, CordAccountOf<T>),
		/// A stream has been updated.
		/// \[stream identifier, controller\]
		Update(IdentifierOf, HashOf<T>, CordAccountOf<T>),
		/// A stream status has been changed.
		/// \[stream identifier\]
		Status(IdentifierOf, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stream idenfier is not unique
		StreamAlreadyAnchored,
		/// Stream idenfier not found
		StreamNotFound,
		/// Stream idenfier marked inactive
		StreamRevoked,
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
		/// * updater: creator (issuer) of the stream.
		/// * stream_hash: hash of the incoming stream.
		/// * holder: \[OPTIONAL\] holder(recipient) of the stream.
		/// * schema: \[OPTIONAL\] stream schema.
		/// * cid: \[OPTIONAL\] CID of the incoming  stream.
		/// * link: \[OPTIONAL\]stream link.
		#[pallet::weight(470_952_000 + T::DbWeight::get().reads_writes(4, 2))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			creator: CordAccountOf<T>,
			stream_hash: HashOf<T>,
			holder: Option<CordAccountOf<T>>,
			schema: Option<IdentifierOf>,
			cid: Option<CidOf>,
			link: Option<IdentifierOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			pallet_schema::SchemaDetails::<T>::is_valid_identifier(
				&identifier,
				STREAM_IDENTIFIER_PREFIX,
			)?;
			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid_cid(cid)?;
			}

			ensure!(!<StreamId<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);
			if let Some(ref schema) = schema {
				pallet_schema::SchemaDetails::<T>::schema_status(schema, creator.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}

			if let Some(ref link) = link {
				let link_hash = <StreamId<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
				let link_details =
					<Streams<T>>::get(&link_hash).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!link_details.revoked, Error::<T>::StreamLinkRevoked);
			}

			<StreamId<T>>::insert(&identifier, &stream_hash);

			<Streams<T>>::insert(
				&stream_hash,
				StreamDetails {
					stream_id: identifier.clone(),
					creator: creator.clone(),
					holder,
					schema,
					link,
					parent: None,
					cid,
					revoked: false,
				},
			);
			Self::deposit_event(Event::Anchor(identifier, stream_hash, creator));

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * updater: controller of the stream.
		/// * hash: hash of the incoming stream.
		/// * cid: storage Id of the incoming stream.
		#[pallet::weight(171_780_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			updater: CordAccountOf<T>,
			stream_hash: HashOf<T>,
			cid: Option<CidOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid_cid(cid)?;
			}

			let tx_prev_hash = <StreamId<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			let tx_prev_details =
				<Streams<T>>::get(&tx_prev_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!tx_prev_details.revoked, Error::<T>::StreamRevoked);
			ensure!(tx_prev_details.creator == updater, Error::<T>::UnauthorizedOperation);

			<StreamId<T>>::insert(&identifier, &stream_hash);

			<Streams<T>>::insert(
				&stream_hash,
				StreamDetails {
					stream_id: identifier.clone(),
					creator: updater.clone(),
					parent: Some(tx_prev_hash),
					cid,
					..tx_prev_details
				},
			);

			Self::deposit_event(Event::Update(identifier, stream_hash, updater));

			Ok(())
		}
		/// Update the status of the stream
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the stream.
		/// * updater: controller of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn status(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			updater: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let stream_hash = <StreamId<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			let tx_status = <Streams<T>>::get(&stream_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(tx_status.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.creator == updater, Error::<T>::UnauthorizedOperation);

			<Streams<T>>::insert(&stream_hash, StreamDetails { revoked: status, ..tx_status });

			Self::deposit_event(Event::Status(identifier, updater));

			Ok(())
		}
	}
}

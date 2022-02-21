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

pub mod products;
pub mod weights;

pub use crate::products::*;

use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of a Product.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
	/// Hash of the Product.
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

	/// products stored on chain.
	/// It maps from product Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn products)]
	pub type Products<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, ProductDetails<T>>;

	/// product commit details stored on chain.
	/// It maps from a stream Id to a vector of commit details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<ProductCommit<T>>>;

	/// product links stored on chain.
	/// It maps from a stream Id to a vector of stream links.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<ProductLink<T>>>;

	/// product ratings stored on chain.
	/// It maps from a stream Id to a vector of stream links.
	#[pallet::storage]
	#[pallet::getter(fn ratings)]
	pub type Ratings<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<u32>>;

	/// product hashes stored on chain.
	/// It maps from a product hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn producthash)]
	pub type ProductHash<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream identifier, controller\]
		TxCreate(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A new product listing has been created.
		/// \[listing identifier, controller\]
		TxList(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A new order has been created.
		/// \[order identifier, controller\]
		TxOrder(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A stream has been updated.
		/// \[stream identifier, controller\]
		TxUpdate(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An order status has been changed.
		/// \[order identifier\]
		TxReturn(IdOf<T>, CordAccountOf<T>),
		/// An order rating has been changed.
		/// \[order identifier\]
		TxRating(IdOf<T>, CordAccountOf<T>),
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
		/// Product idenfier is not unique
		ProductAlreadyCreated,
		/// List idenfier is not unique
		ProductAlreadyListed,
		/// Order idenfier is not unique
		OrderAlreadyCreated,
		/// Product idenfier not found
		ProductNotFound,
		/// Product idenfier marked inactive
		ProductStatus,
		/// CID already anchored
		CidAlreadyAnchored,
		/// No status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Product link does not exist
		ProductLinkNotFound,
		/// Product Availability status
		ProductNotAvailable,
		/// Order idenfier is not found
		OrderNotCreated,
		/// Order idenfier is not found
		OrderNotFound,
		/// Product Link is status
		ProductLinkstatus,
		/// Invalid Rating, Should be between 1 & 10
		InvalidRating,
		/// Invalid Listing
		ListingNotFound,
		/// More than Max Ratings
		TooManyRatings,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new product and associates it with its controller.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * hash: hash of the incoming stream.
		/// * cid: CID of the incoming  stream.
		/// * schema: stream schema.
		#[pallet::weight(470_952_000 + T::DbWeight::get().reads_writes(3, 4))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			creator: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			cid: Option<IdentifierOf>,
			schema: Option<IdOf<T>>,
			quantity: Option<u32>
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != identifier, Error::<T>::SameIdentifierAndHash);
			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(!<Products<T>>::contains_key(&identifier), Error::<T>::ProductAlreadyCreated);
			if let Some(ref schema) = schema {
				pallet_schema::SchemaDetails::<T>::schema_status(schema, creator.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: ProductCommitOf::Create,
				},
			)?;

			<ProductHash<T>>::insert(&tx_hash, &identifier);

			<Products<T>>::insert(
				&identifier,
				ProductDetails {
					tx_hash: tx_hash.clone(),
					cid,
					parent_cid: None,
					store_id: None,
					schema,
					creator: creator.clone(),
					link: None,
					price: None,
					rating: None,
					block: block_number,
					status: true,
					quantity,
				},
			);
			Self::deposit_event(Event::TxCreate(identifier, tx_hash, creator));

			Ok(())
		}

		/// Create a new product listing and associates it with its controller.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * hash: hash of the incoming stream.
		/// * cid: CID of the incoming  stream.
		/// * schema: stream schema.
		#[pallet::weight(470_952_000 + T::DbWeight::get().reads_writes(3, 4))]
		pub fn list(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			creator: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			store_id: Option<IdOf<T>>,
			price: Option<u32>,
			quantity: Option<u32>,
			cid: Option<IdentifierOf>,
			schema: Option<IdOf<T>>,
			link: Option<IdOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(!<Products<T>>::contains_key(&identifier), Error::<T>::ProductAlreadyListed);

			let block_number = <frame_system::Pallet<T>>::block_number();

			if let Some(ref link) = link {
				let links = <Products<T>>::get(&link).ok_or(Error::<T>::ProductLinkNotFound)?;
				ensure!(links.status, Error::<T>::ProductNotAvailable);
				pallet_schema::SchemaDetails::<T>::schema_status(
					&links.schema.unwrap(),
					creator.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
				ProductLink::<T>::link_tx(
					&link,
					ProductLink {
						identifier: identifier.clone(),
						store_id: store_id.clone().unwrap(),
						creator: creator.clone(),
					},
				)?;
			}

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_hash.clone(),
					cid: cid.clone(),
					block: block_number,
					commit: ProductCommitOf::List,
				},
			)?;

			<ProductHash<T>>::insert(&tx_hash, &identifier);

			<Products<T>>::insert(
				&identifier,
				ProductDetails {
					tx_hash: tx_hash.clone(),
					cid,
					parent_cid: None,
					store_id,
					schema,
					link,
					price,
					rating: None,
					quantity,
					creator: creator.clone(),
					block: block_number.clone(),
					status: true,
				},
			);
			Self::deposit_event(Event::TxList(identifier, tx_hash, creator));

			Ok(())
		}

		/// Create a new product order.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * tx_hash: hash of the incoming stream.
		/// * cid: storage Id of the incoming stream.
		/// * link: linit to the product
		#[pallet::weight(171_780_000 + T::DbWeight::get().reads_writes(1, 3))]
		pub fn order(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			buyer: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			store_id: Option<IdOf<T>>,
			price: Option<u32>,
			quantity: Option<u32>,
			cid: Option<IdentifierOf>,
			schema: Option<IdOf<T>>,
			link: Option<IdOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != identifier, Error::<T>::SameIdentifierAndHash);
			ensure!(!<Products<T>>::contains_key(&identifier), Error::<T>::OrderAlreadyCreated);

			if let Some(ref cid) = cid {
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			if let Some(ref link) = link {
				let links = <Products<T>>::get(&link).ok_or(Error::<T>::ProductLinkNotFound)?;
				ensure!(links.status, Error::<T>::ProductNotAvailable);
				ProductLink::<T>::link_tx(
					&link,
					ProductLink {
						identifier: identifier.clone(),
						store_id: links.store_id.clone().unwrap(),
						creator: buyer.clone(),
					},
				)?;
			}

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_hash.clone(),
					cid: cid.clone(),
					block: block_number,
					commit: ProductCommitOf::Order,
				},
			)?;

			<ProductHash<T>>::insert(&tx_hash, &identifier);

			<Products<T>>::insert(
				&identifier,
				ProductDetails {
					tx_hash: tx_hash.clone(),
					cid,
					parent_cid: None,
					store_id,
					schema,
					link,
					price,
					rating: None,
					quantity,
					creator: buyer.clone(),
					block: block_number.clone(),
					status: true,
				},
			);
			Self::deposit_event(Event::TxOrder(identifier, tx_hash, buyer));

			Ok(())
		}

		/// Updates the stream information.
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * hash: hash of the incoming stream.
		/// * cid: storage Id of the incoming stream.
		#[pallet::weight(171_780_000 + T::DbWeight::get().reads_writes(1, 3))]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			cid: Option<IdentifierOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != identifier, Error::<T>::SameIdentifierAndHash);

			let tx_prev = <Products<T>>::get(&identifier).ok_or(Error::<T>::ProductNotFound)?;
			if let Some(ref cid) = cid {
				ensure!(cid != tx_prev.cid.as_ref().unwrap(), Error::<T>::CidAlreadyAnchored);
				pallet_schema::SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(tx_prev.status, Error::<T>::ProductStatus);
			ensure!(tx_prev.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: ProductCommitOf::Update,
				},
			)?;

			<ProductHash<T>>::insert(&tx_hash, &identifier);

			<Products<T>>::insert(
				&identifier,
				ProductDetails {
					tx_hash: tx_hash.clone(),
					cid,
					parent_cid: tx_prev.cid,
					creator: updater.clone(),
					block: block_number,
					..tx_prev
				},
			);

			Self::deposit_event(Event::TxUpdate(identifier, tx_hash, updater));

			Ok(())
		}
		/// Update the status of the order
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn order_return(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			buyer: CordAccountOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let tx_status = <Products<T>>::get(&identifier).ok_or(Error::<T>::OrderNotFound)?;
			ensure!(tx_status.creator == buyer, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_status.tx_hash.clone(),
					cid: tx_status.cid.clone(),
					block: block_number.clone(),
					commit: ProductCommitOf::Return,
				},
			)?;

			<Products<T>>::insert(
				&identifier,
				ProductDetails { block: block_number, status: false, ..tx_status },
			);

			Self::deposit_event(Event::TxReturn(identifier, buyer));

			Ok(())
		}

		/// Update the status of the order
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn rating(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			buyer: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			store_id: Option<IdOf<T>>,
			price: Option<u32>,
			cid: Option<IdentifierOf>,
			schema: Option<IdOf<T>>,
			link: Option<IdOf<T>>,
			rating: u32,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(tx_hash != identifier, Error::<T>::SameIdentifierAndHash);

			let min: u32 = 1;
			let max: u32 = 5;
			ensure!(rating > min && rating <= max, Error::<T>::InvalidRating);

			let tx_order =
				<Products<T>>::get(&link.as_ref().unwrap()).ok_or(Error::<T>::OrderNotFound)?;
			ensure!(tx_order.creator == buyer, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();
			let mut ratings =
				<Ratings<T>>::get(tx_order.link.as_ref().unwrap()).unwrap_or_default();
			ratings.push(rating);
			<Ratings<T>>::insert(tx_order.link.as_ref().unwrap(), ratings);

			ProductCommit::<T>::store_tx(
				&link.as_ref().unwrap(),
				ProductCommit {
					tx_hash: tx_hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: ProductCommitOf::Rating,
				},
			)?;
			<Products<T>>::insert(
				&identifier,
				ProductDetails {
					tx_hash: tx_hash.clone(),
					cid,
					parent_cid: None,
					store_id,
					schema,
					link,
					price,
					rating: Some(rating),
					creator: buyer.clone(),
					block: block_number.clone(),
					status: true,
					quantity: None,
				},
			);
			Self::deposit_event(Event::TxRating(link.unwrap(), buyer));

			Ok(())
		}

		/// Update the status of the stream
		///
		/// * origin: the identity of the stream controller.
		/// * identifier: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn set_status(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let tx_status = <Products<T>>::get(&identifier).ok_or(Error::<T>::ProductNotFound)?;
			ensure!(tx_status.status != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			ProductCommit::<T>::store_tx(
				&identifier,
				ProductCommit {
					tx_hash: tx_status.tx_hash.clone(),
					cid: tx_status.cid.clone(),
					block: block_number.clone(),
					commit: ProductCommitOf::StatusChange,
				},
			)?;

			<Products<T>>::insert(
				&identifier,
				ProductDetails { block: block_number, status, ..tx_status },
			);

			Self::deposit_event(Event::TxStatus(identifier, updater));

			Ok(())
		}
	}
}

// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod links;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::links::*;
pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type StreamLinkHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = pallet_schema::SchemaHashOf<T>;
	/// Type of link transaction owner identifier.
	pub type StreamHashOf<T> = pallet_stream::StreamHashOf<T>;
	/// Type of link owner identifier.
	pub type StreamLinkCreatorOf<T> = pallet_schema::SchemaOwnerOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Stream link CID
	pub type StreamLinkCidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_stream::Config {
		type EnsureOrigin: EnsureOrigin<Success = StreamLinkCreatorOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Linked streams stored on chain.
	/// It maps from a link hash to the details.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, StreamLinkHashOf<T>, StreamLinkDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream link has been created.
		/// \[creator identifier, link hash, link cid\]
		StreamLinkAnchored(StreamLinkCreatorOf<T>, StreamLinkHashOf<T>, StreamLinkCidOf),
		/// A stream link has been revoked.
		/// \[revoker identifier, link hash\]
		StreamLinkRevoked(StreamLinkCreatorOf<T>, StreamLinkHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a stream link with the same hash.
		StreamLinkAlreadyAnchored,
		/// The stream has already been revoked.
		StreamLinkAlreadyRevoked,
		/// No credential on chain matching the content hash.
		StreamLinkNotFound,
		/// Only when the revoker is not the owner.
		UnauthorizedRevocation,
		/// Invalid Stream Link Cid encoding.
		InvalidCidEncoding,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream link.
		///
		///
		/// * origin: the identifier of the owner
		/// * stream_link_hash: the hash of the stream link. It has to be unique
		/// * schema_hash: the hash of the schema used to create link
		/// * stream_link_cid: CID of the stream link content
		/// * stream_hash: hash of the linked stream
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			stream_link_hash: StreamLinkHashOf<T>,
			stream_link_cid: StreamLinkCidOf,
			stream_hash: StreamHashOf<T>,
			schema_hash: SchemaHashOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Links<T>>::contains_key(&stream_link_hash),
				Error::<T>::StreamLinkAlreadyAnchored
			);
			let schema =
				<pallet_schema::Schemas<T>>::get(schema_hash).ok_or(pallet_schema::Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == creator, pallet_schema::Error::<T>::SchemaNotDelegated);
			let stream =
				<pallet_stream::Streams<T>>::get(stream_hash).ok_or(pallet_stream::Error::<T>::StreamNotFound)?;
			ensure!(!stream.revoked, pallet_stream::Error::<T>::StreamRevoked);

			let cid_base = str::from_utf8(&stream_link_cid).unwrap();
			ensure!(
				pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base),
				Error::<T>::InvalidCidEncoding
			);
			log::debug!("Anchor Stream Link");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Links<T>>::insert(
				&stream_link_hash,
				StreamLinkDetails {
					schema_hash,
					creator: creator.clone(),
					stream_link_cid: stream_link_cid.clone(),
					stream_hash,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::StreamLinkAnchored(creator, stream_link_hash, stream_link_cid));

			Ok(())
		}

		/// Revoke an existing stream link
		///
		/// The revoker must be the creator of the stream link
		/// * origin: the identifier of the revoker
		/// * stream_link_hash: the hash of the stream link to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, stream_link_hash: StreamLinkHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let link = <Links<T>>::get(&stream_link_hash).ok_or(Error::<T>::StreamLinkNotFound)?;
			ensure!(link.creator == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!link.revoked, Error::<T>::StreamLinkAlreadyRevoked);

			log::debug!("Revoking Stream Link");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Links<T>>::insert(
				&stream_link_hash,
				StreamLinkDetails {
					block_number,
					revoked: true,
					..link
				},
			);
			Self::deposit_event(Event::StreamLinkRevoked(revoker, stream_link_hash));

			Ok(())
		}
	}
}

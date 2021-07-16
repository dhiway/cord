// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod digests;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::digests::*;
pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type DigestHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of link transaction owner identifier.
	pub type StreamHashOf<T> = pallet_stream::StreamHashOf<T>;
	/// Type of link owner identifier.
	pub type DigestCreatorOf<T> = pallet_stream::StreamCreatorOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Stream link CID
	pub type DigestCidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_stream::Config {
		type EnsureOrigin: EnsureOrigin<Success = DigestCreatorOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Digestsstored on chain.
	/// It maps from a digest hash to the stream details.
	#[pallet::storage]
	#[pallet::getter(fn digests)]
	pub type Digests<T> = StorageMap<_, Blake2_128Concat, DigestHashOf<T>, DigestDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new digest has been created.
		/// \[creator identifier, Digest hash\]
		DigestAnchored(DigestCreatorOf<T>, DigestHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a stream link with the same hash.
		DigestAlreadyAnchored,
		/// No credential on chain matching the content hash.
		DigestNotFound,
		/// Invalid Stream Link Cid encoding.
		InvalidCidEncoding,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream link.
		///
		///
		/// * origin: the identifier of the owner
		/// * digest_hash: the hash of the stream link. It has to be unique
		/// * stream_hash: hash of the linked stream
		/// * digest_cid: \[OPTIONAL\] CID of the stream link content
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			digest_hash: DigestHashOf<T>,
			stream_hash: StreamHashOf<T>,
			digest_cid: Option<DigestCidOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Digests<T>>::contains_key(&digest_hash),
				Error::<T>::DigestAlreadyAnchored
			);
			let stream =
				<pallet_stream::Streams<T>>::get(stream_hash).ok_or(pallet_stream::Error::<T>::StreamNotFound)?;
			ensure!(!stream.revoked, pallet_stream::Error::<T>::StreamRevoked);

			if let Some(ref digest_cid) = digest_cid {
				let cid_base = str::from_utf8(&digest_cid).unwrap();
				ensure!(
					cid_base.len() <= 62
						&& (pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base)),
					Error::<T>::InvalidCidEncoding
				);
			}
			log::debug!("Anchor Digest");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Digests<T>>::insert(
				&digest_hash,
				DigestDetails {
					creator: creator.clone(),
					stream_hash,
					digest_cid,
					block_number,
				},
			);

			Self::deposit_event(Event::DigestAnchored(creator, digest_hash));

			Ok(())
		}
	}
}

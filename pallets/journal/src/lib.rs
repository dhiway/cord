// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod journal_streams;
pub mod weights;

pub use crate::journal_streams::*;
pub use pallet::*;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;
// #[cfg(test)]
// mod tests;
use crate::weights::WeightInfo;
// pub use crate::{marks::*, pallet::*, weights::WeightInfo};
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a journal stream hash.
	pub type JournalStreamHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = pallet_schema::SchemaHashOf<T>;
	/// Type of an creator identifier.
	pub type JournalStreamCreatorOf<T> = pallet_schema::SchemaOwnerOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Type for a Content Identifier (CID)
	pub type JournalStreamCidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type EnsureOrigin: EnsureOrigin<Success = JournalStreamCreatorOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Journal of streams stored on chain.
	/// It maps from a journal stream hash to the details.
	#[pallet::storage]
	#[pallet::getter(fn journal)]
	pub type Journal<T> = StorageMap<_, Blake2_128Concat, JournalStreamHashOf<T>, JournalStreamDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been added to the journal.
		/// \[creator identifier, stream hash, stream cid\]
		StreamAnchored(JournalStreamCreatorOf<T>, JournalStreamHashOf<T>, JournalStreamCidOf),
		/// A stream has been revoked from the jounal.
		/// \[revoker identifier, stream hash\]
		StreamRevoked(JournalStreamCreatorOf<T>, JournalStreamHashOf<T>),
		/// A previously revoked stream has been restored.
		/// \[restorer identifier, stream hash\]
		StreamRestored(JournalStreamCreatorOf<T>, JournalStreamHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a stream with the same hash.
		StreamAlreadyAnchored,
		/// The stream has already been revoked.
		StreamAlreadyRevoked,
		/// No stream on chain matching the content hash.
		StreamNotFound,
		/// Only when the revoker is not the creator.
		UnauthorizedRevocation,
		/// Only when the restorer is not the creator.
		UnauthorizedRestore,
		/// only when trying to restore an active stream.
		StreamStillActive,
		/// Invalid Stream CID encoding.
		InvalidCidEncoding,
		/// Stream have been revoked.
		RevokedStream,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new journal stream.
		///
		///
		/// * origin: the identifier of the issuer
		/// * stream_hash: the hash of the steam. It has to be unique.
		/// * schema_hash: the hash of the schema used for this stream
		/// * stream_cid: CID of the stream content
		/// * parent_cid: CID of the linked parent stream
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			stream_hash: JournalStreamHashOf<T>,
			schema_hash: SchemaHashOf<T>,
			stream_cid: JournalStreamCidOf,
			parent_cid: Option<JournalStreamCidOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema =
				<pallet_schema::Schemas<T>>::get(schema_hash).ok_or(pallet_schema::Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == creator, pallet_schema::Error::<T>::SchemaNotDelegated);

			ensure!(
				!<Journal<T>>::contains_key(&stream_hash),
				Error::<T>::StreamAlreadyAnchored
			);

			let cid_base = str::from_utf8(&stream_cid).unwrap();
			ensure!(
				pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base),
				Error::<T>::InvalidCidEncoding
			);

			if let Some(ref parent_cid) = parent_cid {
				let pcid_base = str::from_utf8(&parent_cid).unwrap();
				ensure!(
					pallet_schema::utils::is_base_32(pcid_base) || pallet_schema::utils::is_base_58(pcid_base),
					Error::<T>::InvalidCidEncoding
				);
			}

			log::debug!("Anchor Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Journal<T>>::insert(
				&stream_hash,
				JournalStreamDetails {
					schema_hash,
					creator: creator.clone(),
					stream_cid: stream_cid.clone(),
					parent_cid,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::StreamAnchored(creator, stream_hash, stream_cid));

			Ok(())
		}

		/// Revoke an existing stream
		///
		/// The revoker must be the creator of the stream
		/// * origin: the identifier of the revoker
		/// * stream_hash: the hash of the stream to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, stream_hash: JournalStreamHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let stream = <Journal<T>>::get(&stream_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream.creator == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!stream.revoked, Error::<T>::StreamAlreadyRevoked);

			log::debug!("Revoking Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Journal<T>>::insert(
				&stream_hash,
				JournalStreamDetails {
					block_number,
					revoked: true,
					..stream
				},
			);
			Self::deposit_event(Event::StreamRevoked(revoker, stream_hash));

			Ok(())
		}
		// Restore a revoked stream.
		///
		/// The restorer must be the creator of the stream being restored
		/// * origin: the identifier of the restorer
		/// * stream_hash: the hash of the stream to restore
		#[pallet::weight(0)]
		pub fn restore(origin: OriginFor<T>, stream_hash: JournalStreamHashOf<T>) -> DispatchResult {
			let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let stream = <Journal<T>>::get(&stream_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream.revoked, Error::<T>::StreamStillActive);
			ensure!(stream.creator == restorer, Error::<T>::UnauthorizedRestore);

			log::debug!("Restoring Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Journal<T>>::insert(
				&stream_hash,
				JournalStreamDetails {
					block_number,
					revoked: false,
					..stream
				},
			);
			Self::deposit_event(Event::StreamRestored(restorer, stream_hash));

			Ok(())
		}
	}
}

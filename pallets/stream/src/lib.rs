// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod streams;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::streams::*;
pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type StreamHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = pallet_schema::SchemaHashOf<T>;
	/// Type of cred parent stream identifier.
	pub type JournalStreamHashOf<T> = pallet_journal::JournalStreamHashOf<T>;
	/// Type of cred owner identifier.
	pub type StreamCreatorOf<T> = pallet_schema::SchemaOwnerOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type StreamCidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_journal::Config {
		type EnsureOrigin: EnsureOrigin<Success = StreamCreatorOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Streams stored on chain.
	/// It maps from a stream hash to the details.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> = StorageMap<_, Blake2_128Concat, StreamHashOf<T>, StreamDetails<T>>;

	/// Streams linked to Journal Streams stored on chain.
	/// It maps from a journal stream hash to a vector of stream hashes.
	#[pallet::storage]
	#[pallet::getter(fn journalstreams)]
	pub type JournalStreams<T> = StorageMap<_, Blake2_128Concat, JournalStreamHashOf<T>, Vec<StreamHashOf<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[creator identifier, stream hash, stream cid\]
		StreamAnchored(StreamCreatorOf<T>, StreamHashOf<T>, StreamCidOf),
		/// A stream has been revoked.
		/// \[revoker identifier, stream hash\]
		StreamRevoked(StreamCreatorOf<T>, StreamHashOf<T>),
		/// A stream has been restored.
		/// \[restorer identifier, stream hash\]
		StreamRestored(StreamCreatorOf<T>, StreamHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a stream with the same hash stored on
		/// chain.
		StreamAlreadyAnchored,
		/// The stream has already been revoked.
		StreamAlreadyRevoked,
		/// No stream on chain matching the content hash.
		StreamNotFound,
		/// The schema hash does not match the schema specified
		SchemaMismatch,
		/// Only when the revoker is not the creator.
		UnauthorizedRevocation,
		/// Only when the restorer is not the creator.
		UnauthorizedRestore,
		/// only when trying to restore an active stream.
		StreamStillActive,
		/// Invalid Stream Cid encoding.
		InvalidCidEncoding,
		/// schema not authorised.
		SchemaNotDelegated,
		/// stream revoked
		StreamRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream.
		///
		///
		/// * origin: the identifier of the creator
		/// * stream_hash: the hash of the conten to attest. It has to be unique
		/// * schema_hash: the hash of the schema used for this stream
		/// * stream_cid: CID of the stream content
		/// * journal_stream_hash: Hash of the journal stream
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			stream_hash: StreamHashOf<T>,
			stream_cid: StreamCidOf,
			schema_hash: SchemaHashOf<T>,
			journal_stream_hash: JournalStreamHashOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Streams<T>>::contains_key(&stream_hash),
				Error::<T>::StreamAlreadyAnchored
			);

			let schema =
				<pallet_schema::Schemas<T>>::get(schema_hash).ok_or(pallet_schema::Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == creator, pallet_schema::Error::<T>::SchemaNotDelegated);

			let journal_stream = <pallet_journal::Journal<T>>::get(journal_stream_hash)
				.ok_or(pallet_journal::Error::<T>::StreamNotFound)?;
			ensure!(!journal_stream.revoked, pallet_journal::Error::<T>::RevokedStream);

			let cid_base = str::from_utf8(&stream_cid).unwrap();
			ensure!(
				cid_base.len() <= 62
					&& (pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of stream hashes linked to journal stream hash
			let mut linked_stream = <JournalStreams<T>>::get(journal_stream_hash).unwrap_or_default();
			linked_stream.push(stream_hash);
			<JournalStreams<T>>::insert(journal_stream_hash, linked_stream);

			log::debug!("Anchor Stream");
			<Streams<T>>::insert(
				&stream_hash,
				StreamDetails {
					creator: creator.clone(),
					stream_cid: stream_cid.clone(),
					journal_stream_hash,
					schema_hash,
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
		pub fn revoke(origin: OriginFor<T>, stream_hash: StreamHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let stream = <Streams<T>>::get(&stream_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream.creator == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!stream.revoked, Error::<T>::StreamAlreadyRevoked);

			log::debug!("Revoking Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Streams<T>>::insert(
				&stream_hash,
				StreamDetails {
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
		pub fn restore(origin: OriginFor<T>, stream_hash: StreamHashOf<T>) -> DispatchResult {
			let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let stream = <Streams<T>>::get(&stream_hash).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream.revoked, Error::<T>::StreamStillActive);
			ensure!(stream.creator == restorer, Error::<T>::UnauthorizedRestore);

			log::debug!("Restoring Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Streams<T>>::insert(
				&stream_hash,
				StreamDetails {
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

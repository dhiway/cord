// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::Decode;

// Re-export RingExponent and RingSize from the support crate.
pub use indiv_support::traits::{PageIndex, RingExponent, RingSize};

pub trait ChunksApi<T: frame_system::Config> {
	type Chunk;
	type Error;

	/// Get a chunk by its absolute index for a specific ring exponent.
	fn get_chunk(ring_exponent: RingExponent, chunk_index: u32)
		-> Result<Self::Chunk, Self::Error>;

	/// Get chunks in a range for a specific ring exponent.
	fn get_chunks(
		ring_exponent: RingExponent,
		start: u32,
		end: u32,
	) -> Result<Vec<Self::Chunk>, Self::Error>;

	#[cfg(feature = "runtime-benchmarks")]
	fn add_all_chunks(ring_size: RingExponent, chunks: Vec<Self::Chunk>);
}

/// Helper trait for benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<Chunk> {
	/// Returns a full page of chunks for benchmarking.
	fn chunk_page() -> Vec<Chunk>;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use core::cmp;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Maximum allowed chunks per page.
	const MAX_PAGE_SIZE: u32 = 1 << 16;

	/// Maximum number of chunk pages per ring exponent.
	pub const MAX_PAGE_COUNT: u32 = 20_000;

	type ChunkPageHash = [u8; 32];

	/// Worst-case buffer length of the SCALE-encoded `BoundedVec<T::Chunk, T::PageSize>`
	/// for a single page.
	pub struct MaxEncodedChunksLen<T>(core::marker::PhantomData<T>);

	impl<T: Config> Get<u32> for MaxEncodedChunksLen<T> {
		fn get() -> u32 {
			<BoundedVec<T::Chunk, T::PageSize> as MaxEncodedLen>::max_encoded_len() as u32
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type WeightInfo: WeightInfo;

		/// The type of a single chunk.
		type Chunk: Parameter + MaxEncodedLen + TypeInfo + verifiable::DecodeUnchecked;

		/// Number of chunks per page. Must be a power of two.
		#[pallet::constant]
		type PageSize: Get<u32>;

		/// The origin allowed to perform privileged management operations on this pallet.
		type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Helper for benchmarks.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self::Chunk>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new chunk page hash set has been initialized (e.g., during genesis).
		ChunkPageHashesInitialized { ring_exponent: RingExponent, total_pages: u32 },
		/// New chunks have been successfully added to an existing or new chunk set.
		ChunksAdded { ring_exponent: RingExponent, start_index: u32, count: u32 },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// The requested chunk index doesn't exist.
		ChunkNotFound,
		/// The provided chunk data couldn't be decoded.
		InvalidChunks,
		/// The start index isn't strictly less than the end index.
		InvalidChunkRange,
	}

	/// Storage newtype around a single chunk that bypasses arkworks
	/// curve-point validation on `Decode`. The validating path is reserved
	/// for the `add_chunks` extrinsic, which decodes user-supplied bytes
	/// once; everything else round-trips through storage and skips the
	/// redundant subgroup checks.
	///
	/// **Warning**: This type wraps a `chunk` which is trusted and decoded without check, it must
	/// have been validated
	#[derive(
		frame_support::CloneNoBound,
		frame_support::DebugNoBound,
		frame_support::PartialEqNoBound,
		frame_support::EqNoBound,
		Encode,
		MaxEncodedLen,
		TypeInfo,
	)]
	#[scale_info(skip_type_params(T))]
	pub struct UncheckedChunk<T: Config>(pub T::Chunk);

	impl<T: Config> codec::Decode for UncheckedChunk<T> {
		fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
			<T::Chunk as verifiable::DecodeUnchecked>::decode_unchecked(input).map(Self)
		}
	}

	impl<T: Config> codec::DecodeWithMemTracking for UncheckedChunk<T> {}

	/// Paginated collection of chunks (RingExponent -> PageIndex -> Chunks).
	#[pallet::storage]
	pub type Chunks<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		RingExponent,
		Twox64Concat,
		PageIndex,
		BoundedVec<UncheckedChunk<T>, T::PageSize>,
		OptionQuery,
	>;

	/// The hash for each page of chunks.
	#[pallet::storage]
	pub type ChunkPageHashes<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		RingExponent,
		Twox64Concat,
		PageIndex,
		ChunkPageHash,
		OptionQuery,
	>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			let page_size = T::PageSize::get();

			assert!(page_size > 0, "page size must be greater than zero");
			assert!(page_size <= MAX_PAGE_SIZE, "page size exceeds the maximum allowed limit");
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a new page of chunks.
		///
		/// The hash of the chunks must match the hash stored on-chain in `ChunkPageHashes`.
		/// The call will fail if the page already exists on-chain.
		#[pallet::call_index(0)]
		#[pallet::authorize(|
			_source: TransactionSource,
			ring_exponent: &RingExponent,
			page_index: &PageIndex,
			encoded_chunks: &BoundedVec<u8, MaxEncodedChunksLen<T>>,
		| -> TransactionValidityWithRefund {
			Pallet::<T>::authorize_add_chunks(ring_exponent, page_index, &encoded_chunks[..])
		})]
		#[pallet::weight(T::WeightInfo::add_chunks())]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_add_chunks())]
		pub fn add_chunks(
			origin: OriginFor<T>,
			ring_exponent: RingExponent,
			page_index: PageIndex,
			encoded_chunks: BoundedVec<u8, MaxEncodedChunksLen<T>>,
		) -> DispatchResult {
			ensure_authorized(origin)?;

			// Decode with curve-point validation (untrusted user input).
			let validated = BoundedVec::<T::Chunk, T::PageSize>::decode(&mut &encoded_chunks[..])
				.map_err(|_| Error::<T>::InvalidChunks)?;
			let count = validated.len() as u32;
			// Wrap each chunk so subsequent storage reads skip the now-redundant validation.
			let page: BoundedVec<UncheckedChunk<T>, T::PageSize> = validated
				.into_iter()
				.map(UncheckedChunk)
				.collect::<Vec<_>>()
				.try_into()
				.map_err(|_| Error::<T>::InvalidChunks)?;
			Chunks::<T>::insert(ring_exponent, page_index, page);

			Self::deposit_event(Event::ChunksAdded {
				ring_exponent,
				start_index: page_index,
				count,
			});

			Ok(())
		}

		/// Sets the expected hashes for chunk pages for a given ring exponent.
		///
		/// Allows setting the expected hashes that chunks must match when added via
		/// `add_chunks`.
		///
		/// The origin must be `ManagerOrigin` or root.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_chunk_page_hashes(page_hashes.len() as u32))]
		pub fn set_chunk_page_hashes(
			origin: OriginFor<T>,
			ring_exponent: RingExponent,
			page_hashes: BoundedVec<ChunkPageHash, ConstU32<MAX_PAGE_COUNT>>,
		) -> DispatchResult {
			T::ManagerOrigin::ensure_origin_or_root(origin)?;

			Pallet::<T>::add_chunk_page_hashes(&ring_exponent, &page_hashes);

			Ok(())
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub encoded_chunk_page_hashes: Vec<(u8, Vec<ChunkPageHash>)>,
		#[serde(skip)]
		pub _phantom: core::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { encoded_chunk_page_hashes: Default::default(), _phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			for (exponent, chunk_page_hashes) in &self.encoded_chunk_page_hashes {
				if chunk_page_hashes.is_empty() {
					continue;
				}
				let Ok(ring_exponent) = RingExponent::new_from_exponent(*exponent) else {
					panic!("Invalid exponent for genesis chunk hashes {exponent:?}");
				};
				Pallet::<T>::add_chunk_page_hashes(&ring_exponent, &chunk_page_hashes[..]);
			}
		}
	}

	impl<T: Config> ChunksApi<T> for Pallet<T> {
		type Chunk = T::Chunk;
		type Error = Error<T>;

		fn get_chunk(
			ring_exponent: RingExponent,
			chunk_index: u32,
		) -> Result<Self::Chunk, Self::Error> {
			Pallet::<T>::get_chunk(ring_exponent, chunk_index)
		}

		fn get_chunks(
			ring_exponent: RingExponent,
			start: u32,
			end: u32,
		) -> Result<Vec<Self::Chunk>, Self::Error> {
			Pallet::<T>::get_chunks(ring_exponent, start, end)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn add_all_chunks(ring_size: RingExponent, chunks: Vec<Self::Chunk>) {
			let page_size = T::PageSize::get() as usize;
			for (page_index, paginated_chunks) in chunks.chunks(page_size).enumerate() {
				let page: BoundedVec<UncheckedChunk<T>, T::PageSize> = paginated_chunks
					.iter()
					.cloned()
					.map(UncheckedChunk)
					.collect::<Vec<_>>()
					.try_into()
					.expect("chunks must fit into page");
				Chunks::<T>::insert(ring_size, page_index as PageIndex, page);
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Validates that a chunk page can be added.
		///
		/// Checks that:
		/// 1. The page doesn't already exist
		/// 2. The hash of the encoded chunks matches the expected hash stored on-chain
		///
		/// Returns a valid transaction if checks pass, or `InvalidTransaction::Call` if not.
		pub fn authorize_add_chunks(
			ring_exponent: &RingExponent,
			page_index: &PageIndex,
			encoded_chunks: &[u8],
		) -> TransactionValidityWithRefund {
			if Chunks::<T>::contains_key(ring_exponent, page_index) {
				return Err(InvalidTransaction::Call.into());
			}
			let page_hash = sp_io::hashing::blake2_256(encoded_chunks);
			if ChunkPageHashes::<T>::get(ring_exponent, page_index)
				.is_none_or(|expected_page_hash| expected_page_hash != page_hash)
			{
				return Err(InvalidTransaction::Call.into());
			}
			let validity = ValidTransaction::with_tag_prefix("pallet-chunk-manager")
				.and_provides((ring_exponent, page_index))
				.into();
			Ok((validity, Weight::zero()))
		}

		/// Sets chunk page hashes for a given ring exponent.
		fn add_chunk_page_hashes(ring_exponent: &RingExponent, page_hashes: &[ChunkPageHash]) {
			for (page_index, page_hash) in page_hashes.iter().enumerate() {
				ChunkPageHashes::<T>::insert(ring_exponent, page_index as PageIndex, page_hash);
			}

			Pallet::<T>::deposit_event(Event::ChunkPageHashesInitialized {
				ring_exponent: *ring_exponent,
				total_pages: page_hashes.len() as u32,
			});
		}

		/// Calculate the location of a chunk.
		fn get_location(ring_exponent: RingExponent, chunk_index: u32) -> (PageIndex, usize) {
			let ring_size = 1u32 << ring_exponent.exponent();
			let page_size = T::PageSize::get();

			// If the ring is smaller than a page, it still occupies one page.
			let total_pages = cmp::max(1, ring_size / page_size);
			let page_mask = total_pages.saturating_sub(1);

			let absolute_page_idx = chunk_index / page_size;
			let page_idx = absolute_page_idx & page_mask;
			let position = (chunk_index % page_size) as usize;

			(page_idx, position)
		}

		/// Get a chunk by its absolute index for a specific ring exponent.
		///
		/// Returns `Err(Error::ChunkNotFound)` if the chunk doesn't exist.
		pub fn get_chunk(
			ring_exponent: RingExponent,
			chunk_index: u32,
		) -> Result<T::Chunk, Error<T>> {
			let (page_idx, position) = Self::get_location(ring_exponent, chunk_index);

			Chunks::<T>::get(ring_exponent, page_idx)
				.and_then(|page| page.get(position).map(|c| c.0.clone()))
				.ok_or(Error::<T>::ChunkNotFound)
		}

		/// Get chunks in a range for a specific ring exponent.
		///
		/// Returns `Err(Error::InvalidChunkRange)` if `start >= end`,
		/// or `Err(Error::ChunkNotFound)` if any chunk doesn't exist.
		pub fn get_chunks(
			ring_exponent: RingExponent,
			start: u32,
			end: u32,
		) -> Result<Vec<T::Chunk>, Error<T>> {
			ensure!(start < end, Error::<T>::InvalidChunkRange);

			let mut result = Vec::with_capacity(end.saturating_sub(start) as usize);
			let page_size = T::PageSize::get() as usize;
			let mut chunk_index = start;

			while chunk_index < end {
				let (page_idx, position) = Self::get_location(ring_exponent, chunk_index);

				let page =
					Chunks::<T>::get(ring_exponent, page_idx).ok_or(Error::<T>::ChunkNotFound)?;

				// Calculate how many chunks to fetch.
				let count = cmp::min(
					page_size.saturating_sub(position),
					end.saturating_sub(chunk_index) as usize,
				);

				// Check that the chunks actually exist in the page.
				if page.len() < position + count {
					return Err(Error::<T>::ChunkNotFound);
				}

				result.extend(page[position..position + count].iter().map(|c| c.0.clone()));

				chunk_index += count as u32;
			}

			Ok(result)
		}
	}
}

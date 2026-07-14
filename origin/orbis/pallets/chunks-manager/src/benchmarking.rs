// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use super::*;
use crate::Pallet as ChunksManager;
use codec::Encode;
use frame_benchmarking::v2::{benchmarks, *};
use frame_support::{
	assert_ok,
	traits::{ConstU32, EnsureOrigin},
	BoundedVec,
};
use sp_io::hashing::blake2_256;

/// Helper to set up a page hash for benchmarking.
fn setup_page_hash<T: Config>(
	ring_exponent: RingExponent,
	page_index: PageIndex,
	encoded_chunks: &[u8],
) {
	let page_hash = blake2_256(encoded_chunks);
	ChunkPageHashes::<T>::insert(ring_exponent, page_index, page_hash);
}

#[benchmarks]
mod benches {
	use super::*;

	// Worst case: a full page of chunks. partial pages are over charged but the absolute weight is
	// tiny and notably simplifies the decode logic of the benchmark.
	#[benchmark]
	fn add_chunks() -> Result<(), BenchmarkError> {
		let ring_exponent = RingExponent::R2e9;
		let page_index: PageIndex = 0;

		let chunks = T::BenchmarkHelper::chunk_page();
		let encoded: Vec<u8> = chunks.encode();
		let encoded_chunks: BoundedVec<u8, crate::pallet::MaxEncodedChunksLen<T>> = encoded
			.try_into()
			.expect("a full page fits within MaxEncodedChunksLen by construction");

		setup_page_hash::<T>(ring_exponent, page_index, &encoded_chunks);

		#[block]
		{
			assert_ok!(ChunksManager::<T>::add_chunks(
				frame_system::Origin::<T>::Authorized.into(),
				ring_exponent,
				page_index,
				encoded_chunks,
			));
		}

		// Verify the chunks were added.
		assert!(Chunks::<T>::contains_key(ring_exponent, page_index));

		Ok(())
	}

	#[benchmark]
	fn authorize_add_chunks() -> Result<(), BenchmarkError> {
		let ring_exponent = RingExponent::R2e9;
		let page_index: PageIndex = 0;

		// Get a full page of chunks and compute the hash.
		let chunks = T::BenchmarkHelper::chunk_page();
		let encoded_chunks = chunks.encode();
		let page_hash = blake2_256(&encoded_chunks);

		ChunkPageHashes::<T>::insert(ring_exponent, page_index, page_hash);

		#[block]
		{
			assert_ok!(ChunksManager::<T>::authorize_add_chunks(
				&ring_exponent,
				&page_index,
				&encoded_chunks,
			));
		}

		Ok(())
	}

	#[benchmark]
	fn set_chunk_page_hashes(
		n: Linear<1, { crate::pallet::MAX_PAGE_COUNT }>,
	) -> Result<(), BenchmarkError> {
		let ring_exponent = RingExponent::R2e9;

		// Create n page hashes.
		let page_hashes: BoundedVec<[u8; 32], ConstU32<{ crate::pallet::MAX_PAGE_COUNT }>> = (0..n)
			.map(|i| [i as u8; 32])
			.collect::<Vec<_>>()
			.try_into()
			.expect("n <= MAX_PAGE_COUNT by Linear bound");

		let origin = <T as Config>::ManagerOrigin::try_successful_origin()
			.map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, ring_exponent, page_hashes);

		// Verify the hashes were set.
		for i in 0..n {
			assert!(ChunkPageHashes::<T>::contains_key(ring_exponent, i));
		}

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}

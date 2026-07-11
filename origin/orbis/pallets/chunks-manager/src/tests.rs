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

use crate::{mock::*, *};
use codec::Encode;
use frame_support::{
	assert_noop, assert_ok,
	traits::{Authorize, ConstU32, Get},
	BoundedVec,
};
use sp_io::hashing::blake2_256;
use sp_runtime::bounded_vec;

/// Helper to set page hashes and add chunks for a single page.
fn setup_and_add_chunks(
	ring_exponent: RingExponent,
	page_index: PageIndex,
	chunks: Vec<MockChunk>,
) -> frame_support::dispatch::DispatchResult {
	let encoded_chunks = chunks.encode();
	let page_hash = blake2_256(&encoded_chunks);

	// Set the expected hash first.
	ChunksManager::set_chunk_page_hashes(
		RuntimeOrigin::root(),
		ring_exponent,
		bounded_vec![page_hash],
	)?;

	// Now add the chunks.
	ChunksManager::add_chunks(
		frame_system::Origin::<Test>::Authorized.into(),
		ring_exponent,
		page_index,
		encoded_chunks.try_into().unwrap(),
	)
}

#[test]
fn add_chunks_works_for_single_chunk() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let ring_exponent = RingExponent::R2e9;

		let chunks = vec![MockChunk(42)];
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks.clone()));

		assert_eq!(Pallet::<Test>::get_chunk(ring_exponent, 0).unwrap(), MockChunk(42));

		// The ChunksAdded event is emitted.
		System::assert_has_event(
			Event::ChunksAdded { ring_exponent, start_index: 0, count: 1 }.into(),
		);
	});
}

#[test]
fn add_chunks_works_for_multiple_chunks() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let chunks: Vec<MockChunk> = (0..10).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks.clone()));

		assert_eq!(Pallet::<Test>::get_chunks(ring_exponent, 0, 10).unwrap(), chunks);
	});
}

#[test]
fn add_chunks_works_for_multiple_pages() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;
		let page_size = <Test as Config>::PageSize::get();

		// Prepare chunks for two pages.
		let chunks1: Vec<MockChunk> = (0..page_size).map(MockChunk).collect();
		let chunks2: Vec<MockChunk> = (page_size..page_size * 2).map(MockChunk).collect();
		let encoded1 = chunks1.encode();
		let encoded2 = chunks2.encode();
		let hash1 = blake2_256(&encoded1);
		let hash2 = blake2_256(&encoded2);

		// Set both page hashes.
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			ring_exponent,
			bounded_vec![hash1, hash2],
		));

		// Add first page.
		assert_ok!(ChunksManager::add_chunks(
			frame_system::Origin::<Test>::Authorized.into(),
			ring_exponent,
			0,
			encoded1.try_into().unwrap(),
		));

		// Add second page.
		assert_ok!(ChunksManager::add_chunks(
			frame_system::Origin::<Test>::Authorized.into(),
			ring_exponent,
			1,
			encoded2.try_into().unwrap(),
		));

		// Verify chunks across pages.
		assert_eq!(Pallet::<Test>::get_chunks(ring_exponent, 0, page_size).unwrap(), chunks1);
		assert_eq!(
			Pallet::<Test>::get_chunks(ring_exponent, page_size, page_size * 2).unwrap(),
			chunks2
		);
	});
}

#[test]
fn add_chunks_fails_due_to_hash_mismatch() {
	use sp_runtime::transaction_validity::{InvalidTransaction, TransactionSource};

	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		// The chunks we will try to add.
		let actual_chunks = vec![MockChunk(42)];
		let encoded_chunks = actual_chunks.encode();

		// The page hash we register is for a different page (i.e. it does not match
		// `actual_chunks`).
		let expected_page_chunks = vec![MockChunk(99)];
		let expected_hash = blake2_256(&expected_page_chunks.encode());

		// Set the expected hash for page 0 (which intentionally does not match the encoded chunks
		// we will submit).
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			ring_exponent,
			bounded_vec![expected_hash],
		));

		// The authorize hook should reject when the provided page's hash does not match the
		// expected one.
		let call = crate::Call::<Test>::add_chunks {
			ring_exponent,
			page_index: 0,
			encoded_chunks: encoded_chunks.try_into().unwrap(),
		};
		let result = call.authorize(TransactionSource::External);
		assert_eq!(result, Some(Err(InvalidTransaction::Call.into())));
	});
}

#[test]
fn add_chunks_fails_for_duplicate_page() {
	use sp_runtime::transaction_validity::{InvalidTransaction, TransactionSource};

	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let chunks = vec![MockChunk(42)];
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks.clone()));

		// Try to add the same page again - the authorize hook should reject it.
		let encoded_chunks = chunks.encode();
		let call = crate::Call::<Test>::add_chunks {
			ring_exponent,
			page_index: 0,
			encoded_chunks: encoded_chunks.try_into().unwrap(),
		};
		let result = call.authorize(TransactionSource::External);
		assert_eq!(result, Some(Err(InvalidTransaction::Call.into())));
	});
}

#[test]
fn add_chunks_fails_for_invalid_chunks() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let invalid_encoded_chunks = vec![0xFF];
		let hash = blake2_256(&invalid_encoded_chunks);

		// Set the hash for the invalid data.
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			ring_exponent,
			bounded_vec![hash],
		));

		assert_noop!(
			ChunksManager::add_chunks(
				frame_system::Origin::<Test>::Authorized.into(),
				ring_exponent,
				0,
				invalid_encoded_chunks.try_into().unwrap(),
			),
			Error::<Test>::InvalidChunks
		);
	});
}

#[test]
fn set_chunk_page_hashes_works() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;
		let hashes: BoundedVec<[u8; 32], ConstU32<{ pallet::MAX_PAGE_COUNT }>> =
			bounded_vec![[1u8; 32], [2u8; 32], [3u8; 32]];

		System::set_block_number(1);
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			ring_exponent,
			hashes.clone(),
		));

		// Verify hashes are stored.
		assert_eq!(ChunkPageHashes::<Test>::get(ring_exponent, 0u32), Some([1u8; 32]));
		assert_eq!(ChunkPageHashes::<Test>::get(ring_exponent, 1u32), Some([2u8; 32]));
		assert_eq!(ChunkPageHashes::<Test>::get(ring_exponent, 2u32), Some([3u8; 32]));

		// Verify event is deposited.
		System::assert_has_event(
			Event::ChunkPageHashesInitialized { ring_exponent, total_pages: 3 }.into(),
		);
	});
}

#[test]
fn set_chunk_page_hashes_fails_for_non_root() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;
		let hashes: BoundedVec<[u8; 32], ConstU32<{ pallet::MAX_PAGE_COUNT }>> =
			bounded_vec![[1u8; 32]];

		assert_noop!(
			ChunksManager::set_chunk_page_hashes(RuntimeOrigin::signed(1), ring_exponent, hashes,),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn get_chunk_works() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let chunks: Vec<MockChunk> = (0..10).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks));

		assert_eq!(Pallet::<Test>::get_chunk(ring_exponent, 0).unwrap(), MockChunk(0));
		assert_eq!(Pallet::<Test>::get_chunk(ring_exponent, 9).unwrap(), MockChunk(9));
	});
}

#[test]
fn get_chunk_fails_for_non_existent_chunk() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		assert_noop!(Pallet::<Test>::get_chunk(ring_exponent, 0), Error::<Test>::ChunkNotFound);

		let chunks: Vec<MockChunk> = (0..5).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks));

		assert_noop!(Pallet::<Test>::get_chunk(ring_exponent, 5), Error::<Test>::ChunkNotFound);
	});
}

#[test]
fn get_chunks_works_for_full_page() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;
		let page_size = <Test as Config>::PageSize::get();

		let chunks: Vec<MockChunk> = (0..page_size).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks.clone()));

		assert_eq!(Pallet::<Test>::get_chunks(ring_exponent, 0, page_size).unwrap(), chunks);
	});
}

#[test]
fn get_chunks_works_for_partial_page() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let chunks: Vec<MockChunk> = (0..100).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks.clone()));

		assert_eq!(Pallet::<Test>::get_chunks(ring_exponent, 10, 20).unwrap(), &chunks[10..20]);
	});
}

#[test]
fn get_chunks_works_across_pages() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;
		let page_size: u32 = <Test as Config>::PageSize::get();

		let chunks1: Vec<MockChunk> = (0..page_size).map(MockChunk).collect();
		let chunks2: Vec<MockChunk> = (page_size..page_size + 10).map(MockChunk).collect();
		let encoded1 = chunks1.encode();
		let encoded2 = chunks2.encode();
		let hash1 = blake2_256(&encoded1);
		let hash2 = blake2_256(&encoded2);

		// Set both page hashes.
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			ring_exponent,
			bounded_vec![hash1, hash2],
		));

		// Add both pages.
		assert_ok!(ChunksManager::add_chunks(
			frame_system::Origin::<Test>::Authorized.into(),
			ring_exponent,
			0,
			encoded1.try_into().unwrap(),
		));
		assert_ok!(ChunksManager::add_chunks(
			frame_system::Origin::<Test>::Authorized.into(),
			ring_exponent,
			1,
			encoded2.try_into().unwrap(),
		));

		let start_index = page_size - 5;
		let end_index = page_size + 5;
		let expected: Vec<MockChunk> = (start_index..end_index).map(MockChunk).collect();
		assert_eq!(
			Pallet::<Test>::get_chunks(ring_exponent, start_index, end_index).unwrap(),
			expected
		);
	});
}

#[test]
fn get_chunks_fails_for_invalid_range() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		assert_noop!(
			Pallet::<Test>::get_chunks(ring_exponent, 5, 5),
			Error::<Test>::InvalidChunkRange
		);

		assert_noop!(
			Pallet::<Test>::get_chunks(ring_exponent, 6, 5),
			Error::<Test>::InvalidChunkRange
		);
	});
}

#[test]
fn get_chunks_fails_for_non_existent_chunk() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		assert_noop!(Pallet::<Test>::get_chunks(ring_exponent, 0, 1), Error::<Test>::ChunkNotFound);
	});
}

#[test]
fn get_chunks_fails_for_missing_chunk_in_range() {
	new_test_ext().execute_with(|| {
		let ring_exponent = RingExponent::R2e9;

		let chunks: Vec<MockChunk> = (0..10).map(MockChunk).collect();
		assert_ok!(setup_and_add_chunks(ring_exponent, 0, chunks));

		assert_noop!(
			Pallet::<Test>::get_chunks(ring_exponent, 5, 11),
			Error::<Test>::ChunkNotFound
		);
	});
}

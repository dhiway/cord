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

//! Benchmarking for Members Notifier pallet

use super::*;
use alloc::vec::Vec;
use cumulus_primitives_core::ParaId;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::pallet_prelude::BlockNumberFor;
use indiv_support::traits::{Identifier, RingExponent};
use sp_runtime::{traits::Zero, transaction_validity::TransactionSource, Saturating};

/// Create a deterministic test identifier from an index.
fn test_identifier(index: u32) -> Identifier {
	let mut id = [0u8; 32];
	id[..4].copy_from_slice(&index.to_be_bytes());
	id
}

/// Default pallet index used in benchmarks.
const BENCHMARK_PALLET_INDEX: u8 = 50;

fn setup_subscriber<T: Config>(para_id: ParaId, collections: Vec<Identifier>) {
	setup_subscriber_with_init_seq::<T>(para_id, collections, 0);
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T: Config> {
	/// Initializes runtime state needed for benchmarks (e.g. timestamp, HRMP channels).
	fn init() {}
	fn setup_ring_roots(count: u32);
	/// Overrides the HRMP `max_message_size`.
	fn set_max_message_size(size: u32);
}

fn setup_subscriber_with_init_seq<T: Config>(
	para_id: ParaId,
	collections: Vec<Identifier>,
	last_init_sequence: u64,
) {
	let paired: Vec<(Identifier, RingExponent)> =
		collections.into_iter().map(|id| (id, RingExponent::R2e9)).collect();
	let bounded: BoundedVec<_, T::MaxCollectionsPerSubscriber> =
		paired.try_into().expect("collections within bounds");
	let info = SubscriberInfo::<T> {
		collections: bounded,
		last_init_sequence,
		pallet_index: BENCHMARK_PALLET_INDEX,
	};
	Subscribers::<T>::insert(para_id, info);
}

fn setup_clear_batch_worst_case<T: Config>() {
	for i in 0..T::MaxCollections::get() {
		let indices: BoundedVec<u32, T::MaxUpdatesPerBatch> =
			(0..T::MaxUpdatesPerBatch::get()).collect::<Vec<_>>().try_into().unwrap();
		SealedBatchIndices::<T>::insert(test_identifier(i), indices);
	}
	for i in 0..T::MaxSubscribers::get() {
		SubscribersWithCurrentBatch::<T>::insert(ParaId::from(i), ());
	}
}

/// Helper to insert a pending update key into the current write page.
fn insert_pending_update<T: Config>(identifier: Identifier, ring_index: u32) {
	let page = PageState::<T>::get().write_page;
	PendingUpdates::<T>::insert((page, identifier, ring_index), ());
	PageUpdatesCount::<T>::mutate(page, |c| *c = c.saturating_add(1));
}

#[benchmarks(
	where
		T: Config,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn subscribe() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		// Subscribers storage almost full
		for i in 0..(T::MaxSubscribers::get() - 1) {
			setup_subscriber::<T>(ParaId::from(i), alloc::vec![]);
		}

		let para_id = ParaId::from(T::MaxSubscribers::get() - 1);
		let collections: BoundedVec<(Identifier, RingExponent), T::MaxCollectionsPerSubscriber> =
			(0..T::MaxCollectionsPerSubscriber::get())
				.map(|i| (test_identifier(i), RingExponent::R2e9))
				.collect::<alloc::vec::Vec<_>>()
				.try_into()
				.expect("within bounds");
		let pallet_index = BENCHMARK_PALLET_INDEX;

		#[extrinsic_call]
		_(frame_system::RawOrigin::Root, para_id, collections, pallet_index);

		assert!(Subscribers::<T>::contains_key(para_id));

		Ok(())
	}

	#[benchmark]
	fn unsubscribe() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let target_para = ParaId::from(T::MaxSubscribers::get() - 1);

		// SealedBatchIndices with MaxCollections entries.
		for i in 0..T::MaxCollections::get() {
			let indices = BoundedVec::<u32, T::MaxUpdatesPerBatch>::truncate_from(
				(0..T::MaxUpdatesPerBatch::get()).collect(),
			);
			SealedBatchIndices::<T>::insert(test_identifier(i), indices);
		}

		// MaxSubscribers number of subscribers.
		// All except target marked as already received updates.
		// Target subscriber has MaxCollectionsPerSubscriber collections (worst case for
		// LastReplayTime cleanup).
		let target_collections: Vec<Identifier> =
			(0..T::MaxCollectionsPerSubscriber::get()).map(test_identifier).collect();
		for i in 0..T::MaxSubscribers::get() {
			let para = ParaId::from(i);
			if para == target_para {
				setup_subscriber::<T>(para, target_collections.clone());
			} else {
				setup_subscriber::<T>(para, alloc::vec![test_identifier(0)]);
				SubscribersWithCurrentBatch::<T>::insert(para, ());
			}
		}

		// LastReplayTime entries for all target subscriber collections.
		for i in 0..T::MaxCollectionsPerSubscriber::get() {
			LastReplayTime::<T>::insert(target_para, test_identifier(i), 1u64);
		}

		// PendingInit for target subscriber.
		let collections: BoundedVec<_, T::MaxCollectionsPerSubscriber> = target_collections
			.into_iter()
			.map(|id| (id, RingExponent::R2e9))
			.collect::<Vec<_>>()
			.try_into()
			.expect("within bounds");
		PendingInit::<T>::insert(
			target_para,
			PendingInitState::<T> {
				collections,
				current_collection_index: 0,
				after_ring_index: None,
				sequence: 0,
				source_time: 0,
				pallet_index: BENCHMARK_PALLET_INDEX,
				_phantom: Default::default(),
			},
		);

		// Current batch where only one subscriber did not receive the batch.
		CurrentBatch::<T>::put(BatchDistributionState {
			sequence: 1,
			source_time: 1,
			sealed_at: frame_system::Pallet::<T>::block_number(),
			remaining_subscribers: 1,
		});

		let origin = T::ManageOrigin::try_successful_origin()
			.expect("ManageOrigin must provide a successful origin for benchmarks");
		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, Some(target_para));

		assert!(!Subscribers::<T>::contains_key(target_para));
		assert!(CurrentBatch::<T>::get().is_none());

		Ok(())
	}

	#[benchmark]
	fn request_replay(
		n: Linear<1, { T::MaxUpdatesPerBatch::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let para_id = ParaId::from(1000);
		let identifier = test_identifier(0);
		setup_subscriber::<T>(para_id, alloc::vec![identifier]);

		T::BenchmarkHelper::setup_ring_roots(n);

		LastReplayTime::<T>::insert(para_id, identifier, 0u64);

		let caller = frame_benchmarking::whitelisted_caller();
		let indices: BoundedVec<u32, T::MaxUpdatesPerBatch> =
			(0..n).collect::<Vec<_>>().try_into().expect("within bounds");

		#[extrinsic_call]
		_(frame_system::RawOrigin::Signed(caller), para_id, identifier, indices);

		frame_system::Pallet::<T>::assert_last_event(
			Event::<T>::ReplayRequested { para_id, identifier, indices_count: n }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn enqueue_updates(
		n: Linear<1, { T::MaxUpdatesPerBatch::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		// Spreading updates across MaxCollections distinct identifiers.
		let max_collections = T::MaxCollections::get();
		for i in 0..n {
			let identifier = test_identifier(i % max_collections);
			insert_pending_update::<T>(identifier, i);
		}

		T::BenchmarkHelper::setup_ring_roots(n);

		// MaxSubscribers number of subscribers. Half with PendingInit.
		let collections: Vec<Identifier> =
			(0..T::MaxCollectionsPerSubscriber::get()).map(test_identifier).collect();
		for i in 0..T::MaxSubscribers::get() {
			let para = ParaId::from(1000 + i);
			setup_subscriber::<T>(para, collections.clone());
			if i % 2 == 0 {
				let bounded_collections: BoundedVec<_, T::MaxCollectionsPerSubscriber> =
					collections
						.iter()
						.map(|id| (*id, RingExponent::R2e9))
						.collect::<Vec<_>>()
						.try_into()
						.expect("within bounds");
				PendingInit::<T>::insert(
					para,
					PendingInitState::<T> {
						collections: bounded_collections,
						current_collection_index: 0,
						after_ring_index: None,
						sequence: 0,
						source_time: 1,
						pallet_index: BENCHMARK_PALLET_INDEX,
						_phantom: Default::default(),
					},
				);
			}
		}

		let send_page = PageState::<T>::get().send_page;

		#[extrinsic_call]
		_(frame_system::RawOrigin::Authorized, send_page, BlockNumberFor::<T>::zero());

		assert!(CurrentBatch::<T>::get().is_some());
		assert_eq!(PageUpdatesCount::<T>::get(send_page), 0);

		Ok(())
	}

	#[benchmark]
	fn send_batch(
		n: Linear<{ T::MaxCollectionsPerSubscriber::get() }, { T::MaxUpdatesPerBatch::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let para_id = ParaId::from(1000);

		T::BenchmarkHelper::setup_ring_roots(n);

		// All MaxCollectionsPerSubscriber slots subscribed.
		let c = T::MaxCollectionsPerSubscriber::get();
		let collections: Vec<Identifier> = (0..c).map(test_identifier).collect();
		setup_subscriber::<T>(para_id, collections.clone());

		// To simulate the most work for clean_batch.
		setup_clear_batch_worst_case::<T>();

		// Spreading n indices across all subscribed collections so every iteration of
		// `send_batch` loop hits the `Some` branch.
		// Lower bound `n >= MaxCollectionsPerSubscriber` guarantees each slice is non-empty.
		for (slot, collection) in collections.iter().enumerate() {
			let start = n.saturating_mul(slot as u32) / c;
			let end = n.saturating_mul((slot as u32).saturating_add(1)) / c;
			let indices: BoundedVec<u32, T::MaxUpdatesPerBatch> =
				(start..end).collect::<Vec<_>>().try_into().expect("within bounds");
			SealedBatchIndices::<T>::insert(*collection, indices);
		}

		// Forcing max_per_xcm == 1 so every index produces a separate XCM message.
		// The midpoint between "fits exactly 1 update" and "fits exactly 2"
		// keeps max_per_xcm == 1 while leaving some room for
		// router-injected framing (e.g. SetTopic added by WithUniqueTopic).
		let payload_for_one_update = ((Pallet::<T>::xcm_message_max_size(1) +
			Pallet::<T>::xcm_message_max_size(2)) /
			2) as u32;
		T::BenchmarkHelper::set_max_message_size(payload_for_one_update);

		let sequence = 1u64;
		CurrentBatch::<T>::put(BatchDistributionState {
			sequence,
			source_time: 1,
			sealed_at: frame_system::Pallet::<T>::block_number(),
			remaining_subscribers: 1,
		});

		#[extrinsic_call]
		_(frame_system::RawOrigin::Authorized, para_id, sequence, BlockNumberFor::<T>::zero());

		// With max_per_xcm forced to 1, every one of the n indices is sent in its
		// own XCM message and emits a single UpdatesSent { update_count: 1 } event.
		let expected_event: <T as frame_system::Config>::RuntimeEvent =
			Event::<T>::UpdatesSent { para_id, update_count: 1 }.into();
		let updates_sent_count = frame_system::Pallet::<T>::events()
			.iter()
			.filter(|r| r.event == expected_event)
			.count() as u32;
		assert_eq!(
			updates_sent_count, n,
			"expected {n} UpdatesSent events, got {updates_sent_count}",
		);
		assert!(CurrentBatch::<T>::get().is_none());

		Ok(())
	}

	#[benchmark]
	fn send_init_page(
		n: Linear<1, { T::MaxUpdatesPerBatch::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let para_id = ParaId::from(1000);
		// Single collection so the benchmark covers both advance_init_pagination branches:
		// PendingInit::mutate (n == max_per_xcm) and PendingInit::remove (n < max_per_xcm).
		let collections: BoundedVec<_, T::MaxCollectionsPerSubscriber> =
			alloc::vec![(test_identifier(0), RingExponent::R2e9)]
				.try_into()
				.expect("Collections within bounds");

		let subscriber_info = SubscriberInfo::<T> {
			collections: collections.clone(),
			last_init_sequence: 1,
			pallet_index: BENCHMARK_PALLET_INDEX,
		};
		Subscribers::<T>::insert(para_id, subscriber_info);

		PendingInit::<T>::insert(
			para_id,
			PendingInitState::<T> {
				collections,
				current_collection_index: 0,
				after_ring_index: None,
				sequence: 1,
				source_time: 1,
				pallet_index: BENCHMARK_PALLET_INDEX,
				_phantom: Default::default(),
			},
		);

		// n ring roots available
		T::BenchmarkHelper::setup_ring_roots(n);

		let init_state = PendingInit::<T>::get(para_id).expect("pending init exists");

		#[extrinsic_call]
		_(
			frame_system::RawOrigin::Authorized,
			para_id,
			init_state.current_collection_index,
			init_state.after_ring_index,
			BlockNumberFor::<T>::zero(),
		);

		// Pagination either progressed within the collection or completed (PendingInit
		// removed). Both are valid outcomes depending on n vs max_per_xcm.
		if let Some(state) = PendingInit::<T>::get(para_id) {
			assert!(
				state.after_ring_index.is_some(),
				"send_init_page should advance pagination within collection",
			);
		}

		Ok(())
	}

	#[benchmark]
	fn abandon_stuck_batch() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		setup_clear_batch_worst_case::<T>();

		// Stuck batch: sealed_at = current block, advancing past timeout.
		let sealed_at = frame_system::Pallet::<T>::block_number();
		CurrentBatch::<T>::put(BatchDistributionState {
			sequence: 1,
			source_time: 0,
			sealed_at,
			remaining_subscribers: T::MaxSubscribers::get(),
		});

		let cleanup_block = sealed_at
			.saturating_add(T::StuckBatchTimeout::get())
			.saturating_add(1u32.into());
		frame_system::Pallet::<T>::set_block_number(cleanup_block);

		#[extrinsic_call]
		_(frame_system::RawOrigin::Authorized, BlockNumberFor::<T>::zero());

		assert!(CurrentBatch::<T>::get().is_none());

		Ok(())
	}

	#[benchmark]
	fn authorize_enqueue_updates() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		// No active batch, pending work exists.
		insert_pending_update::<T>(test_identifier(0), 0);

		#[block]
		{
			Pallet::<T>::authorize_enqueue_updates(
				TransactionSource::Local,
				&PageState::<T>::get().send_page,
			)
			.expect("must authorize");
		}

		Ok(())
	}

	#[benchmark]
	fn authorize_send_batch() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let para_id = ParaId::from(1000);
		setup_subscriber::<T>(para_id, alloc::vec![test_identifier(0)]);

		let sequence = 1u64;
		CurrentBatch::<T>::put(BatchDistributionState {
			sequence,
			source_time: 1,
			sealed_at: frame_system::Pallet::<T>::block_number(),
			remaining_subscribers: 1,
		});

		#[block]
		{
			Pallet::<T>::authorize_send_batch(TransactionSource::Local, &para_id, &sequence)
				.expect("must authorize");
		}

		Ok(())
	}

	#[benchmark]
	fn authorize_send_init_page() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let para_id = ParaId::from(1000);
		let collections: BoundedVec<_, T::MaxCollectionsPerSubscriber> =
			alloc::vec![(test_identifier(0), RingExponent::R2e9)]
				.try_into()
				.expect("within bounds");

		Subscribers::<T>::insert(
			para_id,
			SubscriberInfo::<T> {
				collections: collections.clone(),
				last_init_sequence: 0,
				pallet_index: BENCHMARK_PALLET_INDEX,
			},
		);

		PendingInit::<T>::insert(
			para_id,
			PendingInitState::<T> {
				collections,
				current_collection_index: 0,
				after_ring_index: None,
				sequence: 0,
				source_time: 1,
				pallet_index: BENCHMARK_PALLET_INDEX,
				_phantom: Default::default(),
			},
		);

		#[block]
		{
			let init_state = PendingInit::<T>::get(para_id).expect("pending init exists");
			Pallet::<T>::authorize_send_init_page(
				TransactionSource::Local,
				&para_id,
				&init_state.current_collection_index,
				&init_state.after_ring_index,
			)
			.expect("must authorize");
		}

		Ok(())
	}

	#[benchmark]
	fn authorize_abandon_stuck_batch() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::init();
		let sealed_at = frame_system::Pallet::<T>::block_number();
		CurrentBatch::<T>::put(BatchDistributionState {
			sequence: 1,
			source_time: 0,
			sealed_at,
			remaining_subscribers: 1,
		});

		let cleanup_block = sealed_at
			.saturating_add(T::StuckBatchTimeout::get())
			.saturating_add(1u32.into());
		frame_system::Pallet::<T>::set_block_number(cleanup_block);

		#[block]
		{
			Pallet::<T>::authorize_abandon_stuck_batch(TransactionSource::Local)
				.expect("must authorize");
		}

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}

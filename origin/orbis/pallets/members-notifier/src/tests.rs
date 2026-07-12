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

//! Unit tests for Members Notifier pallet

use crate::{
	mock::*,
	pallet::{
		CurrentBatch, LastReplayTime, PageState, PageUpdatesCount, PendingInit, PendingUpdates,
		SealedBatchIndices, SealedBatchSequence, Subscribers, SubscribersWithCurrentBatch,
	},
	Error,
};
use alloc::vec::Vec;
use cumulus_primitives_core::ParaId;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use indiv_support::traits::{Identifier, RingExponent};
use sp_runtime::bounded_vec;

// ============================================================================
// Helpers
// ============================================================================

/// Returns the current pending updates count for the write page.
fn pending_updates_count() -> u32 {
	let page = PageState::<Test>::get().write_page;
	PageUpdatesCount::<Test>::get(page)
}

/// Returns the current batch sequence number.
fn current_batch_sequence() -> u64 {
	CurrentBatch::<Test>::get().map(|b| b.sequence).unwrap_or(0)
}

/// Calls `send_init_page` for a given para_id.
fn do_send_init_page(para_id: ParaId) -> frame_support::dispatch::DispatchResultWithPostInfo {
	let state = PendingInit::<Test>::get(para_id).expect("pending init for do_send_init_page");
	MembersNotifier::send_init_page(
		authorized_origin(),
		para_id,
		state.current_collection_index,
		state.after_ring_index,
		0,
	)
}

/// Calls `enqueue_updates`.
fn do_enqueue_updates() -> frame_support::dispatch::DispatchResultWithPostInfo {
	let send_page = PageState::<Test>::get().send_page;
	MembersNotifier::enqueue_updates(authorized_origin(), send_page, 0)
}

/// Finalize subscriptions by running send_init_page for all pending inits, then reset state.
fn finalize_subscriptions() {
	// Process all pending inits by calling send_init_page directly.
	loop {
		let keys: Vec<ParaId> = PendingInit::<Test>::iter_keys().collect();
		if keys.is_empty() {
			break;
		}
		for para_id in keys {
			let _ = do_send_init_page(para_id);
		}
	}
	System::reset_events();
	let _ = PendingUpdates::<Test>::clear(u32::MAX, None);
	let _ = PageUpdatesCount::<Test>::clear(u32::MAX, None);
	PageState::<Test>::kill();
	CurrentBatch::<Test>::kill();
	let _ = SealedBatchIndices::<Test>::clear(u32::MAX, None);
	let _ = SubscribersWithCurrentBatch::<Test>::clear(u32::MAX, None);
	SealedBatchSequence::<Test>::put(0);
	for (para_id, mut info) in Subscribers::<Test>::iter() {
		info.last_init_sequence = 0;
		Subscribers::<Test>::insert(para_id, info);
	}
}

// ============================================================================
// Tests
// ============================================================================

mod enqueue_updates {
	use super::*;
	use sp_runtime::transaction_validity::TransactionSource;

	#[test]
	fn authorization_rejects_when_no_pending_work() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			assert!(MembersNotifier::authorize_enqueue_updates(
				TransactionSource::Local,
				&PageState::<Test>::get().send_page
			)
			.is_err());
			assert!(CurrentBatch::<Test>::get().is_none());
			assert_eq!(SealedBatchSequence::<Test>::get(), 0);
		});
	}

	#[test]
	fn authorization_respects_throttle_limits() {
		new_test_ext().execute_with(|| {
			set_throttle_config(5, 100);

			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1);

			// Block 1: throttled (0 blocks since last, need 5).
			System::set_block_number(1);
			assert!(
				MembersNotifier::authorize_enqueue_updates(
					TransactionSource::Local,
					&PageState::<Test>::get().send_page
				)
				.is_err(),
				"throttled — authorization should reject",
			);

			// Block 5: threshold reached.
			System::set_block_number(5);
			assert!(
				MembersNotifier::authorize_enqueue_updates(
					TransactionSource::Local,
					&PageState::<Test>::get().send_page
				)
				.is_ok(),
				"authorization should pass after throttle period",
			);
		});
	}

	#[test]
	fn sealed_pages_bypass_throttle_check() {
		new_test_ext().execute_with(|| {
			set_throttle_config(1000, 1000);

			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			// Page 0 full
			for i in 0..MaxUpdatesPerBatch::get() + 1 {
				use indiv_support::traits::{OnRingRootChange, RingRootOp};
				<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
					PEOPLE_IDENTIFIER,
					i,
					RingRootOp::Built { revision: 1, root: &Default::default() },
				);
			}

			// send_page != write_page -> sealed pages exist
			assert_ne!(
				PageState::<Test>::get().send_page,
				PageState::<Test>::get().write_page,
				"should have sealed pages"
			);

			// Despite high throttle, enqueue_updates succeeds on sealed page and starts a batch
			System::set_block_number(1);
			assert_ok!(do_enqueue_updates());
			assert!(CurrentBatch::<Test>::get().is_some());
			assert_eq!(SealedBatchSequence::<Test>::get(), 1);
		});
	}

	#[test]
	fn creates_sealed_batch_data() {
		new_test_ext().execute_with(|| {
			// No throttle limits - one update will trigger new batch creation

			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1).add_pending_update(1, 1);
			TestCollection::people_lite().add_pending_update(0, 1);

			assert_ok!(do_enqueue_updates());

			assert_eq!(SealedBatchSequence::<Test>::get(), 1);
			let current_batch = CurrentBatch::<Test>::get().expect("batch should exist");
			assert_eq!(current_batch.sequence, 1);
			assert_eq!(current_batch.remaining_subscribers, 2);

			let people_indices = SealedBatchIndices::<Test>::get(PEOPLE_IDENTIFIER);
			assert!(people_indices.is_some());
			assert_eq!(people_indices.unwrap().len(), 2);

			let lite_indices = SealedBatchIndices::<Test>::get(PEOPLE_LITE_IDENTIFIER);
			assert!(lite_indices.is_some());
			assert_eq!(lite_indices.unwrap().len(), 1);
		});
	}

	#[test]
	fn skips_sealing_if_zero_subscribers() {
		new_test_ext().execute_with(|| {
			let seq_before = SealedBatchSequence::<Test>::get();

			TestCollection::people().add_pending_update(5, 1);
			assert_ok!(do_enqueue_updates());

			// No subscribers — no batch created and sequence not bumped.
			assert!(CurrentBatch::<Test>::get().is_none());
			assert_eq!(SealedBatchSequence::<Test>::get(), seq_before);
		});
	}
}

mod send_batch {
	use super::*;

	fn setup_batch_with_subscribers(collections: &[[u8; 32]], subscriber_ids: &[u32]) {
		for &id in subscriber_ids {
			TestSubscriber::new(id).subscribe_to(collections);
		}
		finalize_subscriptions();

		for coll in collections {
			TestCollection(*coll).add_pending_update(0, 1);
		}

		assert_ok!(do_enqueue_updates());
		System::reset_events();
	}

	#[test]
	fn sends_batch_per_collection() {
		new_test_ext().execute_with(|| {
			setup_batch_with_subscribers(&[PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER], &[1000]);

			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				current_batch_sequence(),
				0
			));

			// Two collections -> two UpdatesSent events.
			Events::updates_sent().assert_count(2).assert_received(1000, 2);
		});
	}

	#[test]
	fn portions_updates_by_hrmp_capacity() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			let max_per_xcm = MembersNotifier::max_updates_per_xcm(ParaId::from(1000)).unwrap();
			let total_updates = max_per_xcm + 1;

			// More updates than will fit in one subscriber channel XCM
			for i in 0..total_updates {
				TestCollection::people().add_pending_update(i, 1);
			}

			assert_ok!(do_enqueue_updates());
			System::reset_events();

			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				current_batch_sequence(),
				0
			));

			// 2 XCM messages sent with all the updates
			let events = Events::updates_sent();
			assert_eq!(events.total_for(1000), total_updates);
			assert_eq!(events.count(), 2);
		});
	}

	#[test]
	fn xcm_failure_isolated_to_failing_subscriber() {
		new_test_ext().execute_with(|| {
			setup_batch_with_subscribers(&[PEOPLE_IDENTIFIER], &[1000, 2000]);

			// Subscriber 1000 fails.
			XCM_SEND_SHOULD_FAIL.set(true);
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				current_batch_sequence(),
				0
			));
			XCM_SEND_SHOULD_FAIL.set(false);

			// Subscriber 2000 succeeds.
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(2000),
				current_batch_sequence(),
				0
			));

			// Only 2000 marked as sent.
			assert!(!SubscribersWithCurrentBatch::<Test>::contains_key(ParaId::from(1000)));
			assert!(SubscribersWithCurrentBatch::<Test>::contains_key(ParaId::from(2000)));

			let current_batch = CurrentBatch::<Test>::get().expect("batch should still exist");
			assert_eq!(current_batch.remaining_subscribers, 1);
		});
	}

	#[test]
	fn clears_batch_on_last_subscriber() {
		new_test_ext().execute_with(|| {
			setup_batch_with_subscribers(&[PEOPLE_IDENTIFIER], &[1000, 2000]);

			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				current_batch_sequence(),
				0
			));
			assert!(CurrentBatch::<Test>::get().is_some(), "batch still active");

			System::reset_events();
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(2000),
				current_batch_sequence(),
				0
			));
			assert!(CurrentBatch::<Test>::get().is_none(), "batch cleared");
			assert!(SealedBatchIndices::<Test>::get(PEOPLE_IDENTIFIER).is_none());
		});
	}

	#[test]
	fn rejects_if_no_batch() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			assert_noop!(
				MembersNotifier::send_batch(
					authorized_origin(),
					ParaId::from(1000),
					current_batch_sequence(),
					0
				),
				Error::<Test>::NoBatchActive
			);
		});
	}

	#[test]
	fn new_subscriber_during_batch_is_rejected_by_authorize() {
		use sp_runtime::transaction_validity::TransactionSource;

		new_test_ext().execute_with(|| {
			// Subscriber 1000 set up and initialized.
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1);

			// Batch sealed — only subscriber 1000 counted (remaining_subscribers = 1).
			assert_ok!(do_enqueue_updates());
			let batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(batch.remaining_subscribers, 1);
			let seq = batch.sequence;

			// New subscriber 2000 subscribes AFTER the batch was sealed.
			// Its last_init_sequence == current SealedBatchSequence >= batch sequence.
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			assert!(PendingInit::<Test>::contains_key(ParaId::from(2000)));

			// Authorize rejects send_batch for subscriber 2000 (already initialized
			// with this data).
			assert!(MembersNotifier::authorize_send_batch(
				TransactionSource::Local,
				&ParaId::from(2000),
				&seq,
			)
			.is_err());

			// Sending batch to subscriber 1000 succeeds normally.
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				seq,
				0
			));
			// Batch cleared after last counted subscriber received it.
			assert!(
				CurrentBatch::<Test>::get().is_none(),
				"batch cleared after all counted subscribers served"
			);
		});
	}

	#[test]
	fn subscriber_with_pending_init_receives_batch() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(100);

			// Subscriber 1000 fully initialized.
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			// Subscriber 2000 subscribed but init not yet complete (many ring roots).
			// Subscribing at sequence 0 (from finalize_subscriptions reset).
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			// Partially initializing — send one page but don't finish.
			assert_ok!(do_send_init_page(ParaId::from(2000)));
			assert!(
				PendingInit::<Test>::contains_key(ParaId::from(2000)),
				"init still in progress"
			);

			// Adding a pending update and sealing a batch.
			TestCollection::people().add_pending_update(0, 1);
			assert_ok!(do_enqueue_updates());

			// Both subscribers counted — subscriber 2000 has last_init_sequence = 0 < sequence 1.
			let batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(batch.remaining_subscribers, 2, "both subscribers counted");
			let seq = batch.sequence;

			System::reset_events();

			// Sending batch to subscriber 2000 (mid-init) — should_send is true.
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(2000),
				seq,
				0
			));
			Events::updates_sent().assert_any_received(2000);
			let batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(batch.remaining_subscribers, 1);

			// Sending batch to subscriber 1000.
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				seq,
				0
			));
			assert!(CurrentBatch::<Test>::get().is_none(), "batch cleared");
		});
	}
}

mod send_init_page {
	use super::*;

	#[test]
	fn advances_pagination() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10000);
			clear_sent_xcms();
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_ok!(do_send_init_page(ParaId::from(1000)));

			// Still pending, with advanced ring index.
			assert!(PendingInit::<Test>::contains_key(ParaId::from(1000)));
			let state = PendingInit::<Test>::get(ParaId::from(1000)).unwrap();
			assert!(state.after_ring_index.is_some());

			// The forwarded XCM carries the collection's registered ring exponent.
			let xcms = get_sent_xcms();
			assert!(!xcms.is_empty(), "init XCM should have been sent");
			let (_, encoded) = xcms.last().unwrap();
			match decode_subscriber_call(encoded) {
				Some(crate::pallet::SubscriberCall::InitializeRingRoots {
					ring_exponent,
					roots,
				}) => {
					assert_eq!(roots.identifier, PEOPLE_IDENTIFIER);
					assert_eq!(ring_exponent, RingExponent::R2e9);
				},
				other => panic!("expected InitializeRingRoots, got {:?}", other.is_some()),
			}
		});
	}

	#[test]
	fn completes_all_collections() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(2);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER]);

			// Needs two calls (one per collection).
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(
				PendingInit::<Test>::contains_key(ParaId::from(1000)),
				"second collection pending"
			);
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(!PendingInit::<Test>::contains_key(ParaId::from(1000)), "all done");
		});
	}

	#[test]
	fn forwards_per_collection_exponents_across_multi_collection_init() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(2);
			clear_sent_xcms();

			// Two collections with DIFFERENT exponents.
			TestSubscriber::new(1000).subscribe_to_with_exponents(&[
				(PEOPLE_IDENTIFIER, RingExponent::R2e9),
				(PEOPLE_LITE_IDENTIFIER, RingExponent::R2e14),
			]);

			// Page both collections through.
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(!PendingInit::<Test>::contains_key(ParaId::from(1000)));

			// Collect the two init XCMs and verify each one carried the right exponent
			// for its identifier.
			let calls: Vec<_> = get_sent_xcms()
				.iter()
				.filter_map(|(_, enc)| decode_subscriber_call(enc))
				.collect();
			let init_calls: Vec<(Identifier, RingExponent)> = calls
				.into_iter()
				.filter_map(|c| match c {
					crate::pallet::SubscriberCall::InitializeRingRoots { ring_exponent, roots } =>
						Some((roots.identifier, ring_exponent)),
					_ => None,
				})
				.collect();

			assert!(
				init_calls.contains(&(PEOPLE_IDENTIFIER, RingExponent::R2e9)),
				"PEOPLE init should use R2e9; got {init_calls:?}"
			);
			assert!(
				init_calls.contains(&(PEOPLE_LITE_IDENTIFIER, RingExponent::R2e14)),
				"PEOPLE_LITE init should use R2e14; got {init_calls:?}"
			);
		});
	}

	#[test]
	fn xcm_failure_no_advance() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(5);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			XCM_SEND_SHOULD_FAIL.set(true);
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			XCM_SEND_SHOULD_FAIL.set(false);

			// Still pending — no advance since XCM failed.
			assert!(PendingInit::<Test>::contains_key(ParaId::from(1000)));
			let state = PendingInit::<Test>::get(ParaId::from(1000)).unwrap();
			assert_eq!(state.current_collection_index, 0);
			assert!(state.after_ring_index.is_none());
		});
	}

	#[test]
	fn handles_empty_collection() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(0);
			clear_sent_xcms();
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(!PendingInit::<Test>::contains_key(ParaId::from(1000)));

			// An empty initialize_ring_roots XCM is still emitted so the subscriber
			// transitions to Active even when the collection has no ring roots yet.
			let init_calls: Vec<_> = get_sent_xcms()
				.iter()
				.filter_map(|(_, enc)| decode_subscriber_call(enc))
				.filter_map(|c| match c {
					crate::pallet::SubscriberCall::InitializeRingRoots { roots, .. } => Some(roots),
					_ => None,
				})
				.collect();
			assert_eq!(init_calls.len(), 1, "exactly one init XCM expected");
			assert_eq!(init_calls[0].identifier, PEOPLE_IDENTIFIER);
			assert!(init_calls[0].updates.is_empty(), "updates should be empty");
		});
	}

	#[test]
	fn handles_all_empty_multi_collection() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(0);
			clear_sent_xcms();
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER]);

			// One send_init_page per collection.
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(PendingInit::<Test>::contains_key(ParaId::from(1000)));
			assert_ok!(do_send_init_page(ParaId::from(1000)));
			assert!(!PendingInit::<Test>::contains_key(ParaId::from(1000)));

			// One empty init XCM per collection.
			let init_idents: Vec<_> = get_sent_xcms()
				.iter()
				.filter_map(|(_, enc)| decode_subscriber_call(enc))
				.filter_map(|c| match c {
					crate::pallet::SubscriberCall::InitializeRingRoots { roots, .. } =>
						Some((roots.identifier, roots.updates.len())),
					_ => None,
				})
				.collect();
			assert_eq!(init_idents.len(), 2);
			assert!(init_idents.contains(&(PEOPLE_IDENTIFIER, 0)));
			assert!(init_idents.contains(&(PEOPLE_LITE_IDENTIFIER, 0)));
		});
	}
}

mod offchain_worker {
	use super::*;

	#[test]
	fn processes_init_and_update_to_one_subscriber() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(5);

			// One subscriber with initialization not done
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// Advancing enough blocks for init to complete
			advance_to_block(5);
			assert!(!PendingInit::<Test>::contains_key(ParaId::from(1000)), "init should complete");

			// One pending update
			TestCollection::people().add_pending_update(0, 1);

			System::reset_events();
			// Moving on
			advance_to_block(10);

			// Update received
			Events::updates_sent().assert_any_received(1000);
		});
	}

	#[test]
	fn multiple_subscribers_receive_updates() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			TestSubscriber::new(3000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1).add_pending_update(1, 1);

			advance_to_block(6);

			Events::updates_sent()
				.assert_any_received(1000)
				.assert_any_received(2000)
				.assert_any_received(3000);
		});
	}

	#[test]
	fn xcm_failure_retried_on_next_cycle() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1);

			// Block 2: offchain worker enqueues updates.
			// Block 3: enqueue applied, offchain worker submits send_batch.
			// Block 4: send_batch applied — but XCM fails, subscriber not marked as received.
			XCM_SEND_SHOULD_FAIL.set(true);
			advance_to_block(4);

			Events::update_send_failed().assert_emitted_for(1000u32);
			assert!(CurrentBatch::<Test>::get().is_some(), "batch still active");
			assert!(!SubscribersWithCurrentBatch::<Test>::contains_key(ParaId::from(1000)));

			// XCM recovers — offchain worker retries send_batch on next cycle.
			XCM_SEND_SHOULD_FAIL.set(false);
			System::reset_events();
			advance_to_block(7);

			Events::updates_sent().assert_any_received(1000);
			assert!(CurrentBatch::<Test>::get().is_none(), "batch cleared after retry success");
		});
	}

	#[test]
	fn enqueue_authorize_priority_increases_with_block() {
		use sp_runtime::transaction_validity::TransactionSource;

		new_test_ext().execute_with(|| {
			// A subscriber and one pending update make `enqueue_updates` authorizable.
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			TestCollection::people().add_pending_update(0, 1);

			let send_page = PageState::<Test>::get().send_page;

			System::set_block_number(7);
			let (early, _) =
				MembersNotifier::authorize_enqueue_updates(TransactionSource::Local, &send_page)
					.expect("authorizes with pending work");

			System::set_block_number(20);
			let (late, _) =
				MembersNotifier::authorize_enqueue_updates(TransactionSource::Local, &send_page)
					.expect("authorizes with pending work");

			// Priority tracks block height so a later retry strictly outbids an earlier,
			// possibly-stranded enqueue for the same page.
			assert_eq!(early.priority, 7);
			assert_eq!(late.priority, 20);
			assert!(late.priority > early.priority);

			// Longevity is finite so a stranded retry self-evicts from the pool.
			assert_eq!(late.longevity, crate::TX_LONGEVITY);
		});
	}

	#[test]
	fn all_authorized_txs_are_local_only_with_finite_longevity() {
		use sp_runtime::transaction_validity::TransactionSource;

		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(100);

			// Subscriber 1000 fully initialized; subscriber 2000 mid-init (many ring
			// roots, one page sent); one pending update available for sealing.
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			assert_ok!(do_send_init_page(ParaId::from(2000)));
			TestCollection::people().add_pending_update(0, 1);

			System::set_block_number(3);

			// With no batch active and pending work, `enqueue_updates` authorizes.
			let (enqueue, _) = MembersNotifier::authorize_enqueue_updates(
				TransactionSource::Local,
				&PageState::<Test>::get().send_page,
			)
			.expect("enqueue authorizes");

			// With subscriber 2000 mid-init, `send_init_page` authorizes.
			let init_state =
				PendingInit::<Test>::get(ParaId::from(2000)).expect("init still pending");
			let (init, _) = MembersNotifier::authorize_send_init_page(
				TransactionSource::Local,
				&ParaId::from(2000),
				&init_state.current_collection_index,
				&init_state.after_ring_index,
			)
			.expect("init authorizes");

			// With a sealed batch not yet delivered to subscriber 1000, `send_batch`
			// authorizes.
			assert_ok!(do_enqueue_updates());
			let sequence = current_batch_sequence();
			let (send, _) = MembersNotifier::authorize_send_batch(
				TransactionSource::Local,
				&ParaId::from(1000),
				&sequence,
			)
			.expect("send authorizes");

			// With the batch stuck past the timeout, `abandon_stuck_batch` authorizes.
			System::set_block_number(3 + StuckBatchTimeout::get());
			let (abandon, _) =
				MembersNotifier::authorize_abandon_stuck_batch(TransactionSource::Local)
					.expect("abandon authorizes");

			for validity in [&enqueue, &init, &send, &abandon] {
				// Peers validate gossiped copies with an external source and would ban the
				// bytes their own offchain worker submits, so propagation is disabled.
				assert!(!validity.propagate);
				// Longevity is finite so stranded retries self-evict from the pool.
				assert_eq!(validity.longevity, crate::TX_LONGEVITY);
				// Priority tracks block height for pool replacement across retry windows.
				assert!(validity.priority > 0);
			}
		});
	}

	#[test]
	fn enqueue_retries_reuse_bytes_within_window() {
		use codec::Decode;
		use frame_support::traits::OffchainWorker;

		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			TestCollection::people().add_pending_update(0, 1);

			// Running the offchain worker without applying its transactions, so the enqueue
			// never lands and the pending work never clears — simulating a tx-pool stall.
			for bn in 1..=20u64 {
				System::set_block_number(bn);
				AllPalletsWithSystem::offchain_worker(bn);
			}

			// Decoding the discriminator of every submitted `enqueue_updates`.
			let discriminators: Vec<u64> = pool_transactions()
				.iter()
				.map(|enc| {
					let xt = Extrinsic::decode(&mut &enc[..]).expect("decodes");
					match xt.function {
						RuntimeCall::MembersNotifier(crate::Call::enqueue_updates {
							discriminator,
							..
						}) => discriminator,
						other => panic!("unexpected call: {other:?}"),
					}
				})
				.collect();

			// Retries within a window share the same bytes
			let expected: Vec<u64> =
				(1..=20u64).map(|bn| bn / crate::TX_RETRY_WINDOW as u64).collect();
			assert_eq!(discriminators, expected);
		});
	}
}

mod stuck_batch_timeout {
	use super::*;

	#[test]
	fn abandon_stuck_batch_clears_batch() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			TestCollection::people().add_pending_update(0, 1);

			// Batch created at block 10.
			System::set_block_number(10);
			assert_ok!(do_enqueue_updates());
			assert!(CurrentBatch::<Test>::get().is_some());

			// Block 50: not yet stuck (timeout=100). Authorize rejects.
			System::set_block_number(50);
			assert!(MembersNotifier::authorize_abandon_stuck_batch(
				sp_runtime::transaction_validity::TransactionSource::Local,
			)
			.is_err());
			assert!(CurrentBatch::<Test>::get().is_some(), "not yet timed out");

			// Block 111: stuck. Abandon succeeds.
			System::set_block_number(111);
			System::reset_events();
			assert_ok!(MembersNotifier::abandon_stuck_batch(authorized_origin(), 0));
			assert!(CurrentBatch::<Test>::get().is_none(), "batch should be abandoned");
			Events::batch_abandoned().assert_count(1);
		});
	}
}

mod subscription {
	use super::*;

	#[test]
	fn first_init_fails_for_unauthorized_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				MembersNotifier::subscribe(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					bounded_vec![(PEOPLE_IDENTIFIER, RingExponent::R2e9)],
					TEST_PALLET_INDEX,
				),
				sp_runtime::DispatchError::BadOrigin
			);
			assert!(!subscriber_exists(1000));
		});
	}

	#[test]
	fn first_init_fails_if_subscribers_list_full() {
		new_test_ext().execute_with(|| {
			for i in 0..10 {
				TestSubscriber::new(i).subscribe_to(&[PEOPLE_IDENTIFIER]);
			}
			assert_eq!(Subscribers::<Test>::count(), 10);

			assert_noop!(
				MembersNotifier::subscribe(
					RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
					ParaId::from(9999),
					bounded_vec![(PEOPLE_IDENTIFIER, RingExponent::R2e9)],
					TEST_PALLET_INDEX,
				),
				Error::<Test>::TooManySubscribers
			);
			assert!(!subscriber_exists(9999));
		});
	}

	#[test]
	fn first_init_succeeds() {
		new_test_ext().execute_with(|| {
			assert_ok!(MembersNotifier::subscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				ParaId::from(1000),
				bounded_vec![(PEOPLE_IDENTIFIER, RingExponent::R2e9)],
				TEST_PALLET_INDEX,
			));

			assert!(subscriber_exists(1000));
			Events::subscribed().assert_count(1).assert_emitted_for(1000);
		});
	}

	#[test]
	fn resubscribe_after_unsubscribe_forwards_new_exponent() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(1);

			// First subscription: register PEOPLE with R2e9 and drain the init XCM.
			TestSubscriber::new(1000)
				.subscribe_to_with_exponents(&[(PEOPLE_IDENTIFIER, RingExponent::R2e9)]);
			assert_ok!(do_send_init_page(ParaId::from(1000)));

			// Unsubscribe.
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000)),
			));
			assert!(!subscriber_exists(1000));

			// Resubscribe with a DIFFERENT exponent and send the init page.
			clear_sent_xcms();
			TestSubscriber::new(1000)
				.subscribe_to_with_exponents(&[(PEOPLE_IDENTIFIER, RingExponent::R2e14)]);
			assert_ok!(do_send_init_page(ParaId::from(1000)));

			// The init XCM after resubscribe carries the NEW exponent, not the old one.
			let init_call = get_sent_xcms()
				.iter()
				.filter_map(|(_, enc)| decode_subscriber_call(enc))
				.find_map(|c| match c {
					crate::pallet::SubscriberCall::InitializeRingRoots { ring_exponent, roots } =>
						Some((roots.identifier, ring_exponent)),
					_ => None,
				})
				.expect("init call must be forwarded");
			assert_eq!(init_call, (PEOPLE_IDENTIFIER, RingExponent::R2e14));
		});
	}

	#[test]
	fn subscribe_rejects_unordered_or_duplicate_collections() {
		new_test_ext().execute_with(|| {
			// PEOPLE_LITE_IDENTIFIER = [1; 32] > PEOPLE_IDENTIFIER = [0; 32], so
			// feeding them in descending order must be rejected.
			let unordered: BoundedVec<_, MaxCollectionsPerSubscriber> = bounded_vec![
				(PEOPLE_LITE_IDENTIFIER, RingExponent::R2e9),
				(PEOPLE_IDENTIFIER, RingExponent::R2e14),
			];
			assert_noop!(
				MembersNotifier::subscribe(
					RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
					ParaId::from(1000),
					unordered,
					TEST_PALLET_INDEX,
				),
				Error::<Test>::InvalidCollectionsList,
			);

			// Same identifier appearing twice (even with different exponents) is a
			// duplicate — also rejected by the strictly-ascending check on the tuple's
			// identifier key.
			let duplicated: BoundedVec<_, MaxCollectionsPerSubscriber> = bounded_vec![
				(PEOPLE_IDENTIFIER, RingExponent::R2e9),
				(PEOPLE_IDENTIFIER, RingExponent::R2e14),
			];
			assert_noop!(
				MembersNotifier::subscribe(
					RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
					ParaId::from(1000),
					duplicated,
					TEST_PALLET_INDEX,
				),
				Error::<Test>::InvalidCollectionsList,
			);

			assert!(!subscriber_exists(1000));
		});
	}

	#[test]
	fn subscribe_fails_if_already_subscribed() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			Events::subscribed().assert_count(1);

			assert_noop!(
				MembersNotifier::subscribe(
					RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
					ParaId::from(1000),
					bounded_vec![
						(PEOPLE_IDENTIFIER, RingExponent::R2e9),
						(PEOPLE_LITE_IDENTIFIER, RingExponent::R2e9),
					],
					TEST_PALLET_INDEX,
				),
				Error::<Test>::AlreadySubscribed
			);
		});
	}
}

mod unsubscription {
	use super::*;

	#[test]
	fn fails_for_unauthorized_origin() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_noop!(
				MembersNotifier::unsubscribe(RuntimeOrigin::signed(1), Some(ParaId::from(1000))),
				sp_runtime::DispatchError::BadOrigin
			);
			assert!(subscriber_exists(1000));
		});
	}

	#[test]
	fn fails_if_subscriber_not_found() {
		new_test_ext().execute_with(|| {
			assert_eq!(Subscribers::<Test>::count(), 0);

			assert_noop!(
				MembersNotifier::unsubscribe(
					RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
					Some(ParaId::from(9999))
				),
				Error::<Test>::SubscriberNotFound
			);
		});
	}

	#[test]
	fn governance_sends_terminate_xcm() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			assert!(subscriber_exists(1000));

			let before = XCM_SEND_COUNT.with(|c| c.get());
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000))
			));
			let after = XCM_SEND_COUNT.with(|c| c.get());

			assert!(!subscriber_exists(1000));
			assert!(after > before, "terminate XCM should have been sent");
			Events::unsubscribed().assert_count(1).assert_emitted_for(1000);
		});
	}

	#[test]
	fn subscriber_initiated_does_not_send_terminate_xcm() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			assert!(subscriber_exists(1000));

			// Root origin resolves to subscriber ParaId(1000) via MockEnsureSubscriberOrigin
			let before = XCM_SEND_COUNT.with(|c| c.get());
			assert_ok!(MembersNotifier::unsubscribe(RuntimeOrigin::root(), None));
			let after = XCM_SEND_COUNT.with(|c| c.get());

			assert!(!subscriber_exists(1000));
			assert_eq!(after, before, "no terminate XCM should be sent back to subscriber");
			Events::unsubscribed().assert_count(1).assert_emitted_for(1000);
		});
	}

	#[test]
	fn governance_succeeds_even_when_terminate_xcm_fails() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();
			assert!(subscriber_exists(1000));

			// XCM send fails — unsubscribe should still succeed (best-effort)
			XCM_SEND_SHOULD_FAIL.with(|f| f.set(true));
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000))
			));
			XCM_SEND_SHOULD_FAIL.with(|f| f.set(false));

			assert!(!subscriber_exists(1000));
			Events::unsubscribed().assert_count(1).assert_emitted_for(1000);
		});
	}

	#[test]
	fn decrements_remaining_in_batch_and_clears_if_last() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			// One pending update and gets enqueued
			TestCollection::people().add_pending_update(0, 1);
			assert_ok!(do_enqueue_updates());

			let current_batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(current_batch.remaining_subscribers, 2);

			// Subscriber 1000 unsubscribes
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000)),
			));
			let current_batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(current_batch.remaining_subscribers, 1);

			// Subscriber 2000 unsubscribes
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(2000)),
			));
			assert!(CurrentBatch::<Test>::get().is_none(), "batch cleared");
		});
	}

	#[test]
	fn already_sent_subscriber_unsubscribe_removes_status() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);
			finalize_subscriptions();

			// One pending update and gets enqueued
			TestCollection::people().add_pending_update(0, 1);
			assert_ok!(do_enqueue_updates());

			// Subscriber 1000 receives the update
			assert_ok!(MembersNotifier::send_batch(
				authorized_origin(),
				ParaId::from(1000),
				current_batch_sequence(),
				0
			));
			assert!(SubscribersWithCurrentBatch::<Test>::contains_key(ParaId::from(1000)));

			// Subscriber 1000 unsubscribes
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000)),
			));

			// and is removed from tracking
			assert!(!SubscribersWithCurrentBatch::<Test>::contains_key(ParaId::from(1000)));

			// remaining_subscribers remains unchanged
			let current_batch = CurrentBatch::<Test>::get().unwrap();
			assert_eq!(current_batch.remaining_subscribers, 1);
		});
	}
}

mod request_replay {
	use super::*;
	use frame_support::BoundedVec;

	fn bounded_indices(indices: &[u32]) -> BoundedVec<u32, MaxUpdatesPerBatch> {
		indices.to_vec().try_into().expect("within bounds")
	}

	#[test]
	fn fails_if_subscriber_not_found() {
		new_test_ext().execute_with(|| {
			assert_eq!(Subscribers::<Test>::count(), 0);

			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(9999),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[0, 1])
				),
				Error::<Test>::SubscriberNotFound
			);
		});
	}

	#[test]
	fn fails_if_not_subscribed_to_collection() {
		new_test_ext().execute_with(|| {
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					PEOPLE_LITE_IDENTIFIER,
					bounded_indices(&[0, 1])
				),
				Error::<Test>::NotSubscribedToCollection
			);
		});
	}

	#[test]
	fn fails_with_empty_ring_root_indices() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[])
				),
				Error::<Test>::EmptyRingIndices
			);

			Events::replay_requested().assert_count(0);
		});
	}

	#[test]
	fn fails_if_ring_index_out_of_range() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(5);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[0, 1, 5])
				),
				Error::<Test>::InvalidRingIndex
			);
		});
	}

	#[test]
	fn fails_if_exceeds_channel_capacity() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// Find minimal channel size where exactly 1 update fits.
			let (mut lo, mut hi) = (1u32, 100_000u32);
			while lo < hi {
				let mid = lo + (hi - lo) / 2;
				set_mock_max_message_size(mid);
				if MembersNotifier::max_updates_per_xcm(ParaId::from(1000)).is_some() {
					hi = mid;
				} else {
					lo = mid + 1;
				}
			}
			set_mock_max_message_size(lo);
			assert_eq!(MembersNotifier::max_updates_per_xcm(ParaId::from(1000)), Some(1));

			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[0, 1])
				),
				Error::<Test>::ExceedsChannelCapacity
			);

			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));
		});
	}

	#[test]
	fn fails_during_cooldown() {
		new_test_ext().execute_with(|| {
			// Subscriber subscribed and ring roots available.
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// First replay succeeds.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0, 1])
			));

			// Second replay within cooldown fails.
			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(2),
					ParaId::from(1000),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[0, 1])
				),
				Error::<Test>::ReplayCooldownActive
			);
		});
	}

	#[test]
	fn succeeds_and_respects_cooldown_window() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// First replay at time 1000.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));

			// Advancing time past the cooldown (60 seconds).
			set_mock_clock_time(1000 + 60);

			// Second replay succeeds.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));

			Events::replay_requested().assert_count(2);
		});
	}

	#[test]
	fn cooldown_is_per_subscriber_and_collection() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER]);
			TestSubscriber::new(2000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// Replay for (1000, PEOPLE) triggers cooldown.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));

			// Same subscriber, different collection — not blocked.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_LITE_IDENTIFIER,
				bounded_indices(&[0])
			));

			// Different subscriber, same collection — not blocked.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(2000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));

			// Original (1000, PEOPLE) still on cooldown.
			assert_noop!(
				MembersNotifier::request_replay(
					RuntimeOrigin::signed(1),
					ParaId::from(1000),
					PEOPLE_IDENTIFIER,
					bounded_indices(&[0])
				),
				Error::<Test>::ReplayCooldownActive
			);
		});
	}

	#[test]
	fn unsubscribe_clears_cooldown() {
		new_test_ext().execute_with(|| {
			set_mock_ring_roots_count(10);
			TestSubscriber::new(1000).subscribe_to(&[PEOPLE_IDENTIFIER]);

			// Replay sets cooldown.
			assert_ok!(MembersNotifier::request_replay(
				RuntimeOrigin::signed(1),
				ParaId::from(1000),
				PEOPLE_IDENTIFIER,
				bounded_indices(&[0])
			));
			assert!(LastReplayTime::<Test>::contains_key(ParaId::from(1000), PEOPLE_IDENTIFIER));

			// Unsubscribing clears the cooldown entry.
			assert_ok!(MembersNotifier::unsubscribe(
				RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
				Some(ParaId::from(1000)),
			));
			assert!(!LastReplayTime::<Test>::contains_key(ParaId::from(1000), PEOPLE_IDENTIFIER));
		});
	}
}

mod on_ring_root_change {
	use super::*;
	use indiv_support::traits::{OnRingRootChange, RingRootOp};

	#[test]
	fn adds_update_to_pending_updates() {
		new_test_ext().execute_with(|| {
			assert_eq!(pending_updates_count(), 0);

			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				5,
				RingRootOp::Built { revision: 1, root: &Default::default() },
			);

			assert!(PendingUpdates::<Test>::contains_key((
				PageState::<Test>::get().write_page,
				PEOPLE_IDENTIFIER,
				5
			)));
			assert_eq!(pending_updates_count(), 1);
		});
	}

	#[test]
	fn deduplicates_same_ring_index_and_collection() {
		new_test_ext().execute_with(|| {
			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				5,
				RingRootOp::Built { revision: 1, root: &Default::default() },
			);
			assert_eq!(pending_updates_count(), 1);

			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				5,
				RingRootOp::Built { revision: 2, root: &Default::default() },
			);
			assert_eq!(pending_updates_count(), 1);
		});
	}

	#[test]
	fn keeps_different_ring_indices_separate() {
		new_test_ext().execute_with(|| {
			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				5,
				RingRootOp::Built { revision: 1, root: &Default::default() },
			);
			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				6,
				RingRootOp::Built { revision: 1, root: &Default::default() },
			);

			assert_eq!(pending_updates_count(), 2);
		});
	}

	#[test]
	fn spills_to_next_page_when_current_full() {
		new_test_ext().execute_with(|| {
			for i in 0..MaxUpdatesPerBatch::get() {
				<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
					PEOPLE_IDENTIFIER,
					i,
					RingRootOp::Built { revision: 1, root: &Default::default() },
				);
			}
			assert_eq!(PageState::<Test>::get().write_page, 0);
			assert_eq!(PageUpdatesCount::<Test>::get(0), MaxUpdatesPerBatch::get());

			<MembersNotifier as OnRingRootChange<_>>::on_ring_root_change(
				PEOPLE_IDENTIFIER,
				MaxUpdatesPerBatch::get(),
				RingRootOp::Built { revision: 1, root: &Default::default() },
			);
			assert_eq!(PageState::<Test>::get().write_page, 1);
			assert_eq!(PageUpdatesCount::<Test>::get(1), 1);
		});
	}
}

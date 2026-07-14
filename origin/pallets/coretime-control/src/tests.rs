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

use super::mock::*;
use crate::{HeldOutbound, HeldRequestIds, Outbound, OutboundRecord, ReceiptStatus, TransportHeld};
use frame_support::{assert_noop, assert_ok};

#[test]
fn requester_allocates_monotonic_ids_and_prunes_bounded_history() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			3,
			ReceiptStatus::Accepted,
		));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 1));
		SENT.with(|sent| assert_eq!(&*sent.borrow(), &[(0, 3), (1, 2), (2, 1)]));
		assert_eq!(crate::NextRequestId::<Test>::get(), 3);
		assert!(!Outbound::<Test>::contains_key(0));
		assert!(Outbound::<Test>::contains_key(1));
		assert!(Outbound::<Test>::contains_key(2));
	});
}

#[test]
fn bounded_history_never_prunes_unresolved_sequence_holes() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_noop!(
			CoretimeControl::request_core_count(RuntimeOrigin::root(), 1),
			crate::Error::<Test>::NoTrackingCapacity
		);
		assert_eq!(crate::NextRequestId::<Test>::get(), 2);
		assert!(Outbound::<Test>::contains_key(0));
		assert!(Outbound::<Test>::contains_key(1));
		assert!(!Outbound::<Test>::contains_key(2));
	});
}

#[test]
fn duplicate_transport_delivery_is_acknowledged_but_applied_once() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 3));
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 1, 2));
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 3));
		APPLIED.with(|applied| assert_eq!(&*applied.borrow(), &[3, 2]));
		RECEIPTS.with(|receipts| {
			let receipts = receipts.borrow();
			assert_eq!(receipts[0].status, ReceiptStatus::Accepted);
			assert_eq!(receipts[1].status, ReceiptStatus::Accepted);
			assert_eq!(receipts[2].status, ReceiptStatus::Duplicate);
		});
	});
}

#[test]
fn retry_reuses_request_id_without_allocating_a_nonce() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::retry_request(RuntimeOrigin::root(), 0));
		SENT.with(|sent| assert_eq!(&*sent.borrow(), &[(0, 3), (0, 3)]));
		assert_eq!(crate::NextRequestId::<Test>::get(), 1);
		assert_noop!(
			CoretimeControl::retry_request(RuntimeOrigin::root(), 9),
			crate::Error::<Test>::UnknownRequest
		);
	});
}

#[test]
fn transport_hold_retains_and_releases_the_exact_envelope() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), true));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		SENT.with(|sent| assert!(sent.borrow().is_empty()));
		assert!(TransportHeld::<Test>::get());
		assert!(HeldOutbound::<Test>::contains_key(0));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[0]);
		assert_eq!(Outbound::<Test>::get(0), Some(OutboundRecord { count: 3, status: None }));
		assert_eq!(crate::NextRequestId::<Test>::get(), 1);

		assert_ok!(CoretimeControl::release_held(RuntimeOrigin::root(), 0));
		SENT.with(|sent| assert_eq!(&*sent.borrow(), &[(0, 3)]));
		assert!(!HeldOutbound::<Test>::contains_key(0));
		assert!(HeldRequestIds::<Test>::get().is_empty());
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), false));
	});
}

#[test]
fn failed_held_release_is_atomic_and_retry_cannot_bypass_the_gate() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), true));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::retry_request(RuntimeOrigin::root(), 0));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[0, 1]);
		SENT.with(|sent| assert!(sent.borrow().is_empty()));
		assert_noop!(
			CoretimeControl::release_held(RuntimeOrigin::root(), 1),
			crate::Error::<Test>::HeldReleaseOutOfOrder
		);

		FAIL_SEND.with(|fail| *fail.borrow_mut() = true);
		assert_noop!(
			CoretimeControl::release_held(RuntimeOrigin::root(), 0),
			sp_runtime::DispatchError::Other("request send failed")
		);
		assert!(HeldOutbound::<Test>::contains_key(0));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[0, 1]);
		assert_eq!(Outbound::<Test>::get(0).unwrap().status, None);
	});
}

#[test]
fn terminal_held_receipt_cannot_leave_a_stale_fifo_head() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), true));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_ok!(CoretimeControl::retry_request(RuntimeOrigin::root(), 0));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[0]);

		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			2,
			ReceiptStatus::Accepted,
		));
		assert!(HeldRequestIds::<Test>::get().is_empty());
		assert!(!HeldOutbound::<Test>::contains_key(0));
		assert_noop!(
			CoretimeControl::retry_request(RuntimeOrigin::root(), 0),
			crate::Error::<Test>::RequestAlreadyTerminal
		);

		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), false));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			1,
			3,
			ReceiptStatus::Accepted,
		));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 4));
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), true));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 5));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[3]);
		assert_ok!(CoretimeControl::release_held(RuntimeOrigin::root(), 3));
		assert!(HeldRequestIds::<Test>::get().is_empty());
	});
}

#[test]
fn first_duplicate_receipt_is_terminal_and_untracks_held_request() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::set_transport_hold(RuntimeOrigin::root(), true));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			2,
			ReceiptStatus::Duplicate,
		));
		assert_eq!(Outbound::<Test>::get(0).unwrap().status, Some(ReceiptStatus::Duplicate));
		assert!(HeldRequestIds::<Test>::get().is_empty());
		assert!(!HeldOutbound::<Test>::contains_key(0));
		assert_noop!(
			CoretimeControl::retry_request(RuntimeOrigin::root(), 0),
			crate::Error::<Test>::RequestAlreadyTerminal
		);

		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_eq!(&*HeldRequestIds::<Test>::get(), &[1]);
		assert_ok!(CoretimeControl::release_held(RuntimeOrigin::root(), 1));
		assert!(HeldRequestIds::<Test>::get().is_empty());
	});
}

#[test]
fn failed_enqueue_does_not_allocate_or_persist_a_request() {
	new_test_ext().execute_with(|| {
		FAIL_SEND.with(|fail| *fail.borrow_mut() = true);
		assert_noop!(
			CoretimeControl::request_core_count(RuntimeOrigin::root(), 3),
			sp_runtime::DispatchError::Other("request send failed")
		);
		assert_eq!(crate::NextRequestId::<Test>::get(), 0);
		assert!(!Outbound::<Test>::contains_key(0));
	});
}

#[test]
fn failed_provider_apply_does_not_advance_replay_ledger() {
	new_test_ext().execute_with(|| {
		FAIL_APPLY.with(|fail| *fail.borrow_mut() = true);
		assert_noop!(
			CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 3),
			sp_runtime::DispatchError::Other("request apply failed")
		);
		assert_eq!(crate::LastApplied::<Test>::get(), None);
		assert!(!crate::Applied::<Test>::contains_key(0));
		RECEIPTS.with(|receipts| assert!(receipts.borrow().is_empty()));
	});
}

#[test]
fn delayed_and_out_of_order_requests_apply_only_in_sequence() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 1, 2));
		APPLIED.with(|applied| assert!(applied.borrow().is_empty()));
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 3));
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 1, 2));
		APPLIED.with(|applied| assert_eq!(&*applied.borrow(), &[3, 2]));
		RECEIPTS.with(|receipts| {
			let receipts = receipts.borrow();
			assert_eq!(receipts[0].status, ReceiptStatus::OutOfOrder);
			assert_eq!(receipts[1].status, ReceiptStatus::Accepted);
			assert_eq!(receipts[2].status, ReceiptStatus::Accepted);
		});
	});
}

#[test]
fn conflicting_replay_is_never_applied() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 3));
		assert_ok!(CoretimeControl::submit_request(RuntimeOrigin::root(), 0, 2));
		APPLIED.with(|applied| assert_eq!(&*applied.borrow(), &[3]));
		RECEIPTS.with(|receipts| {
			assert_eq!(receipts.borrow().last().unwrap().status, ReceiptStatus::Conflict)
		});
	});
}

#[test]
fn receipts_may_arrive_out_of_order_but_must_match_original_count() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 2));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			1,
			2,
			ReceiptStatus::Accepted,
		));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			3,
			ReceiptStatus::Accepted,
		));
		assert_eq!(
			Outbound::<Test>::get(0),
			Some(OutboundRecord { count: 3, status: Some(ReceiptStatus::Accepted) })
		);
		assert_noop!(
			CoretimeControl::acknowledge(RuntimeOrigin::root(), 1, 9, ReceiptStatus::Accepted,),
			crate::Error::<Test>::ReceiptCountMismatch
		);
	});
}

#[test]
fn terminal_receipt_cannot_be_downgraded_by_delayed_negative_status() {
	new_test_ext().execute_with(|| {
		assert_ok!(CoretimeControl::request_core_count(RuntimeOrigin::root(), 3));
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			3,
			ReceiptStatus::Duplicate,
		));
		assert_noop!(
			CoretimeControl::acknowledge(RuntimeOrigin::root(), 0, 3, ReceiptStatus::OutOfOrder,),
			crate::Error::<Test>::ReceiptStatusConflict
		);
		assert_ok!(CoretimeControl::acknowledge(
			RuntimeOrigin::root(),
			0,
			3,
			ReceiptStatus::Accepted,
		));
		assert_eq!(Outbound::<Test>::get(0).unwrap().status, Some(ReceiptStatus::Accepted));
	});
}

#[test]
fn configured_origins_reject_signed_control_calls() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			CoretimeControl::request_core_count(RuntimeOrigin::signed(1), 3),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			CoretimeControl::submit_request(RuntimeOrigin::signed(1), 0, 3),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			CoretimeControl::acknowledge(RuntimeOrigin::signed(1), 0, 3, ReceiptStatus::Accepted,),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			CoretimeControl::set_transport_hold(RuntimeOrigin::signed(1), true),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			CoretimeControl::release_held(RuntimeOrigin::signed(1), 0),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

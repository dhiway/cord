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
//

// tests.rs — unit tests for AuthorityManager
#![cfg(test)]

use super::*;
use crate::{
	mock::{
		new_test_ext, remove_ok, run_to_next_session, AccountId, MockSessionKeys, RuntimeEvent,
		System, Test,
	},
	pallet::{Error as Err, Event as Ev, PendingAdditions, PendingRemovals, Registered},
};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use pallet_session::Pallet as Session;
use sp_runtime::testing::UintAuthorityId;

/// Make NextKeys entry so `nominate` passes SessionKeysNotQueued.
fn insert_next_keys(id: AccountId) {
	pallet_session::NextKeys::<Test>::insert(id, MockSessionKeys::from(UintAuthorityId(id)));
}

#[test]
fn genesis_has_initial_authorities_and_session_keys() {
	let mut ext = new_test_ext(3); // seeds {3,6,9}
	ext.execute_with(|| {
		assert!(!Session::<Test>::validators().is_empty());
	});
}

#[test]
fn nominate_fails_without_keys() {
	let mut ext = new_test_ext(2); // {3,6}
	ext.execute_with(|| {
		let who = 12u64;
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::SessionKeysNotQueued
		);
	});
}

#[test]
fn nominate_succeeds_with_keys_and_persists_across_rotation() {
	let mut ext = new_test_ext(2); // {3,6}
	ext.execute_with(|| {
		let who = 12u64;
		insert_next_keys(who);

		// 1) Nominate: candidate enters pool and is queued
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));
		let reg_now = Registered::<Test>::get();
		assert!(reg_now.contains(&who), "candidate should be in Registered immediately");
		let pending_now = PendingAdditions::<Test>::get();
		assert!(pending_now.contains(&who), "candidate should be queued in PendingAdditions");

		// 2) Rotate once: delta should be applied and queue consumed
		System::reset_events();
		run_to_next_session();

		let reg_after = Registered::<Test>::get();
		assert!(reg_after.contains(&who), "candidate must remain registered after rotation");

		let pending_after = PendingAdditions::<Test>::get();
		assert!(!pending_after.contains(&who), "queued add should be consumed on rotation");

		// no duplicates in Registered for `who`
		assert_eq!(
			reg_after.iter().filter(|&&id| id == who).count(),
			1,
			"candidate should not be inserted twice"
		);
	});
}

#[test]
fn planned_is_emitted_when_there_are_deltas() {
	let mut ext = new_test_ext(2); // e.g., {3,6}
	ext.execute_with(|| {
		let add = 12u64;
		insert_next_keys(add);
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), add));
		assert_ok!(crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), 6));

		System::reset_events();
		run_to_next_session();

		let saw_planned = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityManager(Ev::Planned { .. })));
		assert!(saw_planned, "Expected a Planned event when deltas are present");
	});
}

#[test]
fn duplicate_nominate_in_same_block_errors_as_member_and_is_enacted_next_session() {
	let mut ext = new_test_ext(1);
	ext.execute_with(|| {
		let who = 12;
		insert_next_keys(who);

		// 1st nominate OK
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));

		// 2nd nominate (same block) → AlreadyMember
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::AlreadyMember
		);

		// Still Registered & queued
		assert!(Registered::<Test>::get().contains(&who));
		assert!(PendingAdditions::<Test>::get().contains(&who));

		// After rotation, re-nomination → AlreadyMember
		run_to_next_session();
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::AlreadyMember
		);
	});
}

#[test]
fn remove_non_member_fails() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		let who = 999;
		assert_noop!(
			crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), who),
			Err::<Test>::NotMember
		);
	});
}

#[test]
fn remove_member_enacts_next_session() {
	let mut ext = new_test_ext(2); // {3,6}
	ext.execute_with(|| {
		remove_ok(6);
		run_to_next_session();
		assert!(!Registered::<Test>::get().contains(&6));
	});
}

#[test]
fn remove_respects_floor_guard() {
	let mut ext = new_test_ext(1); // start with exactly one
	ext.execute_with(|| {
		let only = Session::<Test>::validators()[0];
		let reg_len = Registered::<Test>::get().len() as u32;
		let min = <Test as crate::pallet::Config>::MinAuthorities::get();
		let res = crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), only);
		if reg_len.saturating_sub(1) < min {
			assert_noop!(res, Err::<Test>::TooLowAuthorityCount);
		} else {
			assert_ok!(res);
		}
	});
}

#[test]
fn enacted_emitted_only_when_set_changes() {
	let mut ext = new_test_ext(2); // e.g., initial active {3,6}
	ext.execute_with(|| {
		// 0) Rotate with no changes: Enacted should reflect actual change
		System::reset_events();
		let active_before = Session::<Test>::validators();
		run_to_next_session();
		let active_after = Session::<Test>::validators();

		let enacted_payload_1: Option<Vec<AccountId>> =
			System::events().iter().find_map(|r| match &r.event {
				RuntimeEvent::AuthorityManager(Ev::Enacted { active }) => Some(active.clone()),
				_ => None,
			});

		let changed_1 = enacted_payload_1.as_ref().map(|v| v != &active_before).unwrap_or(false);
		let enacted_1 = enacted_payload_1.is_some();
		assert_eq!(enacted_1, changed_1, "Enacted must reflect whether the set changed");

		if let Some(payload) = enacted_payload_1 {
			// On the *next* boundary, validators should match the payload we saw
			System::reset_events();
			run_to_next_session();
			assert_eq!(
				Session::<Test>::validators(),
				payload,
				"validators() should match the last Enacted payload on next rotation"
			);
		} else {
			// no Enacted → no change expected
			assert_eq!(active_before, active_after);
		}

		// 1) Force a change by adding a new validator
		let who = 99u64;
		insert_next_keys(who);
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));

		System::reset_events();
		let active_before2 = Session::<Test>::validators();
		run_to_next_session();

		// Read Enacted payload
		let enacted_payload_2: Option<Vec<AccountId>> =
			System::events().iter().find_map(|r| match &r.event {
				RuntimeEvent::AuthorityManager(Ev::Enacted { active }) => Some(active.clone()),
				_ => None,
			});

		assert!(enacted_payload_2.is_some(), "expected Enacted event when the set changed");
		let payload2 = enacted_payload_2.unwrap();
		assert_ne!(payload2, active_before2, "payload must differ from old set");
		assert!(payload2.contains(&who), "payload must include the newcomer");

		// 2) On the next boundary, validators() should equal payload
		System::reset_events();
		run_to_next_session();
		assert_eq!(
			Session::<Test>::validators(),
			payload2,
			"validators() should match Enacted payload on next rotation"
		);
	});
}

#[test]
fn planned_not_emitted_when_no_deltas() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		System::reset_events();
		run_to_next_session();
		let saw_planned = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityManager(Ev::Planned { .. })));
		assert!(!saw_planned, "no deltas => no Planned");
	});
}

#[test]
fn pending_removals_consumed_on_rotation() {
	let mut ext = new_test_ext(2); // {3,6}
	ext.execute_with(|| {
		assert_ok!(crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), 6));
		assert!(PendingRemovals::<Test>::get().contains(&6));
		System::reset_events();
		run_to_next_session();
		assert!(!PendingRemovals::<Test>::get().contains(&6), "remove queue should be consumed");
	});
}

#[test]
fn duplicate_nominate_in_same_block_is_already_member() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		let who = 42u64;
		insert_next_keys(who);
		// First call queues + registers
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));
		// Second call in same block finds it in the pending list
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::AlreadyMember
		);
	});
}

#[test]
fn members_without_keys_are_not_selected() {
	let mut ext = new_test_ext(2); // {3,6} already have keys
	ext.execute_with(|| {
		// nominate a new account but DO NOT insert keys
		let who = 77u64;
		// Intentionally skip insert_next_keys(who);
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::SessionKeysNotQueued
		);

		// If we force it into Registered manually (not via extrinsic), it still shouldn't join
		// active
		Registered::<Test>::mutate(|r| {
			if !r.contains(&who) {
				r.push(who);
			}
		});
		System::reset_events();
		run_to_next_session();
		assert!(!Session::<Test>::validators().contains(&who), "no keys => not in active");
	});
}

#[test]
fn last_active_updates_on_enacted() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		// add one
		let who = 99u64;
		insert_next_keys(who);
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));

		System::reset_events();
		run_to_next_session();

		let enacted = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityManager(Ev::Enacted { .. })));
		assert!(enacted, "expected Enacted after change");

		// On next boundary, `LastActive` must match current validators
		System::reset_events();
		run_to_next_session();
		let active = Session::<Test>::validators();
		let last = crate::pallet::LastActive::<Test>::get();
		assert_eq!(last, active, "LastActive should mirror the current active set");
	});
}

#[test]
fn planned_is_not_emitted_in_same_block_as_calls() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		insert_next_keys(12);
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), 12));
		assert_ok!(crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), 6));
		let saw_planned = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityManager(Ev::Planned { .. })));
		assert!(!saw_planned, "Planned must only be emitted at rotation");
	});
}

#[test]
fn registered_has_no_duplicates_after_many_rotations() {
	let mut ext = new_test_ext(3);
	ext.execute_with(|| {
		for i in 0..10 {
			let who = 1000 + i;
			insert_next_keys(who);
			let _ = crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who);
			run_to_next_session();
		}
		let reg = Registered::<Test>::get();
		let mut set = std::collections::BTreeSet::new();
		for v in &reg {
			assert!(set.insert(*v), "duplicate in Registered: {}", v);
		}
	});
}

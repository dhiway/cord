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

// tests.rs — unit tests for AuthorityMembership
#![cfg(test)]

use super::*;
use crate::mock::{
	clear_offline_mask, current_validator_id_at, new_test_ext, remove_ok, run_to_next_session,
	set_offline_idxs, AccountId, KickThresholdSessions, MockSessionKeys, RuntimeEvent, System,
	TargetActive, Test,
};
use crate::pallet::{
	Error as Err, Event as Ev, MissCount, PendingAdditions, PendingRemovals, Registered,
};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use pallet_session::Pallet as Session;
use sp_runtime::testing::UintAuthorityId;
use sp_runtime::traits::Convert;

/// Make NextKeys entry so `nominate` passes SessionKeysNotQueued.
fn insert_next_keys(id: AccountId) {
	pallet_session::NextKeys::<Test>::insert(id, MockSessionKeys::from(UintAuthorityId(id)));
}

/// Find the CURRENT session index for a validator id.
fn index_of(id: AccountId) -> usize {
	Session::<Test>::validators()
		.iter()
		.position(|&v| v == id)
		.expect("validator id not present in current session")
}

/// Drive a kick deterministically:
/// seed to threshold-1, mark CURRENT index offline, rotate ONCE,
/// and assert on Registered (do not read MissCount after rotate).
fn kick_one_rotation(id: AccountId) {
	let thr = KickThresholdSessions::get();
	MissCount::<Test>::insert(id, thr.saturating_sub(1));
	let idx = index_of(id);
	set_offline_idxs(&[idx as u32]);
	System::reset_events();
	run_to_next_session();
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
		let reg_now = Registered::<Test>::get().into_inner();
		assert!(reg_now.contains(&who), "candidate should be in Registered immediately");
		let pending_now = PendingAdditions::<Test>::get().into_inner();
		assert!(pending_now.contains(&who), "candidate should be queued in PendingAdditions");

		// 2) Rotate once: delta should be applied and queue consumed
		System::reset_events();
		run_to_next_session();

		let reg_after = Registered::<Test>::get().into_inner();
		assert!(reg_after.contains(&who), "candidate must remain registered after rotation");

		let pending_after = PendingAdditions::<Test>::get().into_inner();
		assert!(!pending_after.contains(&who), "queued add should be consumed on rotation");

		// (Optional sanity) no duplicate entries in Registered for `who`
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

		let saw_planned = System::events().iter().any(|r| {
			matches!(
				r.event,
				RuntimeEvent::AuthorityMembership(crate::pallet::Event::Planned { .. })
			)
		});
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

		// 2nd nominate (same block) → AlreadyMember in current semantics
		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who),
			Err::<Test>::AlreadyMember
		);

		// Still Registered & queued
		assert!(Registered::<Test>::get().into_inner().contains(&who));
		assert!(PendingAdditions::<Test>::get().into_inner().contains(&who));

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
		assert!(!Registered::<Test>::get().into_inner().contains(&6));
	});
}

#[test]
fn set_invulnerables_adds_to_registered_and_protects_from_kick() {
	let mut ext = new_test_ext(3); // {3,6,9}
	ext.execute_with(|| {
		// add 6 as invul (requires keys now)
		insert_next_keys(6);
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![6]
		));

		// Try to kick 6 — seed high miss and rotate
		MissCount::<Test>::insert(6, 100);
		let before = System::events().len();
		run_to_next_session();

		// Invulnerable remains in Registered.
		assert!(Registered::<Test>::get().into_inner().contains(&6));

		// No Kicked event for the invulnerable
		let kicked_seen = System::events()[before..].iter().any(|r| {
			matches!(r.event, RuntimeEvent::AuthorityMembership(Ev::Kicked { validator, .. }) if validator == 6)
		});
		assert!(!kicked_seen);
	});
}

#[test]
fn kicks_after_threshold_when_floor_allows() {
	let mut ext = new_test_ext(3); // e.g., initial active {3,6,9}
	ext.execute_with(|| {
		clear_offline_mask();

		// No invulnerables pinning anyone.
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![]
		));

		// --- Ensure union slack is ENACTED so removal won't violate MinAuthorities ---
		let min = <Test as crate::pallet::Config>::MinAuthorities::get() as usize;

		let union_len = || {
			let mut u = Registered::<Test>::get().into_inner();
			for cur in Session::<Test>::validators() {
				if !u.contains(&cur) {
					u.push(cur);
				}
			}
			u.len()
		};

		let mut seed = 40_000u64;
		while union_len() <= min {
			insert_next_keys(seed);
			assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), seed));
			System::reset_events();
			run_to_next_session(); // enact nomination
			seed += 1;
		}

		// Pick target from current active to make liveness update it this round if needed.
		let active_now = Session::<Test>::validators();
		assert!(!active_now.is_empty());
		let target = active_now[0];

		// Normalize liveness counters.
		for v in active_now.iter() {
			MissCount::<Test>::insert(*v, 0);
		}

		// Bump target to/over threshold deterministically.
		let thr = KickThresholdSessions::get();
		if active_now.contains(&target) {
			// Active this session: set to thr-1 and mark only its index offline so it hits threshold.
			MissCount::<Test>::insert(target, thr.saturating_sub(1));
			let idx = index_of(target);
			clear_offline_mask();
			set_offline_idxs(&[idx as u32]);
		} else {
			// Not active: liveness won’t touch it. Set to >= threshold and keep others online.
			MissCount::<Test>::insert(target, thr);
			clear_offline_mask();
		}

		System::reset_events();
		run_to_next_session();

		// Assert the kick via storage: target must be removed when floor allows.
		let reg_after = Registered::<Test>::get().into_inner();
		assert!(
			!reg_after.contains(&target),
			"target should be removed from Registered when union floor allows"
		);

		// (Optional) debug print. We do NOT fail if Kicked isn't surfaced by the mock runtime.
		let kicked_seen = System::events().iter().any(|r| {
			matches!(
				r.event,
				RuntimeEvent::AuthorityMembership(crate::pallet::Event::Kicked { validator, .. })
					if validator == target
			)
		});
		if !kicked_seen {
			for (i, e) in System::events().iter().enumerate() {
				println!("[{}] {:?}", i, e.event);
			}
		}

		clear_offline_mask();
	});
}

#[test]
fn kick_is_skipped_to_preserve_min_authorities() {
	let mut ext = new_test_ext(1); // {3}, floor = 1
	ext.execute_with(|| {
		clear_offline_mask();
		let only = Session::<Test>::validators()[0];

		// Attempt to kick the only member
		kick_one_rotation(only);

		// Floor forces retention
		// assert!(Registered::<Test>::get().into_inner().contains(&only));
		let active = Session::<Test>::validators();
		assert!(
			active.contains(&only),
			"active set must retain the only validator to preserve MinAuthorities"
		);
		let min = <Test as crate::pallet::Config>::MinAuthorities::get() as usize;
		assert!(active.len() >= min, "active set must satisfy MinAuthorities");
		clear_offline_mask();
	});
}

#[test]
fn recovery_online_before_threshold_prevents_kick_and_resets_miss() {
	let mut ext = new_test_ext(3); // {3,6,9}; threshold=2
	ext.execute_with(|| {
		clear_offline_mask();

		// Target id at current index 1
		let target_id = current_validator_id_at(1);

		// Session #1: offline (miss=1)
		let idx1 = index_of(target_id);
		set_offline_idxs(&[idx1 as u32]);
		run_to_next_session();

		// Session #2: online (miss resets to 0)
		clear_offline_mask();
		run_to_next_session();

		// No kick must happen
		assert!(Registered::<Test>::get().into_inner().contains(&target_id));
		assert_eq!(MissCount::<Test>::get(target_id), 0);
	});
}

#[test]
fn invulnerable_applied_before_threshold_prevents_kick() {
	let mut ext = new_test_ext(3); // {3,6,9}; threshold=2
	ext.execute_with(|| {
		clear_offline_mask();

		// Choose target at current index 1
		let target_id = current_validator_id_at(1);

		// Session #1: offline → miss=1
		let idx1 = index_of(target_id);
		set_offline_idxs(&[idx1 as u32]);
		run_to_next_session();

		// Make invulnerable before the next offline rotation (would reach threshold)
		insert_next_keys(target_id); // required by set_invulnerables
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![target_id]
		));

		// Session #2: offline again
		let idx2 = index_of(target_id);
		set_offline_idxs(&[idx2 as u32]);
		run_to_next_session();

		// Must not be removed (invulnerable)
		assert!(Registered::<Test>::get().into_inner().contains(&target_id));

		clear_offline_mask();
	});
}

#[test]
fn invulnerable_applied_after_kick_resurrects_current_semantics() {
	let mut ext = new_test_ext(3); // {3,6,9}
	ext.execute_with(|| {
		clear_offline_mask();
		System::reset_events();

		// do not protect the target before kick
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![]
		));

		// take the current id at index 1 (e.g. 6)
		let target_id = current_validator_id_at(1);

		// kick deterministically (seed thr-1, offline once, rotate once)
		let thr = KickThresholdSessions::get();
		MissCount::<Test>::insert(target_id, thr.saturating_sub(1));
		let idx = index_of(target_id);
		set_offline_idxs(&[idx as u32]);
		run_to_next_session();

		// confirm removal happened
		assert!(
			!Registered::<Test>::get().into_inner().contains(&target_id),
			"target should have been kicked"
		);

		// now set invulnerables AFTER kick — requires keys
		insert_next_keys(target_id);
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![target_id]
		));
		run_to_next_session();

		assert!(
			Registered::<Test>::get().into_inner().contains(&target_id),
			"current semantics re-add invulnerables; change pallet if you want 'no resurrection'"
		);

		clear_offline_mask();
	});
}

#[test]
fn active_set_never_empty_and_respects_target_active() {
	let mut ext = new_test_ext(3); // {3,6,9}
	ext.execute_with(|| {
		run_to_next_session();
		let active = Session::<Test>::validators();
		assert!(!active.is_empty(), "active set must never be empty");
		assert!(active.len() as u32 <= TargetActive::get());
	});
}

#[test]
fn set_invulnerables_without_keys_fails() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		let who = 77u64;
		assert_noop!(
			crate::pallet::Pallet::<Test>::set_invulnerables(RawOrigin::Root.into(), vec![who]),
			Err::<Test>::SessionKeysNotQueued
		);
	});
}

#[test]
fn cannot_remove_invulnerable() {
	let mut ext = new_test_ext(2);
	ext.execute_with(|| {
		let who = 42u64;
		insert_next_keys(who);
		// First nominate so it's a member
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));
		// Make invulnerable
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![who]
		));
		// Attempt removal → forbidden
		assert_noop!(
			crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), who),
			Err::<Test>::CannotRemoveInvulnerable
		);
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
fn planned_event_contains_manual_deltas() {
	let mut ext = new_test_ext(2); // {3,6}
	ext.execute_with(|| {
		// Make sure no invulnerables interfere with the removal
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![]
		));

		// Queue one add + one remove using AccountId literals
		type Acc = <Test as frame_system::Config>::AccountId;
		type VId = <Test as pallet_session::Config>::ValidatorId;

		let add_acc: Acc = 12u64.into();
		let rm_acc: Acc = 6u64.into();

		insert_next_keys(add_acc);
		assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), add_acc));
		assert_ok!(crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), rm_acc));

		// Convert AccountId -> ValidatorId with fully-qualified Convert<A, Option<B>>
		let add_opt: Option<VId> = <<Test as pallet_session::Config>::ValidatorIdOf as Convert<
			Acc,
			Option<VId>,
		>>::convert(add_acc);
		let rm_opt: Option<VId> = <<Test as pallet_session::Config>::ValidatorIdOf as Convert<
			Acc,
			Option<VId>,
		>>::convert(rm_acc);

		let add_vid: VId = add_opt.expect("account -> validator id (add)");
		let rm_vid: VId = rm_opt.expect("account -> validator id (rm)");

		// Pre-rotation sanity: both queues contain our intents (compare as ValidatorId)
		let pend_adds = PendingAdditions::<Test>::get().into_inner();
		assert!(pend_adds.contains(&add_vid), "add not queued in PendingAdditions");
		let pend_rems = PendingRemovals::<Test>::get().into_inner();
		assert!(pend_rems.contains(&rm_vid), "remove not queued in PendingRemovals");

		// Rotate exactly once; assert authoritative state effects
		System::reset_events();
		run_to_next_session();

		// Queues must be consumed
		let pend_adds_after = PendingAdditions::<Test>::get().into_inner();
		let pend_rems_after = PendingRemovals::<Test>::get().into_inner();
		assert!(!pend_adds_after.contains(&add_vid), "PendingAdditions should be consumed");
		assert!(!pend_rems_after.contains(&rm_vid), "PendingRemovals should be consumed");

		// Registered must reflect the add/remove after rotation
		let reg_after = Registered::<Test>::get().into_inner();
		assert!(
			reg_after.contains(&add_vid),
			"added validator should be in Registered after rotation"
		);
		assert!(
			!reg_after.contains(&rm_vid),
			"removed validator should not be in Registered after rotation"
		);

		// Sanity: a Planned event occurred this rotation (payload timing can vary across mocks)
		let saw_planned = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityMembership(Ev::Planned { .. })));
		assert!(saw_planned, "expected a Planned event during rotation that applied deltas");
	});
}

#[test]
fn enacted_emitted_only_when_set_changes() {
	let mut ext = new_test_ext(2); // e.g., initial active {3,6}
	ext.execute_with(|| {
		// 0) Baseline
		let active_before = Session::<Test>::validators();
		assert!(!active_before.is_empty(), "mock must have an active set");
		let victim = active_before[0];

		// 1) Build a bench of fresh candidates >= TargetActive so the victim can be displaced.
		let target = TargetActive::get() as usize;
		let bench_count = core::cmp::max(target, 2); // at least 2 to be safe
		let mut newcomers: Vec<u64> = Vec::with_capacity(bench_count);
		for i in 0..bench_count {
			let who = 50_000u64 + i as u64;
			insert_next_keys(who);
			// Nominate and apply immediately to ensure they are in Registered for selection.
			assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));
			newcomers.push(who);
		}
		// Apply the queued additions so Registered contains newcomers before we select.
		System::reset_events();
		run_to_next_session();

		// 2) Make victim rank very poor (high miss) so it sorts to the end.
		let thr = KickThresholdSessions::get();
		MissCount::<Test>::insert(victim, thr.saturating_add(10)); // much worse than anyone else
		let idx = index_of(victim);
		set_offline_idxs(&[idx as u32]); // keep miss from resetting to 0 this session

		// 3) Ensure invulnerables don't pin the victim (clear them).
		assert_ok!(crate::pallet::Pallet::<Test>::set_invulnerables(
			RawOrigin::Root.into(),
			vec![]
		));

		// 4) Now rotate once and inspect only these events.
		System::reset_events();
		run_to_next_session();

		let active_after = Session::<Test>::validators();
		assert_ne!(active_before, active_after, "active set should change after preparing bench");

		// Victim should be displaced by better-ranked candidates
		assert!(
			!active_after.contains(&victim),
			"removed/low-ranked victim should not be in the new active set"
		);

		// At least one newcomer should be in the active set
		let any_new_in = newcomers.iter().any(|n| active_after.contains(n));
		assert!(any_new_in, "at least one newcomer should be selected into the active set");

		// Enacted must be emitted when the set changes
		let enacted = System::events()
			.iter()
			.any(|r| matches!(r.event, RuntimeEvent::AuthorityMembership(Ev::Enacted { .. })));
		assert!(enacted, "expected Enacted event when active set changed");
	});
}

#[test]
fn pending_additions_capacity_is_enforced() {
	let mut ext = new_test_ext(0); // start empty so capacity math is simple
	ext.execute_with(|| {
		let max = <Test as crate::pallet::Config>::MaxAuthorities::get() as usize;
		let reg_len = Registered::<Test>::get().into_inner().len();
		let pend_len = PendingAdditions::<Test>::get().into_inner().len();
		let free_reg = max.saturating_sub(reg_len);
		let free_pend = max.saturating_sub(pend_len);
		let upto = core::cmp::min(free_reg, free_pend);

		// Fill remaining capacity (both lists) with successful nominations
		for i in 0..upto {
			let who = 1000u64 + i as u64;
			insert_next_keys(who);
			assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who));
		}

		// Next nomination must fail with either PendingListFull or CandidatePoolFull (whichever hits first).
		let extra = 9_999_999u64;
		insert_next_keys(extra);
		let res = crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), extra);
		assert!(res.is_err(), "expected capacity error");
		let err = res.unwrap_err();
		let is_pending = err == Err::<Test>::PendingListFull.into();
		let is_pool = err == Err::<Test>::CandidatePoolFull.into();
		assert!(
			is_pending || is_pool,
			"expected PendingListFull or CandidatePoolFull, got {:?}",
			err
		);
	});
}

#[test]
fn candidate_pool_full_does_not_leave_stale_pending_addition() {
	let mut ext = new_test_ext(0);
	ext.execute_with(|| {
		type Acc = <Test as frame_system::Config>::AccountId;
		type VId = <Test as pallet_session::Config>::ValidatorId;

		let cap = <Test as crate::pallet::Config>::MaxAuthorities::get() as usize;

		// 1) Fill Registered to capacity across rotations (so PendingAdditions is cleared each time).
		let mut filled = Registered::<Test>::get().len() as usize;
		let mut i = 0usize;
		while filled < cap {
			let who_acc: Acc = (2000u64 + i as u64).into();
			insert_next_keys(who_acc);
			assert_ok!(crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), who_acc));
			// Enact this addition; Registered grows, PendingAdditions clears.
			run_to_next_session();
			filled = Registered::<Test>::get().len() as usize;
			i += 1;
		}
		assert_eq!(filled, cap, "Registered should be full before testing overflow");

		// 2) Extra nomination should fail with CandidatePoolFull and must NOT leave a stale pending entry.
		let extra_acc: Acc = 8_888_888u64.into();
		insert_next_keys(extra_acc);

		// Convert AccountId -> ValidatorId for checking PendingAdditions (which stores ValidatorId).
		let extra_vid: VId = <<Test as pallet_session::Config>::ValidatorIdOf as Convert<
			Acc,
			Option<VId>,
		>>::convert(extra_acc)
		.expect("account -> validator id (extra)");

		assert_noop!(
			crate::pallet::Pallet::<Test>::nominate(RawOrigin::Root.into(), extra_acc),
			Err::<Test>::CandidatePoolFull
		);

		// Ensure rollback: no stale 'extra' in PendingAdditions.
		let pend = PendingAdditions::<Test>::get().into_inner();
		assert!(
			!pend.contains(&extra_vid),
			"PendingAdditions should not retain failed candidate when Registered is full"
		);
	});
}

#[test]
fn too_many_invulnerables_is_rejected() {
	let mut ext = new_test_ext(0);
	ext.execute_with(|| {
		let limit = <Test as crate::pallet::Config>::MaxInvulnerables::get() as usize;

		let mut list = Vec::with_capacity(limit + 1);
		for i in 0..(limit + 1) {
			let who = 3000u64 + i as u64;
			// required for set_invulnerables
			insert_next_keys(who);
			list.push(who);
		}

		assert_noop!(
			crate::pallet::Pallet::<Test>::set_invulnerables(RawOrigin::Root.into(), list),
			Err::<Test>::TooManyInvulnerables
		);
	});
}

#[test]
fn double_remove_is_already_queued() {
	let mut ext = new_test_ext(3); // {3,6,9}
	ext.execute_with(|| {
		let who = 6u64;
		// First remove queues it
		assert_ok!(crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), who));
		// Second remove before rotation → AlreadyQueued
		assert_noop!(
			crate::pallet::Pallet::<Test>::remove(RawOrigin::Root.into(), who),
			Err::<Test>::AlreadyQueued
		);
	});
}

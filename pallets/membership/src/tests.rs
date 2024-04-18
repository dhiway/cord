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

use crate as pallet_membership;

use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok, assert_storage_noop};
use sp_runtime::{bounded_vec, traits::BadOrigin, BuildStorage};

#[test]
fn query_membership_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Members::get(), vec![10, 20, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), vec![10, 20, 30]);
	});
}

#[test]
fn prime_member_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(Membership::set_prime(RuntimeOrigin::signed(4), 20), BadOrigin);
		assert_noop!(
			Membership::set_prime(RuntimeOrigin::signed(5), 15),
			Error::<Test, _>::NotMember
		);
		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 20));
		assert_eq!(Prime::get(), Some(20));
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());

		assert_ok!(Membership::clear_prime(RuntimeOrigin::signed(5)));
		assert_eq!(Prime::get(), None);
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
	});
}

#[test]
fn add_member_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 10, true));
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 15, true));
		assert_noop!(Membership::add_member(RuntimeOrigin::signed(5), 15), BadOrigin);
		assert_noop!(
			Membership::add_member(RuntimeOrigin::signed(1), 10),
			Error::<Test, _>::AlreadyMember
		);
		assert_ok!(Membership::add_member(RuntimeOrigin::signed(1), 15));
		assert_eq!(Members::get(), vec![10, 15, 20, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
	});
}

#[test]
fn remove_member_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(Membership::remove_member(RuntimeOrigin::signed(5), 20), BadOrigin);
		assert_noop!(
			Membership::remove_member(RuntimeOrigin::signed(2), 15),
			Error::<Test, _>::NotMember
		);
		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 20));
		assert_ok!(Membership::remove_member(RuntimeOrigin::signed(2), 20));
		assert_eq!(Members::get(), vec![10, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
		assert_eq!(Prime::get(), None);
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
	});
}

#[test]
fn swap_member_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 20, true));
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 25, true));
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 30, true));
		assert_noop!(Membership::swap_member(RuntimeOrigin::signed(5), 10, 25), BadOrigin);
		assert_noop!(
			Membership::swap_member(RuntimeOrigin::signed(3), 15, 25),
			Error::<Test, _>::NotMember
		);
		assert_noop!(
			Membership::swap_member(RuntimeOrigin::signed(3), 10, 30),
			Error::<Test, _>::AlreadyMember
		);

		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 20));
		assert_ok!(Membership::swap_member(RuntimeOrigin::signed(3), 20, 20));
		assert_eq!(Members::get(), vec![10, 20, 30]);
		assert_eq!(Prime::get(), Some(20));
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());

		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 10));
		assert_ok!(Membership::swap_member(RuntimeOrigin::signed(3), 10, 25));
		assert_eq!(Members::get(), vec![20, 25, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
		assert_eq!(Prime::get(), None);
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
	});
}

#[test]
fn swap_member_works_that_does_not_change_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 5, true));
		assert_ok!(Membership::swap_member(RuntimeOrigin::signed(3), 10, 5));
		assert_eq!(Members::get(), vec![5, 20, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
	});
}

#[test]
fn swap_member_with_identical_arguments_changes_nothing() {
	new_test_ext().execute_with(|| {
		assert_storage_noop!(assert_ok!(Membership::swap_member(RuntimeOrigin::signed(3), 10, 10)));
	});
}

#[test]
fn change_key_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 20, true));
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 25, true));
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 40, true));
		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 10));
		assert_noop!(
			Membership::change_key(RuntimeOrigin::signed(3), 25),
			Error::<Test, _>::NotMember
		);
		assert_noop!(
			Membership::change_key(RuntimeOrigin::signed(10), 20),
			Error::<Test, _>::AlreadyMember
		);
		assert_ok!(Membership::change_key(RuntimeOrigin::signed(10), 40));
		assert_eq!(Members::get(), vec![20, 30, 40]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
		assert_eq!(Prime::get(), Some(40));
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
	});
}

#[test]
fn change_key_works_that_does_not_change_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(NetworkMembership::nominate(RuntimeOrigin::root(), 5, true));
		assert_ok!(Membership::change_key(RuntimeOrigin::signed(10), 5));
		assert_eq!(Members::get(), vec![5, 20, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
	});
}
#[test]
fn change_key_with_same_caller_as_argument_changes_nothing() {
	new_test_ext().execute_with(|| {
		assert_storage_noop!(assert_ok!(Membership::change_key(RuntimeOrigin::signed(10), 10)));
	});
}

#[test]
fn reset_members_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Membership::set_prime(RuntimeOrigin::signed(5), 20));
		assert_noop!(
			Membership::reset_members(RuntimeOrigin::signed(1), bounded_vec![20, 40, 30]),
			BadOrigin
		);

		assert_ok!(Membership::reset_members(RuntimeOrigin::signed(4), vec![20, 40, 30]));
		assert_eq!(Members::get(), vec![20, 30, 40]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
		assert_eq!(Prime::get(), Some(20));
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
		assert_ok!(Membership::reset_members(RuntimeOrigin::signed(4), vec![10, 40, 30]));
		assert_eq!(Members::get(), vec![10, 30, 40]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()), Members::get().to_vec());
		assert_eq!(Prime::get(), None);
		assert_eq!(PRIME.with(|m| *m.borrow()), Prime::get());
	});
}

#[test]
#[should_panic(expected = "Members cannot contain duplicate accounts.")]
fn genesis_build_panics_with_duplicate_members() {
	pallet_membership::GenesisConfig::<Test> {
		members: bounded_vec![1, 2, 3, 1],
		phantom: Default::default(),
	}
	.build_storage()
	.unwrap();
}

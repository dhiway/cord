// CORD Blockchain – https://dhiway.network
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::{mock::*, Error, Event, MemberData, Members};

use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;
use network_membership::MembersCount;

#[test]
fn test_genesis_build() {
	new_test_ext().execute_with(|| {
		run_to_block(1);
		// Verify state
		assert_eq!(
			Members::<Test>::get(AccountId::new([11u8; 32])),
			Some(MemberData { expire_on: 5 })
		);
		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_add_member_request() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		// This ensures that the account was successfully added
		assert_eq!(
			Members::<Test>::get(AccountId::new([13u8; 32])),
			Some(MemberData { expire_on: 1 + MembershipPeriod::get() })
		);

		assert_eq!(NetworkMembership::members_count(), 2);
	});
}

#[test]
fn test_add_member_request_non_authority() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert_err!(
			NetworkMembership::nominate(
				RuntimeOrigin::signed(AccountId::new([11u8; 32])),
				AccountId::new([13u8; 32]),
				true
			),
			BadOrigin
		);

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_duplicate_member_request() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert_err!(
			NetworkMembership::nominate(RawOrigin::Root.into(), AccountId::new([11u8; 32]), true),
			Error::<Test>::MembershipAlreadyAcquired
		);
	});
}

#[test]
fn test_renew_membership() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert!(NetworkMembership::is_member(&AccountId::new([11u8; 32])));

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(2);
		assert_ok!(NetworkMembership::renew(RawOrigin::Root.into(), AccountId::new([13u8; 32]),));

		System::assert_has_event(RuntimeEvent::NetworkMembership(
			Event::MembershipRenewalRequested { member: AccountId::new([13u8; 32]) },
		));

		run_to_block(6);
		assert!(NetworkMembership::is_member(&AccountId::new([13u8; 32])));
		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipRenewed {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(10);
		assert!(NetworkMembership::is_member(&AccountId::new([13u8; 32])));

		// This ensures that the account was successfully added
		assert_eq!(
			Members::<Test>::get(AccountId::new([13u8; 32])),
			Some(MemberData { expire_on: 6 + MembershipPeriod::get() })
		);

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_renew_membership_non_authority() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert_err!(
			NetworkMembership::renew(
				RuntimeOrigin::signed(AccountId::new([11u8; 32])),
				AccountId::new([13u8; 32]),
			),
			BadOrigin
		);

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_auto_expire_membership() {
	new_test_ext().execute_with(|| {
		run_to_block(3);

		assert!(NetworkMembership::is_member(&AccountId::new([11u8; 32])));

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(5);
		assert!(NetworkMembership::is_member(&AccountId::new([13u8; 32])));
		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipExpired {
			member: AccountId::new([11u8; 32]),
		}));

		run_to_block(10);
		assert!(!NetworkMembership::is_member(&AccountId::new([13u8; 32])));

		assert_eq!(NetworkMembership::members_count(), 0);
	});
}

#[test]
fn test_revoke_membership() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert!(NetworkMembership::is_member(&AccountId::new([11u8; 32])));

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		assert_eq!(NetworkMembership::members_count(), 2);

		run_to_block(2);
		assert_ok!(NetworkMembership::revoke(RawOrigin::Root.into(), AccountId::new([13u8; 32]),));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipRevoked {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(3);
		assert!(!NetworkMembership::is_member(&AccountId::new([13u8; 32])));

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_revoke_membership_non_authority() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert_err!(
			NetworkMembership::revoke(
				RuntimeOrigin::signed(AccountId::new([11u8; 32])),
				AccountId::new([13u8; 32]),
			),
			BadOrigin
		);

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_renew_membership_again_should_fail() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert!(NetworkMembership::is_member(&AccountId::new([11u8; 32])));

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(2);
		// Membership renewal request is made for first time, should pass
		assert_ok!(NetworkMembership::renew(RawOrigin::Root.into(), AccountId::new([13u8; 32]),));
		// Membership renewal request is made again, renewal request already exists, should fail
		assert_err!(
			NetworkMembership::renew(RawOrigin::Root.into(), AccountId::new([13u8; 32]),),
			Error::<Test>::MembershipRenewalAlreadyRequested
		);

		System::assert_has_event(RuntimeEvent::NetworkMembership(
			Event::MembershipRenewalRequested { member: AccountId::new([13u8; 32]) },
		));

		run_to_block(6);
		assert!(NetworkMembership::is_member(&AccountId::new([13u8; 32])));
		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipRenewed {
			member: AccountId::new([13u8; 32]),
		}));

		run_to_block(10);
		assert!(NetworkMembership::is_member(&AccountId::new([13u8; 32])));

		// This ensures that the account was successfully added
		assert_eq!(
			Members::<Test>::get(AccountId::new([13u8; 32])),
			Some(MemberData { expire_on: 6 + MembershipPeriod::get() })
		);

		assert_eq!(NetworkMembership::members_count(), 1);
	});
}

#[test]
fn test_revoke_membership_with_wrong_account_id_should_fail() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		assert!(NetworkMembership::is_member(&AccountId::new([11u8; 32])));

		assert_ok!(NetworkMembership::nominate(
			RawOrigin::Root.into(),
			AccountId::new([13u8; 32]),
			true
		));

		System::assert_has_event(RuntimeEvent::NetworkMembership(Event::MembershipAcquired {
			member: AccountId::new([13u8; 32]),
		}));

		assert_eq!(NetworkMembership::members_count(), 2);

		run_to_block(2);
		assert_err!(
			NetworkMembership::revoke(RawOrigin::Root.into(), AccountId::new([15u8; 32]),),
			Error::<Test>::MembershipNotFound
		);
	});
}

#[test]
fn test_revoke_membership_non_existent() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		// Attempt to revoke a membership for a non-existent account
		assert_err!(
			NetworkMembership::revoke(RawOrigin::Root.into(), AccountId::new([99u8; 32]),),
			Error::<Test>::MembershipNotFound
		);

		assert_eq!(NetworkMembership::members_count(), 1); // No changes to member count
	});
}

#[test]
fn test_check_membership_non_existent() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		// Check if a non-existent account is a member
		// assert!(!NetworkMembership::is_member(&AccountId::new([99u8; 32])));

		assert_err!(
			NetworkMembership::nominate(
				RuntimeOrigin::signed(AccountId::new([11u8; 32])),
				AccountId::new([13u8; 32]),
				true
			),
			BadOrigin
		);

		assert_eq!(NetworkMembership::is_member(&AccountId::new([99u8; 32])), false);
	});
}

#[test]
fn test_auto_expire_non_existent_membership() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		// Check that a non-existent account cannot expire
		run_to_block(10); // Advance time beyond the expiration period

		assert_err!(
			NetworkMembership::nominate(
				RuntimeOrigin::signed(AccountId::new([11u8; 32])),
				AccountId::new([13u8; 32]),
				true
			),
			BadOrigin
		);

		assert_eq!(NetworkMembership::is_member(&AccountId::new([99u8; 32])), false);
	});
}

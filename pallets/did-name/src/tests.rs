// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use frame_support::{assert_noop, assert_ok, BoundedVec};

use cord_utilities::mock::mock_origin;
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

use crate::{mock::*, Banned, DidNameOwnershipOf, Error, Names, Owner, Pallet};

// #############################################################################
// Registering a DID name

#[test]
fn registering_successful() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default().build().execute_with(|| {
		assert!(Names::<Test>::get(&DID_00).is_none());
		assert!(Owner::<Test>::get(&did_name_00).is_none());

		assert_ok!(Pallet::<Test>::register(
			mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
			did_name_00.clone().0,
		));
		let did_name = Names::<Test>::get(&DID_00).expect("Did name should be stored.");
		let owner_details = Owner::<Test>::get(&did_name_00).expect("Owner should be stored.");

		// Test that the name matches
		assert_eq!(did_name, did_name_00);
		// Test that the ownership details match
		assert_eq!(owner_details, DidNameOwnershipOf::<Test> { owner: DID_00, registered_at: 0 });

		// Test that the same name cannot be claimed again.
		assert_noop!(
			Pallet::<Test>::register(
				mock_origin::DoubleOrigin(ACCOUNT_01, DID_01).into(),
				did_name_00.clone().0,
			),
			Error::<Test>::AlreadyExists
		);

		// Test that the same owner cannot claim a new name.
		let did_name_01 = get_did_name(DID_NAME_01_INPUT);
		assert_noop!(
			Pallet::<Test>::register(
				mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
				did_name_01.0,
			),
			Error::<Test>::OwnerAlreadyExists
		);
	})
}

#[test]
fn registering_invalid() {
	let too_short_did_names = vec![
		// Empty name
		BoundedVec::try_from(b"".to_vec()).unwrap(),
		// Single-char name
		BoundedVec::try_from(b"1".to_vec()).unwrap(),
		// Two-letter name
		BoundedVec::try_from(b"10".to_vec()).unwrap(),
	];

	let invalid_did_names = vec![
		// Not allowed ASCII character name (invalid symbol)
		BoundedVec::try_from(b"10:1".to_vec()).unwrap(),
		// Not allowed ASCII character name (uppercase letter)
		BoundedVec::try_from(b"abcdE".to_vec()).unwrap(),
		// Not allowed ASCII character name (whitespace)
		BoundedVec::try_from(b"    ".to_vec()).unwrap(),
		// Non-ASCII character name
		BoundedVec::try_from(String::from("notascii😁").as_bytes().to_owned()).unwrap(),
	];
	ExtBuilder::default()
		// .with_balances(vec![(ACCOUNT_00, 100)])
		.build()
		.execute_with(|| {
			for too_short_input in too_short_did_names.iter() {
				assert_noop!(
					Pallet::<Test>::register(
						mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
						too_short_input.clone(),
					),
					Error::<Test>::NameTooShort,
				);
			}
			for input in invalid_did_names.iter() {
				assert_noop!(
					Pallet::<Test>::register(
						mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
						input.clone()
					),
					Error::<Test>::InvalidFormat,
				);
			}
		})
}

#[test]
fn registering_banned() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		// .with_balances(vec![(ACCOUNT_00, 100)])
		.with_banned_did_names(vec![did_name_00.clone()])
		.build()
		.execute_with(|| {
			assert_noop!(
				Pallet::<Test>::register(
					mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
					did_name_00.0
				),
				Error::<Test>::Banned
			);
		})
}

// #############################################################################
// Name releasing

#[test]
fn releasing_successful() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		.with_did_names(vec![(DID_00, did_name_00.clone())])
		.build()
		.execute_with(|| {
			assert_ok!(Pallet::<Test>::release(
				mock_origin::DoubleOrigin(ACCOUNT_01, DID_00).into(),
			));
			assert!(Names::<Test>::get(&DID_00).is_none());
			assert!(Owner::<Test>::get(&did_name_00).is_none());
		})
}

#[test]
fn releasing_not_found() {
	ExtBuilder::default().build().execute_with(|| {
		// Fail to claim by owner
		assert_noop!(
			Pallet::<Test>::release(mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into()),
			Error::<Test>::OwnerNotFound
		);
	})
}

#[test]
fn releasing_banned() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		.with_banned_did_names(vec![(did_name_00)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Pallet::<Test>::release(mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into()),
				// A banned name will be removed from the map of used names, so it will be considered not
				// existing.
				Error::<Test>::OwnerNotFound
			);
		})
}

// #############################################################################
// Name banning

#[test]
fn banning_successful() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	let did_name_01 = get_did_name(DID_NAME_01_INPUT);
	ExtBuilder::default()
		.with_did_names(vec![(DID_00, did_name_00.clone())])
		.build()
		.execute_with(|| {
			// Ban a claimed name
			assert_ok!(Pallet::<Test>::ban(RawOrigin::Root.into(), did_name_00.clone().0));

			assert!(Names::<Test>::get(&DID_00).is_none());
			assert!(Owner::<Test>::get(&did_name_00).is_none());
			assert!(Banned::<Test>::get(&did_name_00).is_some());

			// Ban an unclaimed name
			assert_ok!(Pallet::<Test>::ban(RawOrigin::Root.into(), did_name_01.clone().0));

			assert!(Owner::<Test>::get(&did_name_01).is_none());
			assert!(Banned::<Test>::get(&did_name_01).is_some());
		})
}

#[test]
fn banning_already_banned() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		.with_banned_did_names(vec![did_name_00.clone()])
		.build()
		.execute_with(|| {
			assert_noop!(
				Pallet::<Test>::ban(RawOrigin::Root.into(), did_name_00.clone().0),
				Error::<Test>::AlreadyBanned
			);
		})
}

#[test]
fn banning_unauthorized_origin() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default().build().execute_with(|| {
		// Signer origin
		assert_noop!(
			Pallet::<Test>::ban(RawOrigin::Signed(ACCOUNT_00).into(), did_name_00.clone().0),
			DispatchError::BadOrigin
		);
		// Owner origin
		assert_noop!(
			Pallet::<Test>::ban(
				mock_origin::DoubleOrigin(ACCOUNT_00, DID_01).into(),
				did_name_00.clone().0
			),
			DispatchError::BadOrigin
		);
	})
}

// #############################################################################
// Name unbanning

#[test]
fn unbanning_successful() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		.with_banned_did_names(vec![did_name_00.clone()])
		.build()
		.execute_with(|| {
			assert_ok!(Pallet::<Test>::unban(RawOrigin::Root.into(), did_name_00.clone().0));

			// Test that registering is possible again
			assert_ok!(Pallet::<Test>::register(
				mock_origin::DoubleOrigin(ACCOUNT_00, DID_00).into(),
				did_name_00.clone().0,
			));
		})
}

#[test]
fn unbanning_not_banned() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Pallet::<Test>::unban(RawOrigin::Root.into(), did_name_00.clone().0),
			Error::<Test>::NotBanned
		);
	})
}

#[test]
fn unbanning_unauthorized_origin() {
	let did_name_00 = get_did_name(DID_NAME_00_INPUT);
	ExtBuilder::default()
		.with_banned_did_names(vec![did_name_00.clone()])
		.build()
		.execute_with(|| {
			// Signer origin
			assert_noop!(
				Pallet::<Test>::unban(RawOrigin::Signed(ACCOUNT_00).into(), did_name_00.clone().0),
				DispatchError::BadOrigin
			);
			// Owner origin
			assert_noop!(
				Pallet::<Test>::ban(
					mock_origin::DoubleOrigin(ACCOUNT_00, DID_01).into(),
					did_name_00.clone().0
				),
				DispatchError::BadOrigin
			);
		})
}

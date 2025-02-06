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

use crate::mock::*;
use crate::{Error, Registries, RegistryIdentifierOf, Status};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::H256;

// Helper: Get the first registry identifier from storage.
fn get_registry_id() -> RegistryIdentifierOf {
	Registries::<Test>::iter().next().expect("A registry should exist").0
}

#[test]
fn create_registry_positive() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Registry::create(
			RawOrigin::Signed(creator).into(),
			H256::random(),
			None,
			None,
			None
		));
		assert_eq!(Registries::<Test>::iter().count(), 1);
	});
}

#[test]
fn create_registry_duplicate_should_fail() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let tx_hash = H256::repeat_byte(1);
		assert_ok!(Registry::create(RawOrigin::Signed(creator).into(), tx_hash, None, None, None));
		assert_noop!(
			Registry::create(RawOrigin::Signed(creator).into(), tx_hash, None, None, None),
			Error::<Test>::RegistryAlreadyExists
		);
	});
}

#[test]
fn archive_registry_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_ok!(Registry::archive(RawOrigin::Signed(admin).into(), reg_id.clone()));
		let registry = Registries::<Test>::get(&reg_id).unwrap();
		assert_eq!(registry.status, Status::Archived);
	});
}

#[test]
fn archive_registry_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let non_admin: u64 = 3;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_noop!(
			Registry::archive(RawOrigin::Signed(non_admin).into(), reg_id.clone()),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn restore_registry_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_ok!(Registry::archive(RawOrigin::Signed(admin).into(), reg_id.clone()));
		assert_ok!(Registry::restore(RawOrigin::Signed(admin).into(), reg_id.clone()));
		let registry = Registries::<Test>::get(&reg_id).unwrap();
		assert_eq!(registry.status, Status::Active);
	});
}

#[test]
fn restore_registry_negative_if_not_archived() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_noop!(
			Registry::restore(RawOrigin::Signed(admin).into(), reg_id.clone()),
			Error::<Test>::RegistryNotArchived
		);
	});
}

#[test]
fn update_registry_author_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let new_author: u64 = 2;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_ok!(Registry::update_author(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			new_author
		));
		let registry = Registries::<Test>::get(&reg_id).unwrap();
		assert_eq!(registry.doc_author_id, Some(new_author));
	});
}

#[test]
fn update_registry_author_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let non_admin: u64 = 3;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_noop!(
			Registry::update_author(RawOrigin::Signed(non_admin).into(), reg_id.clone(), non_admin),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_registry_creator_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let new_creator: u64 = 2;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_ok!(Registry::update_creator(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			new_creator
		));
		let registry = Registries::<Test>::get(&reg_id).unwrap();
		assert_eq!(registry.creator, new_creator);
	});
}

#[test]
fn update_registry_creator_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let non_admin: u64 = 3;
		assert_ok!(Registry::create(
			RawOrigin::Signed(admin).into(),
			H256::random(),
			None,
			None,
			None
		));
		let reg_id = get_registry_id();
		assert_noop!(
			Registry::update_creator(
				RawOrigin::Signed(non_admin).into(),
				reg_id.clone(),
				non_admin
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

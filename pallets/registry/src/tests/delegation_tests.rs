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

#[cfg(test)]
use crate::mock::*;
use crate::{Delegates, Error, PermissionVariant, Permissions, Registries, RegistryIdentifierOf};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::H256;

// Helper: Create a registry and return its identifier.
fn create_registry() -> RegistryIdentifierOf {
	let creator: u64 = 1;
	assert_ok!(Registry::create(
		RawOrigin::Signed(creator).into(),
		H256::random(),
		None,
		None,
		None
	));
	Registries::<Test>::iter().next().expect("Registry should exist").0
}

/// Helper: Assign delegate permission (without admin) to a given account on a registry.
fn set_delegate_permission(registry_id: &RegistryIdentifierOf, account: u64) {
	let delegate_perms = Permissions::from_variants(&[PermissionVariant::Delegate]);
	Delegates::<Test>::insert(registry_id, &account, delegate_perms);
}

#[test]
fn add_delegate_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry();
		let permission_variants = vec![PermissionVariant::Entry, PermissionVariant::Delegate];
		assert_ok!(Registry::add_delegate(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			delegate,
			permission_variants
		));
		assert!(Delegates::<Test>::contains_key(&reg_id, &delegate));
	});
}

#[test]
fn add_delegate_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let non_admin: u64 = 3;
		let delegate: u64 = 2;
		let reg_id = create_registry();
		let permission_variants = vec![PermissionVariant::Entry];
		assert_noop!(
			Registry::add_delegate(
				RawOrigin::Signed(non_admin).into(),
				reg_id.clone(),
				delegate,
				permission_variants
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn remove_delegate_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry();
		set_delegate_permission(&reg_id, delegate);

		assert_ok!(Registry::remove_delegate(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			delegate
		));
		assert!(!Delegates::<Test>::contains_key(&reg_id, &delegate));
	});
}

#[test]
fn remove_delegate_negative_not_found() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry();
		assert_noop!(
			Registry::remove_delegate(RawOrigin::Signed(admin).into(), reg_id.clone(), delegate),
			Error::<Test>::DelegateNotFound
		);
	});
}

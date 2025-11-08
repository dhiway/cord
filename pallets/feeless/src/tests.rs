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

use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::BuildStorage;

#[test]
fn root_can_add_feeless_accounts() {
	new_test_ext().execute_with(|| {
		let account = account(1);
		assert_ok!(Feeless::add_feeless_account(RuntimeOrigin::root(), account.clone()));
		assert!(Feeless::is_feeless_account(&account));

		System::assert_last_event(Event::FeelessAccountAdded { account }.into());
	});
}

#[test]
fn non_root_cannot_add() {
	new_test_ext().execute_with(|| {
		let account = account(2);
		assert_noop!(
			Feeless::add_feeless_account(RuntimeOrigin::signed(account.clone()), account.clone()),
			frame_support::error::BadOrigin
		);
	});
}

#[test]
fn removing_accounts_works() {
	new_test_ext().execute_with(|| {
		let account = account(3);
		assert_ok!(Feeless::add_feeless_account(RuntimeOrigin::root(), account.clone()));
		assert_ok!(Feeless::remove_feeless_account(RuntimeOrigin::root(), account.clone()));
		assert!(!Feeless::is_feeless_account(&account));
		System::assert_last_event(Event::FeelessAccountRemoved { account }.into());
	});
}

#[test]
fn cannot_remove_unknown_account() {
	new_test_ext().execute_with(|| {
		let account = account(4);
		assert_noop!(
			Feeless::remove_feeless_account(RuntimeOrigin::root(), account.clone()),
			Error::<Test>::AccountNotFeeless
		);
	});
}

#[test]
fn genesis_config_inserts_unique_accounts() {
	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	crate::GenesisConfig::<Test> { feeless_accounts: vec![account(5), account(5), account(6)] }
		.assimilate_storage(&mut storage)
		.unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		assert!(Feeless::is_feeless_account(&account(5)));
		assert!(Feeless::is_feeless_account(&account(6)));
	});
}

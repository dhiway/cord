// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use cord_orb_runtime::{
	RuntimeCall, RuntimeGenesisConfig, RuntimeOrigin, SafeMode, System, TxPause,
};
use cord_orb_runtime_constants::currency::UNITS;
use core::convert::TryFrom;
use frame_support::{
	assert_ok,
	traits::{Contains, GetCallMetadata},
	BoundedVec,
};
use sp_keyring::Sr25519Keyring;
use sp_runtime::BuildStorage;

fn new_test_ext() -> sp_io::TestExternalities {
	let alice = Sr25519Keyring::Alice.to_account_id();
	let bob = Sr25519Keyring::Bob.to_account_id();

	let mut genesis = RuntimeGenesisConfig::default();
	genesis.system = frame_system::GenesisConfig::default();
	genesis.balances = pallet_balances::GenesisConfig {
		balances: vec![(alice.clone(), 10_000 * UNITS), (bob.clone(), 10_000 * UNITS)],
		..Default::default()
	};

	let storage = genesis.build_storage().expect("runtime genesis storage");
	sp_io::TestExternalities::new(storage)
}

#[test]
fn tx_pause_blocks_non_whitelisted_calls() {
	new_test_ext().execute_with(|| {
		let bob = Sr25519Keyring::Bob.to_account_id();
		System::set_block_number(1);

		let transfer_call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: bob.clone().into(),
			value: UNITS,
		});

		let metadata = transfer_call.get_call_metadata();
		let full_name = (
			BoundedVec::try_from(metadata.pallet_name.as_bytes().to_vec()).unwrap(),
			BoundedVec::try_from(metadata.function_name.as_bytes().to_vec()).unwrap(),
		);

		// Pause Balances::transfer_allow_death via root.
		assert_ok!(TxPause::pause(RuntimeOrigin::root(), full_name.clone()));

		// Paused calls are disallowed by the dynamic filter.
		assert!(!TxPause::contains(&transfer_call));
	});
}

#[test]
fn tx_pause_allows_whitelisted_keep_alive() {
	new_test_ext().execute_with(|| {
		let bob = Sr25519Keyring::Bob.to_account_id();
		System::set_block_number(1);

		let transfer_call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: bob.clone().into(),
			value: UNITS,
		});

		let metadata = transfer_call.get_call_metadata();
		let full_name = (
			BoundedVec::try_from(metadata.pallet_name.as_bytes().to_vec()).unwrap(),
			BoundedVec::try_from(metadata.function_name.as_bytes().to_vec()).unwrap(),
		);

		assert_ok!(TxPause::pause(RuntimeOrigin::root(), full_name));

		let keep_alive = RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
			dest: bob.into(),
			value: UNITS,
		});

		// Whitelisted calls remain allowed.
		assert!(TxPause::contains(&keep_alive));
	});
}

#[test]
fn safe_mode_enforces_runtime_whitelist() {
	new_test_ext().execute_with(|| {
		let bob = Sr25519Keyring::Bob.to_account_id();
		System::set_block_number(1);
		assert_ok!(SafeMode::force_enter(RuntimeOrigin::root()));

		let paused = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: bob.into(),
			value: UNITS,
		});

		assert!(!SafeMode::contains(&paused));

		let allowed = RuntimeCall::System(frame_system::Call::remark { remark: b"ping".to_vec() });
		assert!(SafeMode::contains(&allowed));
	});
}

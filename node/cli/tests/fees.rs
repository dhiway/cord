#![cfg(not(feature = "runtime-benchmarks"))]

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

use codec::{Encode, Joiner};
use cord_node_testing::keyring::*;
use cord_orb_runtime::{Balances, CheckedExtrinsic, Runtime, RuntimeCall, TransactionPayment};
use cord_orb_runtime_constants::{currency::*, time::SLOT_DURATION};
use frame_support::{dispatch::GetDispatchInfo, traits::Currency};
use pallet_transaction_payment::Multiplier;
use sp_runtime::traits::One;

pub mod common;
use self::common::{sign, *};

#[test]
fn fee_multiplier_increases_and_decreases_on_big_weight() {
	let mut t = new_test_ext(compact_code_unwrap());

	// initial fee multiplier must be one.
	let mut prev_multiplier = Multiplier::one();

	t.execute_with(|| {
		assert_eq!(TransactionPayment::next_fee_multiplier(), prev_multiplier);
	});

	let mut tt = new_test_ext(compact_code_unwrap());

	let time1 = 42 * 1000;
	// big one in terms of weight.
	let block1 = construct_block(
		&mut tt,
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				format: sp_runtime::generic::ExtrinsicFormat::Bare,
				function: RuntimeCall::Timestamp(pallet_timestamp::Call::set { now: time1 }),
			},
			CheckedExtrinsic {
				format: sp_runtime::generic::ExtrinsicFormat::Signed(charlie(), tx_ext(0, 0)),
				function: RuntimeCall::System(frame_system::Call::remark {
					remark: vec![0; 128 * 1024],
				}),
			},
		],
		(time1 / SLOT_DURATION).into(),
	);

	let time2 = 52 * 1000;
	// small one in terms of weight.
	let block2 = construct_block(
		&mut tt,
		2,
		block1.1,
		vec![
			CheckedExtrinsic {
				format: sp_runtime::generic::ExtrinsicFormat::Bare,
				function: RuntimeCall::Timestamp(pallet_timestamp::Call::set { now: time2 }),
			},
			CheckedExtrinsic {
				format: sp_runtime::generic::ExtrinsicFormat::Signed(charlie(), tx_ext(1, 0)),
				function: RuntimeCall::System(frame_system::Call::remark { remark: vec![0; 1] }),
			},
		],
		(time2 / SLOT_DURATION).into(),
	);

	println!(
		"++ Block 1 size: {} / Block 2 size {}",
		block1.0.encode().len(),
		block2.0.encode().len(),
	);

	// execute a big block.
	executor_call(&mut t, "Core_execute_block", &block1.0).0.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a big block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm > prev_multiplier);
		prev_multiplier = fm;
	});

	// execute a big block.
	executor_call(&mut t, "Core_execute_block", &block2.0).0.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a small block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm < prev_multiplier);
	});
}

fn new_account_info(free_units: u128) -> Vec<u8> {
	frame_system::AccountInfo {
		nonce: 0u32,
		consumers: 0,
		providers: 1,
		sufficients: 0,
		data: (free_units * UNITS, 0 * UNITS, 0 * UNITS, 1u128 << 127),
	}
	.encode()
}

#[test]
fn transaction_fee_is_correct() {
	// This uses the exact values of substrate-node.
	//
	// weight of transfer call as of now: 1_000_000
	// if weight of the cheapest weight would be 10^7, this would be 10^9, which is:
	//   - 1 MILLICENTS in substrate node.
	//   - 1 milli-dot based on current polkadot runtime.
	// (this based on assigning 0.1 CENT to the cheapest tx with `weight = 100`)
	let mut t = new_test_ext(compact_code_unwrap());
	t.insert(<frame_system::Account<Runtime>>::hashed_key_for(alice()), new_account_info(100));
	t.insert(<frame_system::Account<Runtime>>::hashed_key_for(bob()), new_account_info(10));
	t.insert(
		<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(),
		(110 * UNITS).encode(),
	);
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let tip = 1_000_000;
	let xt = sign(CheckedExtrinsic {
		format: sp_runtime::generic::ExtrinsicFormat::Signed(alice(), tx_ext(0, tip)),
		function: RuntimeCall::Balances(default_transfer_call()),
	});

	let r = executor_call(&mut t, "Core_initialize_block", &vec![].and(&from_block_number(1u32))).0;

	assert!(r.is_ok());
	let r = executor_call(&mut t, "BlockBuilder_apply_extrinsic", &vec![].and(&xt.clone())).0;
	assert!(r.is_ok());

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&bob()), (10 + 69) * UNITS);
		let mut info = default_transfer_call().get_dispatch_info();
		info.extension_weight = xt.extension_weight();
		let post_info = (Some(info.total_weight()), info.pays_fee).into();
		let actual_fee = TransactionPayment::compute_actual_fee(
			xt.encode().len() as u32,
			&info,
			&post_info,
			tip, // our 1_000_000 tip
		);

		let expected_alice = 100 * UNITS - 69 * UNITS - actual_fee;
		assert_eq!(Balances::total_balance(&alice()), expected_alice);
	});
}

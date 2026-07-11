// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for hop-promotion pallet.

use crate as pallet_bulletin_hop_promotion;
use bulletin_pallets_common::NoCurrency;
use pallet_bulletin_transaction_storage::AsAuthorizer;
use polkadot_sdk_frame::{
	deps::{frame_support, frame_system},
	prelude::*,
	runtime::prelude::*,
	testing_prelude::*,
};
use sp_runtime::{traits::IdentityLookup, AccountId32};

type Block = MockBlock<Test>;

#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeTask,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeViewFunction
	)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;

	#[runtime::pallet_index(1)]
	pub type Timestamp = pallet_timestamp;

	#[runtime::pallet_index(2)]
	pub type TransactionStorage = pallet_bulletin_transaction_storage;

	#[runtime::pallet_index(3)]
	pub type HopPromotion = pallet_bulletin_hop_promotion;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Nonce = u64;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	// Override the default `u64` so `MultiSigner::into_account()` is compatible.
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<0>;
	type WeightInfo = ();
}

parameter_types! {
	pub const AuthorizationPeriod: BlockNumberFor<Test> = 10;
	pub const StoreRenewPriority: TransactionPriority = TransactionPriority::MAX;
	pub const StoreRenewLongevity: TransactionLongevity = 10;
	pub const RemoveExpiredAuthorizationPriority: TransactionPriority = TransactionPriority::MAX;
	pub const RemoveExpiredAuthorizationLongevity: TransactionLongevity = 10;
}

/// Use a small max transaction size for test efficiency.
pub const TEST_MAX_TRANSACTION_SIZE: u32 = 1024;

/// 48 hours in milliseconds.
pub const TEST_SUBMIT_TIMESTAMP_TOLERANCE_MS: u64 = 48 * 60 * 60 * 1000;

parameter_types! {
	pub const SubmitTimestampTolerance: u64 = TEST_SUBMIT_TIMESTAMP_TOLERANCE_MS;
}

impl pallet_bulletin_transaction_storage::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = NoCurrency<Self::AccountId, RuntimeHoldReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type FeeDestination = ();
	type WeightInfo = ();
	type MaxBlockTransactions = ConstU32<512>;
	type MaxTransactionSize = ConstU32<TEST_MAX_TRANSACTION_SIZE>;
	type MaxPermanentStorageSize = ConstU64<{ u64::MAX }>;
	type AuthorizationPeriod = AuthorizationPeriod;
	type AuthorizerRegistrarOrigin = EnsureRoot<Self::AccountId>;
	type Authorizer =
		AsAuthorizer<EnsureRoot<Self::AccountId>, Self::AccountId, BlockNumberFor<Self>>;
	type StoreRenewPriority = StoreRenewPriority;
	type StoreRenewLongevity = StoreRenewLongevity;
	type RemoveExpiredAuthorizationPriority = RemoveExpiredAuthorizationPriority;
	type RemoveExpiredAuthorizationLongevity = RemoveExpiredAuthorizationLongevity;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper =
		pallet_bulletin_transaction_storage::benchmarking::DefaultCheckProofHelper;
}

impl pallet_bulletin_hop_promotion::Config for Test {
	type SubmitTimestampTolerance = SubmitTimestampTolerance;
	type WeightInfo = ();
}

pub fn new_test_ext() -> TestExternalities {
	let t = RuntimeGenesisConfig {
		system: Default::default(),
		transaction_storage: pallet_bulletin_transaction_storage::GenesisConfig::<Test> {
			retention_period: 10,
			byte_fee: 0,
			entry_fee: 0,
			account_authorizations: vec![],
			preimage_authorizations: vec![],
			allowed_authorizers: vec![],
		},
	}
	.build_storage()
	.unwrap();
	#[allow(unused_mut)]
	let mut ext: TestExternalities = t.into();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt::new(
		sp_keystore::testing::MemoryKeystore::new(),
	));
	ext
}

/// Run to block `n`, advancing pallet-timestamp by 6 seconds per block. Required
/// for any test that crosses a block boundary because pallet-timestamp's
/// `on_finalize` panics if `set_timestamp` wasn't called in the current block.
pub fn run_to_block(n: BlockNumberFor<Test>) {
	let mut last_ts = pallet_timestamp::Pallet::<Test>::get();
	System::run_to_block_with::<AllPalletsWithSystem>(
		n,
		RunToBlockHooks::default().after_initialize(move |_bn| {
			last_ts += 6_000;
			pallet_timestamp::Pallet::<Test>::set_timestamp(last_ts);
		}),
	);
}

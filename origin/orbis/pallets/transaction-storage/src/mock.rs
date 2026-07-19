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

//! Test environment for transaction-storage pallet.

use crate::{
	self as pallet_orbis_transaction_storage, AsAuthorizer, EnsureAllowedAuthorizers,
	TransactionStorageProof, DEFAULT_MAX_BLOCK_TRANSACTIONS, DEFAULT_MAX_TRANSACTION_SIZE,
};
use orbis_pallets_common::NoCurrency;
use indiv_support::traits::{ClaimCleanupOutcome, ResourceClaimLifecycle};
use polkadot_sdk_frame::{
	deps::{frame_support, frame_system},
	prelude::*,
	runtime::prelude::*,
	testing_prelude::*,
	traits::EitherOf,
};

type Block = MockBlock<Test>;

// Configure a mock runtime to test the pallet.
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
	pub type TransactionStorage = pallet_orbis_transaction_storage;
}

parameter_types! {
	pub const TestDbWeight: polkadot_sdk_frame::deps::frame_support::weights::RuntimeDbWeight =
		polkadot_sdk_frame::deps::frame_support::weights::RuntimeDbWeight {
			read: 1_000_000,
			write: 5_000_000,
		};
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Nonce = u64;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = TestDbWeight;
}

parameter_types! {
	pub const AuthorizationPeriod: BlockNumberFor<Test> = 10;
	pub const StoreRenewPriority: TransactionPriority = TransactionPriority::MAX;
	pub const StoreRenewLongevity: TransactionLongevity = 10;
	pub const RemoveExpiredAuthorizationPriority: TransactionPriority = TransactionPriority::MAX;
	pub const RemoveExpiredAuthorizationLongevity: TransactionLongevity = 10;
	pub storage MaxPermanentStorageSize: u64 = u64::MAX;
}

pub struct TestClaimLifecycle;
std::thread_local! {
	static PRUNE_CLAIM_CALLS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

pub fn reset_prune_claim_calls() {
	PRUNE_CLAIM_CALLS.with(|calls| calls.set(0));
}

pub fn prune_claim_calls() -> u32 {
	PRUNE_CLAIM_CALLS.with(core::cell::Cell::get)
}

impl ResourceClaimLifecycle<u64, u32> for TestClaimLifecycle {
	fn prune_claim(id: u64) -> ClaimCleanupOutcome<u64, u32> {
		PRUNE_CLAIM_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
		ClaimCleanupOutcome { id, removed: true, purpose: Some(id as u32) }
	}
}

impl pallet_orbis_transaction_storage::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = NoCurrency<Self::AccountId, RuntimeHoldReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type FeeDestination = ();
	type WeightInfo = ();
	type MaxBlockTransactions = ConstU32<{ DEFAULT_MAX_BLOCK_TRANSACTIONS }>;
	type MaxTransactionSize = ConstU32<{ DEFAULT_MAX_TRANSACTION_SIZE }>;
	type MaxPermanentStorageSize = MaxPermanentStorageSize;
	type MaxReservations = ConstU32<256>;
	type MaxReservationExpiryBlocks = ConstU32<256>;
	type MaxReservationsPerExpiryBlock = ConstU32<256>;
	type MaxReservationLinks = ConstU32<1024>;
	type TombstoneRetention = ConstU64<2>;
	type ReservationPurpose = u32;
	type ResourceClaimLifecycle = TestClaimLifecycle;
	type ProviderAllocation = ();
	type AuthorizationPeriod = AuthorizationPeriod;
	type AuthorizerRegistrarOrigin = EnsureRoot<Self::AccountId>;
	type Authorizer = EitherOf<
		AsAuthorizer<EnsureRoot<Self::AccountId>, Self::AccountId, BlockNumberFor<Self>>,
		EnsureAllowedAuthorizers<Self>,
	>;
	type StoreRenewPriority = StoreRenewPriority;
	type StoreRenewLongevity = StoreRenewLongevity;
	type RemoveExpiredAuthorizationPriority = RemoveExpiredAuthorizationPriority;
	type RemoveExpiredAuthorizationLongevity = RemoveExpiredAuthorizationLongevity;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = crate::benchmarking::DefaultCheckProofHelper;
}

pub fn new_test_ext() -> TestExternalities {
	let t = RuntimeGenesisConfig {
		system: Default::default(),
		transaction_storage: pallet_orbis_transaction_storage::GenesisConfig::<Test> {
			retention_period: 10,
			byte_fee: 2,
			entry_fee: 200,
			account_authorizations: vec![],
			preimage_authorizations: vec![],
			allowed_authorizers: vec![],
		},
	}
	.build_storage()
	.unwrap();
	t.into()
}

pub fn run_to_block(n: u64, f: impl Fn() -> Option<TransactionStorageProof> + 'static) {
	System::run_to_block_with::<AllPalletsWithSystem>(
		n,
		RunToBlockHooks::default().before_finalize(|_| {
			let proof = f();
			TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), proof).unwrap();
		}),
	);
}

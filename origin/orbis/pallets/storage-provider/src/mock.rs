// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

use crate as pallet_orbis_storage_provider;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, BuildStorage};

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		StorageProvider: pallet_orbis_storage_provider,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = u64;
	type Block = frame_system::mocking::MockBlock<Self>;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
}

parameter_types! {
	pub const MaxEndpointBytes: u32 = 64;
	pub const MaxServiceKeyBytes: u32 = 64;
	pub const MaxAgreements: u32 = 16;
	pub const MaxChallengesPerBlock: u32 = 16;
	pub const MaxDeletionProofDepth: u32 = 8;
	pub const MaxProviderRootDepth: u32 = 8;
	pub const MaxRootAppendBatch: u32 = 16;
}

impl pallet_orbis_storage_provider::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AdminOrigin = frame_system::EnsureRoot<u64>;
	type MaxEndpointBytes = MaxEndpointBytes;
	type MaxServiceKeyBytes = MaxServiceKeyBytes;
	type MaxProviders = ConstU32<8>;
	type MaxProviderAgreements = MaxAgreements;
	type MaxOwnerAgreements = MaxAgreements;
	type MaxContainerAgreements = MaxAgreements;
	type MaxChallengesPerBlock = MaxChallengesPerBlock;
	type MaxDeletionProofDepth = MaxDeletionProofDepth;
	type MaxProviderRootDepth = MaxProviderRootDepth;
	type MaxRootAppendBatch = MaxRootAppendBatch;
	type ReservationValidator = ();
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

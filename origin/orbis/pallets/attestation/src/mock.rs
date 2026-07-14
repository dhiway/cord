// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

use crate as pallet_orbis_attestation;
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage, MultiSignature, MultiSigner,
};

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Attestation: pallet_orbis_attestation,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = frame_system::mocking::MockBlock<Self>;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
}

parameter_types! {
	pub const MaxSchemaDefinitionLen: u32 = 64;
	pub const MaxAuthorizedIssuers: u32 = 4;
	pub const MaxSchemasPerCreator: u32 = 4;
	pub const MaxAttestationsPerIndex: u32 = 4;
	pub const MaxBatchSize: u32 = 4;
	pub const MaxBatchEncodedLen: u32 = 8 * 1024;
}

impl pallet_orbis_attestation::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Signer = MultiSigner;
	type Signature = MultiSignature;
	type AdminOrigin = EnsureRoot<AccountId32>;
	type MaxSchemaDefinitionLen = MaxSchemaDefinitionLen;
	type MaxAuthorizedIssuers = MaxAuthorizedIssuers;
	type MaxSchemasPerCreator = MaxSchemasPerCreator;
	type MaxAttestationsPerIndex = MaxAttestationsPerIndex;
	type MaxBatchSize = MaxBatchSize;
	type MaxBatchEncodedLen = MaxBatchEncodedLen;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

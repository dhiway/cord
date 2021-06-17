#![allow(clippy::from_over_into)]

use frame_support::{parameter_types, weights::constants::RocksDbWeight};
use sp_keystore::{testing::KeyStore, KeystoreExt};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
};
use sp_std::sync::Arc;

use crate as mtype;
use crate::*;

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type TestMtypeOwner = cord_primitives::AccountId;
pub type TestMtypeHash = cord_primitives::Hash;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Mtype: mtype::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = cord_primitives::Hash;
	type Hashing = BlakeTwo256;
	type AccountId = <<cord_primitives::Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

impl Config for Test {
	type MtypeCreatorId = TestMtypeOwner;
	type EnsureOrigin = frame_system::EnsureSigned<TestMtypeOwner>;
	type Event = ();
	type WeightInfo = ();
}

#[cfg(test)]
pub(crate) const ALICE: TestMtypeOwner = TestMtypeOwner::new([0u8; 32]);

const DEFAULT_MTYPE_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_MTYPE_HASH_SEED: u64 = 2u64;

pub fn get_origin(account: TestMtypeOwner) -> Origin {
	Origin::signed(account)
}

pub fn get_mtype_hash(default: bool) -> TestMtypeHash {
	if default {
		TestMtypeHash::from_low_u64_be(DEFAULT_MTYPE_HASH_SEED)
	} else {
		TestMtypeHash::from_low_u64_be(ALTERNATIVE_MTYPE_HASH_SEED)
	}
}

pub struct MtypeCreationDetails {
	pub hash: TestMtypeHash,
}

pub fn generate_base_mtype_creation_details() -> MtypeCreationDetails {
	MtypeCreationDetails {
		hash: get_mtype_hash(true),
	}
}

#[derive(Clone)]
pub struct ExtBuilder {
	mtypes_stored: Vec<(TestMtypeHash, TestMtypeOwner)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { mtypes_stored: vec![] }
	}
}

impl ExtBuilder {
	pub fn with_mtypes(mut self, mtypes: Vec<(TestMtypeHash, TestMtypeOwner)>) -> Self {
		self.mtypes_stored = mtypes;
		self
	}

	pub fn build(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = if let Some(ext) = ext {
			ext
		} else {
			let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			sp_io::TestExternalities::new(storage)
		};

		if !self.mtypes_stored.is_empty() {
			ext.execute_with(|| {
				self.mtypes_stored.iter().for_each(|mtype| {
					mtype::Mtypes::<Test>::insert(mtype.0, mtype.1.clone());
				})
			});
		}

		ext
	}

	pub fn build_with_keystore(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = self.build(ext);

		let keystore = KeyStore::new();
		ext.register_extension(KeystoreExt(Arc::new(keystore)));
		ext
	}
}

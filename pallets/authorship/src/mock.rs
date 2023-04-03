// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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
pub use crate::mock::runtime::*;

// Mocks that are only used internally
#[cfg(test)]
pub(crate) mod runtime {
	use cord_utilities::mock::{mock_origin, SubjectId};
	use frame_support::{parameter_types, traits::GenesisBuild};
	//use frame_system::{EnsureRoot, EnsureSignedBy};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		MultiSignature,
	};

	// importing authorship as pallet_authorship
	use crate as pallet_authorship;

	type Index = u64;
	type BlockNumber = u64;
	pub(crate) type Balance = u128;

	type Hash = sp_core::H256;
	type Signature = MultiSignature;
	type AccountPublic = <Signature as Verify>::Signer;
	type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			Authorship: pallet_authorship::{Pallet, Storage, Call, Event<T>},
			MockOrigin: mock_origin::{Pallet, Origin<T>},
		}
	);


	impl mock_origin::Config for Test {
		type RuntimeOrigin = RuntimeOrigin;
		type AccountId = AccountId;
		type SubjectId = SubjectId;
	}

	parameter_types! {
		pub const SS58Prefix: u8 = 29;
		pub const BlockHashCount: BlockNumber = 2400;
	}

	impl frame_system::Config for Test {
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type DbWeight = ();
		type RuntimeOrigin = RuntimeOrigin;
		type RuntimeCall = RuntimeCall;
		type Index = Index;
		type BlockNumber = BlockNumber;
		type Hash = Hash;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type RuntimeEvent = RuntimeEvent;
		type BlockHashCount = BlockHashCount;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = SS58Prefix;
		type OnSetCode = ();
		type MaxConsumers = frame_support::traits::ConstU32<16>;
	}

	parameter_types! {
		pub const ExistentialDeposit: Balance = 10;
		pub const MaxLocks: u32 = 50;
		pub const MaxReserves: u32 = 50;
		pub const MaxAuthorityProposals: u32 = 5;
	}

	pub(crate) type TestDidNameOwner = SubjectId;
	pub(crate) type TestDidNamePayer = AccountId;
	pub(crate) type TestOwnerOrigin =
		mock_origin::EnsureDoubleOrigin<TestDidNamePayer, TestDidNameOwner>;
		
	impl pallet_authorship::Config for Test {
		type AuthorApproveOrigin = TestOwnerOrigin;
		type RuntimeEvent = RuntimeEvent;
		type MaxAuthorityProposals = MaxAuthorityProposals;
		type WeightInfo = ();
	}

	// These are test variable to be used in test cases
	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	pub(crate) const TEST_AUTHOR1: AccountId = AccountId::new([1u8; 32]);
	pub(crate) const TEST_AUTHOR2: AccountId = AccountId::new([1u8; 32]);
	pub(crate) const TEST_AUTHOR3: AccountId = AccountId::new([1u8; 32]);
	//pub(crate) static mut TEST_MOCK_AUTHORS: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
	//pub(crate) const DID_01: TestDidNameOwner = SubjectId(TEST_AUTHOR1);
	pub(crate) const DID_02: TestDidNameOwner = SubjectId(TEST_AUTHOR2);
	//pub(crate) const DID_03: TestDidNameOwner = SubjectId(TEST_AUTHOR3);

	#[derive(Clone, Default)]
	pub struct ExtBuilder {
		authors: Vec<AccountId>,
	}

	impl ExtBuilder {
		pub fn build(self) -> sp_io::TestExternalities {
			let mut storage =
				frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			pallet_authorship::GenesisConfig::<Test> { authors: vec![(TEST_AUTHOR1, ())] }
				.assimilate_storage(&mut storage)
				.unwrap();
			let mut ext = sp_io::TestExternalities::new(storage);

			ext.execute_with(|| {});
			ext
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub fn build_with_keystore(self) -> sp_io::TestExternalities {
			let mut ext = self.build();

			let keystore = sp_keystore::testing::KeyStore::new();
			ext.register_extension(sp_keystore::KeystoreExt(std::sync::Arc::new(keystore)));

			ext
		}
	}
}

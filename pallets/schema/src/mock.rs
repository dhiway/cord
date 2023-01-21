// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

use crate::{ss58identifier, Config, HashOf, IdentifierOf, SCHEMA_PREFIX};
use codec::Encode;
use sp_core::H256;

const DEFAULT_SCHEMA_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_SCHEMA_HASH_SEED: u64 = 2u64;

pub fn get_schema_hash<T>(default: bool) -> HashOf<T>
where
	T: Config,
	T::Hash: From<H256>,
{
	if default {
		H256::from_low_u64_be(DEFAULT_SCHEMA_HASH_SEED).into()
	} else {
		H256::from_low_u64_be(ALTERNATIVE_SCHEMA_HASH_SEED).into()
	}
}

pub fn generate_schema_id<T: Config>(digest: &HashOf<T>) -> IdentifierOf {
	let identifier: IdentifierOf = ss58identifier::generate(&(&digest).encode()[..], SCHEMA_PREFIX)
		.into_bytes()
		.try_into()
		.unwrap();
	identifier
}

#[cfg(test)]
pub mod runtime {
	use cord_utilities::mock::{mock_origin, ControllerId};
	use frame_support::{parameter_types, weights::constants::RocksDbWeight};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		AccountId32, MultiSignature,
	};

	use crate::{BalanceOf, SchemaHashes, Schemas};

	use super::*;

	pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	pub type Block = frame_system::mocking::MockBlock<Test>;
	pub type Hash = sp_core::H256;
	pub type Balance = u128;
	pub type Signature = MultiSignature;
	pub type AccountPublic = <Signature as Verify>::Signer;
	pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

	pub const UNIT: Balance = 10u128.pow(12);
	pub const MILLI_UNIT: Balance = 10u128.pow(9);

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			Schema: crate::{Pallet, Call, Storage, Event<T>},
			Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
			MockOrigin: mock_origin::{Pallet, Origin<T>},
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
		type Hash = Hash;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
		type DbWeight = RocksDbWeight;
		type Version = ();

		type PalletInfo = PalletInfo;
		type AccountData = pallet_balances::AccountData<Balance>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type BaseCallFilter = frame_support::traits::Everything;
		type SystemWeightInfo = ();
		type BlockWeights = ();
		type BlockLength = ();
		type SS58Prefix = SS58Prefix;
		type OnSetCode = ();
		type MaxConsumers = frame_support::traits::ConstU32<16>;
	}

	parameter_types! {
		pub const ExistentialDeposit: Balance = 500;
		pub const MaxLocks: u32 = 50;
		pub const MaxReserves: u32 = 50;
	}

	impl pallet_balances::Config for Test {
		type Balance = Balance;
		type DustRemoval = ();
		type Event = ();
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
		type WeightInfo = ();
		type MaxLocks = MaxLocks;
		type MaxReserves = MaxReserves;
		type ReserveIdentifier = [u8; 8];
	}

	impl mock_origin::Config for Test {
		type Origin = Origin;
		type AccountId = AccountId;
		type ControllerId = ControllerId;
	}

	parameter_types! {
		pub const Fee: Balance = 500;
	}

	impl Config for Test {
		type Signature = (ControllerId, Vec<u8>);
		type SignatureVerification = Verify<ControllerId, Vec<u8>>;
		type SchemaCreatorId = ControllerId;
		type EnsureOrigin = mock_origin::EnsureDoubleOrigin<AccountId, ControllerId>;
		type OriginSuccess = mock_origin::DoubleOrigin<AccountId, ControllerId>;
		type Event = ();
		type WeightInfo = ();

		type Currency = Balances;
		type Fee = Fee;
		type FeeCollector = ();
	}

	pub(crate) const DID_00: ControllerId = ControllerId(AccountId32::new([1u8; 32]));
	pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

	#[derive(Clone, Default)]
	pub(crate) struct ExtBuilder {
		schemas_stored: Vec<(IdentifierOf, ControllerId)>,
		schema_hashes_stored: Vec<(HashOf<Test>, IdentifierOf)>,
		balances: Vec<(AccountId, BalanceOf<Test>)>,
	}

	impl ExtBuilder {
		pub(crate) fn with_schemas(mut self, schemas: Vec<(IdentifierOf, ControllerId)>) -> Self {
			self.schemas_stored = schemas;
			self
		}

		pub(crate) fn with_balances(mut self, balances: Vec<(AccountId, BalanceOf<Test>)>) -> Self {
			self.balances = balances;
			self
		}

		pub(crate) fn build(self) -> sp_io::TestExternalities {
			let mut storage =
				frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			pallet_balances::GenesisConfig::<Test> { balances: self.balances.clone() }
				.assimilate_storage(&mut storage)
				.expect("assimilate should not fail");
			let mut ext = sp_io::TestExternalities::new(storage);

			ext.execute_with(|| {
				for (identifier, owner) in self.schemas_stored.iter() {
					Schemas::<Test>::insert(identifier, owner);
				}
				for (schema_hash, identifier) in self.schema_hashes_stored.iter() {
					SchemaHashes::<Test>::insert(schema_hash, identifier);
				}
			});

			ext
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub(crate) fn build_with_keystore(self) -> sp_io::TestExternalities {
			use sp_keystore::{testing::KeyStore, KeystoreExt};
			use sp_std::sync::Arc;

			let mut ext = self.build();

			let keystore = KeyStore::new();
			ext.register_extension(KeystoreExt(Arc::new(keystore)));

			ext
		}
	}
}

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

use crate::{
	curi, Config, CreatorSignatureTypeOf, IdentifierOf, InputSchemaOf, SchemaHashOf, SCHEMA_PREFIX,
};
use codec::Encode;
use sp_core::H256;
// use cord_utilities::signature;
#[cfg(test)]
pub use self::runtime::*;

const DEFAULT_SCHEMA_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_SCHEMA_HASH_SEED: u64 = 2u64;

// Generate a schema input using a many Default::default() as possible.
pub fn generate_base_schema_creation_op<T: Config>(
	digest: SchemaHashOf<T>,
	creator: T::SchemaCreatorId,
	signature: CreatorSignatureTypeOf<Test>,
) -> InputSchemaOf<T> {
	InputSchemaOf::<T> { digest, creator, signature }
}

pub fn get_schema_hash<T>(default: bool) -> SchemaHashOf<T>
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

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> IdentifierOf {
	let identifier: IdentifierOf = curi::generate(&(&digest).encode()[..], SCHEMA_PREFIX)
		.into_bytes()
		.try_into()
		.unwrap();
	identifier
}

#[cfg(test)]
pub mod runtime {
	use frame_support::{
		parameter_types,
		traits::{ConstU32, ConstU64},
		weights::constants::RocksDbWeight,
	};
	use sp_core::{ed25519, sr25519, Pair};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		AccountId32, MultiSignature, MultiSigner,
	};

	use super::*;
	use crate::{BalanceOf, SchemaHashes, Schemas};
	use cord_utilities::{mock::CreatorId, signature::EqualVerify};

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
		}
	);

	parameter_types! {
		pub const SS58Prefix: u8 = 29;
	}

	impl frame_system::Config for Test {
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type RuntimeOrigin = RuntimeOrigin;
		type RuntimeCall = RuntimeCall;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = Hash;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type RuntimeEvent = ();
		type BlockHashCount = ConstU64<250>;
		type DbWeight = RocksDbWeight;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = pallet_balances::AccountData<Balance>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = SS58Prefix;
		type OnSetCode = ();
		type MaxConsumers = ConstU32<16>;
	}

	parameter_types! {
		pub const ExistentialDeposit: Balance = 500;
	}

	impl pallet_balances::Config for Test {
		type Balance = Balance;
		type DustRemoval = ();
		type RuntimeEvent = ();
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
		type WeightInfo = ();
		type MaxLocks = ConstU32<50>;
		type MaxReserves = ConstU32<50>;
		type ReserveIdentifier = [u8; 8];
	}

	parameter_types! {
		pub const Fee: Balance = 500;
		pub const MaxSignatureByteLength: u16 = 64;
		pub const MaxEncodedMetaLength: u32 = 5_000;
	}

	impl Config for Test {
		type Signature = (CreatorId, Vec<u8>);
		type CreatorSignatureVerification = EqualVerify<Self::SchemaCreatorId, Vec<u8>>;
		type SchemaCreatorId = CreatorId;
		type EnsureOrigin = frame_system::EnsureSigned<Self::AccountId>;
		type RuntimeEvent = ();
		type Currency = Balances;
		type Fee = Fee;
		type MaxSignatureByteLength = MaxSignatureByteLength;
		type MaxEncodedMetaLength = MaxEncodedMetaLength;
		type FeeCollector = ();
		type WeightInfo = ();
	}

	pub(crate) const DID_00: CreatorId = CreatorId(AccountId32::new([1u8; 32]));
	pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

	pub(crate) fn ed25519_did_from_seed(seed: &[u8; 32]) -> CreatorId {
		MultiSigner::from(ed25519::Pair::from_seed(seed).public()).into_account().into()
	}

	pub(crate) fn sr25519_did_from_seed(seed: &[u8; 32]) -> CreatorId {
		MultiSigner::from(sr25519::Pair::from_seed(seed).public()).into_account().into()
	}

	pub(crate) fn hash_to_u8<Hash: Encode>(hash: Hash) -> Vec<u8> {
		hash.encode()
	}

	#[derive(Clone, Default)]
	pub(crate) struct ExtBuilder {
		schemas_stored: Vec<(IdentifierOf,)>,
		schema_hashes_stored: Vec<(SchemaHashOf<Test>, IdentifierOf)>,
		balances: Vec<(AccountId, BalanceOf<Test>)>,
	}

	impl ExtBuilder {
		pub(crate) fn with_schemas(mut self, schemas: Vec<(IdentifierOf, CreatorId)>) -> Self {
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

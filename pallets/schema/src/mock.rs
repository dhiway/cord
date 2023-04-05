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

// use crate::{
// 	curi, Config, CreatorSignatureTypeOf, IdentifierOf, InputSchemaOf, SchemaHashOf,
// 	SCHEMA_PREFIX,
// };

use crate::{
	self as pallet_schema, Config, SchemaCreatorOf, SchemaEntryOf, SchemaHashOf, SchemaIdOf,
	Ss58Identifier, Timepoint,
};

#[cfg(test)]
pub use self::runtime::*;
use codec::Encode;
use cord_primitives::BlockNumber;
use sp_core::H256;

const DEFAULT_SCHEMA_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_SCHEMA_HASH_SEED: u64 = 2u64;

// Generate a schema input using a many Default::default() as possible.
// pub fn generate_base_schema_creation_op<T: Config>(
// 	digest: SchemaHashOf<T>,
// 	creator: T::SchemaCreatorId,
// 	signature: SchemaCreatorOf<Test>,
// 	created_at: Timepoint<BlockNumber>,
// ) -> SchemaEntryOf<T> {
// 	SchemaEntryOf::<T> { digest, creator, signature, created_at }
// }

// pub fn get_schema_hash<T>(default: bool) -> SchemaHashOf<T>
// where
// 	T: Config,
// 	T::Hash: From<H256>,
// {
// 	if default {
// 		H256::from_low_u64_be(DEFAULT_SCHEMA_HASH_SEED).into()
// 	} else {
// 		H256::from_low_u64_be(ALTERNATIVE_SCHEMA_HASH_SEED).into()
// 	}
// }

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	let identifier = Ss58Identifier::to_schema_id(digest.as_ref()).into_bytes().try_into().unwrap();
	identifier
}

#[cfg(test)]
pub mod runtime {
	use cord_utilities::mock::{mock_origin, SubjectId};
	use frame_support::parameter_types;
	use frame_system::EnsureRoot;
	use sp_core::{ed25519, sr25519, Pair};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		AccountId32, MultiSignature, MultiSigner,
	};

	use super::*;
	use crate::{AccountIdOf, Schemas, WeightInfo};

	pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	pub type Block = frame_system::mocking::MockBlock<Test>;
	pub type Hash = sp_core::H256;
	pub type Signature = MultiSignature;
	pub type AccountPublic = <Signature as Verify>::Signer;
	pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

	type Index = u64;
	type BlockNumber = u64;
	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			Schema: pallet_schema::{Pallet, Call, Storage, Event<T>},
			MockOrigin: mock_origin::{Pallet, Origin<T>},
		}
	);

	parameter_types! {
		pub const SS58Prefix: u8 = 29;
		pub const BlockHashCount: u64 = 250;
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
		pub const MaxSignatureByteLength: u16 = 64;
		pub const MaxEncodedMetaLength: u32 = 5_000;
		pub const MaxEncodedSchemaLength:u32 = 5_000;
	}
	pub(crate) type TestSchemaCreator = SubjectId;
	pub(crate) type TestSchemaPayer = AccountId;
	pub(crate) type TestOwnerOrigin =
		mock_origin::EnsureDoubleOrigin<TestSchemaPayer, TestSchemaCreator>;
	pub(crate) type TestOriginSuccess =
		mock_origin::DoubleOrigin<TestSchemaPayer, TestSchemaCreator>;

	impl pallet_schema::Config for Test {
		type EnsureOrigin = TestOwnerOrigin;
		type OriginSuccess = TestOriginSuccess;
		type RuntimeEvent = RuntimeEvent;
		type SchemaCreatorId = AccountId32;
		type MaxEncodedSchemaLength = MaxEncodedSchemaLength;
		type WeightInfo = ();
	}

	impl mock_origin::Config for Test {
		type RuntimeOrigin = RuntimeOrigin;
		type AccountId = AccountId;
		type SubjectId = SubjectId;
	}

	pub(crate) const ACCOUNT_00: TestSchemaPayer = AccountId::new([1u8; 32]);
	pub(crate) const ACCOUNT_01: TestSchemaPayer = AccountId::new([2u8; 32]);
	pub(crate) const DID_00: TestSchemaCreator = SubjectId(ACCOUNT_00);
	pub(crate) const DID_01: TestSchemaCreator = SubjectId(ACCOUNT_01);

	pub(crate) fn ed25519_did_from_seed(seed: &[u8; 32]) -> TestSchemaCreator {
		MultiSigner::from(ed25519::Pair::from_seed(seed).public()).into_account().into()
	}

	pub(crate) fn sr25519_did_from_seed(seed: &[u8; 32]) -> TestSchemaCreator {
		MultiSigner::from(sr25519::Pair::from_seed(seed).public()).into_account().into()
	}

	pub(crate) fn hash_to_u8<Hash: Encode>(hash: Hash) -> Vec<u8> {
		hash.encode()
	}

	#[derive(Clone, Default)]
	pub(crate) struct ExtBuilder {
		schemas_stored: Vec<(Ss58Identifier, SubjectId)>,
		schema_hashes_stored: Vec<(SchemaHashOf<Test>, Ss58Identifier)>,
	}

	impl ExtBuilder {
		pub(crate) fn with_schemas(mut self, schemas: Vec<(Ss58Identifier, SubjectId)>) -> Self {
			self.schemas_stored = schemas;
			self
		}

		pub(crate) fn build(self) -> sp_io::TestExternalities {
			let mut storage =
				frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			let mut ext = sp_io::TestExternalities::new(storage);
			ext.execute_with(|| {
				for (identifier, owner) in self.schemas_stored.iter() {
					Schemas::<Test>::insert(identifier, owner);
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

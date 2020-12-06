// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019  BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use crate::*;

use frame_support::{
	assert_err, assert_ok, impl_outer_origin, parameter_types,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		Weight,
	},
};
use cord_node_runtime::Signature;
use sp_core::{ed25519, Pair, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Saturating, Verify},
	MultiSigner, Perbill,
};

impl_outer_origin! {
	pub enum Origin for Test {}
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	pub MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
		.saturating_sub(Perbill::from_percent(10)) * MaximumBlockWeight::get();
}

impl frame_system::Trait for Test {
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = RocksDbWeight;
	type BlockExecutionWeight = BlockExecutionWeight;
	type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
	type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();

	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}

impl error::Trait for Test {
	type Event = ();
	type ErrorCode = u16;
}

impl Trait for Test {
	type Event = ();
}

type CType = Module<Test>;

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn it_works_for_default_value() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let ctype_hash = H256::from_low_u64_be(1);
		let account = MultiSigner::from(pair.public()).into_account();
		assert_ok!(CType::add(Origin::signed(account.clone()), ctype_hash));
		assert_eq!(<CTYPEs<Test>>::contains_key(ctype_hash), true);
		assert_eq!(CType::ctypes(ctype_hash), Some(account.clone()));
		assert_err!(
			CType::add(Origin::signed(account), ctype_hash),
			CType::ERROR_CTYPE_ALREADY_EXISTS.1
		);
	});
}

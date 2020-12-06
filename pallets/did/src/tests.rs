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
	assert_ok, impl_outer_origin,
	weights::constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
};
use cord_node_runtime::{
	AvailableBlockRatio, BlockHashCount, MaximumBlockLength, MaximumBlockWeight,
	MaximumExtrinsicWeight, Signature,
};
use sp_core::{ed25519, Pair, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	MultiSigner,
};

impl_outer_origin! {
	pub enum Origin for Test {}
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Test;

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

impl Trait for Test {
	type Event = ();
	type PublicSigningKey = H256;
	type PublicBoxKey = H256;
}

type DID = Module<Test>;

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn check_add_did() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let signing_key = H256::from_low_u64_be(1);
		let box_key = H256::from_low_u64_be(2);
		let account = MultiSigner::from(pair.public()).into_account();
		assert_ok!(DID::add(
			Origin::signed(account.clone()),
			signing_key,
			box_key,
			Some(b"http://kilt.org/submit".to_vec())
		));

		assert_eq!(<DIDs<Test>>::contains_key(account.clone()), true);
		let did = {
			let opt = DID::dids(account.clone());
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(did.0, signing_key);
		assert_eq!(did.1, box_key);
		assert_eq!(did.2, Some(b"http://kilt.org/submit".to_vec()));

		assert_ok!(DID::remove(Origin::signed(account.clone())));
		assert_eq!(<DIDs<Test>>::contains_key(account), false);
	});
}

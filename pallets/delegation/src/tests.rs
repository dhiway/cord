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

use codec::Encode;
use frame_support::{
	assert_err, assert_ok, impl_outer_origin,
	weights::constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
};
use cord_node_runtime::{
	AvailableBlockRatio, BlockHashCount, MaximumBlockLength, MaximumBlockWeight,
	MaximumExtrinsicWeight, Signature,
};
use sp_core::{ed25519, Pair, H256, H512};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Verify},
	MultiSignature, MultiSigner,
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

impl ctype::Trait for Test {
	type Event = ();
}

impl error::Trait for Test {
	type Event = ();
	type ErrorCode = u16;
}

impl Trait for Test {
	type Event = ();
	type Signature = MultiSignature;
	type Signer = <Self::Signature as Verify>::Signer;
	type DelegationNodeId = H256;
}

type CType = ctype::Module<Test>;
type Delegation = Module<Test>;

fn hash_to_u8<T: Encode>(hash: T) -> Vec<u8> {
	hash.encode()
}

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn check_add_and_revoke_delegations() {
	new_test_ext().execute_with(|| {
		let pair_alice = ed25519::Pair::from_seed(&*b"Alice                           ");
		let account_hash_alice = MultiSigner::from(pair_alice.public()).into_account();
		let pair_bob = ed25519::Pair::from_seed(&*b"Bob                             ");
		let account_hash_bob = MultiSigner::from(pair_bob.public()).into_account();
		let pair_charlie = ed25519::Pair::from_seed(&*b"Charlie                         ");
		let account_hash_charlie = MultiSigner::from(pair_charlie.public()).into_account();

		let ctype_hash = H256::from_low_u64_be(1);
		let id_level_0 = H256::from_low_u64_be(1);
		let id_level_1 = H256::from_low_u64_be(2);
		let id_level_2_1 = H256::from_low_u64_be(21);
		let id_level_2_2 = H256::from_low_u64_be(22);
		let id_level_2_2_1 = H256::from_low_u64_be(221);
		assert_ok!(CType::add(
			Origin::signed(account_hash_alice.clone()),
			ctype_hash
		));

		assert_ok!(Delegation::create_root(
			Origin::signed(account_hash_alice.clone()),
			id_level_0,
			ctype_hash
		));
		assert_err!(
			Delegation::create_root(
				Origin::signed(account_hash_alice.clone()),
				id_level_0,
				ctype_hash
			),
			Delegation::ERROR_ROOT_ALREADY_EXISTS.1
		);
		assert_err!(
			Delegation::create_root(
				Origin::signed(account_hash_alice.clone()),
				id_level_1,
				H256::from_low_u64_be(2)
			),
			CType::ERROR_CTYPE_NOT_FOUND.1
		);

		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			id_level_1,
			id_level_0,
			None,
			account_hash_bob.clone(),
			Permissions::DELEGATE,
			MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				id_level_1,
				id_level_0,
				None,
				Permissions::DELEGATE
			))))
		));
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_alice.clone()),
				id_level_1,
				id_level_0,
				None,
				account_hash_bob.clone(),
				Permissions::DELEGATE,
				MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_1,
					id_level_0,
					None,
					Permissions::DELEGATE
				))))
			),
			Delegation::ERROR_DELEGATION_ALREADY_EXISTS.1
		);
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_bob.clone()),
				id_level_2_1,
				id_level_0,
				Some(id_level_1),
				account_hash_charlie.clone(),
				Permissions::ATTEST,
				MultiSignature::from(ed25519::Signature::from_h512(H512::from_low_u64_be(0)))
			),
			Delegation::ERROR_BAD_DELEGATION_SIGNATURE.1
		);
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_charlie.clone()),
				id_level_2_1,
				id_level_0,
				None,
				account_hash_bob.clone(),
				Permissions::DELEGATE,
				MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_2_1,
					id_level_0,
					None,
					Permissions::DELEGATE
				))))
			),
			Delegation::ERROR_NOT_OWNER_OF_ROOT.1
		);
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_alice.clone()),
				id_level_2_1,
				id_level_1,
				None,
				account_hash_bob.clone(),
				Permissions::DELEGATE,
				MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_2_1,
					id_level_1,
					None,
					Permissions::DELEGATE
				))))
			),
			Delegation::ERROR_ROOT_NOT_FOUND.1
		);

		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_bob.clone()),
			id_level_2_1,
			id_level_0,
			Some(id_level_1),
			account_hash_charlie.clone(),
			Permissions::ATTEST,
			MultiSignature::from(pair_charlie.sign(&hash_to_u8(Delegation::calculate_hash(
				id_level_2_1,
				id_level_0,
				Some(id_level_1),
				Permissions::ATTEST
			))))
		));
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_alice.clone()),
				id_level_2_2,
				id_level_0,
				Some(id_level_1),
				account_hash_charlie.clone(),
				Permissions::ATTEST,
				MultiSignature::from(pair_charlie.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_2_2,
					id_level_0,
					Some(id_level_1),
					Permissions::ATTEST
				))))
			),
			Delegation::ERROR_NOT_OWNER_OF_PARENT.1
		);
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_charlie.clone()),
				id_level_2_2,
				id_level_0,
				Some(id_level_2_1),
				account_hash_alice.clone(),
				Permissions::ATTEST,
				MultiSignature::from(pair_alice.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_2_2,
					id_level_0,
					Some(id_level_2_1),
					Permissions::ATTEST
				))))
			),
			Delegation::ERROR_NOT_AUTHORIZED_TO_DELEGATE.1
		);
		assert_err!(
			Delegation::add_delegation(
				Origin::signed(account_hash_bob.clone()),
				id_level_2_2,
				id_level_0,
				Some(id_level_0),
				account_hash_charlie.clone(),
				Permissions::ATTEST,
				MultiSignature::from(pair_charlie.sign(&hash_to_u8(Delegation::calculate_hash(
					id_level_2_2,
					id_level_0,
					Some(id_level_0),
					Permissions::ATTEST
				))))
			),
			Delegation::ERROR_PARENT_NOT_FOUND.1
		);

		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_bob.clone()),
			id_level_2_2,
			id_level_0,
			Some(id_level_1),
			account_hash_charlie.clone(),
			Permissions::ATTEST | Permissions::DELEGATE,
			MultiSignature::from(pair_charlie.sign(&hash_to_u8(Delegation::calculate_hash(
				id_level_2_2,
				id_level_0,
				Some(id_level_1),
				Permissions::ATTEST | Permissions::DELEGATE
			))))
		));

		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_charlie.clone()),
			id_level_2_2_1,
			id_level_0,
			Some(id_level_2_2),
			account_hash_alice.clone(),
			Permissions::ATTEST,
			MultiSignature::from(pair_alice.sign(&hash_to_u8(Delegation::calculate_hash(
				id_level_2_2_1,
				id_level_0,
				Some(id_level_2_2),
				Permissions::ATTEST
			))))
		));

		let root = {
			let opt = Delegation::root(id_level_0);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(root.0, ctype_hash);
		assert_eq!(root.1, account_hash_alice.clone());
		assert_eq!(root.2, false);

		let delegation_1 = {
			let opt = Delegation::delegation(id_level_1);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(delegation_1.0, id_level_0);
		assert_eq!(delegation_1.1, None);
		assert_eq!(delegation_1.2, account_hash_bob.clone());
		assert_eq!(delegation_1.3, Permissions::DELEGATE);
		assert_eq!(delegation_1.4, false);

		let delegation_2 = {
			let opt = Delegation::delegation(id_level_2_2);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(delegation_2.0, id_level_0);
		assert_eq!(delegation_2.1, Some(id_level_1));
		assert_eq!(delegation_2.2, account_hash_charlie.clone());
		assert_eq!(delegation_2.3, Permissions::ATTEST | Permissions::DELEGATE);
		assert_eq!(delegation_2.4, false);

		let children = Delegation::children(id_level_1);
		assert_eq!(children.len(), 2);
		assert_eq!(children[0], id_level_2_1);
		assert_eq!(children[1], id_level_2_2);

		// check is_delgating
		assert_eq!(
			Delegation::is_delegating(&account_hash_alice, &id_level_1),
			Ok(true)
		);
		assert_eq!(
			Delegation::is_delegating(&account_hash_alice, &id_level_2_1),
			Ok(true)
		);
		assert_eq!(
			Delegation::is_delegating(&account_hash_bob, &id_level_2_1),
			Ok(true)
		);
		assert_eq!(
			Delegation::is_delegating(&account_hash_charlie, &id_level_2_1),
			Ok(true)
		);
		assert_eq!(
			Delegation::is_delegating(&account_hash_charlie, &id_level_1),
			Ok(false)
		);
		assert_err!(
			Delegation::is_delegating(&account_hash_charlie, &id_level_0),
			Delegation::ERROR_DELEGATION_NOT_FOUND.1
		);

		assert_err!(
			Delegation::revoke_delegation(
				Origin::signed(account_hash_charlie.clone()),
				H256::from_low_u64_be(999)
			),
			Delegation::ERROR_DELEGATION_NOT_FOUND.1
		);
		assert_err!(
			Delegation::revoke_delegation(Origin::signed(account_hash_charlie.clone()), id_level_1),
			Delegation::ERROR_NOT_PERMITTED_TO_REVOKE.1
		);
		assert_ok!(Delegation::revoke_delegation(
			Origin::signed(account_hash_charlie),
			id_level_2_2
		));

		assert_eq!(Delegation::delegation(id_level_2_2).unwrap().4, true);
		assert_eq!(Delegation::delegation(id_level_2_2_1).unwrap().4, true);
		assert_err!(
			Delegation::revoke_root(
				Origin::signed(account_hash_bob.clone()),
				H256::from_low_u64_be(999)
			),
			Delegation::ERROR_ROOT_NOT_FOUND.1
		);
		assert_err!(
			Delegation::revoke_root(Origin::signed(account_hash_bob), id_level_0),
			Delegation::ERROR_NOT_PERMITTED_TO_REVOKE.1
		);
		assert_ok!(Delegation::revoke_root(
			Origin::signed(account_hash_alice),
			id_level_0
		));
		assert_eq!(Delegation::root(id_level_0).unwrap().2, true);
		assert_eq!(Delegation::delegation(id_level_1).unwrap().4, true);
		assert_eq!(Delegation::delegation(id_level_2_1).unwrap().4, true);
	});
}

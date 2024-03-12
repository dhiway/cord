// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
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

// Tests for Identity Pallet

use super::*;
use crate::{
	self as pallet_identity,
	simple::{IdentityField as SimpleIdentityField, IdentityInfo},
};
use codec::{Decode, Encode};
use frame_support::{
	assert_noop, assert_ok, construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU32, ConstU64, EitherOfDiverse},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Identity: pallet_identity::{Pallet, Call, Storage, Event<T>},
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type Hash = H256;
	type RuntimeCall = RuntimeCall;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const MaxAdditionalFields: u32 = 2;
	pub const MaxRegistrars: u32 = 20;
}

ord_parameter_types! {
	pub const One: u64 = 1;
	pub const Two: u64 = 2;
}
type EnsureOneOrRoot = EitherOfDiverse<EnsureRoot<u64>, EnsureSignedBy<One, u64>>;
impl pallet_identity::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxAdditionalFields = MaxAdditionalFields;
	type IdentityInformation = IdentityInfo<MaxAdditionalFields>;
	type MaxRegistrars = MaxRegistrars;
	type RegistrarOrigin = EnsureOneOrRoot;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	t.into()
}

fn ten() -> IdentityInfo<MaxAdditionalFields> {
	IdentityInfo {
		display: Data::Raw(b"ten".to_vec().try_into().unwrap()),
		legal: Data::Raw(b"The Right Ordinal Ten, Esq.".to_vec().try_into().unwrap()),
		..Default::default()
	}
}

fn twenty() -> IdentityInfo<MaxAdditionalFields> {
	IdentityInfo {
		display: Data::Raw(b"twenty".to_vec().try_into().unwrap()),
		legal: Data::Raw(b"The Right Ordinal Twenty, Esq.".to_vec().try_into().unwrap()),
		..Default::default()
	}
}

#[test]
fn identity_fields_repr_works() {
	// `SimpleIdentityField` sanity checks.
	assert_eq!(SimpleIdentityField::Display as u64, 1 << 0);
	assert_eq!(SimpleIdentityField::Legal as u64, 1 << 1);
	assert_eq!(SimpleIdentityField::Web as u64, 1 << 2);
	assert_eq!(SimpleIdentityField::Email as u64, 1 << 3);

	let fields = IdentityFields(SimpleIdentityField::Legal | SimpleIdentityField::Web);

	assert!(!fields.0.contains(SimpleIdentityField::Display));
	assert!(fields.0.contains(SimpleIdentityField::Legal));
	assert!(fields.0.contains(SimpleIdentityField::Web));
	assert!(!fields.0.contains(SimpleIdentityField::Email));

	// The `IdentityFields` inner `BitFlags::bits` is used for `Encode`/`Decode`, so
	// we ensure that the `u64` representation matches what we expect during
	// encode/decode operations.
	assert_eq!(
		fields.0.bits(),
		0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000110
	);
}

#[test]
fn setting_identity_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(20), Box::new(twenty())));
	});
}

#[test]
fn trailing_zeros_decodes_into_default_data() {
	let encoded = Data::Raw(b"Hello".to_vec().try_into().unwrap()).encode();
	assert!(<(Data, Data)>::decode(&mut &encoded[..]).is_err());
	let input = &mut &encoded[..];
	let (a, b) = <(Data, Data)>::decode(&mut AppendZerosInput::new(input)).unwrap();
	assert_eq!(a, Data::Raw(b"Hello".to_vec().try_into().unwrap()));
	assert_eq!(b, Data::None);
}

#[test]
fn adding_registrar_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		let fields = IdentityFields(SimpleIdentityField::Display | SimpleIdentityField::Legal);
		assert_ok!(Identity::set_fields(RuntimeOrigin::signed(3), fields));
		assert_eq!(Identity::registrars(), vec![Some(RegistrarInfo { account: 3, fields })]);
	});
}

#[test]
fn removing_registrar_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		let fields = IdentityFields(SimpleIdentityField::Display | SimpleIdentityField::Legal);

		assert_ok!(Identity::set_fields(RuntimeOrigin::signed(3), fields));
		assert_eq!(Identity::registrars(), vec![Some(RegistrarInfo { account: 3, fields })]);
		assert_ok!(Identity::remove_registrar(RuntimeOrigin::signed(1), 3));
		assert_eq!(Identity::registrars(), vec![]);
	});
}

#[test]
fn amount_of_registrars_is_limited() {
	new_test_ext().execute_with(|| {
		for i in 1..MaxRegistrars::get() + 1 {
			assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), i as u64));
		}
		let last_registrar = MaxRegistrars::get() as u64 + 1;
		assert_noop!(
			Identity::add_registrar(RuntimeOrigin::signed(1), last_registrar),
			Error::<Test>::TooManyRegistrars
		);
	});
}

#[test]
fn registration_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		let mut three_fields = ten();
		three_fields.additional.try_push(Default::default()).unwrap();
		three_fields.additional.try_push(Default::default()).unwrap();
		assert!(three_fields.additional.try_push(Default::default()).is_err());
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_eq!(Identity::identity(10).unwrap().info, ten());
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(10)));
		assert_noop!(Identity::clear_identity(RuntimeOrigin::signed(10)), Error::<Test>::NotNamed);
	});
}
//
#[test]
fn uninvited_judgement_should_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(3),
				10,
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::RegistrarNotFound
		);

		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(3),
				10,
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::InvalidTarget
		);

		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(3),
				10,
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::JudgementForDifferentIdentity
		);

		let identity_hash = BlakeTwo256::hash_of(&ten());

		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(10),
				10,
				Judgement::Reasonable,
				identity_hash
			),
			Error::<Test>::RegistrarNotFound
		);
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(3),
				10,
				Judgement::Requested,
				identity_hash
			),
			Error::<Test>::InvalidJudgement
		);

		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(3),
			10,
			Judgement::Reasonable,
			identity_hash
		));
		assert_eq!(Identity::identity(10).unwrap().judgements, vec![(3, Judgement::Reasonable)]);
	});
}

#[test]
fn clearing_identity_and_judgement_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(3),
			10,
			Judgement::Reasonable,
			BlakeTwo256::hash_of(&ten())
		));
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(10)));
		assert_eq!(Identity::identity(10), None);
	});
}

#[test]
fn killing_account_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_ok!(Identity::kill_identity(RuntimeOrigin::signed(1), 10));
		assert_eq!(Identity::identity(10), None);
	});
}

#[test]
fn cancelling_requested_judgement_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(10), 0),
			Error::<Test>::NoIdentity
		);
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(10), 3));
		assert_ok!(Identity::cancel_request(RuntimeOrigin::signed(10), 3));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(10), 1),
			Error::<Test>::NotFound
		);

		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(3),
			10,
			Judgement::Reasonable,
			BlakeTwo256::hash_of(&ten())
		));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(10), 3),
			Error::<Test>::JudgementGiven
		);
	});
}

#[test]
fn requesting_judgement_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(10), 3));

		// Re-requesting won't work.
		assert_noop!(
			Identity::request_judgement(RuntimeOrigin::signed(10), 3),
			Error::<Test>::StickyJudgement
		);
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(3),
			10,
			Judgement::Erroneous,
			BlakeTwo256::hash_of(&ten())
		));

		// Re-requesting still won't work as it's erroneous.
		assert_noop!(
			Identity::request_judgement(RuntimeOrigin::signed(10), 3),
			Error::<Test>::StickyJudgement
		);

		// Requesting from a second registrar still works.
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 4));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(10), 4));

		// Re-requesting after the judgement has been reduced works.
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(3),
			10,
			Judgement::OutOfDate,
			BlakeTwo256::hash_of(&ten())
		));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(10), 3));
	});
}

#[test]
fn test_has_identity() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(10), Box::new(ten())));
		assert!(Identity::has_identity(&10, SimpleIdentityField::Display as u64));
		assert!(Identity::has_identity(&10, SimpleIdentityField::Legal as u64));
		assert!(Identity::has_identity(
			&10,
			SimpleIdentityField::Display as u64 | SimpleIdentityField::Legal as u64
		));
		assert!(!Identity::has_identity(
			&10,
			SimpleIdentityField::Display as u64 |
				SimpleIdentityField::Legal as u64 |
				SimpleIdentityField::Web as u64
		));
	});
}
#[test]
fn add_registrar_should_fail_if_registrar_already_exists() {
	new_test_ext().execute_with(|| {
		assert_ok!(Identity::add_registrar(RuntimeOrigin::signed(1), 3));
		assert_noop!(
			Identity::add_registrar(RuntimeOrigin::signed(1), 3),
			Error::<Test>::RegistrarAlreadyExists
		);
	});
}

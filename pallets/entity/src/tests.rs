// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
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

use super::*;
use crate::pallet::Pallet as EntityPallet;
use crate::{
	entity::{IdentityField, IdentityInfo},
	types::{Attribute, Data, IdentityUpdateOp, ProfileCid},
	PalletInfoAccess,
};
use cord_primitives::identifier::Ss58Identifier;
use enumflags2::BitFlags;
use frame_support::{assert_noop, assert_ok};
use pallet_identifier::Identifier;
use sp_runtime::DispatchError;
// use super::*;

use crate::mock::*;

/// Construct a minimal but valid IdentityInfo with just a display name.
fn sample_info() -> IdentityInfo<MaxAdditionalFields> {
	let mut info = IdentityInfo::<MaxAdditionalFields>::default();
	// set a non-empty display
	info.display = Data::Raw(b"alice".to_vec().try_into().unwrap());
	info
}

/// A helper for constructing a bogus update: change display back to "bob"
fn sample_update_ops() -> Vec<IdentityUpdateOp> {
	vec![IdentityUpdateOp::SetDisplay(Data::Raw(b"bob".to_vec().try_into().unwrap()))]
}

fn plain_data(s: &[u8]) -> Data {
	Data::Raw(s.to_vec().try_into().unwrap())
}

/// Build a “test” identifier from arbitrary bytes for use in negative tests.
fn test_id(input: &[u8]) -> Ss58Identifier {
	let digest = <Test as frame_system::Config>::Hashing::hash(input);
	let pallet_name = <Pallet<Test> as PalletInfoAccess>::name();
	<pallet_identifier::Pallet<Test> as Identifier<Test>>::build(digest.as_ref(), pallet_name)
		.expect("test_id: build should never fail with a 32‐byte seed")
}

/// Construct a minimal IdentityInfo with a custom display name.
fn sample_info_with(name: &[u8]) -> IdentityInfo<MaxAdditionalFields> {
	let mut info = IdentityInfo::<MaxAdditionalFields>::default();
	info.display = Data::Raw(name.to_vec().try_into().unwrap());
	info
}

#[test]
fn full_identity_info_roundtrip_and_has_identity_bits() {
	new_test_ext().execute_with(|| {
		// Build a fully‐populated IdentityInfo:
		let mut info = IdentityInfo::<MaxAdditionalFields>::default();
		info.display = plain_data(b"DisplayName");
		info.legal = plain_data(b"Legal Name, Esq.");
		info.web = plain_data(b"https://example.com");
		info.profile = Some(ProfileCid([0u8; 64]));
		info.additional = Some(BoundedVec::default());
		for i in 0..MaxAdditionalFields::get() {
			let key = vec![b'k', i as u8];
			let attr: Attribute = key.clone().try_into().unwrap();
			let val = plain_data(&[i as u8]);
			info.additional.as_mut().unwrap().try_push((attr, val)).unwrap();
		}

		let who = account(12);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(info.clone())
		));

		// Round-trip check
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		let stored = IdentityOf::<Test>::get(&id).unwrap();
		assert_eq!(stored.display, info.display);
		assert_eq!(stored.legal, info.legal);
		assert_eq!(stored.web, info.web);
		assert_eq!(stored.profile, info.profile);
		assert_eq!(stored.additional(), info.additional());

		// Single-flag masks
		let display_bit = BitFlags::from_flag(IdentityField::Display).bits();
		let legal_bit = BitFlags::from_flag(IdentityField::Legal).bits();
		let web_bit = BitFlags::from_flag(IdentityField::Web).bits();
		let profile_bit = BitFlags::from_flag(IdentityField::Profile).bits();
		let additional_bit = BitFlags::from_flag(IdentityField::Additional).bits();

		assert!(EntityPallet::<Test>::has_identity(&who, display_bit));
		assert!(EntityPallet::<Test>::has_identity(&who, legal_bit));
		assert!(EntityPallet::<Test>::has_identity(&who, web_bit));
		assert!(EntityPallet::<Test>::has_identity(&who, profile_bit));
		assert!(EntityPallet::<Test>::has_identity(&who, additional_bit));

		// Combined mask of *all* flags:
		let all_flags: BitFlags<IdentityField> = BitFlags::all();
		let all_bits = all_flags.bits();
		assert!(EntityPallet::<Test>::has_identity(&who, all_bits));

		// Now clear “web” and verify that only that bit goes false:
		assert_ok!(Entity::update_identity(
			RuntimeOrigin::signed(who.clone()),
			vec![IdentityUpdateOp::SetWeb(Data::None)],
		));
		assert!(!EntityPallet::<Test>::has_identity(&who, web_bit));
		assert!(EntityPallet::<Test>::has_identity(&who, display_bit));
		assert!(!EntityPallet::<Test>::has_identity(&who, all_bits));
	});
}

#[test]
fn set_identity_positive() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let info = sample_info();

		// first call succeeds
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(info.clone())
		));
		// storage populated
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		assert_eq!(IdentityOf::<Test>::get(&id).unwrap(), info);
	});
}

#[test]
fn set_identity_errors_and_edge() {
	new_test_ext().execute_with(|| {
		let who = account(10);

		// 1. Bad origin: must be signed
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::none(), Box::new(IdentityInfo::default())),
			DispatchError::BadOrigin
		);

		// 2. Duplicate additional key → DuplicateAttributeKey
		//    Build an info with two entries using the same key.
		let mut info_dup = IdentityInfo::<MaxAdditionalFields>::default();
		// Need at least one non‐empty field so the pallet proceeds to the additional‐check
		info_dup.display = plain_data(b"d");
		let k = b"dup".to_vec();
		let attr: Attribute = k.clone().try_into().unwrap();
		info_dup.additional = Some(BoundedVec::default());
		info_dup
			.additional
			.as_mut()
			.unwrap()
			.try_push((attr.clone(), plain_data(b"v1")))
			.unwrap();
		info_dup
			.additional
			.as_mut()
			.unwrap()
			.try_push((attr.clone(), plain_data(b"v2")))
			.unwrap();
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::signed(who.clone()), Box::new(info_dup)),
			Error::<Test>::DuplicateAttributeKey
		);

		// 3. Happy path: first registration works
		let info = sample_info();
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(info.clone())
		));

		// 4. Re‐registering on same account ⇒ IdentifierSubAccount
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::signed(who.clone()), Box::new(info)),
			Error::<Test>::IdentifierSubAccount
		);
	});
}

#[test]
fn set_identity_negative_already_registered() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let info = sample_info();
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(info.clone())
		));
		// second time fails
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::signed(who.clone()), Box::new(info)),
			Error::<Test>::IdentifierSubAccount
		);
	});
}

#[test]
fn set_identity_empty_additional_key_errors() {
	new_test_ext().execute_with(|| {
		let who = account(50);
		// Start with a “default” info and then hack in a single empty‐key entry.
		let mut bad = IdentityInfo::<MaxAdditionalFields>::default();
		let empty_attr: Attribute = Vec::new().try_into().unwrap(); // length 0 is allowed by TryFrom
		bad.additional.try_push((empty_attr.clone(), plain_data(b"v"))).unwrap();
		let mut bad = IdentityInfo::<MaxAdditionalFields>::default();
		let empty_attr: Attribute = Vec::new().try_into().unwrap(); // length 0 is allowed by TryFrom
		bad.additional = Some(BoundedVec::default());
		bad.additional
			.as_mut()
			.unwrap()
			.try_push((empty_attr.clone(), plain_data(b"v")))
			.unwrap();

		// That should trigger our ensure!(!key.is_empty(), Error::InvalidAttributeEntry)
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::signed(who.clone()), Box::new(bad)),
			Error::<Test>::InvalidAttributeEntry
		);
	});
}

#[test]
fn set_identity_identifier_already_exists_errors() {
	new_test_ext().execute_with(|| {
		let who1 = account(51);
		let who2 = account(52);
		// Use exactly the same info for both accounts...
		let info = sample_info_with(b"collision");

		// first succeeds
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who1.clone()),
			Box::new(info.clone())
		));
		// second must hit IdentifierAlreadyExists
		assert_noop!(
			Entity::set_identity(RuntimeOrigin::signed(who2.clone()), Box::new(info.clone())),
			Error::<Test>::IdentifierAlreadyExists
		);
	});
}

#[test]
fn update_identity_bad_origin_and_not_found_and_bad_ops() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		// No identity → AccountNotFound
		let ops = sample_update_ops();
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(who.clone()), ops.clone()),
			Error::<Test>::AccountNotFound
		);

		// Now set identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));

		// Bad origin: not controller
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(account(2)), ops.clone()),
			Error::<Test>::AccountNotFound
		);

		// Invalid op: UpdateAdditional on missing key → AttributeNotFound
		let bad_ops = vec![IdentityUpdateOp::UpdateAdditional(
			b"nope".to_vec().try_into().unwrap(),
			plain_data(b"x"),
		)];
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(who.clone()), bad_ops),
			Error::<Test>::AttributeNotFound
		);
	});
}

#[test]
fn update_identity_positive() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let info = sample_info();
		assert_ok!(Entity::set_identity(RuntimeOrigin::signed(who.clone()), Box::new(info)));
		// apply one update op
		let ops = sample_update_ops();
		assert_ok!(Entity::update_identity(RuntimeOrigin::signed(who.clone()), ops.clone()));

		// check that display changed
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		let stored = IdentityOf::<Test>::get(&id).unwrap();
		assert_eq!(stored.display, Data::Raw(b"bob".to_vec().try_into().unwrap()));
	});
}

#[test]
fn update_identity_negative_no_identity() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let ops = sample_update_ops();
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(who), ops),
			Error::<Test>::AccountNotFound
		);
	});
}

#[test]
fn update_identity_too_many_additional_attributes_errors() {
	new_test_ext().execute_with(|| {
		let who = account(60);
		// give them a base identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));

		// now fill to capacity with add_attribute calls
		for i in 0..MaxAdditionalFields::get() {
			let key = vec![i as u8];
			assert_ok!(Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				key.clone(),
				plain_data(&[i as u8])
			));
		}

		// one more via update_identity → should error TooManyAttributes
		let overflow_attr: Attribute = vec![99u8].try_into().unwrap();
		let ops = vec![IdentityUpdateOp::AddAdditional(overflow_attr, plain_data(b"x"))];
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(who.clone()), ops),
			Error::<Test>::TooManyAttributes
		);
	});
}

#[test]
fn add_attribute_positive() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		// insert a new attribute key/value
		let key = b"k".to_vec();
		let val = Data::Raw(b"v".to_vec().try_into().unwrap());
		assert_ok!(Entity::add_attribute(
			RuntimeOrigin::signed(who.clone()),
			key.clone(),
			val.clone()
		));
		// stored in IdentityOf
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		let info = IdentityOf::<Test>::get(&id).unwrap();
		assert!(info
			.additional()
			.unwrap()
			.iter()
			.any(|(attr, data)| &attr[..] == &key[..] && data == &val));
	});
}

#[test]
fn add_attribute_negative_duplicate() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let key = b"k".to_vec();
		let val = Data::Raw(b"v".to_vec().try_into().unwrap());
		// first succeeds
		assert_ok!(Entity::add_attribute(
			RuntimeOrigin::signed(who.clone()),
			key.clone(),
			val.clone()
		));
		// second fails as duplicate
		assert_noop!(
			Entity::add_attribute(RuntimeOrigin::signed(who), key, val),
			Error::<Test>::AttributeExists
		);
	});
}

#[test]
fn add_attribute_not_found_and_invalid_and_toomany() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		// No identity → IdentifierNotFound
		assert_noop!(
			Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"k".to_vec(),
				plain_data(b"v")
			),
			Error::<Test>::AccountNotFound
		);

		// Set up identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));

		// Bad key (not valid Attribute) → InvalidAttributeEntry
		assert_noop!(
			Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				vec![0xff; 100], // too long, won't parse
				plain_data(b"v")
			),
			Error::<Test>::InvalidAttributeEntry
		);

		// Fill to capacity then overflow → TooManyAttributes
		for i in 0..MaxAdditionalFields::get() {
			let k = vec![i as u8];
			assert_ok!(Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				k.clone(),
				plain_data(b"v")
			));
		}
		// one more
		assert_noop!(
			Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"overflow".to_vec(),
				plain_data(b"x")
			),
			Error::<Test>::TooManyAttributes
		);
	});
}

#[test]
fn add_attribute_capacity_direct_and_exhaust() {
	new_test_ext().execute_with(|| {
		let who = account(11);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		// Fill additional via the direct extrinsic until capacity:
		for i in 0..MaxAdditionalFields::get() {
			let key = vec![i as u8];
			let val = plain_data(b"v");
			assert_ok!(Entity::add_attribute(RuntimeOrigin::signed(who.clone()), key, val));
		}
		// One more should hit TooManyAttributes
		assert_noop!(
			Entity::add_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"overflow".to_vec(),
				plain_data(b"x")
			),
			Error::<Test>::TooManyAttributes
		);
	});
}

#[test]
fn add_attribute_capacity_via_update_and_exhaust() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		// First register a minimal identity so we can use update_identity:
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();

		// Prepare a sequence of UpdateAdditional ops to push us right to capacity
		let mut ops = Vec::new();
		for i in 0..MaxAdditionalFields::get() {
			let key = vec![i as u8];
			let attr: Attribute = key.clone().try_into().unwrap();
			let val = plain_data(b"v");
			ops.push(IdentityUpdateOp::AddAdditional(attr.clone(), val.clone()));
		}
		// This should succeed:
		assert_ok!(Entity::update_identity(RuntimeOrigin::signed(who.clone()), ops.clone()));

		// Now one more AddAdditional should overflow:
		let overflow_attr: Attribute = b"overflow".to_vec().try_into().unwrap();
		let overflow_op = vec![IdentityUpdateOp::AddAdditional(overflow_attr, plain_data(b"x"))];
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(who.clone()), overflow_op),
			Error::<Test>::TooManyAttributes
		);

		// Verify that stored IdentityInfo.additional.len() == MaxAdditionalFields:
		let stored = IdentityOf::<Test>::get(&id).unwrap();
		assert_eq!(
			stored.additional().map(|a| a.len() as u32).unwrap_or(0),
			MaxAdditionalFields::get()
		);
	});
}

#[test]
fn set_sub_account_positive_and_edge_max() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub1 = account(1);
		let sub2 = account(2);
		let sub3 = account(3);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		// first two subs work (MaxSubAccounts = 2)
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub1.clone()));
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub2.clone()));
		// a third one should hit the limit
		assert_noop!(
			Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub3),
			Error::<Test>::TooManySubAccounts
		);
	});
}

#[test]
fn set_sub_account_not_identity_and_already_claimed_and_self() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub = account(1);

		// No identity → AccountNotFound
		assert_noop!(
			Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()),
			Error::<Test>::AccountNotFound
		);

		// Give `main` an identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info_with(b"main")),
		));

		// Give `sub` its *own* identity (distinct display → distinct Ss58Identifier)
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(sub.clone()),
			Box::new(sample_info_with(b"sub")),
		));

		// Now trying to make `sub` into a sub-account should fail
		assert_noop!(
			Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()),
			Error::<Test>::SubAccountAlreadyClaimed
		);

		// And a controller may never add *itself* as a sub-account
		let ctrl = account(20);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(ctrl.clone()),
			Box::new(sample_info_with(b"ctrl")),
		));
		assert_noop!(
			Entity::set_sub_account(RuntimeOrigin::signed(ctrl.clone()), ctrl.clone()),
			Error::<Test>::SubAccountAlreadyClaimed
		);
	});
}

#[test]
fn revoke_sub_account_positive_and_negative() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub = account(1);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));
		// now revoke
		assert_ok!(Entity::revoke_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));
		// sub no longer linked
		assert!(!SubAccounts::<Test>::get(&Ss58OfActiveAccounts::<Test>::get(&main).unwrap())
			.contains(&sub));
		// negative: non‐controller cannot revoke
		assert_noop!(
			Entity::revoke_sub_account(RuntimeOrigin::signed(account(2)), sub.clone()),
			Error::<Test>::AccountNotFound
		);
	});
}

#[test]
fn revoke_sub_account_for_root() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub = account(1);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));
		// root forcibly revokes
		assert_ok!(Entity::revoke_sub_account_for(
			RuntimeOrigin::root(),
			Ss58OfActiveAccounts::<Test>::get(&main).unwrap(),
			sub.clone()
		));
		// negative: bad origin
		assert_noop!(
			Entity::revoke_sub_account_for(
				RuntimeOrigin::signed(account(2)),
				Ss58OfActiveAccounts::<Test>::get(&main).unwrap(),
				sub.clone()
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn revoke_sub_account_not_linked_and_self_and_bad_origin() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub = account(1);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		// revoke not‐added → SubAccountNotFound
		assert_noop!(
			Entity::revoke_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()),
			Error::<Test>::SubAccountNotFound
		);

		// add and then try revoking yourself
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));
		assert_noop!(
			Entity::revoke_sub_account(RuntimeOrigin::signed(main.clone()), main.clone()),
			Error::<Test>::ControllerAccount
		);

		// wrong origin
		assert_noop!(
			Entity::revoke_sub_account(RuntimeOrigin::signed(account(2)), sub.clone()),
			Error::<Test>::AccountNotFound
		);
	});
}

#[test]
fn revoke_sub_account_for_not_found_and_bad_origin_and_self() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let sub = account(1);
		let test_identifier = test_id(b"test_identifier");

		let id = Ss58OfActiveAccounts::<Test>::get(&account(10)).unwrap_or_else(|| {
			// force identity on main
			Entity::set_identity(RuntimeOrigin::signed(main.clone()), Box::new(sample_info()))
				.unwrap();
			Ss58OfActiveAccounts::<Test>::get(&main).unwrap()
		});

		// no identity in map → IdentifierNotFound
		assert_noop!(
			Entity::revoke_sub_account_for(RuntimeOrigin::root(), test_identifier, sub.clone()),
			Error::<Test>::IdentifierNotFound
		);

		// add sub
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));

		// cannot revoke controller
		assert_noop!(
			Entity::revoke_sub_account_for(RuntimeOrigin::root(), id.clone(), main.clone()),
			Error::<Test>::ControllerAccount
		);

		// bad origin
		assert_noop!(
			Entity::revoke_sub_account_for(
				RuntimeOrigin::signed(account(2)),
				id.clone(),
				sub.clone()
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn rotate_controller_positive_and_negatives() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let newc = account(20);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		// only controller can rotate
		assert_ok!(Entity::rotate_controller(
			RuntimeOrigin::signed(main.clone()),
			Ss58OfActiveAccounts::<Test>::get(&main).unwrap(),
			newc.clone()
		));
		// now newc is controller
		let id = Ss58OfActiveAccounts::<Test>::get(&newc).unwrap();
		assert_eq!(ControllerOfSs58::<Test>::get(&id).unwrap(), newc.clone());

		// negative: cannot rotate to self
		assert_noop!(
			Entity::rotate_controller(
				RuntimeOrigin::signed(newc.clone()),
				id.clone(),
				newc.clone()
			),
			Error::<Test>::AlreadyController
		);
		// negative: bad origin
		assert_noop!(
			Entity::rotate_controller(RuntimeOrigin::signed(account(1)), id.clone(), account(2)),
			Error::<Test>::BadOrigin
		);
	});
}

#[test]
fn rotate_controller_for_root() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let newc = account(20);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();
		// root may rotate even if not signed by controller
		assert_ok!(Entity::rotate_controller_for(RuntimeOrigin::root(), id.clone(), newc.clone()));
		assert_eq!(ControllerOfSs58::<Test>::get(&id).unwrap(), newc.clone());
		// negative: cannot rotate to same controller
		assert_noop!(
			Entity::rotate_controller_for(RuntimeOrigin::root(), id.clone(), newc.clone()),
			Error::<Test>::AlreadyController
		);
	});
}

#[test]
fn rotate_controller_not_found_and_bad_origin_and_same() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let newc = account(20);
		let test_identifier = test_id(b"test_identifier");

		// not in map → IdentifierNotFound
		assert_noop!(
			Entity::rotate_controller(
				RuntimeOrigin::signed(main.clone()),
				test_identifier,
				newc.clone()
			),
			Error::<Test>::IdentifierNotFound
		);

		// set identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();

		// bad origin
		assert_noop!(
			Entity::rotate_controller(RuntimeOrigin::signed(account(1)), id.clone(), newc.clone()),
			Error::<Test>::BadOrigin
		);

		// rotate to self
		assert_noop!(
			Entity::rotate_controller(
				RuntimeOrigin::signed(main.clone()),
				id.clone(),
				main.clone()
			),
			Error::<Test>::AlreadyController
		);
	});
}

#[test]
fn rotate_controller_for_not_found_and_bad_origin_and_same() {
	new_test_ext().execute_with(|| {
		let main = account(10);
		let newc = account(20);
		let test_identifier = test_id(b"test_identifier");

		// not found
		assert_noop!(
			Entity::rotate_controller_for(RuntimeOrigin::root(), test_identifier, newc.clone()),
			Error::<Test>::IdentifierNotFound
		);

		// set identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();

		// same controller
		assert_noop!(
			Entity::rotate_controller_for(RuntimeOrigin::root(), id.clone(), main.clone()),
			Error::<Test>::AlreadyController
		);

		// bad origin
		assert_noop!(
			Entity::rotate_controller_for(
				RuntimeOrigin::signed(account(1)),
				id.clone(),
				newc.clone()
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn clear_identity_and_for_root() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		// self-clear
		assert_ok!(Entity::clear_identity(RuntimeOrigin::signed(who.clone()), id.clone()));
		assert!(!IdentityOf::<Test>::contains_key(&id));

		// re-create and then root-clear
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let id2 = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		assert_ok!(Entity::clear_identity_for(RuntimeOrigin::root(), id2.clone()));
		assert!(!IdentityOf::<Test>::contains_key(&id2));
	});
}

#[test]
fn clear_identity_not_found_and_bad_origin_and_with_subs() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let other = account(20);
		let test_identifier = test_id(b"test_identifier");

		// no identity
		assert_noop!(
			Entity::clear_identity(RuntimeOrigin::signed(who.clone()), test_identifier),
			Error::<Test>::IdentifierNotFound
		);

		// set identity & add sub
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(who.clone()), other.clone()));

		// bad origin for root‐clear
		assert_noop!(
			Entity::clear_identity_for(RuntimeOrigin::signed(account(2)), id.clone()),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_and_remove_username_positive_and_negative() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		// valid prefix
		let prefix = b"alice".to_vec();
		assert_ok!(Entity::set_username(RuntimeOrigin::signed(who.clone()), prefix.clone()));
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
		let uname = UsernameOf::<Test>::get(&id).unwrap();
		// reverse lookup
		assert_eq!(UsernameInfoOf::<Test>::get(&uname).unwrap(), id);

		// cannot re‐set same name
		assert_noop!(
			Entity::set_username(RuntimeOrigin::signed(who.clone()), prefix),
			Error::<Test>::UsernameTaken
		);

		// remove
		assert_ok!(Entity::remove_username(RuntimeOrigin::signed(who.clone()), id.clone()));
		assert!(!UsernameOf::<Test>::contains_key(&id));
	});
}

#[test]
fn set_username_invalid_prefix_and_bad_origin_and_taken() {
	new_test_ext().execute_with(|| {
		let who = account(10);

		// no identity
		assert_noop!(
			Entity::set_username(RuntimeOrigin::signed(who.clone()), b"a".to_vec()),
			Error::<Test>::AccountNotFound
		);

		// set identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));

		// invalid chars
		assert_noop!(
			Entity::set_username(RuntimeOrigin::signed(who.clone()), b"Bad!".to_vec()),
			Error::<Test>::InvalidUsername
		);

		// valid first
		assert_ok!(Entity::set_username(RuntimeOrigin::signed(who.clone()), b"ok".to_vec()));

		// taken
		assert_noop!(
			Entity::set_username(RuntimeOrigin::signed(who.clone()), b"ok".to_vec()),
			Error::<Test>::UsernameTaken
		);

		// wrong origin
		assert_noop!(
			Entity::set_username(RuntimeOrigin::signed(account(2)), b"other".to_vec()),
			Error::<Test>::AccountNotFound
		);
	});
}

#[test]
fn remove_username_not_found_and_bad_origin() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		let test_identifier = test_id(b"test_identifier");

		// no identity
		assert_noop!(
			Entity::remove_username(RuntimeOrigin::signed(who.clone()), test_identifier),
			Error::<Test>::IdentifierNotFound
		);

		// set identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(who.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();

		// no username
		assert_noop!(
			Entity::remove_username(RuntimeOrigin::signed(who.clone()), id.clone()),
			Error::<Test>::NoUsername
		);

		// bad origin
		assert_noop!(
			Entity::remove_username(RuntimeOrigin::signed(account(2)), id.clone()),
			Error::<Test>::BadOrigin
		);
	});
}

#[test]
fn controller_mismatch_update_identity_errors_bad_origin() {
	new_test_ext().execute_with(|| {
		let main = account(70);
		let sub = account(71);

		// 1) main registers identity
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));

		// 2) main adds sub
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));

		// 3) sub attempts update_identity → must be BadOrigin
		let ops = sample_update_ops();
		assert_noop!(
			Entity::update_identity(RuntimeOrigin::signed(sub.clone()), ops),
			Error::<Test>::BadOrigin
		);
	});
}

#[test]
fn revoke_sub_account_for_not_linked_errors() {
	new_test_ext().execute_with(|| {
		let main = account(80);
		let other = account(81);

		// give each their own identity (so other is "mapped" but to a different id)
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info_with(b"main"))
		));
		let id_main = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();

		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(other.clone()),
			Box::new(sample_info_with(b"other"))
		));
		// other → id_other, not id_main

		// root now tries to revoke_sub_account_for(id_main, other)
		assert_noop!(
			Entity::revoke_sub_account_for(RuntimeOrigin::root(), id_main.clone(), other.clone()),
			Error::<Test>::SubAccountNotLinked
		);
	});
}

#[test]
fn clear_identity_self_clears_all() {
	new_test_ext().execute_with(|| {
		let main = account(90);
		let sub = account(91);

		// 1) register main and add a sub-account
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();

		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));

		// precondition: both are mapped
		assert!(IdentityOf::<Test>::contains_key(&id));
		assert_eq!(Ss58OfActiveAccounts::<Test>::get(&sub), Some(id.clone()));

		// 2) self-clear — should succeed and remove *all* storage
		assert_ok!(Entity::clear_identity(RuntimeOrigin::signed(main.clone()), id.clone()));

		// main identity gone
		assert!(!IdentityOf::<Test>::contains_key(&id));
		// main mapping gone
		assert!(!Ss58OfActiveAccounts::<Test>::contains_key(&main));
		// sub mapping gone
		assert!(!Ss58OfActiveAccounts::<Test>::contains_key(&sub));
	});
}

#[test]
fn clear_identity_for_root_clears_all() {
	new_test_ext().execute_with(|| {
		let main = account(100);
		let sub = account(101);

		// register and add sub
		assert_ok!(Entity::set_identity(
			RuntimeOrigin::signed(main.clone()),
			Box::new(sample_info())
		));
		let id = Ss58OfActiveAccounts::<Test>::get(&main).unwrap();
		assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));

		// precondition check
		assert!(IdentityOf::<Test>::contains_key(&id));
		assert_eq!(Ss58OfActiveAccounts::<Test>::get(&sub), Some(id.clone()));

		// root-clears
		assert_ok!(Entity::clear_identity_for(RuntimeOrigin::root(), id.clone()));

		// everything removed
		assert!(!IdentityOf::<Test>::contains_key(&id));
		assert!(!Ss58OfActiveAccounts::<Test>::contains_key(&main));
		assert!(!Ss58OfActiveAccounts::<Test>::contains_key(&sub));
	});
}

#[test]
fn lookup_variants_and_matches() {
	new_test_ext().execute_with(|| {
		let who = account(10);
		// no identity yet
		assert!(matches!(
			EntityPallet::<Test>::lookup_id_of(&who),
			Err(Error::<Test>::AccountNotFound)
		));
		// fake id
		let fid = test_id(b"xyz");
		assert!(matches!(
			EntityPallet::<Test>::lookup_controller_of(&fid),
			Err(Error::<Test>::IdentifierNotFound)
		));
	});
}

#[test]
fn is_valid_username_prefix_edge() {
	// empty
	assert!(!EntityPallet::<Test>::is_valid_user_name_prefix(b""));
	// consecutive dots
	assert!(!EntityPallet::<Test>::is_valid_user_name_prefix(b"a..b"));
	// leading/trailing dot
	assert!(!EntityPallet::<Test>::is_valid_user_name_prefix(b".abc"));
	assert!(!EntityPallet::<Test>::is_valid_user_name_prefix(b"abc."));
	// too long
	let max = 20;
	let long = vec![b'a'; max + 1];
	assert!(!EntityPallet::<Test>::is_valid_user_name_prefix(&long));
	// valid
	assert!(EntityPallet::<Test>::is_valid_user_name_prefix(b"ok123"));
}

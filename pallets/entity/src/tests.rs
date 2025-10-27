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

#![cfg(test)]
use super::*;
use crate::{entity::EntityInfo, mock::*, pallet::Pallet as EntityPallet, Error};
use cord_primitives::packet::{Attribute, Element};
use frame_support::{assert_noop, assert_ok};
use pallet_token::Token;

/// Shortcut to wrap raw bytes into our `Data` type.
fn plain_data(s: &[u8]) -> Element<MaxRawDataLength> {
	Element::Raw(s.to_vec().try_into().unwrap())
}

/// Common init helper: register an identity with only `display` set.
fn init_with_display(who: AccountId, disp: &[u8]) -> Ss58Identifier {
	let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
	info.display = plain_data(disp);
	assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)));
	Ss58OfActiveAccounts::<Test>::get(&who).unwrap()
}

/// Build a “fake” identifier from arbitrary input.
fn _test_id(input: &[u8]) -> Ss58Identifier {
	let hash = <Test as frame_system::Config>::Hashing::hash(input);
	let name = <Pallet<Test> as PalletInfoAccess>::name();
	<pallet_token::Pallet<Test> as Token<Test>>::build(&hash.as_ref(), name)
		.expect("should never fail")
}

mod set_info_tests {
	use super::*;

	#[test]
	fn happy_path_registers_identity() {
		new_test_ext().execute_with(|| {
			let who = account(1);
			let token = init_with_display(who.clone(), b"alice");
			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			assert_eq!(stored.display, plain_data(b"alice"));
		});
	}

	#[test]
	fn duplicate_registration_fails() {
		new_test_ext().execute_with(|| {
			let who = account(2);
			let _ = init_with_display(who.clone(), b"x");
			assert_noop!(
				Entity::set_info(
					RuntimeOrigin::signed(who.clone()),
					Box::new(EntityInfo::default())
				),
				Error::<Test>::EntitySubAccount
			);
		});
	}

	#[test]
	fn invalid_empty_attribute_key_fails() {
		new_test_ext().execute_with(|| {
			let who = account(3);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"d");
			// force a single empty key in attributes
			let empty: Attribute = Vec::new().try_into().unwrap();
			info.attributes = Some(Default::default());
			info.attributes.as_mut().unwrap().try_push((empty, plain_data(b"v"))).unwrap();

			assert_noop!(
				Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)),
				Error::<Test>::InvalidAttributeEntry
			);
		});
	}

	#[test]
	fn duplicate_attribute_key_in_initial_info_fails() {
		new_test_ext().execute_with(|| {
			let who = account(4);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"d");
			let attr: Attribute = b"dup".to_vec().try_into().unwrap();
			info.attributes = Some(Default::default());
			let v1 = plain_data(b"v1");
			info.attributes.as_mut().unwrap().try_push((attr.clone(), v1.clone())).unwrap();
			info.attributes.as_mut().unwrap().try_push((attr.clone(), v1)).unwrap();

			assert_noop!(
				Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)),
				Error::<Test>::DuplicateAttributeKey
			);
		});
	}
}

mod add_attributes_tests {
	use super::*;

	#[test]
	fn add_attribute_positive() {
		new_test_ext().execute_with(|| {
			let who = account(5);
			let token = init_with_display(who.clone(), b"x");

			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"foo".to_vec(), plain_data(b"v"))]
			));

			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			let attrs = stored.attributes.unwrap();
			assert!(attrs.iter().any(|(k, v)| &k[..] == b"foo" && v == &plain_data(b"v")));
		});
	}

	#[test]
	fn add_attribute_duplicate_key_fails() {
		new_test_ext().execute_with(|| {
			let who = account(6);
			let _ = init_with_display(who.clone(), b"x");

			// first insertion
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"foo".to_vec(), plain_data(b"v1"))]
			));
			// duplicate
			assert_noop!(
				Entity::add_attributes(
					RuntimeOrigin::signed(who.clone()),
					vec![(b"foo".to_vec(), plain_data(b"v2"))]
				),
				Error::<Test>::AttributeExists
			);
		});
	}

	#[test]
	fn add_attribute_too_many_fails() {
		new_test_ext().execute_with(|| {
			let who = account(7);
			let _ = init_with_display(who.clone(), b"x");

			// fill to capacity
			const ATTR_CAP: usize = 32;
			for i in 0..ATTR_CAP {
				let key = vec![i as u8];
				assert_ok!(Entity::add_attributes(
					RuntimeOrigin::signed(who.clone()),
					vec![(key.clone(), plain_data(b"v"))]
				));
			}

			// one more overflows
			assert_noop!(
				Entity::add_attributes(
					RuntimeOrigin::signed(who.clone()),
					vec![(b"overflow".to_vec(), plain_data(b"x"))]
				),
				Error::<Test>::TooManyAttributes
			);
		});
	}

	#[test]
	fn add_attribute_bad_origin_fails() {
		new_test_ext().execute_with(|| {
			let who = account(8);
			// no identity yet
			assert_noop!(
				Entity::add_attributes(
					RuntimeOrigin::signed(who.clone()),
					vec![(b"foo".to_vec(), plain_data(b"v"))]
				),
				Error::<Test>::AccountNotFound
			);
		});
	}
}

mod update_info_tests {
	use super::*;

	#[test]
	fn update_existing_attribute_positive() {
		new_test_ext().execute_with(|| {
			let who = account(9);
			let token = init_with_display(who.clone(), b"x");

			// add then update
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"key".to_vec(), plain_data(b"v1"))]
			));
			assert_ok!(Entity::update_info(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"key".to_vec(), plain_data(b"v2"))]
			));

			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			let attrs = stored.attributes.unwrap();
			assert!(attrs.iter().any(|(k, v)| &k[..] == b"key" && v == &plain_data(b"v2")));
		});
	}

	#[test]
	fn update_missing_attribute_fails() {
		new_test_ext().execute_with(|| {
			let who = account(10);
			let _ = init_with_display(who.clone(), b"x");

			assert_noop!(
				Entity::update_info(
					RuntimeOrigin::signed(who.clone()),
					vec![(b"nope".to_vec(), plain_data(b"x"))]
				),
				Error::<Test>::AttributeNotFound
			);
		});
	}

	#[test]
	fn update_bad_origin_fails() {
		new_test_ext().execute_with(|| {
			let who = account(11);
			let _ = init_with_display(who.clone(), b"x");
			let other = account(12);

			assert_noop!(
				Entity::update_info(
					RuntimeOrigin::signed(other.clone()),
					vec![(b"some".to_vec(), plain_data(b"v"))]
				),
				Error::<Test>::AccountNotFound
			);
		});
	}
}

mod remove_attribute_tests {
	use super::*;

	#[test]
	fn remove_existing_attribute_positive() {
		new_test_ext().execute_with(|| {
			let who = account(13);
			let token = init_with_display(who.clone(), b"x");

			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"rm".to_vec(), plain_data(b"v"))]
			));
			assert_ok!(Entity::remove_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"rm".to_vec()
			));

			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			assert!(stored.attributes.is_none());
		});
	}

	#[test]
	fn remove_missing_attribute_fails() {
		new_test_ext().execute_with(|| {
			let who = account(14);
			let _ = init_with_display(who.clone(), b"x");
			assert_noop!(
				Entity::remove_attribute(RuntimeOrigin::signed(who.clone()), b"nope".to_vec()),
				Error::<Test>::AttributeNotFound
			);
		});
	}
}

mod rotate_attribute_tests {
	use super::*;

	#[test]
	fn rotate_existing_attribute_bumps_history() {
		new_test_ext().execute_with(|| {
			let who = account(15);
			let token = init_with_display(who.clone(), b"x");

			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"rot".to_vec(), plain_data(b"old"))]
			));
			assert_ok!(Entity::rotate_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"rot".to_vec(),
				plain_data(b"new")
			));

			// Verify new value
			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			let attrs = stored.attributes.unwrap();
			assert!(attrs.iter().any(|(k, v)| &k[..] == b"rot" && v == &plain_data(b"new")));

			// Verify history entry exists
			let hist = EntityPallet::<Test>::get_attribute_history(token.clone());
			assert!(!hist.is_empty());
			assert_eq!(hist[0].0, b"rot".to_vec());
			assert_eq!(hist[0].2, b"old".to_vec());
		});
	}

	#[test]
	fn rotate_missing_attribute_fails() {
		new_test_ext().execute_with(|| {
			let who = account(16);
			let _ = init_with_display(who.clone(), b"x");
			assert_noop!(
				Entity::rotate_attribute(
					RuntimeOrigin::signed(who.clone()),
					b"absent".to_vec(),
					plain_data(b"v")
				),
				Error::<Test>::AttributeNotFound
			);
		});
	}
}

mod sub_accounts_tests {
	use super::*;

	#[test]
	fn set_sub_account_flow() {
		new_test_ext().execute_with(|| {
			let main = account(13);
			let sub1 = account(14);
			let sub2 = account(15);
			let sub3 = account(16);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(main.clone()), Box::new(info)));

			// two subs ok if MaxSubAccounts = 2
			assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub1.clone()));
			assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub2.clone()));
			assert_noop!(
				Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub3.clone()),
				Error::<Test>::TooManySubAccounts
			);
		});
	}

	#[test]
	fn revoke_sub_account_flow() {
		new_test_ext().execute_with(|| {
			let main = account(17);
			let sub = account(18);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(main.clone()), Box::new(info)));
			assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()));

			// revoke
			assert_ok!(Entity::revoke_sub_account(
				RuntimeOrigin::signed(main.clone()),
				sub.clone()
			));
			// can't revoke again
			assert_noop!(
				Entity::revoke_sub_account(RuntimeOrigin::signed(main.clone()), sub.clone()),
				Error::<Test>::SubAccountNotFound
			);
		});
	}
}

mod controller_rotation_and_clear_tests {
	use super::*;

	#[test]
	fn rotate_controller_and_for_root() {
		new_test_ext().execute_with(|| {
			let owner = account(19);
			let newc = account(20);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(owner.clone()), Box::new(info)));

			// rotate self
			assert_ok!(Entity::rotate_controller(
				RuntimeOrigin::signed(owner.clone()),
				Ss58OfActiveAccounts::<Test>::get(&owner).unwrap(),
				newc.clone()
			));

			// cannot rotate back by old owner
			assert_noop!(
				Entity::rotate_controller(
					RuntimeOrigin::signed(owner.clone()),
					Ss58OfActiveAccounts::<Test>::get(&newc).unwrap(),
					owner.clone()
				),
				Error::<Test>::BadOrigin
			);
		});
	}

	#[test]
	fn clear_everything_and_for_root() {
		new_test_ext().execute_with(|| {
			let who = account(21);
			let sub = account(22);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)));
			assert_ok!(Entity::set_sub_account(RuntimeOrigin::signed(who.clone()), sub.clone()));

			let token = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
			assert_ok!(Entity::clear_everything(RuntimeOrigin::signed(who.clone()), token.clone()));
			assert!(!EntityInfoOf::<Test>::contains_key(&token));

			// re-set and then root-clear
			let mut info2 = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info2.display = plain_data(b"y");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info2)));
			let id2 = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
			assert_ok!(Entity::clear_everything_for(RuntimeOrigin::root(), id2.clone()));
			assert!(!EntityInfoOf::<Test>::contains_key(&id2));
		});
	}
}

mod id_name_tests {
	use super::*;

	#[test]
	fn set_and_remove_id_name() {
		new_test_ext().execute_with(|| {
			let who = account(23);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)));

			// invalid prefix
			assert_noop!(
				Entity::set_id_name(RuntimeOrigin::signed(who.clone()), b"Bad!".to_vec()),
				Error::<Test>::InvalidSs58IdName
			);

			// valid
			assert_ok!(Entity::set_id_name(RuntimeOrigin::signed(who.clone()), b"alice".to_vec()));

			// duplicate fails
			assert_noop!(
				Entity::set_id_name(RuntimeOrigin::signed(who.clone()), b"alice".to_vec()),
				Error::<Test>::Ss58IdNameTaken
			);

			// remove
			let token = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
			assert_ok!(Entity::remove_id_name(RuntimeOrigin::signed(who.clone()), token.clone()));
			assert_noop!(
				Entity::remove_id_name(RuntimeOrigin::signed(who), token),
				Error::<Test>::NoUsername
			);
		});
	}
}

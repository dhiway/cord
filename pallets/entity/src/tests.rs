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
use crate::{
	entity::EntityInfo, mock::*, pallet::Pallet as EntityPallet,
	signature::SignatureVerificationError, Error,
};
use alloc::format;
use codec::Decode;
use cord_primitives::{
	packet::{Attribute, Attributes, AttributesError, Element},
	Signature,
};
use core::sync::atomic::{AtomicU64, Ordering};
use frame_support::{assert_noop, assert_ok};
use pallet_token::Token;
use sp_core::{sr25519, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSigner};

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

static VIEW_AUTH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn view_auth(account: &AccountId) -> ViewAuthorizationOf<Test> {
	let counter = VIEW_AUTH_COUNTER.fetch_add(1, Ordering::Relaxed);
	let payload_text = format!("entity-view-{counter}");
	let payload_vec = payload_text.into_bytes();
	let payload: ViewAuthPayloadOf<Test> =
		payload_vec.clone().try_into().expect("payload within bounds");
	let signature = mock::ACCOUNT_KEYS.with(|keys| {
		let map = keys.borrow();
		let pair = map.get(account).cloned().expect("account key seeded via mock::account helper");
		Signature::from(pair.sign(&payload_vec))
	});
	ViewAuthorizationOf::<Test> { account: account.clone(), payload, signature }
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
				Error::<Test>::AccountAlreadyLinked
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
			let attrs = info.attributes.get_or_insert_with(Attributes::default);
			attrs.try_insert(empty, plain_data(b"v")).unwrap();

			assert_noop!(
				Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)),
				Error::<Test>::InvalidAttributeEntry
			);
		});
	}

	#[test]
	fn duplicate_attribute_key_in_initial_info_fails() {
		new_test_ext().execute_with(|| {
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"d");
			let attr: Attribute = b"dup".to_vec().try_into().unwrap();
			info.attributes = Some(Attributes::default());
			let v1 = plain_data(b"v1");
			let attrs = info.attributes.as_mut().unwrap();
			assert!(attrs.try_insert(attr.clone(), v1.clone()).is_ok());
			assert!(matches!(
				attrs.try_insert(attr.clone(), v1),
				Err(AttributesError::DuplicateKey)
			));
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
			let hist = EntityPallet::<Test>::attribute_history_plain(&token);
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

mod linked_accounts_tests {
	use super::*;

	#[test]
	fn set_linked_account_flow() {
		new_test_ext().execute_with(|| {
			let main = account(13);
			let sub1 = account(14);
			let sub2 = account(15);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(main.clone()), Box::new(info)));

			// MaxLinkedAccounts = 2 counts the controller, so only one additional account fits.
			assert_ok!(Entity::set_linked_account(
				RuntimeOrigin::signed(main.clone()),
				sub1.clone()
			));
			assert_noop!(
				Entity::set_linked_account(RuntimeOrigin::signed(main.clone()), sub2.clone()),
				Error::<Test>::TooManyLinkedAccounts
			);
			let linked =
				LinkedAccounts::<Test>::get(&Ss58OfActiveAccounts::<Test>::get(&main).unwrap());
			assert_eq!(linked.len(), 2);
			assert!(linked.contains(&main));
			assert!(linked.contains(&sub1));
		});
	}

	#[test]
	fn controller_is_auto_linked() {
		new_test_ext().execute_with(|| {
			let owner = account(30);
			let token = init_with_display(owner.clone(), b"auto");
			let linked = LinkedAccounts::<Test>::get(&token);
			assert_eq!(linked.len(), 1);
			assert_eq!(linked[0], owner.clone());
			let view = EntityPallet::<Test>::linked_accounts(view_auth(&owner), token.clone());
			assert_eq!(view, vec![owner.into()]);
		});
	}

	#[test]
	fn controller_cannot_be_unlinked() {
		new_test_ext().execute_with(|| {
			let owner = account(31);
			let token = init_with_display(owner.clone(), b"ctrl");
			assert_noop!(
				Entity::revoke_linked_account(RuntimeOrigin::signed(owner.clone()), owner.clone()),
				Error::<Test>::ControllerAccount
			);
			assert_noop!(
				Entity::revoke_linked_account_for(
					RuntimeOrigin::root(),
					token.clone(),
					owner.clone()
				),
				Error::<Test>::ControllerAccount
			);
		});
	}

	#[test]
	fn revoke_linked_account_flow() {
		new_test_ext().execute_with(|| {
			let main = account(17);
			let sub = account(18);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(main.clone()), Box::new(info)));
			assert_ok!(Entity::set_linked_account(
				RuntimeOrigin::signed(main.clone()),
				sub.clone()
			));

			// revoke
			assert_ok!(Entity::revoke_linked_account(
				RuntimeOrigin::signed(main.clone()),
				sub.clone()
			));
			// can't revoke again
			assert_noop!(
				Entity::revoke_linked_account(RuntimeOrigin::signed(main.clone()), sub.clone()),
				Error::<Test>::LinkedAccountNotFound
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
			assert_ok!(Entity::set_linked_account(RuntimeOrigin::signed(who.clone()), sub.clone()));

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

mod entity_nym_tests {
	use super::*;

	#[test]
	fn set_and_remove_entity_nym() {
		new_test_ext().execute_with(|| {
			let who = account(23);
			let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info.display = plain_data(b"x");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)));

			// invalid prefix
			assert_noop!(
				Entity::set_entity_nym(RuntimeOrigin::signed(who.clone()), b"Bad!".to_vec()),
				Error::<Test>::InvalidEntityNym
			);

			// valid
			assert_ok!(Entity::set_entity_nym(
				RuntimeOrigin::signed(who.clone()),
				b"alice".to_vec()
			));

			// duplicate fails
			assert_noop!(
				Entity::set_entity_nym(RuntimeOrigin::signed(who.clone()), b"alice".to_vec()),
				Error::<Test>::EntityNymTaken
			);

			// remove
			let token = Ss58OfActiveAccounts::<Test>::get(&who).unwrap();
			assert_ok!(Entity::remove_entity_nym(
				RuntimeOrigin::signed(who.clone()),
				token.clone()
			));
			assert_noop!(
				Entity::remove_entity_nym(RuntimeOrigin::signed(who), token),
				Error::<Test>::NoEntityNym
			);
		});
	}
}

#[test]
fn verify_account_signature_returns_token() {
	new_test_ext().execute_with(|| {
		let pair = sr25519::Pair::from_seed(&[1; 32]);
		let signer = MultiSigner::from(pair.public());
		let account = signer.into_account();
		store_account_pair(account.clone(), pair.clone());
		let token = init_with_display(account.clone(), b"entity-sig");
		let payload = b"registry-view";
		let signature = Signature::from(pair.sign(payload));
		let verified =
			EntityPallet::<Test>::verify_account_signature(&account, payload, &signature)
				.expect("signature should verify");
		assert_eq!(verified, token);
	});
}

#[test]
fn verify_account_signature_rejects_invalid_signature() {
	new_test_ext().execute_with(|| {
		let pair = sr25519::Pair::from_seed(&[2; 32]);
		let signer = MultiSigner::from(pair.public());
		let account = signer.into_account();
		store_account_pair(account.clone(), pair.clone());
		let _token = init_with_display(account.clone(), b"entity-sig");
		let payload = b"registry-view";
		let wrong_pair = sr25519::Pair::from_seed(&[9; 32]);
		let signature = Signature::from(wrong_pair.sign(payload));
		let err = EntityPallet::<Test>::verify_account_signature(&account, payload, &signature)
			.expect_err("signature must be rejected");
		assert_eq!(err, SignatureVerificationError::SignatureInvalid);
	});
}

mod view_tests {
	use super::*;

	#[test]
	fn history_view_requires_valid_authorization() {
		new_test_ext().execute_with(|| {
			let who = account(50);
			let token = init_with_display(who.clone(), b"h");
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"rot".to_vec(), plain_data(b"old"))]
			));
			assert_ok!(Entity::rotate_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"rot".to_vec(),
				plain_data(b"new")
			));

			let auth = view_auth(&who);
			let records = EntityPallet::<Test>::get_attribute_history(auth.clone(), token.clone())
				.expect("authorized history view");
			assert_eq!(records.len(), 1);
			assert_eq!(records[0].0, b"rot".to_vec());

			let mut tampered = auth;
			tampered.signature = Signature::from(sr25519::Pair::from_seed(&[99; 32]).sign(b"nope"));
			assert!(
				EntityPallet::<Test>::get_attribute_history(tampered, token).is_none(),
				"tampered signature must be rejected"
			);
		});
	}

	#[test]
	fn history_json_view_renders_dev_payload() {
		new_test_ext().execute_with(|| {
			let who = account(51);
			let token = init_with_display(who.clone(), b"j");
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"rot".to_vec(), plain_data(b"old"))]
			));
			assert_ok!(Entity::rotate_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"rot".to_vec(),
				plain_data(b"new")
			));

			let hist = EntityPallet::<Test>::attribute_history_plain(&token);
			let auth = view_auth(&who);
			let entries =
				EntityPallet::<Test>::attribute_history_entries(auth, token).expect("dev entries");
			assert_eq!(entries.len(), hist.len());
			assert_eq!(entries[0].key_utf8.as_deref(), Some("rot"));
			assert_eq!(entries[0].version, hist[0].1);
		});
	}

	#[test]
	fn entity_info_view_roundtrip() {
		new_test_ext().execute_with(|| {
			let who = account(60);
			let token = init_with_display(who.clone(), b"info-view");
			let auth = view_auth(&who);
			let info =
				EntityPallet::<Test>::entity_info(auth, token.clone()).expect("entity info view");
			assert_eq!(info.display, plain_data(b"info-view"));

			let bytes = EntityPallet::<Test>::entity_info_bytes(view_auth(&who), token.clone())
				.expect("entity info bytes view");
			let decoded =
				EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::decode(&mut &bytes[..])
					.expect("decode");
			assert_eq!(decoded.display, plain_data(b"info-view"));
		});
	}

	#[test]
	fn account_and_controller_views_return_expected_data() {
		new_test_ext().execute_with(|| {
			let owner = account(61);
			let sub = account(62);
			let token = init_with_display(owner.clone(), b"account-view");
			assert_ok!(Entity::set_linked_account(
				RuntimeOrigin::signed(owner.clone()),
				sub.clone()
			));

			let resolved = EntityPallet::<Test>::account_token(view_auth(&owner), owner.clone())
				.expect("account token view");
			assert_eq!(resolved, token);

			let listed = EntityPallet::<Test>::linked_accounts(view_auth(&owner), token.clone());
			assert_eq!(listed, vec![owner.clone().into(), sub.clone().into()]);

			let controller =
				EntityPallet::<Test>::controller_account_view(view_auth(&owner), token.clone())
					.expect("controller view");
			assert_eq!(controller, owner.clone().into());
		});
	}

	#[test]
	fn entity_nym_and_attribute_views_roundtrip() {
		new_test_ext().execute_with(|| {
			let who = account(63);
			let token = init_with_display(who.clone(), b"names");
			assert_ok!(Entity::set_entity_nym(
				RuntimeOrigin::signed(who.clone()),
				b"alice".to_vec()
			));
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"rot".to_vec(), plain_data(b"old"))],
			));
			assert_ok!(Entity::rotate_attribute(
				RuntimeOrigin::signed(who.clone()),
				b"rot".to_vec(),
				plain_data(b"new"),
			));

			let name_bytes =
				EntityPallet::<Test>::entity_nym(view_auth(&who), token.clone()).expect("name");
			assert!(core::str::from_utf8(&name_bytes).unwrap().ends_with(".myn.social"));

			let lookup = EntityPallet::<Test>::entity_nym_lookup(view_auth(&who), name_bytes.clone())
				.expect("name lookup");
			assert_eq!(lookup, token);

			let version = EntityPallet::<Test>::attribute_version(
				view_auth(&who),
				token.clone(),
				b"rot".to_vec(),
			)
			.expect("attribute version");
			assert_eq!(version, 1);

			let versions = EntityPallet::<Test>::attribute_versions(view_auth(&who), token.clone());
			assert_eq!(versions, vec![(b"rot".to_vec(), 1)]);
		});
	}

	#[test]
	fn account_history_view_lists_prior_controllers() {
		new_test_ext().execute_with(|| {
			let owner = account(64);
			let next = account(65);
			let token = init_with_display(owner.clone(), b"history");
			assert_ok!(Entity::rotate_controller(
				RuntimeOrigin::signed(owner.clone()),
				token.clone(),
				next.clone(),
			));

			let entries = EntityPallet::<Test>::account_history(view_auth(&next), token.clone());
			assert_eq!(entries.len(), 1);
			assert_eq!(entries[0].0, owner.into());
			assert!(entries[0].1.height > 0);
		});
	}
}

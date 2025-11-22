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
use codec::Encode;
use core::sync::atomic::{AtomicU64, Ordering};
use frame_support::{assert_noop, assert_ok};
use origin_primitives::{
	attribute::{Attribute, Attributes, AttributesError, Element},
	authorization::AuthorizationError,
	AccountId as ViewAccount32, Signature,
};
use pallet_token::Token;
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_io::hashing::twox_128;
use sp_runtime::{
	traits::{IdentifyAccount, SaturatedConversion},
	AccountId32 as RuntimeAccountId32, MultiSigner,
};

/// Shortcut to wrap raw bytes into our `Data` type.
fn plain_data(s: &[u8]) -> Element<MaxRawDataLength> {
	Element::Raw(s.to_vec().try_into().unwrap())
}

/// Common init helper: register an identity with only `display` set.
fn init_with_display(who: AccountId, disp: &[u8]) -> Ss58Identifier {
	let mut info = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
	info.display = plain_data(disp);
	assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info)));
	EntityTokenOfAccount::<Test>::get(&who).unwrap()
}

fn view_account(account: &AccountId) -> ViewAccount32 {
	let runtime: RuntimeAccountId32 = account.clone().into();
	ViewAccount32::from(runtime)
}

/// Build a “fake” identifier from arbitrary input.
fn _test_id(input: &[u8]) -> Ss58Identifier {
	let hash = <Test as frame_system::Config>::Hashing::hash(input);
	let name = <Pallet<Test> as PalletInfoAccess>::name();
	<pallet_token::Pallet<Test> as Token<Test>>::build(&hash.as_ref(), name)
		.expect("should never fail")
}

static VIEW_AUTH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn view_payload(account: &AccountId, nonce: &[u8], reference_block: u32) -> Vec<u8> {
	let account_bytes = account.encode();
	let mut preimage = Vec::with_capacity(
		nonce.len() + account_bytes.len() + core::mem::size_of::<u32>() + b"entity::tests".len(),
	);
	preimage.extend_from_slice(nonce);
	preimage.extend_from_slice(b"entity::tests");
	preimage.extend_from_slice(&account_bytes);
	preimage.extend_from_slice(&reference_block.to_le_bytes());
	let digest = twox_128(&preimage);
	let mut payload = Vec::with_capacity(digest.len() + account_bytes.len() + 4);
	payload.extend_from_slice(&digest);
	payload.extend_from_slice(&account_bytes);
	payload.extend_from_slice(&reference_block.to_le_bytes());
	payload
}

fn authorization(account: &AccountId) -> AuthorizationOf<Test> {
	let counter = VIEW_AUTH_COUNTER.fetch_add(1, Ordering::Relaxed);
	let payload_text = format!("entity-view-{counter}");
	let issued_at = frame_system::Pallet::<Test>::block_number().saturated_into::<u32>();
	let nonce = payload_text.into_bytes();
	let payload_vec = view_payload(account, &nonce, issued_at);
	let payload: AuthorizationPayloadOf<Test> =
		payload_vec.clone().try_into().expect("payload within bounds");
	let signature = mock::ACCOUNT_KEYS.with(|keys| {
		let map = keys.borrow();
		let pair = map.get(account).cloned().expect("account key seeded via mock::account helper");
		Signature::from(pair.sign(&payload_vec))
	});
	AuthorizationOf::<Test> { account: account.clone(), payload, signature }
}

fn authorization_with_pair<P>(
	account: &AccountId,
	pair: &P,
	reference_block: u32,
) -> AuthorizationOf<Test>
where
	P: Pair,
	Signature: From<P::Signature>,
{
	let counter = VIEW_AUTH_COUNTER.fetch_add(1, Ordering::Relaxed);
	let payload_text = format!("entity-view-{counter}");
	let nonce = payload_text.into_bytes();
	let payload_vec = view_payload(account, &nonce, reference_block);
	let payload: AuthorizationPayloadOf<Test> =
		payload_vec.clone().try_into().expect("payload within bounds");
	let signature = Signature::from(pair.sign(&payload_vec));
	AuthorizationOf::<Test> { account: account.clone(), payload, signature }
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

mod rotate_attributes_tests {
	use super::*;

	#[test]
	fn rotates_reserved_and_user_attributes() {
		new_test_ext().execute_with(|| {
			let who = account(9);
			let token = init_with_display(who.clone(), b"orig");
			assert_ok!(Entity::add_attributes(
				RuntimeOrigin::signed(who.clone()),
				vec![(b"custom".to_vec(), plain_data(b"old"))]
			));

			let ops = vec![
				(b"display".to_vec(), plain_data(b"new-display")),
				(b"custom".to_vec(), plain_data(b"new-custom")),
			];
			assert_ok!(Entity::rotate_attributes(RuntimeOrigin::signed(who.clone()), ops));

			let stored = EntityInfoOf::<Test>::get(&token).unwrap();
			assert_eq!(stored.display, plain_data(b"new-display"));
			let attrs = stored.attributes.unwrap();
			assert!(attrs
				.iter()
				.any(|(k, v)| &k[..] == b"custom" && v == &plain_data(b"new-custom")));
		});
	}

	#[test]
	fn rotate_attributes_missing_key_fails() {
		new_test_ext().execute_with(|| {
			let who = account(10);
			let _ = init_with_display(who.clone(), b"orig");
			let ops = vec![(b"missing".to_vec(), plain_data(b"v"))];
			assert_noop!(
				Entity::rotate_attributes(RuntimeOrigin::signed(who.clone()), ops),
				Error::<Test>::AttributeNotFound
			);
		});
	}

	#[test]
	fn rotate_attributes_duplicate_key_fails() {
		new_test_ext().execute_with(|| {
			let who = account(11);
			let _ = init_with_display(who.clone(), b"orig");
			let ops = vec![
				(b"display".to_vec(), plain_data(b"first")),
				(b"display".to_vec(), plain_data(b"second")),
			];
			assert_noop!(
				Entity::rotate_attributes(RuntimeOrigin::signed(who), ops),
				Error::<Test>::DuplicateAttributeKey
			);
		});
	}

	#[test]
	fn rotate_attributes_bad_origin_fails() {
		new_test_ext().execute_with(|| {
			let who = account(11);
			assert_noop!(
				Entity::rotate_attributes(
					RuntimeOrigin::signed(who.clone()),
					vec![(b"display".to_vec(), plain_data(b"x"))]
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

	#[test]
	fn cannot_remove_reserved_attribute() {
		new_test_ext().execute_with(|| {
			let who = account(90);
			let _ = init_with_display(who.clone(), b"preset");
			assert_noop!(
				Entity::remove_attribute(RuntimeOrigin::signed(who.clone()), b"display".to_vec()),
				Error::<Test>::ReservedAttribute
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
				LinkedAccounts::<Test>::get(&EntityTokenOfAccount::<Test>::get(&main).unwrap());
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
			let view = EntityPallet::<Test>::linked_accounts(authorization(&owner), token.clone())
				.expect("view");
			assert_eq!(view, vec![view_account(&owner)]);
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
	#[test]
	fn root_can_revoke_linked_account_for() {
		new_test_ext().execute_with(|| {
			let owner = account(40);
			let sub = account(41);
			let token = init_with_display(owner.clone(), b"root-revoke");
			assert_ok!(Entity::set_linked_account(
				RuntimeOrigin::signed(owner.clone()),
				sub.clone()
			));

			assert_ok!(Entity::revoke_linked_account_for(
				RuntimeOrigin::root(),
				token.clone(),
				sub.clone()
			));
			let linked = LinkedAccounts::<Test>::get(&token);
			assert!(!linked.contains(&sub));
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
				EntityTokenOfAccount::<Test>::get(&owner).unwrap(),
				newc.clone()
			));

			// cannot rotate back by old owner
			assert_noop!(
				Entity::rotate_controller(
					RuntimeOrigin::signed(owner.clone()),
					EntityTokenOfAccount::<Test>::get(&newc).unwrap(),
					owner.clone()
				),
				Error::<Test>::BadOrigin
			);

			let token = EntityTokenOfAccount::<Test>::get(&newc).unwrap();
			// remove the stale owner link to simulate an admin cleanup before rotating back
			LinkedAccounts::<Test>::mutate(&token, |list| {
				if let Some(pos) = list.iter().position(|acct| acct == &owner) {
					list.swap_remove(pos);
				}
			});
			assert_ok!(Entity::rotate_controller_for(
				RuntimeOrigin::root(),
				token.clone(),
				owner.clone()
			));
			assert_eq!(ControllerAccountOf::<Test>::get(&token), Some(owner));
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

			let token = EntityTokenOfAccount::<Test>::get(&who).unwrap();
			assert_ok!(Entity::clear_everything(RuntimeOrigin::signed(who.clone()), token.clone()));
			assert!(!EntityInfoOf::<Test>::contains_key(&token));

			// re-set and then root-clear
			let mut info2 = EntityInfo::<MaxRawDataLength, MaxAdditionalAttributes>::default();
			info2.display = plain_data(b"y");
			assert_ok!(Entity::set_info(RuntimeOrigin::signed(who.clone()), Box::new(info2)));
			let id2 = EntityTokenOfAccount::<Test>::get(&who).unwrap();
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
				Error::<Test>::EntityNymAlreadySet
			);

			// remove
			let token = EntityTokenOfAccount::<Test>::get(&who).unwrap();
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

			let auth = authorization(&who);
			let records = EntityPallet::<Test>::attribute_history(auth.clone(), token.clone())
				.expect("history");
			assert_eq!(records.len(), 1);
			assert_eq!(records[0].key, b"rot".to_vec());

			let mut tampered = auth;
			tampered.signature = Signature::from(sr25519::Pair::from_seed(&[99; 32]).sign(b"nope"));
			assert!(matches!(
				EntityPallet::<Test>::attribute_history(tampered, token),
				Err(AuthorizationError::Unauthorized)
			));
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
				plain_data(b"new"),
			));

			let hist = EntityPallet::<Test>::attribute_history_plain(&token);
			let auth = authorization(&who);
			let entries = EntityPallet::<Test>::attribute_history(auth, token.clone())
				.expect("history entries");
			assert_eq!(entries.len(), hist.len());
			assert_eq!(entries[0].key, hist[0].0);
			assert_eq!(entries[0].version, hist[0].1);
		});
	}

	#[test]
	fn entity_details_roundtrip() {
		new_test_ext().execute_with(|| {
			let who = account(60);
			let token = init_with_display(who.clone(), b"info-view");
			let auth = authorization(&who);
			let view = EntityPallet::<Test>::details(auth, token.clone()).expect("entity details");
			assert_eq!(view.display, ElementView::from(&plain_data(b"info-view")));
		});
	}

	#[test]
	fn account_and_controller_queries_return_expected_data() {
		new_test_ext().execute_with(|| {
			let owner = account(61);
			let sub = account(62);
			let token = init_with_display(owner.clone(), b"account-view");
			assert_ok!(Entity::set_linked_account(
				RuntimeOrigin::signed(owner.clone()),
				sub.clone()
			));

			let resolved =
				EntityPallet::<Test>::account_token(authorization(&owner), owner.clone())
					.expect("account token view");
			assert_eq!(resolved, token);

			let listed =
				EntityPallet::<Test>::linked_accounts(authorization(&owner), token.clone())
					.expect("links");
			assert_eq!(listed, vec![view_account(&owner), view_account(&sub)]);

			let controller =
				EntityPallet::<Test>::controller_account(authorization(&owner), token.clone())
					.expect("controller account");
			assert_eq!(controller, view_account(&owner));
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
				EntityPallet::<Test>::entity_nym(authorization(&who), token.clone()).expect("name");
			assert!(core::str::from_utf8(&name_bytes).unwrap().ends_with(".nym.org.in"));

			let attr: Attribute = b"rot".to_vec().try_into().unwrap();
			let version =
				EntityPallet::<Test>::attribute_version(authorization(&who), token.clone(), attr)
					.expect("attribute version");
			assert_eq!(version, 1);

			let versions =
				EntityPallet::<Test>::attribute_versions(authorization(&who), token.clone())
					.expect("versions");
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

			let entries =
				EntityPallet::<Test>::account_history(authorization(&next), token.clone())
					.expect("history");
			assert_eq!(entries.len(), 1);
			assert_eq!(entries[0].account, owner.into());
			assert!(entries[0].block.height > 0);
		});
	}
}

mod authorization_flow_tests {
	use super::*;

	#[test]
	fn authorization_expires_after_ttl() {
		new_test_ext().execute_with(|| {
			let who = account(70);
			let token = init_with_display(who.clone(), b"ttl");
			let auth = authorization(&who);
			let now = frame_system::Pallet::<Test>::block_number();
			let ttl = <Test as Config>::MaxAuthorizationTTL::get();
			frame_system::Pallet::<Test>::set_block_number(now + u64::from(ttl) + 1);
			assert!(matches!(
				EntityPallet::<Test>::details(auth, token.clone()),
				Err(AuthorizationError::Expired)
			));
		});
	}

	#[test]
	fn account_lookup_rejects_mismatched_authorization_account() {
		new_test_ext().execute_with(|| {
			let owner = account(71);
			let other = account(72);
			let token = init_with_display(owner.clone(), b"lookup-mismatch");
			let auth = authorization(&other);
			assert!(matches!(
				EntityPallet::<Test>::account_token(auth, owner.clone()),
				Err(AuthorizationError::InvalidInput)
			));
			assert!(EntityInfoOf::<Test>::contains_key(&token));
		});
	}

	#[test]
	fn multisignature_authorization_accepts_ed25519() {
		new_test_ext().execute_with(|| {
			let pair = ed25519::Pair::from_seed(&[88; 32]);
			let signer = MultiSigner::from(pair.public());
			let account: AccountId = signer.into_account();
			let token = init_with_display(account.clone(), b"ed25519");
			let auth = authorization_with_pair(&account, &pair, 1);
			let controller = EntityPallet::<Test>::controller_account(auth, token.clone())
				.expect("ed25519 signature should authorize");
			assert_eq!(controller, view_account(&account));
		});
	}

	#[test]
	fn multisignature_rejects_signature_from_wrong_scheme() {
		new_test_ext().execute_with(|| {
			let sr_pair = sr25519::Pair::from_seed(&[77; 32]);
			let sr_signer = MultiSigner::from(sr_pair.public());
			let account: AccountId = sr_signer.into_account();
			store_account_pair(account.clone(), sr_pair.clone());
			let token = init_with_display(account.clone(), b"ms-reject");
			let ecdsa_pair = ecdsa::Pair::from_seed_slice(&[5u8; 32]).expect("seeded ecdsa");
			let auth = authorization_with_pair(&account, &ecdsa_pair, 1);
			assert!(matches!(
				EntityPallet::<Test>::details(auth, token.clone()),
				Err(AuthorizationError::Unauthorized)
			));
		});
	}
}

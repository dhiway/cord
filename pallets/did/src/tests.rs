// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

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

use frame_support::{assert_err, assert_noop, assert_ok};
use sp_core::*;
use sp_runtime::SaturatedConversion;

use sp_std::{collections::btree_set::BTreeSet, convert::TryFrom};

use crate::{self as did, mock::*, mock_utils::*};

use ctype::mock as ctype_mock;

// create

#[test]
fn check_successful_simple_ed25519_creation() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let details = generate_base_did_creation_details::<Test>(alice_did.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_ok!(Did::create(
			Origin::signed(DEFAULT_ACCOUNT),
			details,
			did::DidSignature::from(signature),
		));
	});

	let stored_did = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(stored_did.authentication_key, generate_key_id(&auth_did_key.clone().into()));
	assert_eq!(stored_did.key_agreement_keys.len(), 0);
	assert_eq!(stored_did.delegation_key, None);
	assert_eq!(stored_did.attestation_key, None);
	assert_eq!(stored_did.public_keys.len(), 1);
	assert!(stored_did.public_keys.contains_key(&generate_key_id(&auth_did_key.into())));
	assert_eq!(stored_did.service_endpoints, None);
	assert_eq!(stored_did.last_tx_counter, 0u64);
}

#[test]
fn check_successful_simple_sr25519_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let details = generate_base_did_creation_details::<Test>(alice_did.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_ok!(Did::create(
			Origin::signed(DEFAULT_ACCOUNT),
			details,
			did::DidSignature::from(signature),
		));
	});

	let stored_did = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(stored_did.authentication_key, generate_key_id(&auth_did_key.clone().into()));
	assert_eq!(stored_did.key_agreement_keys.len(), 0);
	assert_eq!(stored_did.delegation_key, None);
	assert_eq!(stored_did.attestation_key, None);
	assert_eq!(stored_did.public_keys.len(), 1);
	assert!(stored_did.public_keys.contains_key(&generate_key_id(&auth_did_key.into())));
	assert_eq!(stored_did.service_endpoints, None);
	assert_eq!(stored_did.last_tx_counter, 0u64);
}

#[test]
fn check_successful_simple_ecdsa_creation() {
	let auth_key = get_ecdsa_authentication_key(true);
	let alice_did = get_did_identifier_from_ecdsa_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let details = generate_base_did_creation_details::<Test>(alice_did.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_ok!(Did::create(
			Origin::signed(DEFAULT_ACCOUNT),
			details,
			did::DidSignature::from(signature),
		));
	});

	let stored_did = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(stored_did.authentication_key, generate_key_id(&auth_did_key.clone().into()));
	assert_eq!(stored_did.key_agreement_keys.len(), 0);
	assert_eq!(stored_did.delegation_key, None);
	assert_eq!(stored_did.attestation_key, None);
	assert_eq!(stored_did.public_keys.len(), 1);
	assert!(stored_did.public_keys.contains_key(&generate_key_id(&auth_did_key.into())));
	assert_eq!(stored_did.service_endpoints, None);
	assert_eq!(stored_did.last_tx_counter, 0u64);
}

#[test]
fn check_successful_complete_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let enc_keys = did::DidNewKeyAgreementKeySet::<Test>::try_from(
		vec![get_x25519_encryption_key(true), get_x25519_encryption_key(false)]
			.iter()
			.copied()
			.collect::<BTreeSet<did::DidEncryptionKey>>(),
	)
	.expect("Exceeded BoundedBTreeSet bounds when creating new key agreement keys");
	let del_key = get_sr25519_delegation_key(true);
	let att_key = get_ecdsa_attestation_key(true);
	let new_service_endpoints = get_service_endpoints::<Test>(1, 10);
	let mut details = generate_base_did_creation_details::<Test>(alice_did.clone());
	details.new_key_agreement_keys = enc_keys.clone();
	details.new_attestation_key = Some(did::DidVerificationKey::from(att_key.public()));
	details.new_delegation_key = Some(did::DidVerificationKey::from(del_key.public()));
	details.new_service_endpoints = Some(new_service_endpoints.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_ok!(Did::create(
			Origin::signed(DEFAULT_ACCOUNT),
			details.clone(),
			did::DidSignature::from(signature),
		));
	});

	let stored_did = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(stored_did.authentication_key, generate_key_id(&auth_did_key.clone().into()));
	assert_eq!(stored_did.key_agreement_keys.len(), 2);
	for key in enc_keys.iter().copied() {
		assert!(stored_did.key_agreement_keys.contains(&generate_key_id(&key.into())))
	}
	assert_eq!(
		stored_did.delegation_key,
		Some(generate_key_id(&details.new_delegation_key.clone().unwrap().into()))
	);
	assert_eq!(
		stored_did.attestation_key,
		Some(generate_key_id(&details.new_attestation_key.clone().unwrap().into()))
	);
	// Authentication key + 2 * Encryption key + Delegation key + Attestation key =
	// 5
	assert_eq!(stored_did.public_keys.len(), 5);
	assert!(stored_did.public_keys.contains_key(&generate_key_id(&auth_did_key.into())));
	let mut key_agreement_keys_iterator = details.new_key_agreement_keys.iter().copied();
	assert!(stored_did
		.public_keys
		.contains_key(&generate_key_id(&key_agreement_keys_iterator.next().unwrap().into())));
	assert!(stored_did
		.public_keys
		.contains_key(&generate_key_id(&key_agreement_keys_iterator.next().unwrap().into())));
	assert!(stored_did
		.public_keys
		.contains_key(&generate_key_id(&details.new_attestation_key.clone().unwrap().into())));
	assert!(stored_did
		.public_keys
		.contains_key(&generate_key_id(&details.new_delegation_key.clone().unwrap().into())));
	assert!(stored_did.service_endpoints.is_some());
	assert_eq!(stored_did.service_endpoints.clone().unwrap().urls.len(), 1);
	assert_eq!(stored_did.service_endpoints.unwrap().urls[0], new_service_endpoints.urls[0]);
}

#[test]
fn check_duplicate_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let mock_did = generate_base_did_details::<Test>(auth_did_key);
	let details = generate_base_did_creation_details::<Test>(alice_did.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(alice_did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::DidAlreadyPresent
		);
	});
}

#[test]
fn check_invalid_signature_format_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Using an Ed25519 key where an Sr25519 is expected
	let invalid_key = get_ed25519_authentication_key(true);
	// DID creation contains auth_key, but signature is generated using invalid_key
	let details = generate_base_did_creation_details::<Test>(alice_did);

	let signature = invalid_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn check_invalid_signature_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	let alternative_key = get_sr25519_authentication_key(false);
	let details = generate_base_did_creation_details::<Test>(alice_did);

	let signature = alternative_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn check_swapped_did_subject_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let swapped_key = get_sr25519_authentication_key(false);
	let swapped_did = get_did_identifier_from_sr25519_key(swapped_key.public());
	let details = generate_base_did_creation_details::<Test>(swapped_did);

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::InvalidSignature
		);
	});
}

#[test]
#[should_panic = "Failed to convert key_agreement_keys to BoundedBTreeSet"]
fn check_max_limit_key_agreement_keys_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Max keys allowed + 1
	let enc_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxNewKeyAgreementKeys::get().saturating_add(1),
	);
	let mut details = generate_base_did_creation_details::<Test>(alice_did);
	details.new_key_agreement_keys = enc_keys;

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::MaxKeyAgreementKeysLimitExceeded
		);
	});
}

#[test]
#[should_panic = "Failed to create default URL with provided length"]
fn check_url_too_long_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Max URL length allowed + 1
	let service_endpoints = get_service_endpoints::<Test>(
		1,
		<Test as did::Config>::MaxUrlLength::get().saturating_add(1),
	);

	let mut details = generate_base_did_creation_details::<Test>(alice_did);
	details.new_service_endpoints = Some(service_endpoints);

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::MaxUrlLengthExceeded
		);
	});
}

#[test]
#[should_panic = "Exceeded max endpoint urls when creating service endpoints"]
fn check_too_many_urls_did_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Max number of URLs allowed + 1
	let service_endpoints = get_service_endpoints(
		<Test as did::Config>::MaxEndpointUrlsCount::get().saturating_add(1),
		10,
	);

	let mut details = generate_base_did_creation_details(alice_did);
	details.new_service_endpoints = Some(service_endpoints);

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::create(
				Origin::signed(DEFAULT_ACCOUNT),
				details,
				did::DidSignature::from(signature),
			),
			did::Error::<Test>::MaxUrlsCountExceeded
		);
	});
}

// updates

#[test]
fn check_successful_authentication_key_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let new_auth_key = get_ed25519_authentication_key(false);

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update authentication key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_authentication_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_auth_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.authentication_key,
		generate_key_id(&did::DidVerificationKey::from(new_auth_key.public()).into())
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new auth key, -1 for the old auth key (replaced) = 1
	assert_eq!(public_keys.len(), 1);
	// Check for new authentication key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_auth_key.public()).into()
	)));
}

#[test]
fn check_successful_authentication_key_max_public_keys_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, yet the update should still go through since the old key is removed
	// before the new one is added. So the # of new key agreement keys is max public
	// key - 1 (since we already have the old authentication key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);
	let new_auth_key = get_ed25519_authentication_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update authentication key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_authentication_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_auth_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.authentication_key,
		generate_key_id(&did::DidVerificationKey::from(new_auth_key.public()).into())
	);
	let public_keys = new_did_details.public_keys;
	// Total is the maximum allowed
	assert_eq!(
		public_keys.len(),
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturated_into::<usize>()
	);
	// Check for new authentication key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_auth_key.public()).into()
	)));
}

#[test]
fn check_reused_key_authentication_key_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let old_delegation_key = old_auth_key.clone();
	let new_auth_key = get_ed25519_authentication_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	// Same key for auth and del key
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_delegation_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_authentication_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_auth_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.authentication_key,
		generate_key_id(&did::DidVerificationKey::from(new_auth_key.public()).into())
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new auth key (the old key is still used as delegation
	// key, so it is not removed)
	assert_eq!(public_keys.len(), 2);
	// Check for new authentication key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_auth_key.public()).into()
	)));
	// Check for old authentication key (delegation key)
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_auth_key.public()).into()
	)));
}

#[test]
fn check_max_keys_authentication_key_update_error() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let old_delegation_key = old_auth_key.clone();
	// Key agreement keys = max keys - 1, as auth key and delegation key are the
	// same
	println!("{:?}", <Test as did::Config>::MaxPublicKeysPerDid::get());
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);
	let new_auth_key = get_ed25519_authentication_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_delegation_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update authentication key. Since the old one is not removed because it is the
	// same as the delegation key, the update should fail as the max number of
	// public keys is already present.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_authentication_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_auth_key.public())
			),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_did_not_present_authentication_key_update_error() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let new_auth_key = get_ed25519_authentication_key(false);

	let mut ext = ExtBuilder::default().build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update authentication key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_authentication_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_auth_key.public())
			),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_successful_delegation_key_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_del_key = get_sr25519_delegation_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_del_key = get_sr25519_delegation_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_delegation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_del_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.delegation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_del_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new del key, -1 for the old del key (replaced) + auth key
	// = 2
	assert_eq!(public_keys.len(), 2);
	// Check for new delegation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_del_key.public()).into()
	)));
}

#[test]
fn check_successful_delegation_key_max_public_keys_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let old_del_key = get_sr25519_delegation_key(true);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, yet the update should still go through since the old key is removed
	// before the new one is added. So the # of new key agreement keys is max public
	// key - 2 (plus old authentication key and delegation key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(2),
	);
	let new_del_key = get_sr25519_delegation_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_delegation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_del_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.delegation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_del_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is the maximum allowed
	assert_eq!(
		public_keys.len(),
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturated_into::<usize>()
	);
	// Check for new delegation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_del_key.public()).into()
	)));
}

#[test]
fn check_reused_key_delegation_key_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let old_del_key = old_auth_key.clone();
	let new_del_key = get_sr25519_delegation_key(true);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	// Same key for auth and del key
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_delegation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_del_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.delegation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_del_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new del key (the old key is still used as authentication
	// key, so it is not removed)
	assert_eq!(public_keys.len(), 2);
	// Check for new delegation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_del_key.public()).into()
	)));
	// Check for old delegation key (authentication key)
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_del_key.public()).into()
	)));
}

#[test]
fn check_max_public_keys_delegation_key_addition_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_del_key = get_sr25519_delegation_key(false);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, so that key update would fail (since key is added and not replaced). So
	// the # of new key agreement keys is max public key - 1 (since we already have
	// the authentication key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_delegation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_del_key.public())
			),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_max_public_keys_reused_key_delegation_key_update_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let old_del_key = auth_key.clone();
	let new_del_key = get_sr25519_delegation_key(true);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, so that key update would fail (since key is added and not replaced). So
	// the # of new key agreement keys is max public key - 1 (since we already have
	// the authentication and delegation keys which are actually the same key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));
	// Same key for auth and delegation
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should not be removed as it is still used
	// as authentication key.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_delegation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_del_key.public())
			),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_did_not_present_delegation_key_update_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_del_key = get_sr25519_delegation_key(false);

	let mut ext = ExtBuilder::default().build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_delegation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_del_key.public())
			),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_successful_delegation_key_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_del_key = get_sr25519_delegation_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_delegation_key(Origin::signed(alice_did.clone())));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert!(new_did_details.delegation_key.is_none());
	let public_keys = new_did_details.public_keys;
	// Total is -1 for the removal + auth key = 1
	assert_eq!(public_keys.len(), 1);
	// Check for new delegation key
	assert!(!public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_del_key.public()).into()
	)));
}

#[test]
fn check_successful_reused_delegation_key_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_del_key = auth_key.clone();
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_delegation_key(did::DidVerificationKey::from(old_del_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_delegation_key(Origin::signed(alice_did.clone())));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert!(new_did_details.delegation_key.is_none());
	let public_keys = new_did_details.public_keys;
	// Total should be unchanged as the key was re-used so it is not completely
	// deleted
	assert_eq!(public_keys.len(), old_did_details.public_keys.len());
	// Check for presence of old delegation key (authentication key)
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_del_key.public()).into()
	)));
}

#[test]
fn check_did_not_present_delegation_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_delegation_key(Origin::signed(alice_did.clone())),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_key_not_present_delegation_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_delegation_key(Origin::signed(alice_did.clone())),
			did::Error::<Test>::VerificationKeyNotPresent
		);
	});
}

#[test]
fn check_successful_attestation_key_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_att_key = get_sr25519_attestation_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_att_key = get_sr25519_attestation_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update attestation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_attestation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_att_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.attestation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_att_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new att key, -1 for the old att key (replaced) + auth key
	// = 2
	assert_eq!(public_keys.len(), 2);
	// Check for new attestation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
}

#[test]
fn check_successful_attestation_key_max_public_keys_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let old_att_key = get_sr25519_attestation_key(true);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, yet the update should still go through since the old key is removed
	// before the new one is added. So the # of new key agreement keys is max public
	// key - 2 (plus old authentication key and attestation key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(2),
	);
	let new_att_key = get_sr25519_attestation_key(false);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update attestation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_attestation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_att_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.attestation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_att_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is the maximum allowed
	assert_eq!(
		public_keys.len(),
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturated_into::<usize>()
	);
	// Check for new attestation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
}

#[test]
fn check_reused_key_attestation_key_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let old_att_key = old_auth_key.clone();
	let new_att_key = get_sr25519_attestation_key(true);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	// Same key for auth and att key
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::set_attestation_key(
			Origin::signed(alice_did.clone()),
			did::DidVerificationKey::from(new_att_key.public())
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.attestation_key,
		Some(generate_key_id(&did::DidVerificationKey::from(new_att_key.public()).into()))
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new att key (the old key is still used as authentication
	// key, so it is not removed)
	assert_eq!(public_keys.len(), 2);
	// Check for new attestation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
	// Check for old attestation key (authentication key)
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_att_key.public()).into()
	)));
}

#[test]
fn check_max_public_keys_attestation_key_addition_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_att_key = get_sr25519_attestation_key(false);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, so that key update would fail (since key is added and not replaced). So
	// the # of new key agreement keys is max public key - 1 (since we already have
	// the authentication key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update attestation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_attestation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_att_key.public())
			),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_max_public_keys_reused_key_attestation_key_update_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let old_att_key = auth_key.clone();
	let new_att_key = get_sr25519_delegation_key(true);
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, so that key update would fail (since key is added and not replaced). So
	// the # of new key agreement keys is max public key - 1 (since we already have
	// the authentication and attestation keys which are actually the same key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));
	// Same key for auth and attestation
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update attestation key. The old one should not be removed as it is still used
	// as authentication key.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_attestation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_att_key.public())
			),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_did_not_present_attestation_key_update_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_att_key = get_sr25519_delegation_key(false);

	let mut ext = ExtBuilder::default().build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::set_delegation_key(
				Origin::signed(alice_did.clone()),
				did::DidVerificationKey::from(new_att_key.public())
			),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_successful_attestation_key_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_att_key = get_sr25519_attestation_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_attestation_key(Origin::signed(alice_did.clone())));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert!(new_did_details.attestation_key.is_none());
	let public_keys = new_did_details.public_keys;
	// Total is -1 for the removal + auth key = 1
	assert_eq!(public_keys.len(), 1);
	// Check for new attestation key
	assert!(!public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_att_key.public()).into()
	)));
}

#[test]
fn check_successful_reused_attestation_key_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let old_att_key = auth_key.clone();
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details
		.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_attestation_key(Origin::signed(alice_did.clone())));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert!(new_did_details.attestation_key.is_none());
	let public_keys = new_did_details.public_keys;
	// Total should be unchanged as the key was re-used so it is not completely
	// deleted
	assert_eq!(public_keys.len(), old_did_details.public_keys.len());
	// Check for presence of old delegation key (authentication key)
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_att_key.public()).into()
	)));
}

#[test]
fn check_did_not_present_attestation_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_attestation_key(Origin::signed(alice_did.clone())),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_key_not_present_attestation_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_attestation_key(Origin::signed(alice_did.clone())),
			did::Error::<Test>::VerificationKeyNotPresent
		);
	});
}

#[test]
fn check_successful_key_agreement_key_addition() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_key_agreement_key = get_x25519_encryption_key(true);

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::add_key_agreement_key(
			Origin::signed(alice_did.clone()),
			new_key_agreement_key,
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(new_did_details.key_agreement_keys.len(), 1);
	assert_eq!(
		new_did_details.key_agreement_keys.iter().next().unwrap(),
		&generate_key_id(&new_key_agreement_key.into())
	);
	let public_keys = new_did_details.public_keys;
	// Total is +1 for the new enc key + auth key = 2
	assert_eq!(public_keys.len(), 2);
	// Check for new key agreement key
	assert!(public_keys.contains_key(&generate_key_id(&new_key_agreement_key.into())));
}

#[test]
fn check_max_public_keys_key_agreement_key_addition_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	// Set maximum number of keys possible for the DID all inside the key agreement
	// keys, so that key update would fail (since key is added and not replaced). So
	// the # of new key agreement keys is max public key - 1 (since we already have
	// the authentication key)
	let old_key_agreement_keys = get_key_agreement_keys::<Test>(
		<Test as did::Config>::MaxPublicKeysPerDid::get().saturating_sub(1),
	);
	let new_key_agreement_key = get_x25519_encryption_key(true);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(old_key_agreement_keys, 0u64,));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::add_key_agreement_key(Origin::signed(alice_did.clone()), new_key_agreement_key,),
			did::Error::<Test>::MaxPublicKeysPerDidExceeded
		);
	});
}

#[test]
fn check_did_not_present_key_agreement_key_addition_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_enc_key = get_x25519_encryption_key(true);

	let mut ext = ExtBuilder::default().build(None);

	let new_block_number: TestBlockNumber = 1;

	// Update delegation key. The old one should be removed.
	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::add_key_agreement_key(Origin::signed(alice_did.clone()), new_enc_key),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_successful_key_agreement_key_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let old_enc_key = get_x25519_encryption_key(true);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_key(old_enc_key, 0u64));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_key_agreement_key(
			Origin::signed(alice_did.clone()),
			generate_key_id(&old_enc_key.into()),
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert!(new_did_details.key_agreement_keys.is_empty());
	let public_keys = new_did_details.public_keys;
	// Total is -1 for the enc key removal + auth key = 1
	assert_eq!(public_keys.len(), 1);
	// Check for new key agreement key
	assert!(!public_keys.contains_key(&generate_key_id(&old_enc_key.into())));
}

#[test]
fn check_did_not_found_key_agreement_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let test_enc_key = get_x25519_encryption_key(true);

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_key_agreement_key(
				Origin::signed(alice_did.clone()),
				generate_key_id(&test_enc_key.into()),
			),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_key_not_found_key_agreement_key_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let test_enc_key = get_x25519_encryption_key(true);

	// No enc key added
	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_key_agreement_key(
				Origin::signed(alice_did.clone()),
				generate_key_id(&test_enc_key.into()),
			),
			did::Error::<Test>::VerificationKeyNotPresent
		);
	});
}

#[test]
fn check_successful_service_endpoints_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let new_service_endpoints = get_service_endpoints::<Test>(1, 10);

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::set_service_endpoints(
			Origin::signed(alice_did.clone()),
			new_service_endpoints.clone(),
		));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	// Check for new service endpoints
	assert!(new_did_details.service_endpoints.is_some());
	assert_eq!(new_did_details.service_endpoints.clone().unwrap().urls.len(), 1);
	assert_eq!(new_did_details.service_endpoints.unwrap().urls[0], new_service_endpoints.urls[0]);
}

#[test]
#[should_panic = "Failed to create default URL with provided length"]
fn check_url_too_long_endpoint_update_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Max URL length allowed + 1
	let new_service_endpoints = get_service_endpoints::<Test>(
		1,
		<Test as did::Config>::MaxUrlLength::get().saturating_add(1),
	);

	let did_details = generate_base_did_details(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::set_service_endpoints(Origin::signed(alice_did.clone()), new_service_endpoints,),
			did::Error::<Test>::MaxUrlLengthExceeded
		);
	});
}

#[test]
#[should_panic = "Exceeded max endpoint urls when creating service endpoints"]
fn check_too_many_urls_endpoint_update_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Max number of URLs allowed + 1
	let new_service_endpoints = get_service_endpoints(
		<Test as did::Config>::MaxEndpointUrlsCount::get().saturating_add(1),
		10,
	);

	let did_details = generate_base_did_details(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::set_service_endpoints(Origin::signed(alice_did.clone()), new_service_endpoints,),
			did::Error::<Test>::MaxUrlsCountExceeded
		);
	});
}

#[test]
fn check_successful_service_endpoints_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let service_endpoints = get_service_endpoints::<Test>(1, 10);

	let mut old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	old_did_details.service_endpoints = Some(service_endpoints);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::remove_service_endpoints(Origin::signed(alice_did.clone())));
	});

	let new_did_details = ext
		.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	// Check for new service endpoints
	assert!(new_did_details.service_endpoints.is_none());
}

#[test]
fn check_did_not_present_service_endpoints_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_service_endpoints(Origin::signed(alice_did.clone())),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_endpoints_not_present_service_endpoints_deletion_error() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let old_did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::remove_service_endpoints(Origin::signed(alice_did.clone())),
			did::Error::<Test>::DidFragmentNotPresent
		);
	});
}

// delete

#[test]
fn check_successful_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let did_details =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), did_details)])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::delete(Origin::signed(alice_did.clone()),));
	});

	assert!(ext.execute_with(|| Did::get_did(alice_did.clone())).is_none());

	// Re-adding the same DID identifier, which should not fail.
	let details = generate_base_did_creation_details::<Test>(alice_did.clone());

	let signature = auth_key.sign(details.encode().as_ref());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_ok!(Did::create(
			Origin::signed(alice_did.clone()),
			details,
			did::DidSignature::from(signature),
		));
	});
}

#[test]
fn check_did_not_present_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(Did::delete(Origin::signed(alice_did)), did::Error::<Test>::DidNotPresent);
	});
}

// submit_did_call

#[test]
fn check_did_not_found_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;

	// No DID added
	let mut ext = ExtBuilder::default().build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
fn check_max_counter_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = u64::MAX;

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::MaxTxCounterValue
		);
	});
}

#[test]
fn check_too_small_tx_counter_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = 1u64;

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	call_operation.operation.tx_counter = 0u64;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::InvalidNonce
		);
	});
}

#[test]
fn check_equal_tx_counter_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	call_operation.operation.tx_counter = mock_did.last_tx_counter;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::InvalidNonce
		);
	});
}

#[test]
fn check_too_large_tx_counter_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	call_operation.operation.tx_counter = mock_did.last_tx_counter + 2u64;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::InvalidNonce
		);
	});
}

#[test]
fn check_verification_key_not_present_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	// The operation requires the delegation key that is currently not stored for
	// the given DID.
	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::VerificationKeyNotPresent
		);
	});
}

#[test]
fn check_invalid_signature_format_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let alternative_auth_key = get_ed25519_authentication_key(true);
	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = alternative_auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::InvalidSignatureFormat
		);
	});
}

#[test]
fn check_invalid_signature_call_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let alternative_auth_key = get_sr25519_authentication_key(false);
	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = alternative_auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn check_call_attestation_key_successful() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let attestation_key = get_ed25519_attestation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0)
	);

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
	let signature = attestation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::submit_did_call(
			Origin::signed(caller),
			Box::new(call_operation.operation),
			did::DidSignature::from(signature)
		));
	});
}

#[test]
fn check_call_attestation_key_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let attestation_key = get_ed25519_attestation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0)
	);

	let ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_attestation_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
	let signature = attestation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_err!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			ctype::Error::<Test>::CTypeAlreadyExists
		);
	});
}

#[test]
fn check_call_delegation_key_successful() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let delegation_key = get_ed25519_delegation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0)
	);

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = delegation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::submit_did_call(
			Origin::signed(caller),
			Box::new(call_operation.operation),
			did::DidSignature::from(signature)
		));
	});
}

#[test]
fn check_call_delegation_key_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;
	let delegation_key = get_ed25519_delegation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0)
	);

	let ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_delegation_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = delegation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_err!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			ctype::Error::<Test>::CTypeAlreadyExists
		);
	});
}

#[test]
fn check_call_authentication_key_successful() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::submit_did_call(
			Origin::signed(caller),
			Box::new(call_operation.operation),
			did::DidSignature::from(signature)
		));
	});
}

#[test]
fn check_call_authentication_key_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let ext = ExtBuilder::default().with_dids(vec![(did.clone(), mock_did)]).build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_authentication_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_err!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			ctype::Error::<Test>::CTypeAlreadyExists
		);
	});
}

#[test]
fn check_null_key_error() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let caller = DEFAULT_ACCOUNT;

	let mut ext = ExtBuilder::default().build(None);

	// CapabilityInvocation is not supported at the moment, so it should return no
	// key and hence the operation fail.
	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityInvocation, did);
	let signature = ed25519::Signature::default();

	ext.execute_with(|| {
		assert_noop!(
			Did::submit_did_call(
				Origin::signed(caller),
				Box::new(call_operation.operation),
				did::DidSignature::from(signature)
			),
			did::Error::<Test>::UnsupportedDidAuthorizationCall
		);
	});
}

// Internal function: verify_did_operation_signature_and_increase_nonce

#[test]
fn check_authentication_successful_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details = ext.execute_with(|| {
		Did::get_did(&call_operation.operation.did).expect("DID should be present on chain.")
	});
	assert_eq!(did_details.get_tx_counter_value(), mock_did.get_tx_counter_value() + 1u64);
}

#[test]
fn check_attestation_successful_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let attestation_key = get_ed25519_attestation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0)
	);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
	let signature = attestation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details = ext.execute_with(|| {
		Did::get_did(&call_operation.operation.did).expect("DID should be present on chain.")
	});
	assert_eq!(did_details.get_tx_counter_value(), mock_did.get_tx_counter_value() + 1u64);
}

#[test]
fn check_delegation_successful_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let delegation_key = get_ecdsa_delegation_key(true);

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(
		mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0)
	);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = delegation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details = ext.execute_with(|| {
		Did::get_did(&call_operation.operation.did).expect("DID should be present on chain.")
	});
	assert_eq!(did_details.get_tx_counter_value(), mock_did.get_tx_counter_value() + 1u64);
}

#[test]
fn check_did_not_present_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::StorageError(did::StorageError::DidNotPresent)
		);
	});
}

#[test]
fn check_max_tx_counter_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.set_tx_counter(u64::MAX);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	call_operation.operation.tx_counter = mock_did.last_tx_counter;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::StorageError(did::StorageError::MaxTxCounterValue)
		);
	});
}

#[test]
fn check_smaller_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = 1;

	let mut call_operation = generate_test_did_call(
		did::DidVerificationKeyRelationship::CapabilityDelegation,
		did.clone(),
	);
	call_operation.operation.tx_counter = 0u64;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_equal_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut call_operation = generate_test_did_call(
		did::DidVerificationKeyRelationship::CapabilityDelegation,
		did.clone(),
	);
	call_operation.operation.tx_counter = mock_did.last_tx_counter;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_too_large_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut call_operation = generate_test_did_call(
		did::DidVerificationKeyRelationship::CapabilityDelegation,
		did.clone(),
	);
	call_operation.operation.tx_counter = mock_did.last_tx_counter + 2;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_verification_key_not_present_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did.clone());
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::StorageError(did::StorageError::DidKeyNotPresent(
				did::DidVerificationKeyRelationship::AssertionMethod
			))
		);
	});
}

#[test]
fn check_invalid_signature_format_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Expected an Sr25519, given an Ed25519
	let invalid_key = get_ed25519_authentication_key(true);

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did.clone());
	let signature = invalid_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::SignatureError(did::SignatureError::InvalidSignatureFormat)
		);
	});
}

#[test]
fn check_invalid_signature_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Using same key type but different seed (default = false)
	let alternative_key = get_sr25519_authentication_key(false);

	let mock_did =
		generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did.clone());
	let signature = alternative_key.sign(&call_operation.encode());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			did::DidError::SignatureError(did::SignatureError::InvalidSignature)
		);
	});
}

// Internal function: did::HttpUrl try_from

#[test]
fn check_http_url() {
	assert_ok!(did::HttpUrl::<Test>::try_from("http://kilt.io".as_bytes()));

	assert_ok!(did::HttpUrl::<Test>::try_from("https://kilt.io".as_bytes()));

	assert_ok!(did::HttpUrl::<Test>::try_from(
		"https://super.long.domain.kilt.io:12345/public/files/test.txt".as_bytes()
	));

	// All other valid ASCII characters
	assert_ok!(did::HttpUrl::<Test>::try_from("http://:/?#[]@!$&'()*+,;=-._~".as_bytes()));

	assert_eq!(
		did::HttpUrl::<Test>::try_from("".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	// Non-printable ASCII characters
	assert_eq!(
		did::HttpUrl::<Test>::try_from("http://kilt.io/\x00".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Some invalid ASCII characters
	assert_eq!(
		did::HttpUrl::<Test>::try_from("http://kilt.io/<tag>".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Non-ASCII characters
	assert_eq!(
		did::HttpUrl::<Test>::try_from("http://¶.com".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		did::HttpUrl::<Test>::try_from("htt://kilt.io".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		did::HttpUrl::<Test>::try_from("httpss://kilt.io".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

// Internal function: did::FtpUrl try_from

#[test]
fn check_ftp_url() {
	assert_ok!(did::FtpUrl::<Test>::try_from("ftp://kilt.io".as_bytes()));

	assert_ok!(did::FtpUrl::<Test>::try_from("ftps://kilt.io".as_bytes()));

	assert_ok!(did::FtpUrl::<Test>::try_from(
		"ftps://user@super.long.domain.kilt.io:12345/public/files/test.txt".as_bytes()
	));

	// All other valid ASCII characters
	assert_ok!(did::FtpUrl::<Test>::try_from("ftps://:/?#[]@%!$&'()*+,;=-._~".as_bytes()));

	assert_eq!(
		did::FtpUrl::<Test>::try_from("".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	// Non-printable ASCII characters
	assert_eq!(
		did::HttpUrl::<Test>::try_from("http://kilt.io/\x00".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Some invalid ASCII characters
	assert_eq!(
		did::FtpUrl::<Test>::try_from("ftp://kilt.io/<tag>".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Non-ASCII characters
	assert_eq!(
		did::FtpUrl::<Test>::try_from("ftps://¶.com".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		did::FtpUrl::<Test>::try_from("ft://kilt.io".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		did::HttpUrl::<Test>::try_from("ftpss://kilt.io".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

// Internal function: did::IpfsUrl try_from

#[test]
fn check_ipfs_url() {
	// Base58 address
	assert_ok!(did::IpfsUrl::<Test>::try_from(
		"ipfs://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()
	));

	// Base32 address (at the moment, padding characters can appear anywhere in the
	// string)
	assert_ok!(did::IpfsUrl::<Test>::try_from(
		"ipfs://OQQHHHTGMMYDQQ364YB4GDE=HREJQL==".as_bytes()
	));

	assert_eq!(
		did::IpfsUrl::<Test>::try_from("".as_bytes()),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		did::IpfsUrl::<Test>::try_from(
			"ipfs://¶QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()
		),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		did::IpfsUrl::<Test>::try_from(
			"ipf://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()
		),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		did::IpfsUrl::<Test>::try_from(
			"ipfss://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQk".as_bytes()
		),
		Err(did::DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

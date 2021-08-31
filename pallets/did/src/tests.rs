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
use sp_std::{collections::btree_set::BTreeSet, convert::TryFrom};

use crate::{
	self as did, mock::*, mock_utils::*, DidError, DidNewKeyAgreementKeys, DidVerificationKeysToRevoke, FtpUrl,
	HttpUrl, IpfsUrl,
};
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

	let stored_did = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		stored_did.get_authentication_key_id(),
		generate_key_id(&auth_did_key.clone().into())
	);
	assert_eq!(stored_did.get_key_agreement_keys_ids().len(), 0);
	assert_eq!(stored_did.get_delegation_key_id(), &None);
	assert_eq!(stored_did.get_attestation_key_id(), &None);
	assert_eq!(stored_did.get_public_keys().len(), 1);
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&auth_did_key.into())));
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

	let stored_did = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		stored_did.get_authentication_key_id(),
		generate_key_id(&auth_did_key.clone().into())
	);
	assert_eq!(stored_did.get_key_agreement_keys_ids().len(), 0);
	assert_eq!(stored_did.get_delegation_key_id(), &None);
	assert_eq!(stored_did.get_attestation_key_id(), &None);
	assert_eq!(stored_did.get_public_keys().len(), 1);
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&auth_did_key.into())));
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

	let stored_did = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		stored_did.get_authentication_key_id(),
		generate_key_id(&auth_did_key.clone().into())
	);
	assert_eq!(stored_did.get_key_agreement_keys_ids().len(), 0);
	assert_eq!(stored_did.get_delegation_key_id(), &None);
	assert_eq!(stored_did.get_attestation_key_id(), &None);
	assert_eq!(stored_did.get_public_keys().len(), 1);
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&auth_did_key.into())));
	assert_eq!(stored_did.service_endpoints, None);
	assert_eq!(stored_did.last_tx_counter, 0u64);
}

#[test]
fn check_successful_complete_creation() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());
	let auth_did_key = did::DidVerificationKey::from(auth_key.public());
	let enc_keys = DidNewKeyAgreementKeys::<Test>::try_from(
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

	let stored_did = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		stored_did.get_authentication_key_id(),
		generate_key_id(&auth_did_key.clone().into())
	);
	assert_eq!(stored_did.get_key_agreement_keys_ids().len(), 2);
	for key in enc_keys.iter().copied() {
		assert!(stored_did
			.get_key_agreement_keys_ids()
			.contains(&generate_key_id(&key.into())))
	}
	assert_eq!(
		stored_did.get_delegation_key_id(),
		&Some(generate_key_id(&details.new_delegation_key.clone().unwrap().into()))
	);
	assert_eq!(
		stored_did.get_attestation_key_id(),
		&Some(generate_key_id(&details.new_attestation_key.clone().unwrap().into()))
	);
	// Authentication key + 2 * Encryption key + Delegation key + Attestation key =
	// 5
	assert_eq!(stored_did.get_public_keys().len(), 5);
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&auth_did_key.into())));
	let mut key_agreement_keys_iterator = details.new_key_agreement_keys.iter().copied();
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&key_agreement_keys_iterator.next().unwrap().into())));
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&key_agreement_keys_iterator.next().unwrap().into())));
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&details.new_attestation_key.clone().unwrap().into())));
	assert!(stored_did
		.get_public_keys()
		.contains_key(&generate_key_id(&details.new_delegation_key.clone().unwrap().into())));
	assert!(stored_did.service_endpoints.is_some());
	assert_eq!(stored_did.service_endpoints.clone().unwrap().urls.len(), 1);
	assert_eq!(
		stored_did.service_endpoints.unwrap().urls[0],
		new_service_endpoints.urls[0]
	);
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
	let enc_keys =
		get_key_agreement_keys::<Test>(<Test as did::Config>::MaxNewKeyAgreementKeys::get().saturating_add(1));
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
	let service_endpoints =
		get_service_endpoints::<Test>(1, <Test as did::Config>::MaxUrlLength::get().saturating_add(1));

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
	let service_endpoints =
		get_service_endpoints(<Test as did::Config>::MaxEndpointUrlsCount::get().saturating_add(1), 10);

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

// update

#[test]
fn check_successful_complete_update() {
	let old_auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(old_auth_key.public());
	let new_auth_key = get_ed25519_authentication_key(false);
	let old_enc_key = get_x25519_encryption_key(true);
	let new_enc_key = get_x25519_encryption_key(false);
	let old_att_key = get_ed25519_attestation_key(true);
	let new_att_key = get_ed25519_attestation_key(false);
	let new_del_key = get_sr25519_delegation_key(true);
	let new_service_endpoints = get_service_endpoints::<Test>(1, 10);

	let mut old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(old_auth_key.public()));
	assert_ok!(old_did_details.add_key_agreement_keys(
		DidNewKeyAgreementKeys::<Test>::try_from(
			vec![old_enc_key]
				.iter()
				.copied()
				.collect::<BTreeSet<did::DidEncryptionKey>>(),
		)
		.expect("Should not fail to create BoundedBTreeSet from a single element"),
		0u64,
	));
	assert_ok!(old_did_details.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	// Update all keys, URL endpoint and tx counter. The old key agreement key is
	// removed.
	let mut details = generate_base_did_update_details::<Test>();
	details.new_authentication_key = Some(did::DidVerificationKey::from(new_auth_key.public()));
	details.new_key_agreement_keys = DidNewKeyAgreementKeys::<Test>::try_from(
		vec![new_enc_key]
			.iter()
			.copied()
			.collect::<BTreeSet<did::DidEncryptionKey>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");
	details.attestation_key_update =
		did::DidFragmentUpdateAction::Change(did::DidVerificationKey::from(new_att_key.public()));
	details.delegation_key_update =
		did::DidFragmentUpdateAction::Change(did::DidVerificationKey::from(new_del_key.public()));
	details.public_keys_to_remove = DidVerificationKeysToRevoke::<Test>::try_from(
		vec![generate_key_id(&old_enc_key.into())]
			.iter()
			.copied()
			.collect::<BTreeSet<TestKeyId>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");
	details.service_endpoints_update = did::DidFragmentUpdateAction::Change(new_service_endpoints.clone());

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		generate_key_id(&did::DidVerificationKey::from(new_auth_key.public()).into())
	);
	// Old one deleted, new one added -> Total keys = 1
	assert_eq!(new_did_details.get_key_agreement_keys_ids().len(), 1);
	assert_eq!(
		new_did_details.get_key_agreement_keys_ids().iter().next().unwrap(),
		&generate_key_id(&new_enc_key.into())
	);
	assert_eq!(
		new_did_details.get_attestation_key_id(),
		&Some(generate_key_id(
			&did::DidVerificationKey::from(new_att_key.public()).into()
		))
	);
	assert_eq!(
		new_did_details.get_delegation_key_id(),
		&Some(generate_key_id(
			&did::DidVerificationKey::from(new_del_key.public()).into()
		))
	);

	// Total is +1 for the new auth key, -1 for the old auth key (replaced), +1 for
	// the new key agreement key, -1 for the old key agreement key (deleted), +1 for
	// the old attestation key, +1 for the new attestation key, +1 for the new
	// delegation key = 5
	let public_keys = new_did_details.get_public_keys();
	assert_eq!(public_keys.len(), 5);
	// Check for new authentication key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
	// Check for new key agreement key
	assert!(public_keys.contains_key(&generate_key_id(&new_enc_key.into())));
	// Check for old attestation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(old_att_key.public()).into()
	)));
	// Check for new attestation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
	// Check for new delegation key
	assert!(public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_del_key.public()).into()
	)));
	// Check for new service endpoints
	assert!(new_did_details.service_endpoints.is_some());
	assert_eq!(new_did_details.service_endpoints.clone().unwrap().urls.len(), 1);
	assert_eq!(
		new_did_details.service_endpoints.unwrap().urls[0],
		new_service_endpoints.urls[0]
	);
}

#[test]
fn check_successful_keys_deletion_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let att_key = get_ed25519_attestation_key(true);
	let del_key = get_sr25519_delegation_key(true);

	let mut old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.update_attestation_key(did::DidVerificationKey::from(att_key.public()), 0u64));
	assert_ok!(old_did_details.update_delegation_key(did::DidVerificationKey::from(del_key.public()), 0u64));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	details.attestation_key_update = did::DidFragmentUpdateAction::Delete;
	details.delegation_key_update = did::DidFragmentUpdateAction::Delete;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	// Auth key and key agreement key unchanged
	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		old_did_details.get_authentication_key_id()
	);
	assert_eq!(
		new_did_details.get_key_agreement_keys_ids(),
		old_did_details.get_key_agreement_keys_ids()
	);
	assert_eq!(new_did_details.get_attestation_key_id(), &None);
	assert_eq!(new_did_details.get_delegation_key_id(), &None);

	// Public keys should now contain only the authentication key and the revoked
	// attestation key
	let stored_public_keys = new_did_details.get_public_keys();
	assert_eq!(stored_public_keys.len(), 2);
	assert!(stored_public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(auth_key.public()).into()
	)));
	assert!(stored_public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(att_key.public()).into()
	)));
}

#[test]
fn check_successful_endpoints_deletion_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let service_endpoints = get_service_endpoints(1, 10);

	let mut old_did_details = generate_base_did_details(did::DidVerificationKey::from(auth_key.public()));
	old_did_details.service_endpoints = Some(service_endpoints);

	// Remove the service endpoints
	let mut details = generate_base_did_update_details();
	details.service_endpoints_update = did::DidFragmentUpdateAction::Delete;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	// Auth key and key agreement key unchanged
	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		old_did_details.get_authentication_key_id()
	);
	assert_eq!(
		new_did_details.get_key_agreement_keys_ids(),
		old_did_details.get_key_agreement_keys_ids()
	);
	assert_eq!(new_did_details.get_attestation_key_id(), &None);
	assert_eq!(new_did_details.get_delegation_key_id(), &None);

	// Service endpoints should now be None
	assert!(new_did_details.service_endpoints.is_none());
}

#[test]
fn check_successful_endpoints_ignore_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let service_endpoints = get_service_endpoints(1, 10);

	let mut old_did_details = generate_base_did_details(did::DidVerificationKey::from(auth_key.public()));
	old_did_details.service_endpoints = Some(service_endpoints.clone());

	// By default all actions are `Ignore`, including the service endpoint action.
	let details = generate_base_did_update_details();

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	// Auth key and key agreement key unchanged
	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		old_did_details.get_authentication_key_id()
	);
	assert_eq!(
		new_did_details.get_key_agreement_keys_ids(),
		old_did_details.get_key_agreement_keys_ids()
	);
	assert_eq!(new_did_details.get_attestation_key_id(), &None);
	assert_eq!(new_did_details.get_delegation_key_id(), &None);

	// Service endpoints should remain unchanged
	assert_eq!(new_did_details.service_endpoints, Some(service_endpoints));
}

#[test]
fn check_successful_keys_overwrite_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	// Same as the authentication key -> leads to two keys having the same ID
	let new_att_key = auth_key.clone();

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	details.attestation_key_update =
		did::DidFragmentUpdateAction::Change(did::DidVerificationKey::from(new_att_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	// Auth key unchanged
	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		old_did_details.get_authentication_key_id()
	);
	// New attestation key and authentication key should now have the same ID
	assert_eq!(
		new_did_details.get_attestation_key_id(),
		&Some(old_did_details.get_authentication_key_id())
	);

	// As the two keys have the same ID, public keys still contain only one element
	let stored_public_keys = new_did_details.get_public_keys();
	assert_eq!(stored_public_keys.len(), 1);
	assert!(stored_public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(auth_key.public()).into()
	)));
	assert!(stored_public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(new_att_key.public()).into()
	)));
	// The block number should be the updated to the latest one, even if the ID was
	// also present before.
	let stored_key_details = stored_public_keys
		.get(&old_did_details.get_authentication_key_id())
		.expect("There should be a key with the given ID stored on chain.");
	assert_eq!(stored_key_details.block_number, new_block_number);
}

#[test]
fn check_successful_keys_multiuse_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	// Same as the authentication key -> leads to two keys having the same ID
	let old_att_key = auth_key.clone();

	let mut old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.update_attestation_key(did::DidVerificationKey::from(old_att_key.public()), 0u64));

	// Remove attestation key
	let mut details = generate_base_did_update_details::<Test>();
	details.attestation_key_update = did::DidFragmentUpdateAction::Delete;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details.clone())])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_ok!(Did::update(Origin::signed(alice_did.clone()), details));
	});

	// Auth key unchanged
	let new_did_details = ext.execute_with(|| Did::get_did(&alice_did).expect("ALICE_DID should be present on chain."));
	assert_eq!(
		new_did_details.get_authentication_key_id(),
		old_did_details.get_authentication_key_id()
	);
	// Attestation key should now be set to None
	assert_eq!(new_did_details.get_attestation_key_id(), &None);

	// As the two keys have the same ID, public keys still contain only one element
	let stored_public_keys = new_did_details.get_public_keys();
	assert_eq!(stored_public_keys.len(), 1);
	assert!(stored_public_keys.contains_key(&generate_key_id(
		&did::DidVerificationKey::from(auth_key.public()).into()
	)));
}

#[test]
fn check_did_not_present_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let bob_auth_key = get_ed25519_authentication_key(false);
	let bob_did = get_did_identifier_from_ed25519_key(bob_auth_key.public());
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	let details = generate_base_did_update_details::<Test>();

	let mut ext = ExtBuilder::default().with_dids(vec![(bob_did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::DidNotPresent
		);
	});
}

#[test]
#[should_panic = "Failed to convert key_agreement_keys to BoundedBTreeSet"]
fn check_max_limit_key_agreement_keys_did_update() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Max keys allowed + 1
	let new_enc_keys =
		get_key_agreement_keys::<Test>(<Test as did::Config>::MaxNewKeyAgreementKeys::get().saturating_add(1));

	let mut details = generate_base_did_update_details::<Test>();
	details.new_key_agreement_keys = new_enc_keys;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::MaxKeyAgreementKeysLimitExceeded
		);
	});
}

#[test]
#[should_panic = "Failed to convert get_public_keys to BoundedBTreeSet"]
fn check_max_limit_public_keys_to_remove_did_update() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Max keys allowed + 1
	let keys_ids_to_remove =
		get_public_keys::<Test>(<Test as did::Config>::MaxNewKeyAgreementKeys::get().saturating_add(1));

	let mut details = generate_base_did_update_details::<Test>();
	details.public_keys_to_remove = keys_ids_to_remove;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::MaxVerificationKeysToRemoveLimitExceeded
		);
	});
}

#[test]
#[should_panic = "Failed to create default URL with provided length"]
fn check_url_too_long_did_update() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Max URL length allowed + 1
	let new_service_endpoints =
		get_service_endpoints::<Test>(1, <Test as did::Config>::MaxUrlLength::get().saturating_add(1));

	let mut details = generate_base_did_update_details::<Test>();
	details.service_endpoints_update = did::DidFragmentUpdateAction::Change(new_service_endpoints);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::MaxUrlLengthExceeded
		);
	});
}

#[test]
#[should_panic = "Exceeded max endpoint urls when creating service endpoints"]
fn check_too_many_urls_did_update() {
	let auth_key = get_sr25519_authentication_key(true);
	let alice_did = get_did_identifier_from_sr25519_key(auth_key.public());

	let old_did_details = generate_base_did_details(did::DidVerificationKey::from(auth_key.public()));

	// Max URL length allowed + 1
	let new_service_endpoints =
		get_service_endpoints(<Test as did::Config>::MaxEndpointUrlsCount::get().saturating_add(1), 10);

	let mut details = generate_base_did_update_details();
	details.service_endpoints_update = did::DidFragmentUpdateAction::Change(new_service_endpoints);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::MaxUrlsCountExceeded
		);
	});
}

#[test]
fn check_currently_active_authentication_key_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	// Trying to remove the currently active authentication key
	details.public_keys_to_remove = DidVerificationKeysToRevoke::<Test>::try_from(
		vec![generate_key_id(
			&did::DidVerificationKey::from(auth_key.public()).into(),
		)]
		.iter()
		.copied()
		.collect::<BTreeSet<TestKeyId>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::CurrentlyActiveKey
		);
	});
}

#[test]
fn check_currently_active_delegation_key_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let del_key = get_ecdsa_delegation_key(true);

	let mut old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.update_delegation_key(did::DidVerificationKey::from(del_key.public()), 0u64));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	// Trying to remove the currently active delegation key
	details.public_keys_to_remove = DidVerificationKeysToRevoke::<Test>::try_from(
		vec![generate_key_id(&did::DidVerificationKey::from(del_key.public()).into())]
			.iter()
			.copied()
			.collect::<BTreeSet<TestKeyId>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::CurrentlyActiveKey
		);
	});
}

#[test]
fn check_currently_active_attestation_key_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let att_key = get_sr25519_attestation_key(true);

	let mut old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(old_did_details.update_attestation_key(did::DidVerificationKey::from(att_key.public()), 0u64));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	// Trying to remove the currently active attestation key
	details.public_keys_to_remove = DidVerificationKeysToRevoke::<Test>::try_from(
		vec![generate_key_id(&did::DidVerificationKey::from(att_key.public()).into())]
			.iter()
			.copied()
			.collect::<BTreeSet<TestKeyId>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::CurrentlyActiveKey
		);
	});
}

#[test]
fn check_verification_key_not_present_update() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let key_to_delete = get_sr25519_authentication_key(true);

	let old_did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	// Remove both attestation and delegation key
	let mut details = generate_base_did_update_details::<Test>();
	// Trying to remove the currently active authentication key
	details.public_keys_to_remove = DidVerificationKeysToRevoke::<Test>::try_from(
		vec![generate_key_id(
			&did::DidVerificationKey::from(key_to_delete.public()).into(),
		)]
		.iter()
		.copied()
		.collect::<BTreeSet<TestKeyId>>(),
	)
	.expect("Should not fail to create BoundedBTreeSet from a single element");

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(alice_did.clone(), old_did_details)])
		.build(None);

	let new_block_number: TestBlockNumber = 1;

	ext.execute_with(|| {
		System::set_block_number(new_block_number);
		assert_noop!(
			Did::update(Origin::signed(alice_did.clone()), details),
			did::Error::<Test>::VerificationKeyNotPresent
		);
	});
}

// delete

#[test]
fn check_successful_deletion() {
	let auth_key = get_ed25519_authentication_key(true);
	let alice_did = get_did_identifier_from_ed25519_key(auth_key.public());
	let did_details = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

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
		assert_noop!(
			Did::delete(Origin::signed(alice_did)),
			did::Error::<Test>::DidNotPresent
		);
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

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = u64::MAX;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = 1u64;

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let mut call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	// The operation requires the delegation key that is currently not stored for
	// the given DID.
	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
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
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
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

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0));

	let ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_attestation_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
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

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
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

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0));

	let ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_delegation_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
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

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did)])
		.build(None);
	// CType already added to storage
	let mut ext = ctype_mock::ExtBuilder::default()
		.with_ctypes(vec![(get_authentication_key_test_input(), did.clone())])
		.build(Some(ext));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
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
	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityInvocation, did);
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

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details =
		ext.execute_with(|| Did::get_did(&call_operation.operation.did).expect("DID should be present on chain."));
	assert_eq!(
		did_details.get_tx_counter_value(),
		mock_did.get_tx_counter_value() + 1u64
	);
}

#[test]
fn check_attestation_successful_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let attestation_key = get_ed25519_attestation_key(true);

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_attestation_key(did::DidVerificationKey::from(attestation_key.public()), 0));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did);
	let signature = attestation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details =
		ext.execute_with(|| Did::get_did(&call_operation.operation.did).expect("DID should be present on chain."));
	assert_eq!(
		did_details.get_tx_counter_value(),
		mock_did.get_tx_counter_value() + 1u64
	);
}

#[test]
fn check_delegation_successful_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	let delegation_key = get_ecdsa_delegation_key(true);

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	assert_ok!(mock_did.update_delegation_key(did::DidVerificationKey::from(delegation_key.public()), 0));

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = delegation_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_ok!(Did::verify_did_operation_signature_and_increase_nonce(
			&call_operation,
			&did::DidSignature::from(signature)
		));
	});

	// Verify that the DID tx counter has increased
	let did_details =
		ext.execute_with(|| Did::get_did(&call_operation.operation.did).expect("DID should be present on chain."));
	assert_eq!(
		did_details.get_tx_counter_value(),
		mock_did.get_tx_counter_value() + 1u64
	);
}

#[test]
fn check_did_not_present_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());

	let mut ext = ExtBuilder::default().build(None);

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::StorageError(did::StorageError::DidNotPresent)
		);
	});
}

#[test]
fn check_max_tx_counter_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.set_tx_counter(u64::MAX);

	let mut ext = ExtBuilder::default()
		.with_dids(vec![(did.clone(), mock_did.clone())])
		.build(None);

	let mut call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did);
	call_operation.operation.tx_counter = mock_did.last_tx_counter;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::StorageError(did::StorageError::MaxTxCounterValue)
		);
	});
}

#[test]
fn check_smaller_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mut mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));
	mock_did.last_tx_counter = 1;

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did.clone());
	call_operation.operation.tx_counter = 0u64;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_equal_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did.clone());
	call_operation.operation.tx_counter = mock_did.last_tx_counter;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_too_large_counter_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let mut call_operation =
		generate_test_did_call(did::DidVerificationKeyRelationship::CapabilityDelegation, did.clone());
	call_operation.operation.tx_counter = mock_did.last_tx_counter + 2;
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::SignatureError(did::SignatureError::InvalidNonce)
		);
	});
}

#[test]
fn check_verification_key_not_present_operation_verification() {
	let auth_key = get_ed25519_authentication_key(true);
	let did = get_did_identifier_from_ed25519_key(auth_key.public());

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::AssertionMethod, did.clone());
	let signature = auth_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::StorageError(did::StorageError::DidKeyNotPresent(
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

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did.clone());
	let signature = invalid_key.sign(call_operation.encode().as_ref());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::SignatureError(did::SignatureError::InvalidSignatureFormat)
		);
	});
}

#[test]
fn check_invalid_signature_operation_verification() {
	let auth_key = get_sr25519_authentication_key(true);
	let did = get_did_identifier_from_sr25519_key(auth_key.public());
	// Using same key type but different seed (default = false)
	let alternative_key = get_sr25519_authentication_key(false);

	let mock_did = generate_base_did_details::<Test>(did::DidVerificationKey::from(auth_key.public()));

	let call_operation = generate_test_did_call(did::DidVerificationKeyRelationship::Authentication, did.clone());
	let signature = alternative_key.sign(&call_operation.encode());

	let mut ext = ExtBuilder::default().with_dids(vec![(did, mock_did)]).build(None);

	ext.execute_with(|| {
		assert_noop!(
			Did::verify_did_operation_signature_and_increase_nonce(
				&call_operation,
				&did::DidSignature::from(signature)
			),
			DidError::SignatureError(did::SignatureError::InvalidSignature)
		);
	});
}

// Internal function: HttpUrl try_from

#[test]
fn check_http_url() {
	assert_ok!(HttpUrl::<Test>::try_from("http://kilt.io".as_bytes()));

	assert_ok!(HttpUrl::<Test>::try_from("https://kilt.io".as_bytes()));

	assert_ok!(HttpUrl::<Test>::try_from(
		"https://super.long.domain.kilt.io:12345/public/files/test.txt".as_bytes()
	));

	// All other valid ASCII characters
	assert_ok!(HttpUrl::<Test>::try_from("http://:/?#[]@!$&'()*+,;=-._~".as_bytes()));

	assert_eq!(
		HttpUrl::<Test>::try_from("".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	// Non-printable ASCII characters
	assert_eq!(
		HttpUrl::<Test>::try_from("http://kilt.io/\x00".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Some invalid ASCII characters
	assert_eq!(
		HttpUrl::<Test>::try_from("http://kilt.io/<tag>".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Non-ASCII characters
	assert_eq!(
		HttpUrl::<Test>::try_from("http://¶.com".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		HttpUrl::<Test>::try_from("htt://kilt.io".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		HttpUrl::<Test>::try_from("httpss://kilt.io".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

// Internal function: FtpUrl try_from

#[test]
fn check_ftp_url() {
	assert_ok!(FtpUrl::<Test>::try_from("ftp://kilt.io".as_bytes()));

	assert_ok!(FtpUrl::<Test>::try_from("ftps://kilt.io".as_bytes()));

	assert_ok!(FtpUrl::<Test>::try_from(
		"ftps://user@super.long.domain.kilt.io:12345/public/files/test.txt".as_bytes()
	));

	// All other valid ASCII characters
	assert_ok!(FtpUrl::<Test>::try_from("ftps://:/?#[]@%!$&'()*+,;=-._~".as_bytes()));

	assert_eq!(
		FtpUrl::<Test>::try_from("".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	// Non-printable ASCII characters
	assert_eq!(
		HttpUrl::<Test>::try_from("http://kilt.io/\x00".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Some invalid ASCII characters
	assert_eq!(
		FtpUrl::<Test>::try_from("ftp://kilt.io/<tag>".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	// Non-ASCII characters
	assert_eq!(
		FtpUrl::<Test>::try_from("ftps://¶.com".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		FtpUrl::<Test>::try_from("ft://kilt.io".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		HttpUrl::<Test>::try_from("ftpss://kilt.io".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

// Internal function: IpfsUrl try_from

#[test]
fn check_ipfs_url() {
	// Base58 address
	assert_ok!(IpfsUrl::<Test>::try_from(
		"ipfs://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()
	));

	// Base32 address (at the moment, padding characters can appear anywhere in the
	// string)
	assert_ok!(IpfsUrl::<Test>::try_from(
		"ipfs://OQQHHHTGMMYDQQ364YB4GDE=HREJQL==".as_bytes()
	));

	assert_eq!(
		IpfsUrl::<Test>::try_from("".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		IpfsUrl::<Test>::try_from("ipfs://¶QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlEncoding))
	);

	assert_eq!(
		IpfsUrl::<Test>::try_from("ipf://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQkL".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);

	assert_eq!(
		IpfsUrl::<Test>::try_from("ipfss://QmdQ1rHHHTbgbGorfuMMYDQQ36q4sxvYcB4GDEHREuJQk".as_bytes()),
		Err(DidError::UrlError(did::UrlError::InvalidUrlScheme))
	);
}

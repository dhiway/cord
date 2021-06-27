// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mtype

use codec::Encode;
use cord_primitives::AccountId;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_core::{crypto::KeyTypeId, ecdsa, ed25519, sr25519};
use sp_io::crypto::{ecdsa_generate, ecdsa_sign, ed25519_generate, ed25519_sign, sr25519_generate, sr25519_sign};
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use sp_std::{collections::btree_set::BTreeSet, convert::TryInto};

use crate::*;
use did_details::*;

const DEFAULT_ACCOUNT_ID: &str = "tx_submitter";
const DEFAULT_ACCOUNT_SEED: u32 = 0;
const AUTHENTICATION_KEY_ID: KeyTypeId = KeyTypeId(*b"0000");
const MARK_KEY_ID: KeyTypeId = KeyTypeId(*b"0001");
const DELEGATION_KEY_ID: KeyTypeId = KeyTypeId(*b"0002");
const DEFAULT_URL_SCHEME: [u8; 8] = *b"https://";

fn get_ed25519_public_authentication_key() -> ed25519::Public {
	ed25519_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_sr25519_public_authentication_key() -> sr25519::Public {
	sr25519_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_ecdsa_public_authentication_key() -> ecdsa::Public {
	ecdsa_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_key_agreement_keys(n_keys: u32) -> BTreeSet<DidEncryptionKey> {
	(1..=n_keys)
		.map(|i| {
			// Converts the loop index to a 32-byte array;
			let mut seed_vec = i.to_be_bytes().to_vec();
			seed_vec.resize(32, 0u8);
			let seed: [u8; 32] = seed_vec
				.try_into()
				.expect("Failed to create encryption key from raw seed.");
			DidEncryptionKey::X25519(seed)
		})
		.collect::<BTreeSet<DidEncryptionKey>>()
}

fn get_public_keys<T: Config>(n_keys: u32) -> BTreeSet<KeyIdOf<T>> {
	(1..=n_keys)
		.map(|i| {
			// Converts the loop index to a 32-byte array;
			let mut seed_vec = i.to_be_bytes().to_vec();
			seed_vec.resize(32, 0u8);
			let seed: [u8; 32] = seed_vec
				.try_into()
				.expect("Failed to create encryption key from raw seed.");
			let key = DidEncryptionKey::X25519(seed);
			utils::calculate_key_id::<T>(&key.into())
		})
		.collect::<BTreeSet<KeyIdOf<T>>>()
}

fn get_ed25519_public_mark_key() -> ed25519::Public {
	ed25519_generate(MARK_KEY_ID, None)
}

fn get_sr25519_public_mark_key() -> sr25519::Public {
	sr25519_generate(MARK_KEY_ID, None)
}

fn get_ecdsa_public_mark_key() -> ecdsa::Public {
	ecdsa_generate(MARK_KEY_ID, None)
}

fn get_ed25519_public_delegation_key() -> ed25519::Public {
	ed25519_generate(DELEGATION_KEY_ID, None)
}

fn get_sr25519_public_delegation_key() -> sr25519::Public {
	sr25519_generate(DELEGATION_KEY_ID, None)
}

fn get_ecdsa_public_delegation_key() -> ecdsa::Public {
	ecdsa_generate(DELEGATION_KEY_ID, None)
}

// Assumes that the length of the URL is larger than 8 (length of the prefix https://)
fn get_url_endpoint(length: u32) -> Url {
	let total_length = usize::try_from(length).expect("Failed to convert URL max length value to usize value.");
	let mut url_encoded_string = DEFAULT_URL_SCHEME.to_vec();
	url_encoded_string.resize(total_length, b'0');
	Url::Http(
		HttpUrl::try_from(url_encoded_string.as_ref()).expect("Failed to create default URL with provided length."),
	)
}

fn get_did_base_details<T: Config>(auth_key: DidVerificationKey) -> DidDetails<T> {
	DidDetails::new(auth_key, BlockNumberOf::<T>::default())
}

fn generate_base_did_creation_operation<T: Config>(did: DidIdentifierOf<T>) -> DidCreationOperation<T> {
	DidCreationOperation {
		did,
		new_key_agreement_keys: BTreeSet::new(),
		new_mark_key: None,
		new_delegation_key: None,
		new_endpoint_url: None,
	}
}

fn generate_base_did_update_operation<T: Config>(did: DidIdentifierOf<T>) -> DidUpdateOperation<T> {
	DidUpdateOperation {
		did,
		new_authentication_key: None,
		new_key_agreement_keys: BTreeSet::new(),
		mark_key_update: DidVerificationKeyUpdateAction::default(),
		delegation_key_update: DidVerificationKeyUpdateAction::default(),
		new_endpoint_url: None,
		public_keys_to_remove: BTreeSet::new(),
		tx_counter: 1u64,
	}
}

fn generate_base_did_deletion_operation<T: Config>(did: DidIdentifierOf<T>) -> DidDeletionOperation<T> {
	DidDeletionOperation { did, tx_counter: 1u64 }
}

// Must always be dispatched with the DID authentication key
fn generate_base_did_call_operation<T: Config>(did: DidIdentifierOf<T>) -> DidAuthorizedCallOperation<T> {
	let test_call = <T as Config>::Call::get_call_for_did_call_benchmark();

	DidAuthorizedCallOperation {
		did,
		call: test_call,
		tx_counter: 1u64,
	}
}

benchmarks! {
	where_clause { where T::DidIdentifier: From<AccountId>, <T as frame_system::Config>::Origin: From<RawOrigin<T::DidIdentifier>>}

	submit_did_create_operation_ed25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();

		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys(n);
		let did_public_att_key = get_ed25519_public_mark_key();
		let did_public_del_key = get_ed25519_public_delegation_key();
		let did_endpoint = get_url_endpoint(u);

		let mut did_creation_op = generate_base_did_creation_operation::<T>(did_subject.clone());
		did_creation_op.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_op.new_mark_key = Some(DidVerificationKey::from(did_public_att_key));
		did_creation_op.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key));
		did_creation_op.new_endpoint_url = Some(did_endpoint);

		let did_creation_signature = ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");
	}: submit_did_create_operation(RawOrigin::Signed(submitter), did_creation_op.clone(), DidSignature::from(did_creation_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_mark_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());

		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_creation_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_mark_key_id(),
			&Some(expected_mark_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_creation_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	submit_did_create_operation_sr25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();

		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys(n);
		let did_public_att_key = get_sr25519_public_mark_key();
		let did_public_del_key = get_sr25519_public_delegation_key();
		let did_endpoint = get_url_endpoint(u);

		let mut did_creation_op = generate_base_did_creation_operation::<T>(did_subject.clone(), DidVerificationKey::from(did_public_auth_key));
		did_creation_op.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_op.new_mark_key = Some(DidVerificationKey::from(did_public_att_key));
		did_creation_op.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key));
		did_creation_op.new_endpoint_url = Some(did_endpoint);

		let did_creation_signature = sr25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");
	}: submit_did_create_operation(RawOrigin::Signed(submitter), did_creation_op.clone(), DidSignature::from(did_creation_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_mark_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());

		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_creation_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_attestation_key_id(),
			&Some(expected_attestation_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_creation_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	submit_did_create_operation_ecdsa_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();
		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);
		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key.clone()).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys(n);
		let did_public_att_key = get_ecdsa_public_attestation_key();
		let did_public_del_key = get_ecdsa_public_delegation_key();
		let did_endpoint = get_url_endpoint(u);
		let mut did_creation_op = generate_base_did_creation_operation::<T>(did_subject.clone());
		did_creation_op.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_op.new_attestation_key = Some(DidVerificationKey::from(did_public_att_key.clone()));
		did_creation_op.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key.clone()));
		did_creation_op.new_endpoint_url = Some(did_endpoint);
		let did_creation_signature = ecdsa_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_op.encode().as_ref()).expect("Failed to create DID signature from raw ecdsa signature.");
	}: submit_did_create_operation(RawOrigin::Signed(submitter), did_creation_op.clone(), DidSignature::from(did_creation_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();
		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_attestation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());
		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_creation_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_attestation_key_id(),
			&Some(expected_attestation_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_creation_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	submit_did_update_operation_ed25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let m in 1 .. T::MaxVerificationKeysToRevoke::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();

		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		// To cover cases in which m > n without failing, we add m + n keys to the set of keys before the update operation
		let did_key_agreement_keys = get_key_agreement_keys(m + n);

		let mut did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key));
		did_details.add_key_agreement_keys(did_key_agreement_keys, BlockNumberOf::<T>::default());
		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = get_ed25519_public_authentication_key();
		let new_key_agreement_keys = get_key_agreement_keys(n);
		let new_did_public_att_key = get_ed25519_public_mark_key();
		let new_did_public_del_key = get_ed25519_public_delegation_key();
		// Public keys obtained are generated using the same logic as the key agreement keys, so that we are sure they do not generate KeyNotPresent errors
		let public_keys_to_remove = get_public_keys::<T>(m);
		let new_url = get_url_endpoint(u);

		let mut did_update_op = generate_base_did_update_operation::<T>(did_subject.clone());
		did_update_op.new_authentication_key = Some(DidVerificationKey::from(new_did_public_auth_key));
		did_update_op.new_key_agreement_keys = new_key_agreement_keys;
		did_update_op.mark_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_att_key));
		did_update_op.delegation_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_del_key));
		did_update_op.public_keys_to_remove = public_keys_to_remove;
		did_update_op.new_endpoint_url = Some(new_url);

		let did_update_signature = ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_update_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");
	}: submit_did_update_operation(RawOrigin::Signed(submitter), did_update_op.clone(), DidSignature::from(did_update_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_auth_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_del_key).into());
		let expected_mark_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_att_key).into());

		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_update_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_mark_key_id(),
			&Some(expected_mark_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_update_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, did_update_op.tx_counter);
	}

	submit_did_update_operation_sr25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let m in 1 .. T::MaxVerificationKeysToRevoke::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();

		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		// To cover cases in which m > n without failing, we add m + n keys to the set of keys before the update operation
		let did_key_agreement_keys = get_key_agreement_keys(m + n);

		let mut did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key));
		did_details.add_key_agreement_keys(did_key_agreement_keys, BlockNumberOf::<T>::default());
		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = get_sr25519_public_authentication_key();
		let new_key_agreement_keys = get_key_agreement_keys(n);
		let new_did_public_att_key = get_sr25519_public_mark_key();
		let new_did_public_del_key = get_sr25519_public_delegation_key();
		// Public keys obtained are generated using the same logic as the key agreement keys, so that we are sure they do not generate KeyNotPresent errors
		let public_keys_to_remove = get_public_keys::<T>(m);
		let new_url = get_url_endpoint(u);

		let mut did_update_op = generate_base_did_update_operation::<T>(did_subject.clone());
		did_update_op.new_authentication_key = Some(DidVerificationKey::from(new_did_public_auth_key));
		did_update_op.new_key_agreement_keys = new_key_agreement_keys;
		did_update_op.mark_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_att_key));
		did_update_op.delegation_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_del_key));
		did_update_op.public_keys_to_remove = public_keys_to_remove;
		did_update_op.new_endpoint_url = Some(new_url);

		let did_update_signature = sr25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_update_op.encode().as_ref()).expect("Failed to create DID signature from raw sr25519 signature.");
	}: submit_did_update_operation(RawOrigin::Signed(submitter), did_update_op.clone(), DidSignature::from(did_update_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_auth_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_del_key).into());
		let expected_mark_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_att_key).into());

		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_update_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_attestation_key_id(),
			&Some(expected_attestation_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_update_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, did_update_op.tx_counter);
	}
	submit_did_update_operation_ecdsa_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let m in 1 .. T::MaxVerificationKeysToRevoke::get();
		let u in (DEFAULT_URL_SCHEME.len() as u32) .. T::MaxUrlLength::get();

		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key.clone()).into_account().into();
		// To cover cases in which m > n without failing, we add m + n keys to the set of keys before the update operation
		let did_key_agreement_keys = get_key_agreement_keys(m + n);

		let mut did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key.clone()));
		did_details.add_key_agreement_keys(did_key_agreement_keys, BlockNumberOf::<T>::default());
		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = get_ecdsa_public_authentication_key();
		let new_key_agreement_keys = get_key_agreement_keys(n);
		let new_did_public_att_key = get_ecdsa_public_attestation_key();
		let new_did_public_del_key = get_ecdsa_public_delegation_key();
		// Public keys obtained are generated using the same logic as the key agreement keys, so that we are sure they do not generate KeyNotPresent errors
		let public_keys_to_remove = get_public_keys::<T>(m);
		let new_url = get_url_endpoint(u);

		let mut did_update_op = generate_base_did_update_operation::<T>(did_subject.clone());
		did_update_op.new_authentication_key = Some(DidVerificationKey::from(new_did_public_auth_key.clone()));
		did_update_op.new_key_agreement_keys = new_key_agreement_keys;
		did_update_op.attestation_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_att_key.clone()));
		did_update_op.delegation_key_update = DidVerificationKeyUpdateAction::Change(DidVerificationKey::from(new_did_public_del_key.clone()));
		did_update_op.public_keys_to_remove = public_keys_to_remove;
		did_update_op.new_endpoint_url = Some(new_url);

		let did_update_signature = ecdsa_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_update_op.encode().as_ref()).expect("Failed to create DID signature from raw ecdsa signature.");
	}: submit_did_update_operation(RawOrigin::Signed(submitter), did_update_op.clone(), DidSignature::from(did_update_signature))
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.get_key_agreement_keys_ids();

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_auth_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_del_key).into());
		let expected_attestation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(new_did_public_att_key).into());

		assert_eq!(
			stored_did.get_authentication_key_id(),
			expected_authentication_key_id
		);
		for new_key in did_update_op.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.get_delegation_key_id(),
			&Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.get_attestation_key_id(),
			&Some(expected_attestation_key_id)
		);
		assert_eq!(stored_did.endpoint_url, did_update_op.new_endpoint_url);
		assert_eq!(stored_did.last_tx_counter, did_update_op.tx_counter);
	}

	submit_did_delete_operation {
		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_deletion_op = generate_base_did_deletion_operation::<T>(did_subject.clone());

		let did_deletion_signature = ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_deletion_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");
	}: _(RawOrigin::Signed(submitter), did_deletion_op.clone(), DidSignature::from(did_deletion_signature))
	verify {
		assert_eq!(
			Did::<T>::get(&did_subject),
			None
		);
	}

	submit_did_call_ed25519_key {
		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject);

		let did_call_signature = ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");
	}: submit_did_call(RawOrigin::Signed(submitter), Box::new(did_call_op.clone()), DidSignature::from(did_call_signature))

	submit_did_call_sr25519_key {
		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject);

		let did_call_signature = sr25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw sr25519 signature.");
	}: submit_did_call(RawOrigin::Signed(submitter), Box::new(did_call_op.clone()), DidSignature::from(did_call_signature))

	submit_did_call_ecdsa_key {
		let submitter: AccountIdentifierOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key.clone()).into_account().into();

		let did_details = get_did_base_details(DidVerificationKey::from(did_public_auth_key.clone()));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject);

		let did_call_signature = ecdsa_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw ecdsa signature.");
	}: submit_did_call(RawOrigin::Signed(submitter), Box::new(did_call_op.clone()), DidSignature::from(did_call_signature))
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::ExtBuilder::default().build_with_keystore(None),
	crate::mock::Test
}

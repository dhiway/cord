// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
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

use super::*;
use codec::Encode;
use frame_benchmarking::{account, benchmarks};
use frame_support::assert_ok;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_core::{crypto::KeyTypeId, ecdsa, ed25519, sr25519};
use sp_io::crypto::{
	ecdsa_generate, ecdsa_sign, ed25519_generate, ed25519_sign, sr25519_generate, sr25519_sign,
};
use sp_runtime::{traits::IdentifyAccount, AccountId32, MultiSigner};
use sp_std::{convert::TryInto, vec::Vec};

use cord_utilities::signature::VerifySignature;

use crate::{
	did_details::{
		DeriveDidCallAuthorizationVerificationKeyRelationship, DidAuthorizedCallOperation,
		DidPublicKey, DidSignature, DidVerificationKey,
	},
	mock_utils::{
		generate_base_did_creation_details, generate_base_did_details, get_key_agreement_keys,
		get_service_endpoints,
	},
	service_endpoints::DidEndpoint,
	signature::DidSignatureVerify,
	AccountIdOf, DidAuthorizedCallOperationOf, DidIdentifierOf,
};

const DEFAULT_ACCOUNT_ID: &str = "tx_submitter";
const DEFAULT_ACCOUNT_SEED: u32 = 0;
const AUTHENTICATION_KEY_ID: KeyTypeId = KeyTypeId(*b"0000");
const ASSERTION_KEY_ID: KeyTypeId = KeyTypeId(*b"0001");
const DELEGATION_KEY_ID: KeyTypeId = KeyTypeId(*b"0002");
const UNUSED_KEY_ID: KeyTypeId = KeyTypeId(*b"1111");
const MAX_PAYLOAD_BYTE_LENGTH: u32 = 5 * 1024 * 1024;

fn get_ed25519_public_authentication_key() -> ed25519::Public {
	ed25519_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_sr25519_public_authentication_key() -> sr25519::Public {
	sr25519_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_ecdsa_public_authentication_key() -> ecdsa::Public {
	ecdsa_generate(AUTHENTICATION_KEY_ID, None)
}

fn get_ed25519_public_assertion_key() -> ed25519::Public {
	ed25519_generate(ASSERTION_KEY_ID, None)
}

fn get_sr25519_public_assertion_key() -> sr25519::Public {
	sr25519_generate(ASSERTION_KEY_ID, None)
}

fn get_ecdsa_public_assertion_key() -> ecdsa::Public {
	ecdsa_generate(ASSERTION_KEY_ID, None)
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

// Must always be dispatched with the DID authentication key
fn generate_base_did_call_operation<T: Config>(
	did: DidIdentifierOf<T>,
	submitter: AccountIdOf<T>,
) -> DidAuthorizedCallOperationOf<T> {
	let test_call = <T as Config>::RuntimeCall::get_call_for_did_call_benchmark();

	DidAuthorizedCallOperation {
		did,
		call: test_call,
		tx_counter: 1u64,
		block_number: BlockNumberFor::<T>::default(),
		submitter,
	}
}

fn save_service_endpoints<T: Config>(
	did_subject: &DidIdentifierOf<T>,
	endpoints: &[DidEndpoint<T>],
) {
	for endpoint in endpoints.iter() {
		ServiceEndpoints::<T>::insert(did_subject, &endpoint.id, endpoint.clone());
	}
	DidEndpointsCount::<T>::insert(did_subject, endpoints.len().saturated_into::<u32>());
}

benchmarks! {
	where_clause {
		where
		T::DidIdentifier: From<AccountId32>,
		<T as frame_system::Config>::RuntimeOrigin: From<RawOrigin<T::DidIdentifier>>,
		<T as frame_system::Config>::AccountId: From<AccountId32>,
		T::AccountId: AsRef<[u8; 32]> + From<[u8; 32]>,

	}

	/* create extrinsic */
	create_ed25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		// We only calculate weights based on how many endpoints are specified. For each endpoint, we use the max possible length and count for its components.
		// This makes weight computation easier at runtime, at the cost of always having worst-case weights for any # of endpoints c.
		let c in 1 .. T::MaxNumberOfServicesPerDid::get();

		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys::<T>(n);
		let did_public_att_key = get_ed25519_public_assertion_key();
		let did_public_del_key = get_ed25519_public_delegation_key();
		let service_endpoints = get_service_endpoints::<T>(
			c,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);

		let mut did_creation_details = generate_base_did_creation_details::<T>(did_subject.clone(), submitter.clone());
		did_creation_details.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_details.new_assertion_key = Some(DidVerificationKey::from(did_public_att_key));
		did_creation_details.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key));
		did_creation_details.new_service_details = service_endpoints.clone();

		let did_creation_signature = ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_details.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature.");

		let origin = RawOrigin::Signed(submitter);
		let boxed_did_creation_details = Box::new(did_creation_details.clone());
		let did_sig = DidSignature::from(did_creation_signature);
	}: create(origin, boxed_did_creation_details, did_sig)
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.key_agreement_keys;

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_assertion_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());

		assert_eq!(
			stored_did.authentication_key,
			expected_authentication_key_id
		);
		for new_key in did_creation_details.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.delegation_key,
			Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.assertion_key,
			Some(expected_assertion_key_id)
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject).saturated_into::<usize>(),
			service_endpoints.len()
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			service_endpoints.len()
		);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	create_sr25519_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let c in 1 .. T::MaxNumberOfServicesPerDid::get();

		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys::<T>(n);
		let did_public_att_key = get_sr25519_public_assertion_key();
		let did_public_del_key = get_sr25519_public_delegation_key();
		let service_endpoints = get_service_endpoints::<T>(
			c,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);

		let mut did_creation_details = generate_base_did_creation_details::<T>(did_subject.clone(),submitter.clone());
		did_creation_details.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_details.new_assertion_key = Some(DidVerificationKey::from(did_public_att_key));
		did_creation_details.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key));
		did_creation_details.new_service_details = service_endpoints.clone();

		let did_creation_signature = DidSignature::from(sr25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_details.encode().as_ref()).expect("Failed to create DID signature from raw sr25519 signature."));
		let boxed_did_creation_details = Box::new(did_creation_details.clone());
		let origin = RawOrigin::Signed(submitter);
	}: create(origin, boxed_did_creation_details, did_creation_signature)
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.key_agreement_keys;

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_assertion_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());

		assert_eq!(
			stored_did.authentication_key,
			expected_authentication_key_id
		);
		for new_key in did_creation_details.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.delegation_key,
			Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.assertion_key,
			Some(expected_assertion_key_id)
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject).saturated_into::<usize>(),
			service_endpoints.len()
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			service_endpoints.len()
		);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	create_ecdsa_keys {
		let n in 1 .. T::MaxNewKeyAgreementKeys::get();
		let c in 1 .. T::MaxNumberOfServicesPerDid::get();

		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_key_agreement_keys = get_key_agreement_keys::<T>(n);
		let did_public_att_key = get_ecdsa_public_assertion_key();
		let did_public_del_key = get_ecdsa_public_delegation_key();
		let service_endpoints = get_service_endpoints::<T>(
			c,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);

		let mut did_creation_details = generate_base_did_creation_details::<T>(did_subject.clone(),submitter.clone());
		did_creation_details.new_key_agreement_keys = did_key_agreement_keys;
		did_creation_details.new_assertion_key = Some(DidVerificationKey::from(did_public_att_key));
		did_creation_details.new_delegation_key = Some(DidVerificationKey::from(did_public_del_key));
		did_creation_details.new_service_details = service_endpoints.clone();

		let did_creation_signature = DidSignature::from(ecdsa_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_creation_details.encode().as_ref()).expect("Failed to create DID signature from raw ecdsa signature."));
		let boxed_did_creation_details = Box::new(did_creation_details.clone());
		let origin = RawOrigin::Signed(submitter);
	}: create(origin, boxed_did_creation_details, did_creation_signature)
	verify {
		let stored_did = Did::<T>::get(&did_subject).expect("New DID should be stored on chain.");
		let stored_key_agreement_keys_ids = stored_did.key_agreement_keys;

		let expected_authentication_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_auth_key).into());
		let expected_assertion_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_att_key).into());
		let expected_delegation_key_id = utils::calculate_key_id::<T>(&DidVerificationKey::from(did_public_del_key).into());

		assert_eq!(
			stored_did.authentication_key,
			expected_authentication_key_id
		);
		for new_key in did_creation_details.new_key_agreement_keys.iter().copied() {
			assert!(
				stored_key_agreement_keys_ids.contains(&utils::calculate_key_id::<T>(&new_key.into())))
		}
		assert_eq!(
			stored_did.delegation_key,
			Some(expected_delegation_key_id)
		);
		assert_eq!(
			stored_did.assertion_key,
			Some(expected_assertion_key_id)
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject).saturated_into::<usize>(),
			service_endpoints.len()
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			service_endpoints.len()
		);
		assert_eq!(stored_did.last_tx_counter, 0u64);
	}

	delete {
		let c in 1 .. T::MaxNumberOfServicesPerDid::get();

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(did_public_auth_key));
		let service_endpoints = get_service_endpoints::<T>(
			c,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);

		Did::<T>::insert(&did_subject, did_details);
		save_service_endpoints(&did_subject, &service_endpoints);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: _(origin, c)
	verify {
		assert!(
			Did::<T>::get(&did_subject).is_none()
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject), 0
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			0
		);
	}

	/* submit_did_call extrinsic */
	submit_did_call_ed25519_key {
		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject, submitter.clone());

		let did_call_signature = DidSignature::from(ed25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw ed25519 signature."));
		let origin = RawOrigin::Signed(submitter);
		let boxed_did_call = Box::new(did_call_op);
	}: submit_did_call(origin, boxed_did_call, did_call_signature)

	submit_did_call_sr25519_key {
		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject, submitter.clone());

		let did_call_signature = DidSignature::from(sr25519_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw sr25519 signature."));
		let origin = RawOrigin::Signed(submitter);
		let boxed_did_call = Box::new(did_call_op);
	}: submit_did_call(origin, boxed_did_call, did_call_signature)

	submit_did_call_ecdsa_key {
		let submitter: AccountIdOf<T> = account(DEFAULT_ACCOUNT_ID, 0, DEFAULT_ACCOUNT_SEED);

		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(did_public_auth_key));
		Did::<T>::insert(&did_subject, did_details);

		let did_call_op = generate_base_did_call_operation::<T>(did_subject, submitter.clone());

		let did_call_signature = DidSignature::from(ecdsa_sign(AUTHENTICATION_KEY_ID, &did_public_auth_key, did_call_op.encode().as_ref()).expect("Failed to create DID signature from raw ecdsa signature."));
		let origin = RawOrigin::Signed(submitter);
		let boxed_did_call = Box::new(did_call_op);
	}: submit_did_call(origin, boxed_did_call, did_call_signature)

	/* set_authentication_key extrinsic */
	set_ed25519_authentication_key {
		let block_number = BlockNumberFor::<T>::zero();
		let old_did_public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(old_did_public_auth_key).into_account().into();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(old_did_public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ed25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ed25519_public_delegation_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = DidVerificationKey::from(ed25519_generate(UNUSED_KEY_ID, None));
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_did_public_auth_key = new_did_public_auth_key.clone();
	}: set_authentication_key(origin, cloned_new_did_public_auth_key)
	verify {
		let auth_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_did_public_auth_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().authentication_key, auth_key_id);
	}

	set_sr25519_authentication_key {
		let block_number = BlockNumberFor::<T>::zero();
		let old_did_public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(old_did_public_auth_key).into_account().into();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(old_did_public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_sr25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_sr25519_public_delegation_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = DidVerificationKey::from(sr25519_generate(UNUSED_KEY_ID, None));
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_did_public_auth_key = new_did_public_auth_key.clone();
	}: set_authentication_key(origin, cloned_new_did_public_auth_key)
	verify {
		let auth_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_did_public_auth_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().authentication_key, auth_key_id);
	}

	set_ecdsa_authentication_key {
		let block_number = BlockNumberFor::<T>::zero();
		let old_did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(old_did_public_auth_key).into_account().into();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(old_did_public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);

		let new_did_public_auth_key = DidVerificationKey::from(ecdsa_generate(UNUSED_KEY_ID, None));
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_did_public_auth_key = new_did_public_auth_key.clone();
	}: set_authentication_key(origin, cloned_new_did_public_auth_key)
	verify {
		let auth_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_did_public_auth_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().authentication_key, auth_key_id);
	}

	/* set_delegation_key extrinsic */
	set_ed25519_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_ed25519_public_delegation_key();
		let new_delegation_key = DidVerificationKey::from(ed25519_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ed25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_delegation_key = new_delegation_key.clone();
	}: set_delegation_key(origin, cloned_new_delegation_key)
	verify {
		let new_delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_delegation_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().delegation_key, Some(new_delegation_key_id));
	}

	set_sr25519_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_sr25519_public_delegation_key();
		let new_delegation_key = DidVerificationKey::from(sr25519_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_sr25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_delegation_key = new_delegation_key.clone();
	}: set_delegation_key(origin, cloned_new_delegation_key)
	verify {
		let new_delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_delegation_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().delegation_key, Some(new_delegation_key_id));
	}

	set_ecdsa_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_ecdsa_public_delegation_key();
		let new_delegation_key = DidVerificationKey::from(ecdsa_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_delegation_key = new_delegation_key.clone();
	}: set_delegation_key(origin, cloned_new_delegation_key)
		verify {
		let new_delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_delegation_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().delegation_key, Some(new_delegation_key_id));
	}

	/* remove_delegation_key extrinsic */
	remove_ed25519_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_ed25519_public_delegation_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ed25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_delegation_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_delegation_key)));
		assert!(did_details.delegation_key.is_none());
		assert!(!did_details.public_keys.contains_key(&delegation_key_id));
	}

	remove_sr25519_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_sr25519_public_delegation_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_sr25519_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_delegation_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_delegation_key)));
		assert!(did_details.delegation_key.is_none());
		assert!(!did_details.public_keys.contains_key(&delegation_key_id));
	}

	remove_ecdsa_delegation_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_delegation_key = get_ecdsa_public_delegation_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(old_delegation_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_delegation_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let delegation_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_delegation_key)));
		assert!(did_details.delegation_key.is_none());
		assert!(!did_details.public_keys.contains_key(&delegation_key_id));
	}

	/* set_assertion_key extrinsic */
	set_ed25519_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_ed25519_public_assertion_key();
		let new_assertion_key = DidVerificationKey::from(ed25519_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ed25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_assertion_key = new_assertion_key.clone();
	}: set_assertion_key(origin, cloned_new_assertion_key)
	verify {
		let new_assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_assertion_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().assertion_key, Some(new_assertion_key_id));
	}

	set_sr25519_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_sr25519_public_assertion_key();
		let new_assertion_key = DidVerificationKey::from(sr25519_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_sr25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_assertion_key = new_assertion_key.clone();
	}: set_assertion_key(origin, cloned_new_assertion_key)
	verify {
		let new_assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_assertion_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().assertion_key, Some(new_assertion_key_id));
	}

	set_ecdsa_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_ecdsa_public_assertion_key();
		let new_assertion_key = DidVerificationKey::from(ecdsa_generate(UNUSED_KEY_ID, None));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_new_assertion_key = new_assertion_key.clone();
	}: set_assertion_key(origin, cloned_new_assertion_key)
	verify {
		let new_assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_assertion_key));
		assert_eq!(Did::<T>::get(&did_subject).unwrap().assertion_key, Some(new_assertion_key_id));
	}

	/* remove_assertion_key extrinsic */
	remove_ed25519_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_ed25519_public_assertion_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ed25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_assertion_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_assertion_key)));
		assert!(did_details.assertion_key.is_none());
		assert!(!did_details.public_keys.contains_key(&assertion_key_id));
	}

	remove_sr25519_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_sr25519_public_assertion_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_sr25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_assertion_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_assertion_key)));
		assert!(did_details.assertion_key.is_none());
		assert!(!did_details.public_keys.contains_key(&assertion_key_id));
	}

	remove_ecdsa_assertion_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let old_assertion_key = get_ecdsa_public_assertion_key();

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get()), block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(old_assertion_key), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_assertion_key(origin)
	verify {
		let did_details = Did::<T>::get(&did_subject).unwrap();
		let assertion_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(DidVerificationKey::from(old_assertion_key)));
		assert!(did_details.assertion_key.is_none());
		assert!(!did_details.public_keys.contains_key(&assertion_key_id));
	}

	/* add_key_agreement_keys extrinsic */
	add_ed25519_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let mut key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// remove first entry
		let new_key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		assert!(key_agreement_keys.remove(&new_key_agreement_key));

		// fill up public keys to its max minus one size (due to removal of new key_agreement_key)
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ed25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ed25519_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: add_key_agreement_key(origin, new_key_agreement_key)
	verify {
		let new_key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_key_agreement_key));
		assert!(Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&new_key_agreement_key_id));
	}

	add_sr25519_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let mut key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// remove first entry
		let new_key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		assert!(key_agreement_keys.remove(&new_key_agreement_key));

		// fill up public keys to its max minus one size (due to removal of new key_agreement_key)
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_sr25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_sr25519_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: add_key_agreement_key(origin, new_key_agreement_key)
	verify {
		let new_key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_key_agreement_key));
		assert!(Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&new_key_agreement_key_id));
	}

	add_ecdsa_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let mut key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// remove first entry
		let new_key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		assert!(key_agreement_keys.remove(&new_key_agreement_key));

		// fill up public keys to its max minus one size (due to removal of new key_agreement_key)
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: add_key_agreement_key(origin, new_key_agreement_key)
	verify {
		let new_key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(new_key_agreement_key));
		assert!(Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&new_key_agreement_key_id));
	}

	/* remove_key_agreement_keys extrinsic */
	remove_ed25519_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ed25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ed25519_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_key_agreement_key(origin, key_agreement_key_id)
	verify {
		assert!(!Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&key_agreement_key_id));
	}

	remove_sr25519_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_sr25519_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_sr25519_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_key_agreement_key(origin, key_agreement_key_id)
	verify {
		assert!(!Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&key_agreement_key_id));
	}

	remove_ecdsa_key_agreement_key {
		let block_number = BlockNumberFor::<T>::zero();
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: remove_key_agreement_key(origin, key_agreement_key_id)
	verify {
		assert!(!Did::<T>::get(&did_subject).unwrap().key_agreement_keys.contains(&key_agreement_key_id));
	}

	add_service_endpoint {
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		// Max allowed - 1.
		let old_service_endpoints = get_service_endpoints::<T>(
			T::MaxNumberOfServicesPerDid::get() - 1,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);
		// New endpoint with max length and count for all the properties.
		let mut new_service_endpoint = get_service_endpoints::<T>(
			1,
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		)[0].clone();
		// Changing from the default ID otherwise it would be the same as the one first one in `old_service_endpoints`.
		new_service_endpoint.id = b"new_id".to_vec().try_into().unwrap();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		Did::<T>::insert(&did_subject, did_details);
		save_service_endpoints(&did_subject, &old_service_endpoints);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_service_endpoint = new_service_endpoint.clone();
	}: _(origin, cloned_service_endpoint)
	verify {
		assert_eq!(
			ServiceEndpoints::<T>::get(&did_subject, &new_service_endpoint.id),
			Some(new_service_endpoint)
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject),
			T::MaxNumberOfServicesPerDid::get()
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			T::MaxNumberOfServicesPerDid::get().saturated_into::<usize>()
		);
	}

	remove_service_endpoint {
		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		// All set to max.
		let old_service_endpoints = get_service_endpoints::<T>(
			T::MaxNumberOfServicesPerDid::get(),
			T::MaxServiceIdLength::get(),
			T::MaxNumberOfTypesPerService::get(),
			T::MaxServiceTypeLength::get(),
			T::MaxNumberOfUrlsPerService::get(),
			T::MaxServiceUrlLength::get(),
		);
		let endpoint_id = old_service_endpoints[0].id.clone();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		Did::<T>::insert(&did_subject, did_details);
		save_service_endpoints(&did_subject, &old_service_endpoints);
		let origin = RawOrigin::Signed(did_subject.clone());
		let cloned_endpoint_id = endpoint_id.clone();
	}: _(origin, cloned_endpoint_id)
		verify {
		assert!(
			ServiceEndpoints::<T>::get(&did_subject, &endpoint_id).is_none()
		);
		assert_eq!(
			DidEndpointsCount::<T>::get(&did_subject),
			T::MaxNumberOfServicesPerDid::get() - 1
		);
		assert_eq!(
			ServiceEndpoints::<T>::iter_prefix(&did_subject).count(),
			T::MaxNumberOfServicesPerDid::get().saturated_into::<usize>() - 1
		);
	}

	signature_verification_sr25519 {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let payload: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let block_number = BlockNumberFor::<T>::zero();

		let public_auth_key = get_sr25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let signature = sr25519_sign(AUTHENTICATION_KEY_ID, &public_auth_key, &payload).expect("Failed to create DID signature from raw sr25519 signature.");
		let did_signature = DidSignature::Sr25519(signature);
	}: {
		DidSignatureVerify::<T>::verify(&did_subject, &payload, &did_signature).expect("should verify");
	}
	signature_verification_ed25519 {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let payload: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let block_number = BlockNumberFor::<T>::zero();

		let public_auth_key = get_ed25519_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let signature = ed25519_sign(AUTHENTICATION_KEY_ID, &public_auth_key, &payload).expect("Failed to create DID signature from raw ed25519 signature.");
		let did_signature = DidSignature::Ed25519(signature);
	}: {
		DidSignatureVerify::<T>::verify(&did_subject, &payload, &did_signature).expect("should verify");
	}
	signature_verification_ecdsa {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let payload: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let block_number = BlockNumberFor::<T>::zero();

		let public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(public_auth_key).into_account().into();
		let key_agreement_keys = get_key_agreement_keys::<T>(T::MaxNewKeyAgreementKeys::get());

		// get first entry
		let key_agreement_key = *key_agreement_keys.clone().into_inner().iter().next().unwrap();
		let key_agreement_key_id = utils::calculate_key_id::<T>(&DidPublicKey::from(key_agreement_key));

		// fill up public keys to its max size because max public keys = # of max key agreement keys + 3
		let mut did_details = generate_base_did_details::<T>(DidVerificationKey::from(public_auth_key));
		assert_ok!(did_details.add_key_agreement_keys(key_agreement_keys, block_number));
		assert_ok!(did_details.update_delegation_key(DidVerificationKey::from(get_ecdsa_public_delegation_key()), block_number));
		assert_ok!(did_details.update_assertion_key(DidVerificationKey::from(get_ecdsa_public_assertion_key()), block_number));

		Did::<T>::insert(&did_subject, did_details);
		let signature = ecdsa_sign(AUTHENTICATION_KEY_ID, &public_auth_key, &payload).expect("Failed to create DID signature from raw ecdsa signature.");
		let did_signature = DidSignature::Ecdsa(signature);
	}: {
		DidSignatureVerify::<T>::verify(&did_subject, &payload, &did_signature).expect("should verify");
	}
	dispatch_as {
		// ecdsa keys are the most expensive since they require an additional hashing step
		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_account: AccountIdOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let did_details = generate_base_did_details::<T>(DidVerificationKey::from(did_public_auth_key));

		Did::<T>::insert(&did_subject, did_details);

		let test_call = <T as Config>::RuntimeCall::get_call_for_did_call_benchmark();
		let origin = RawOrigin::Signed(did_subject.clone());
	}: _(origin, did_subject, Box::new(test_call))

	create_from_account {
		// ecdsa keys are the most expensive since they require an additional hashing step
		let did_public_auth_key = get_ecdsa_public_authentication_key();
		let did_subject: DidIdentifierOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();
		let did_account: AccountIdOf<T> = MultiSigner::from(did_public_auth_key).into_account().into();

		let authentication_key = DidVerificationKey::from(did_public_auth_key);
		let origin = RawOrigin::Signed(did_subject.clone());
	}: _(origin, authentication_key)
	verify {
			Did::<T>::get(&did_subject).expect("DID entry should be created");
	}
	impl_benchmark_test_suite! (
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	)
}

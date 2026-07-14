// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Focused P6 enterprise journey proof.
//!
//! These tests deliberately exercise the real Orbis runtime and native pallets in-process. They
//! do not claim to prove provider-node byte retrieval, automatic failover, a live relay/parachain
//! topology, or production SLOs. Those boundaries are recorded in the accompanying P6 manifest.

use super::*;
use bulletin_transaction_storage_primitives::{BulletinRef, StorageActor};
use codec::{DecodeAll, Encode};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::GetDispatchInfo,
	traits::{fungible::Mutate, BuildGenesisConfig, Hooks},
};
use pallet_orbis_dotns_runtime_api::runtime_decl_for_dotns_api::DotnsApiV1;
use sp_core::{ed25519, Pair};
use sp_runtime::{
	traits::{BlakeTwo256, Hash as HashT, IdentifyAccount, TransactionExtension},
	MultiSigner,
};

#[test]
fn enterprise_identity_attestation_name_and_storage_lifecycle_is_native_and_fail_closed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		System::set_extrinsic_index(0);

		let owner = pallet_revive::test_utils::ALICE;
		let registrar = AccountId::from([3u8; 32]);
		let unavailable_provider = AccountId::from([7u8; 32]);
		let selected_provider = AccountId::from([8u8; 32]);
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);

		// Self-claimed identity is followed by an independent, native registrar judgement.
		let mut identity = pallet_orbis_people::identity_info::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		identity.display =
			pallet_orbis_people::Data::Raw(b"Enterprise Alice".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(identity.clone()),
		));
		assert_ok!(People::add_registrar(RuntimeOrigin::root(), registrar.clone().into()));
		assert_ok!(People::provide_judgement(
			RuntimeOrigin::signed(registrar.clone()),
			owner.clone().into(),
			pallet_orbis_people::Judgement::Reasonable,
			BlakeTwo256::hash_of(&identity),
		));
		assert!(People::has_identity(&owner, 1));
		assert!(System::events().iter().any(|record| matches!(
			&record.event,
			crate::RuntimeEvent::People(pallet_orbis_people::Event::JudgementGiven {
				target,
				registrar: event_registrar,
			}) if target == &owner && event_registrar == &registrar
		)));

		// Entity remains the authoritative SubjectId source for the native attestation and DotNS.
		let mut entity_info = pallet_orbis_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		entity_info.display =
			origin_primitives::Element::Raw(b"Enterprise Alice".to_vec().try_into().unwrap());
		assert_ok!(Entity::set_info(RuntimeOrigin::signed(owner.clone()), Box::new(entity_info),));
		let subject_id = pallet_orbis_entity::EntityTokenOfAccount::<Runtime>::get(&owner)
			.expect("Entity creates the canonical subject");
		let subject_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(subject_id.as_ref()));

		// Bulletin stores the real call payload, indexes its content commitment, and records who
		// submitted it. Runtime state proves the commitment/provenance, not provider byte serving.
		let content = b"festival-enterprise-admission-v1".to_vec();
		let content_hash = sp_io::hashing::blake2_256(&content);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner.clone(),
			4,
			4096,
		));
		let store_call =
			pallet_bulletin_transaction_storage::Call::<Runtime>::store { data: content.clone() };
		let (_, scope) = TransactionStorage::validate_signed(&owner, &store_call).unwrap();
		let scope = scope.expect("signed storage carries its validated authorization scope");
		assert_ok!(TransactionStorage::pre_dispatch_signed(&owner, &store_call));
		let authorized = pallet_bulletin_transaction_storage::Origin::<Runtime>::Authorized {
			who: owner.clone(),
			scope,
		};
		assert_ok!(TransactionStorage::store(RuntimeOrigin::from(authorized), content));
		assert!(TransactionStorage::contains_transaction(content_hash));
		assert_eq!(
			TransactionStorage::stored_content_provenance(BulletinRef {
				block: 1,
				transaction_index: 0,
			}),
			Some(StorageActor::Account(owner.clone())),
		);
		<TransactionStorage as Hooks<u32>>::on_finalize(1);
		assert_eq!(TransactionStorage::transactions_at(1).unwrap()[0].content_hash, content_hash);

		let definition: pallet_orbis_attestation::SchemaDefinitionOf<Runtime> =
			b"enterprise-admission-v1".to_vec().try_into().unwrap();
		let definition_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(definition.as_slice()));
		let issuers: pallet_orbis_attestation::AuthorizedIssuersOf<Runtime> =
			vec![owner.clone()].try_into().unwrap();
		assert_ok!(Attestation::create_schema(
			RuntimeOrigin::signed(owner.clone()),
			definition,
			issuers,
			true,
			true,
			pallet_orbis_attestation::IndexPolicy::IssuerAndSubjectSchema,
		));
		let schema = Attestation::schema_id(
			&owner,
			&definition_commitment,
			true,
			true,
			pallet_orbis_attestation::IndexPolicy::IssuerAndSubjectSchema,
		);
		let input = pallet_orbis_attestation::AttestationInput::<Runtime> {
			schema,
			subject_commitment,
			payload_commitment: sp_core::H256::from(content_hash),
			status_commitment: sp_core::H256::from_low_u64_be(1),
			parent: None,
			expiry: Some(100),
			uniqueness_commitment: Some(sp_core::H256::from_low_u64_be(1)),
			revocable: true,
		};
		let attestation = Attestation::attestation_id(&owner, &input, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), input));
		assert!(Attestation::is_live(attestation));

		let label = Dotns::validate_label(b"enterprise-alice".to_vec()).unwrap();
		let salt: pallet_orbis_dotns::SaltOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		let commitment = Dotns::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(3);
		System::set_extrinsic_index(1);
		assert_ok!(Dotns::register(
			RuntimeOrigin::signed(owner.clone()),
			None,
			label.clone(),
			salt,
		));
		let name = Dotns::derive_name_id(None, &label);
		assert_ok!(Dotns::set_subject(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(subject_id),
		));
		assert_ok!(Dotns::set_attestation(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(attestation),
		));
		assert_ok!(Dotns::set_content(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(content_hash),
		));
		assert_eq!(Runtime::resolve_attestation(name).value, Some(attestation));

		// The first provider is unavailable. Runtime admission fails closed and the owner can
		// explicitly select a second registered provider; no automatic failover is claimed.
		let endpoint_a: pallet_orbis_storage_provider::EndpointOf<Runtime> =
			b"https://unavailable.enterprise.invalid".to_vec().try_into().unwrap();
		let endpoint_b: pallet_orbis_storage_provider::EndpointOf<Runtime> =
			b"https://selected.enterprise.invalid".to_vec().try_into().unwrap();
		let key_a: pallet_orbis_storage_provider::ServiceKeyOf<Runtime> =
			b"provider-a-key".to_vec().try_into().unwrap();
		let key_b: pallet_orbis_storage_provider::ServiceKeyOf<Runtime> =
			b"provider-b-key".to_vec().try_into().unwrap();
		assert_ok!(crate::StorageProvider::register_provider(
			RuntimeOrigin::root(),
			unavailable_provider.clone(),
			endpoint_a,
			key_a,
			1_000_000,
		));
		assert_ok!(crate::StorageProvider::register_provider(
			RuntimeOrigin::root(),
			selected_provider.clone(),
			endpoint_b,
			key_b,
			1_000_000,
		));
		assert_ok!(crate::StorageProvider::set_provider_status(
			RuntimeOrigin::root(),
			unavailable_provider.clone(),
			pallet_orbis_storage_provider::ProviderStatus::Suspended,
		));
		let container_ref = sp_core::H256::from_low_u64_be(44);
		let content_commitment = sp_core::H256::from(content_hash);
		assert_noop!(
			crate::StorageProvider::propose_agreement(
				RuntimeOrigin::signed(owner.clone()),
				unavailable_provider,
				container_ref,
				content_commitment,
				None,
				content_hash.len() as u64,
				50,
			),
			pallet_orbis_storage_provider::Error::<Runtime>::ProviderNotActive
		);
		assert_ok!(crate::StorageProvider::propose_agreement(
			RuntimeOrigin::signed(owner.clone()),
			selected_provider.clone(),
			container_ref,
			content_commitment,
			None,
			content_hash.len() as u64,
			50,
		));
		let agreement = pallet_orbis_storage_provider::OwnerAgreements::<Runtime>::get(&owner)[0];
		assert_ok!(crate::StorageProvider::accept_agreement(
			RuntimeOrigin::signed(selected_provider.clone()),
			agreement,
		));
		assert_ok!(crate::StorageProvider::request_renewal(
			RuntimeOrigin::signed(owner.clone()),
			agreement,
			80,
		));
		assert_ok!(crate::StorageProvider::accept_renewal(
			RuntimeOrigin::signed(selected_provider.clone()),
			agreement,
		));
		let agreement_record =
			pallet_orbis_storage_provider::Agreements::<Runtime>::get(agreement).unwrap();
		assert_eq!(agreement_record.status, pallet_orbis_storage_provider::AgreementStatus::Active);
		assert_eq!(agreement_record.expires_at, 80);

		// A provider commitment/checkpoint proves the runtime challenge path against the selected
		// agreement. It is not a substitute for retrieving bytes from a live provider node.
		assert_ok!(crate::StorageProvider::commit_provider_root(
			RuntimeOrigin::signed(selected_provider.clone()),
			1,
			vec![content_commitment].try_into().unwrap(),
		));
		let root = pallet_orbis_storage_provider::ProviderRoots::<Runtime>::get(&selected_provider)
			.unwrap()
			.root;
		assert_ok!(crate::StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			agreement,
			root,
			20,
		));
		let (challenge_id, challenge) =
			pallet_orbis_storage_provider::Challenges::<Runtime>::iter().next().unwrap();
		let proof = crate::StorageProvider::expected_challenge_proof(challenge_id, &challenge)
			.expect("the active provider root yields the expected proof commitment");
		assert_ok!(crate::StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(selected_provider),
			challenge_id,
			proof,
		));
		assert_eq!(
			pallet_orbis_storage_provider::ProviderCheckpoint::<Runtime>::get(
				agreement_record.provider
			)
			.unwrap()
			.proof_commitment,
			proof,
		);

		let drive_name: pallet_orbis_drive::DriveNameOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		assert_ok!(Drive::create_drive(
			RuntimeOrigin::signed(owner.clone()),
			drive_name,
			Some(content_hash),
		));
		let bucket_name: pallet_orbis_s3::BucketNameOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		let bucket = S3::bucket_id(&owner, &bucket_name);
		assert_ok!(S3::create_bucket(RuntimeOrigin::signed(owner.clone()), bucket_name));
		let key: pallet_orbis_s3::ObjectKeyOf<Runtime> =
			b"admission/alice".to_vec().try_into().unwrap();
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(owner.clone()),
			bucket,
			key.clone(),
			content_hash,
			None,
		));
		let resolved_object = pallet_orbis_s3::Objects::<Runtime>::get(bucket, key).unwrap();
		assert_eq!(resolved_object.content_hash, Some(content_hash));
		assert!(!resolved_object.deleted);

		// Revocation is authoritative: DotNS resolution no longer returns a revoked credential.
		assert_ok!(Attestation::revoke(RuntimeOrigin::signed(owner), attestation));
		assert!(!Attestation::is_live(attestation));
		assert_eq!(Runtime::resolve_attestation(name).value, None);
	});
}

#[test]
fn enterprise_sponsored_meta_boundaries_reject_exhaustion_and_version_drift() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);

		// This checked-in fixture is produced by the runtime fixture generator from the actual
		// UncheckedExtrinsic and MetaTxExtension types. With an unfunded sponsor the ordinary paid
		// outer envelope must be rejected before it can subsidize the inner actor.
		let bytes = include_bytes!("../fixtures/meta-v8/sponsored-outer-extrinsic.scale");
		let extrinsic = crate::UncheckedExtrinsic::decode_all(&mut bytes.as_slice()).unwrap();
		let outer = extrinsic.0.function;
		assert!(matches!(outer, RuntimeCall::MetaTx(..)));
		let sponsor_pair = ed25519::Pair::from_seed(&[0x45; 32]);
		let sponsor = MultiSigner::Ed25519(sponsor_pair.public()).into_account();
		assert_eq!(Balances::free_balance(&sponsor), 0);
		let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into();
		let implicit = payment.implicit().unwrap();
		let exhaustion = payment.validate(
			RuntimeOrigin::signed(sponsor),
			&outer,
			&outer.get_dispatch_info(),
			outer.encoded_size(),
			implicit,
			&sp_runtime::traits::TxBaseImplication((0u8, &outer)),
			sp_runtime::transaction_validity::TransactionSource::External,
		);
		assert!(matches!(
			exhaustion,
			Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
				sp_runtime::transaction_validity::InvalidTransaction::Payment,
			))
		));

		// The mutated fixture changes only the signed spec-version intent field. Inspection of the
		// real runtime Meta envelope fails closed rather than silently accepting version drift.
		let drifted = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
			&mut include_bytes!("../fixtures/meta-v8/mutate-spec.scale").as_slice(),
		)
		.unwrap();
		let drifted_len = drifted.encoded_size() as u32;
		let drifted_outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(drifted),
			meta_tx_encoded_len: drifted_len,
		});
		assert!(crate::meta_v6::inspect_paid_meta::<
			crate::meta_v6::ProductionMetadataImplicitResolver,
		>(&drifted_outer, 0)
		.is_err());
	});
}

// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Focused P6 enterprise journey proof.
//!
//! These tests deliberately exercise the real Orbis runtime and native pallets in-process. They
//! do not claim to prove provider-node byte retrieval, automatic failover, a live relay/parachain
//! topology, or production SLOs. Those boundaries are recorded in the accompanying P6 manifest.

use super::*;
use orbis_transaction_storage_primitives::{StorageRef, StorageActor};
use codec::{DecodeAll, Encode};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::GetDispatchInfo,
	traits::{fungible::Mutate, BuildGenesisConfig, Hooks, SignedTransactionBuilder},
};
use pallet_orbis_names_runtime_api::runtime_decl_for_names_api::NamesApiV1;
use sp_core::{ed25519, sr25519, Pair};
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{BlakeTwo256, Hash as HashT, IdentifyAccount, TransactionExtension},
	MultiSignature, MultiSigner,
};

fn apply_sponsored_call(
	call: RuntimeCall,
	participant_pair: &sr25519::Pair,
	sponsor_pair: &sr25519::Pair,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
	type MetaBareExtension = (
		crate::meta_v6::ConsumePaidMetaIngress,
		pallet_meta_tx::MetaTxMarker<Runtime>,
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckMortality<Runtime>,
		frame_system::CheckNonce<Runtime>,
		crate::MetaIdentityBoundPolicies,
		pallet_orbis_transaction_storage::extension::ValidateStorageCalls<
			Runtime,
			crate::OrbisStorageCallInspector,
		>,
		frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	);

	let participant = MultiSigner::Sr25519(participant_pair.public()).into_account();
	let sponsor = MultiSigner::Sr25519(sponsor_pair.public()).into_account();
	let finalized_block = System::block_number().saturating_sub(1);
	let era = Era::mortal(4, finalized_block.into());
	let mortality = frame_system::CheckMortality::<Runtime>::from(era);
	let nonce = frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&participant));
	let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(Default::default());
	let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
		Runtime,
		crate::OrbisStorageCallInspector,
	>::default();
	let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
	let preimage = crate::meta_v6::IntentPreimageV7 {
		domain: crate::meta_v6::META_DOMAIN.to_vec(),
		extension_version: 0,
		genesis_hash: System::block_hash(0),
		spec_version: crate::VERSION.spec_version,
		transaction_version: crate::VERSION.transaction_version,
		inner_signer: participant.clone(),
		call_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&call.encode())),
		mortality: era,
		nonce: System::account_nonce(&participant),
		policy_proofs_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&policy.0.encode())),
		storage_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&storage.encode())),
		metadata_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(
			&metadata.encode(),
		)),
		metadata_implicit: None,
	};
	let bare: MetaBareExtension = (
		crate::meta_v6::ConsumePaidMetaIngress(preimage),
		pallet_meta_tx::MetaTxMarker::new(),
		frame_system::CheckNonZeroSender::new(),
		frame_system::CheckSpecVersion::new(),
		frame_system::CheckTxVersion::new(),
		frame_system::CheckGenesis::new(),
		mortality,
		nonce,
		(
			pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
			policy,
			pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
		),
		storage,
		metadata,
	);
	let implicit = bare.implicit().expect("test externalities provide MetaTx implicit data");
	let inner_signature = (0u8, call.clone(), bare.clone(), implicit)
		.using_encoded(|payload| participant_pair.sign(&sp_io::hashing::blake2_256(payload)));
	let verify = pallet_verify_signature::VerifySignature::new_with_signature(
		MultiSignature::Sr25519(inner_signature),
		participant.clone(),
	);
	let (
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = bare;
	let meta = pallet_meta_tx::MetaTxFor::<Runtime>::new(
		call,
		0,
		(
			verify,
			consume,
			marker,
			nonzero,
			spec,
			tx,
			genesis,
			mortality,
			nonce,
			identity_policies,
			storage,
			metadata,
		),
	);
	let meta_len = meta.encoded_size() as u32;
	let outer_call = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
		meta_tx: Box::new(meta),
		meta_tx_encoded_len: meta_len,
	});
	let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
		pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
	)
	.into();
	let outer_extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
		System::account_nonce(&sponsor),
		payment,
		Default::default(),
	));
	let payload = SignedPayload::new(outer_call.clone(), outer_extension.clone()).unwrap();
	let outer_signature = payload.using_encoded(|bytes| sponsor_pair.sign(bytes));
	let extrinsic = <crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
		outer_call,
		sponsor.into(),
		MultiSignature::Sr25519(outer_signature),
		outer_extension,
	);
	assert_ok!(crate::Executive::validate_transaction(
		sp_runtime::transaction_validity::TransactionSource::External,
		extrinsic.clone(),
		System::block_hash(0),
	));
	crate::Executive::apply_extrinsic(extrinsic)
		.expect("the signed outer MetaTx is valid")
		.expect("the outer MetaTx dispatch succeeds");
	System::events()
		.into_iter()
		.rev()
		.find_map(|record| match record.event {
			RuntimeEvent::MetaTx(pallet_meta_tx::Event::Dispatched { result }) => Some(result),
			_ => None,
		})
		.expect("MetaTx emits the inner dispatch result")
}

#[test]
fn enterprise_identity_attestation_name_and_storage_lifecycle_is_native_and_fail_closed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(2);
		frame_system::BlockHash::<Runtime>::insert(1, sp_core::H256::repeat_byte(0x42));
		System::set_extrinsic_index(0);

		let owner_pair = sr25519::Pair::from_seed(&[0x41; 32]);
		let owner = MultiSigner::Sr25519(owner_pair.public()).into_account();
		let sponsor_pair = sr25519::Pair::from_seed(&[0x51; 32]);
		let sponsor = MultiSigner::Sr25519(sponsor_pair.public()).into_account();
		let registrar = AccountId::from([3u8; 32]);
		let unavailable_provider = AccountId::from([7u8; 32]);
		let selected_provider = AccountId::from([8u8; 32]);
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);
		<Balances as Mutate<AccountId>>::set_balance(&sponsor, 100_000_000_000_000_000);

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

		// Entity remains the authoritative SubjectId source for the native attestation and Orbis Names.
		let mut entity_info = pallet_origin_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		entity_info.display =
			origin_primitives::Element::Raw(b"Enterprise Alice".to_vec().try_into().unwrap());
		assert_ok!(Entity::set_info(RuntimeOrigin::signed(owner.clone()), Box::new(entity_info),));
		let subject_id = pallet_origin_entity::EntityTokenOfAccount::<Runtime>::get(&owner)
			.expect("Entity creates the canonical subject");
		let subject_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(subject_id.as_ref()));

		// Orbis Storage stores the real call payload, indexes its content commitment, and records who
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
			pallet_orbis_transaction_storage::Call::<Runtime>::store { data: content.clone() };
		let (_, scope) = TransactionStorage::validate_signed(&owner, &store_call).unwrap();
		let scope = scope.expect("signed storage carries its validated authorization scope");
		assert_ok!(TransactionStorage::pre_dispatch_signed(&owner, &store_call));
		let authorized = pallet_orbis_transaction_storage::Origin::<Runtime>::Authorized {
			who: owner.clone(),
			scope,
		};
		assert_ok!(TransactionStorage::store(RuntimeOrigin::from(authorized), content));
		assert!(TransactionStorage::contains_transaction(content_hash));
		assert_eq!(
			TransactionStorage::stored_content_provenance(StorageRef {
				block: 2,
				transaction_index: 0,
			}),
			Some(StorageActor::Account(owner.clone())),
		);
		<TransactionStorage as Hooks<u32>>::on_finalize(2);
		assert_eq!(TransactionStorage::transactions_at(2).unwrap()[0].content_hash, content_hash);

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
		let sponsored_result = apply_sponsored_call(
			RuntimeCall::Attestation(pallet_orbis_attestation::Call::issue { input }),
			&owner_pair,
			&sponsor_pair,
		);
		assert_ok!(sponsored_result);
		assert_eq!(System::account_nonce(&owner), 1);
		assert_eq!(System::account_nonce(&sponsor), 1);
		assert!(Attestation::is_live(attestation));

		let label = Names::validate_label(b"enterprise-alice".to_vec()).unwrap();
		let salt: pallet_orbis_names::SaltOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		let commitment = Names::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Names::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(5);
		System::set_extrinsic_index(1);
		assert_ok!(Names::register(
			RuntimeOrigin::signed(owner.clone()),
			None,
			label.clone(),
			salt,
		));
		let name = Names::derive_name_id(None, &label);
		assert_ok!(Names::set_subject(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(subject_id),
		));
		assert_ok!(Names::set_attestation(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(attestation),
		));
		assert_ok!(Names::set_content(
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

		// Revocation is authoritative: Orbis Names resolution no longer returns a revoked credential.
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
		let bytes = include_bytes!("../vectors/transaction-policy-v8/sponsored-outer-extrinsic.scale");
		let extrinsic = crate::UncheckedExtrinsic::decode_all(&mut bytes.as_slice()).unwrap();
		let outer = extrinsic.0.function;
		assert!(matches!(outer, RuntimeCall::MetaTx(..)));
		let sponsor_pair = ed25519::Pair::from_seed(&[0x45; 32]);
		let sponsor = MultiSigner::Ed25519(sponsor_pair.public()).into_account();
		assert_eq!(Balances::free_balance(&sponsor), 0);
		let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
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
			&mut include_bytes!("../vectors/transaction-policy-v8/mutate-spec.scale").as_slice(),
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

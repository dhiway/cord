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

//! Focused P6 enterprise journey proof.
//!
//! These tests deliberately exercise the real Commons runtime and native pallets in-process. They
//! do not claim to prove provider-node byte retrieval, automatic failover, a live relay/parachain
//! topology, or production SLOs. Those boundaries are recorded in the accompanying P6 manifest.

use super::*;
use codec::{DecodeAll, Encode};
use frame_support::{
	assert_ok,
	dispatch::GetDispatchInfo,
	traits::{fungible::Mutate, BuildGenesisConfig},
};
use orbis_storage_runtime_api::runtime_decl_for_storage_provider_api::StorageProviderApi;
use pallet_orbis_names_runtime_api::runtime_decl_for_names_api::NamesApiV1;
use sp_core::{ed25519, sr25519, Pair};
use sp_runtime::{
	traits::{IdentifyAccount, TransactionExtension},
	MultiSigner,
};

#[test]
fn enterprise_identity_attestation_name_and_storage_lifecycle_is_native_and_fail_closed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let owner_pair = sr25519::Pair::from_seed(&[0x41; 32]);
		let owner = MultiSigner::Sr25519(owner_pair.public()).into_account();
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);

		let mut identity = pallet_orbis_people::identity_info::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		identity.display =
			pallet_orbis_people::Data::Raw(b"Enterprise Alice".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(identity),
		));
		let mut entity_info = pallet_origin_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		entity_info.display =
			origin_primitives::Element::Raw(b"Enterprise Alice".to_vec().try_into().unwrap());
		assert_ok!(Entity::set_info(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(entity_info),
		));
		let subject_id = pallet_origin_entity::EntityTokenOfAccount::<Runtime>::get(&owner)
			.expect("Entity creates the canonical subject");
		let subject_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(subject_id.as_ref()));

		let manifest = sp_io::hashing::blake2_256(b"festival-enterprise-admission-v1");
		let admission = crate::tests::admit_canonical_manifest(&owner, manifest);
		assert_eq!(Runtime::governed_finalized_checkpoint(), Some(101));
		assert!(Runtime::provider_is_eligible(admission.primary.clone()));
		assert_eq!(
			Runtime::provider(admission.primary.clone())
				.value
				.unwrap()
				.authority_validated_at,
			Some(101),
		);
		assert_eq!(
			<crate::OrbisStorageControl as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::manifest_state(&manifest),
			pallet_orbis_storage_control_primitives::CommitmentState::Publishable,
		);

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
			payload_commitment: sp_core::H256::from(manifest),
			status_commitment: sp_core::H256::from_low_u64_be(1),
			parent: None,
			expiry: Some(1_000),
			uniqueness_commitment: Some(sp_core::H256::from_low_u64_be(1)),
			revocable: true,
		};
		let nonce = pallet_orbis_attestation::NextIssuerAttestationNonce::<Runtime>::get(&owner);
		let app_attestation = Attestation::attestation_id(&owner, &input, nonce);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), input));

		let label = Names::validate_label(b"enterprise-alice".to_vec()).unwrap();
		let salt: pallet_orbis_names::SaltOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		let commitment = Names::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Names::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(103);
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
			Some(app_attestation),
		));
		assert_ok!(Names::publish_content(
			RuntimeOrigin::signed(owner.clone()),
			name,
			Some(manifest),
			None,
			System::block_number().saturating_add(10),
			[7; 16]));

		let drive_name: pallet_orbis_drive::DriveNameOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		assert_ok!(Drive::create_drive(RuntimeOrigin::signed(owner.clone()), drive_name));
		let drive_id = pallet_orbis_drive::OwnerDrives::<Runtime>::get(&owner)[0];
		assert_ok!(Drive::update_root(
			RuntimeOrigin::signed(owner.clone()),
			drive_id,
			1,
			None,
			manifest,
			admission.provider_commitment,
		));
		let bucket_name: pallet_orbis_s3::BucketNameOf<Runtime> =
			b"enterprise-festival".to_vec().try_into().unwrap();
		let s3_bucket = S3::bucket_id(&owner, &bucket_name);
		assert_ok!(S3::create_bucket(RuntimeOrigin::signed(owner.clone()), bucket_name));
		let object_key: pallet_orbis_s3::ObjectKeyOf<Runtime> =
			b"admission/alice".to_vec().try_into().unwrap();
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(owner.clone()),
			s3_bucket,
			object_key.clone(),
			manifest,
			admission.provider_commitment,
			Default::default(),
			Default::default(),
			[0x61; 32],
			None,
			None,
		));
		assert!(!pallet_orbis_s3::Objects::<Runtime>::get(s3_bucket, object_key)
			.unwrap()
			.deleted);

		// Concrete revocation is reported through one bounded provider/bucket refresh. The signed
		// checkpoint evidence is retained and event order is suspension -> selection -> promotion.
		System::reset_events();
		assert_ok!(Attestation::revoke(
			RuntimeOrigin::signed(owner.clone()),
			admission.organization_attestations[0],
		));
		System::set_block_number(104);
		assert_ok!(StorageProvider::advance_finalized_checkpoint(
			RuntimeOrigin::root(),
			104,
		));
		assert_ok!(StorageProvider::refresh_bucket_authority(
			RuntimeOrigin::signed(owner.clone()),
			admission.bucket_id,
		));
		let expected_promoted = admission
			.replicas
			.iter()
			.min_by(|left, right| left.encode().cmp(&right.encode()))
			.unwrap()
			.clone();
		assert_eq!(
			pallet_orbis_storage_provider::Providers::<Runtime>::get(&admission.primary)
				.unwrap()
				.status,
			pallet_orbis_storage_provider::ProviderStatus::Suspended,
		);
		assert_eq!(
			pallet_orbis_storage_provider::Buckets::<Runtime>::get(admission.bucket_id)
				.unwrap()
				.primary,
			expected_promoted,
		);
		assert_eq!(
			pallet_orbis_storage_provider::CheckpointClaims::<Runtime>::iter_prefix(
				admission.bucket_id,
			)
			.count(),
			1,
			"signed checkpoint evidence survives organization revocation",
		);
		let events = System::events();
		let suspended = events.iter().position(|record| matches!(
			&record.event,
			RuntimeEvent::StorageProvider(
				pallet_orbis_storage_provider::Event::ProviderStatusChanged {
					provider,
					status: pallet_orbis_storage_provider::ProviderStatus::Suspended,
				}
			) if provider == &admission.primary
		)).unwrap();
		let selected = events.iter().position(|record| matches!(
			&record.event,
			RuntimeEvent::StorageProvider(
				pallet_orbis_storage_provider::Event::ReplicaSelected { provider, .. }
			) if provider == &expected_promoted
		)).unwrap();
		let promoted = events.iter().position(|record| matches!(
			&record.event,
			RuntimeEvent::StorageProvider(
				pallet_orbis_storage_provider::Event::PrimaryPromoted { new_provider, .. }
			) if new_provider == &expected_promoted
		)).unwrap();
		assert!(suspended < selected && selected < promoted);

		assert_ok!(Attestation::revoke(
			RuntimeOrigin::signed(owner),
			app_attestation,
		));
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
		let bytes =
			include_bytes!("../vectors/transaction-policy-v8/sponsored-outer-extrinsic.scale");
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

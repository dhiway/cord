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

use crate::{
	xcm_config::LocationToAccountId, AssetConversion, AssetRate, AssetTxPayment, Assets,
	AssetsFreezer, AssetsHolder, Attestation, Balances, Broker, ChunksManager, Drive, Entity,
	Feeless, ForeignAssets, ForeignAssetsFreezer, HopPromotion, Members, MembersNotifier, Names,
	Nfts, People, PeopleLite, Period, Personhood, PoolAssets, PoolAssetsFreezer, Revive, Runtime,
	RuntimeCall, RuntimeOrigin, System, TransactionStorage, Uniques, S3,
};
use codec::{Decode, Encode};
use cumulus_primitives_core::ParaId;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::CheckIfFeeless,
	traits::{fungible::Mutate, Contains, Get, Hooks, PalletInfoAccess},
};
use pallet_broker::{CoreAssignment, CoreMask, Reservations, Schedule, ScheduleItem};
use pallet_orbis_attestation_runtime_api as attestation_api;
use polkadot_primitives::AccountId;
use sp_core::crypto::Ss58Codec;
use sp_runtime::traits::AsSystemOriginSigner;
use xcm::prelude::*;
use xcm_runtime_apis::conversions::LocationToAccountHelper;

#[test]
fn attestation_page_zero_and_boundaries_are_explicit() {
	let ids: Vec<sp_core::H256> = (0..=attestation_api::MAX_PAGE_SIZE)
		.map(|value| sp_core::H256::from_low_u64_be(value as u64 + 1))
		.collect();
	let empty = crate::attestation_id_page(&ids, None, 0);
	assert!(empty.items.is_empty());
	assert_eq!(empty.next_cursor, None);
	let bounded = crate::attestation_id_page(&ids, None, u32::MAX);
	assert_eq!(bounded.items.len(), attestation_api::MAX_PAGE_SIZE as usize);
	assert_eq!(bounded.next_cursor, Some(attestation_api::MAX_PAGE_SIZE));
	let past_end = crate::attestation_id_page(&ids, Some(u32::MAX), 10);
	assert!(past_end.items.is_empty());
	assert_eq!(past_end.next_cursor, None);
}

#[test]
fn runtime_signing_payloads_match_shared_sdk_vectors() {
	let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../../../docs/sdk/vectors/attestation-v1.json"
	)))
	.unwrap();
	let issuer = AccountId::new([1; 32]);
	let delegate = AccountId::new([2; 32]);
	let issue = pallet_orbis_attestation::DelegatedIntent::<Runtime> {
		genesis_hash: sp_core::H256::repeat_byte(0x11),
		spec_version: 42,
		action: pallet_orbis_attestation::DelegatedAction::Issue,
		issuer: issuer.clone(),
		delegate: delegate.clone(),
		schema: sp_core::H256::repeat_byte(0x22),
		subject_commitment: sp_core::H256::repeat_byte(0x33),
		payload_commitment: sp_core::H256::repeat_byte(0x44),
		status_commitment: sp_core::H256::repeat_byte(0x55),
		parent: None,
		expiry: Some(100),
		uniqueness_commitment: Some(sp_core::H256::repeat_byte(0x66)),
		revocable: true,
		nonce: 7,
		deadline: 90,
	};
	let revoke = pallet_orbis_attestation::DelegatedRevokeIntent::<Runtime> {
		genesis_hash: sp_core::H256::repeat_byte(0x11),
		spec_version: 42,
		action: pallet_orbis_attestation::DelegatedAction::Revoke,
		revoker: issuer,
		delegate,
		attestation: sp_core::H256::repeat_byte(0x77),
		nonce: 8,
		deadline: 91,
	};
	assert_eq!(
		format!(
			"0x{}",
			hex::encode(pallet_orbis_attestation::delegated_signing_payload::<Runtime>(&issue))
		),
		vectors["signing"][0]["payload"].as_str().unwrap(),
	);
	assert_eq!(
		format!(
			"0x{}",
			hex::encode(pallet_orbis_attestation::delegated_revoke_signing_payload::<Runtime>(
				&revoke,
			))
		),
		vectors["signing"][3]["payload"].as_str().unwrap(),
	);
}

#[test]
fn session_period_preserves_production_and_hash_bound_fast_profiles() {
	#[cfg(feature = "fast-runtime")]
	{
		assert_eq!(Period::get(), 2 * origin_commons_runtime_constants::async_backing::MINUTES);
		assert!(<<Runtime as pallet_coretime_control::Config>::TransportControlEnabled as Get<
			bool,
		>>::get());
	}
	#[cfg(not(feature = "fast-runtime"))]
	{
		assert_eq!(Period::get(), 6 * origin_commons_runtime_constants::async_backing::HOURS);
		assert!(!<<Runtime as pallet_coretime_control::Config>::TransportControlEnabled as Get<
			bool,
		>>::get());
	}
}

#[path = "remediation_v3.rs"]
mod remediation_v3;

const ALICE: [u8; 32] = [1u8; 32];

#[test]
#[should_panic(expected = "Orbis paid Meta token leaked")]
fn post_transactions_rejects_a_leaked_paid_meta_token() {
	use frame_support::traits::PostTransactions;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV7 {
			payer: AccountId::new([1; 32]),
			intent_commitment: sp_core::H256::repeat_byte(2),
			outer_nonce: 0,
			genesis_hash: sp_core::H256::zero(),
			spec_version: 27,
			transaction_version: 6,
			consumed: false,
		});
		crate::meta_v6::MetaTokenMustBeEmpty::post_transactions();
	});
}

#[test]
fn completion_manifest_is_parseable_unique_and_clean_genesis() {
	use std::collections::BTreeSet;

	let manifest: toml::Value =
		toml::from_str(include_str!("../../../../docs/orbis-completion-manifest.toml"))
			.expect("the current completion manifest must be valid TOML");
	assert_eq!(manifest["manifest_version"].as_integer(), Some(22));
	let mut identities = BTreeSet::new();
	for (table, value) in manifest.as_table().unwrap() {
		let Some(rows) = value.as_array() else { continue };
		for row in rows {
			let id = ["id", "name", "package", "revision"]
				.into_iter()
				.find_map(|key| row.get(key).and_then(toml::Value::as_str))
				.expect("every manifest row has an identity");
			assert!(identities.insert((table.as_str(), id)), "duplicate {table}:{id}");
		}
	}
	assert!(manifest.get("protocol_migration").is_none());
	assert!(manifest
		.as_table()
		.expect("completion manifest is a table")
		.keys()
		.all(|key| !key.contains("v7_rehearsal") && !key.contains("v7_contract")));
	assert!(manifest["provider_v8_contract"]
		.as_array()
		.unwrap()
		.iter()
		.all(|row| row["status"].as_str() == Some("present")));
	let text = include_str!("../../../../docs/orbis-completion-manifest.toml");
	for stale in
		["LegacyUnknown", "LegacyContentUnrenewable", "MigrateV6ToV7", "provider_ref migration"]
	{
		assert!(!text.contains(stale), "stale clean-break manifest symbol: {stale}");
	}
}

#[test]
fn commons_storage_control_worst_case_weights_fit_the_runtime_block_budget() {
	use pallet_orbis_drive::weights::WeightInfo as DriveWeightInfo;
	use pallet_orbis_s3::weights::WeightInfo as S3WeightInfo;
	use pallet_orbis_storage_provider::weights::WeightInfo as StorageWeightInfo;

	type StorageWeights = pallet_orbis_storage_provider::weights::SubstrateWeight<Runtime>;
	type DriveWeights = pallet_orbis_drive::weights::SubstrateWeight<Runtime>;
	type S3Weights = pallet_orbis_s3::weights::SubstrateWeight<Runtime>;
	let block = crate::RuntimeBlockWeights::get().max_block;
	assert_eq!(crate::ProviderMaxDutiesPerBlock::get(), 256);
	assert_eq!(crate::ProviderMaxChallengeBacklog::get(), 256);
	assert_eq!(crate::ProviderMaxCapacityReleasesPerBlock::get(), 256);
	assert_eq!(crate::ProviderMaxChallengesPerBlock::get(), 128);
	assert_eq!(crate::ProviderMaxReconciliationRecords::get(), 128);
	let mandatory = StorageWeights::submit_checkpoint(crate::ProviderMaxReplicas::get())
		.max(StorageWeights::submit_challenge_proof(crate::ProviderMaxProofNodes::get()))
		.max(StorageWeights::publish_manifest(crate::ProviderMaxProofNodes::get()))
		.max(StorageWeights::refresh_bucket_authority_valid(crate::ProviderMaxReplicas::get()))
		.max(StorageWeights::refresh_bucket_authority_failover(
			crate::ProviderMaxReplicas::get(),
			crate::ProviderMaxBucketAgreements::get(),
		));
	let release =
		StorageWeights::on_initialize_release(crate::ProviderMaxCapacityReleasesPerBlock::get());
	let reconcile = StorageWeights::on_initialize_reconcile(
		crate::ProviderMaxReconciliationRecords::get(),
		crate::ProviderMaxBucketAgreements::get(),
	);
	let challenges =
		StorageWeights::on_initialize_challenges(crate::ProviderMaxChallengesPerBlock::get());
	for weight in [mandatory, release, reconcile, challenges] {
		assert!(weight.all_lte(block), "storage-control weight {weight:?} exceeds {block:?}");
	}
	assert!(
		release.saturating_add(mandatory).all_lte(block),
		"release phase leaves no mandatory storage-control extrinsic headroom"
	);
	assert!(
		reconcile.saturating_add(mandatory).all_lte(block),
		"reconciliation phase leaves no mandatory storage-control extrinsic headroom"
	);
	assert!(
		challenges.saturating_add(mandatory).all_lte(block),
		"challenge phase leaves no mandatory storage-control extrinsic headroom"
	);
	assert!(release.all_gte(StorageWeights::on_initialize_release(255)));
	assert!(reconcile.all_gte(StorageWeights::on_initialize_reconcile(
		crate::ProviderMaxReconciliationRecords::get() - 1,
		crate::ProviderMaxBucketAgreements::get(),
	)));
	assert!(reconcile.all_gte(StorageWeights::on_initialize_reconcile(
		crate::ProviderMaxReconciliationRecords::get(),
		crate::ProviderMaxBucketAgreements::get() - 1,
	)));
	assert!(challenges.all_gte(StorageWeights::on_initialize_challenges(127)));
	let provider_dispatches = [
		(
			"provider.register",
			StorageWeights::register_provider(),
			StorageWeights::register_provider(),
		),
		("provider.update", StorageWeights::update_provider(), StorageWeights::update_provider()),
		(
			"provider.rotate_key",
			StorageWeights::rotate_service_key(),
			StorageWeights::rotate_service_key(),
		),
		(
			"provider.rotate_org",
			StorageWeights::rotate_provider_organization(),
			StorageWeights::rotate_provider_organization(),
		),
		(
			"provider.set_status",
			StorageWeights::set_provider_status(),
			StorageWeights::set_provider_status(),
		),
		("provider.remove", StorageWeights::remove_provider(), StorageWeights::remove_provider()),
		("provider.heartbeat", StorageWeights::heartbeat(), StorageWeights::heartbeat()),
		(
			"provider.create_bucket",
			StorageWeights::create_bucket(2),
			StorageWeights::create_bucket(4),
		),
		(
			"provider.change_grant",
			StorageWeights::change_bucket_grant(),
			StorageWeights::change_bucket_grant(),
		),
		(
			"provider.propose_agreement",
			StorageWeights::propose_agreement(2),
			StorageWeights::propose_agreement(4),
		),
		(
			"provider.accept_agreement",
			StorageWeights::accept_agreement(2),
			StorageWeights::accept_agreement(4),
		),
		(
			"provider.suspend_agreement",
			StorageWeights::set_agreement_suspension(),
			StorageWeights::set_agreement_suspension(),
		),
		(
			"provider.terminate_agreement",
			StorageWeights::terminate_agreement(),
			StorageWeights::terminate_agreement(),
		),
		(
			"provider.expire_agreement",
			StorageWeights::expire_agreement(),
			StorageWeights::expire_agreement(),
		),
		(
			"provider.submit_checkpoint",
			StorageWeights::submit_checkpoint(2),
			StorageWeights::submit_checkpoint(crate::ProviderMaxReplicas::get()),
		),
		(
			"provider.issue_challenge",
			StorageWeights::issue_challenge(),
			StorageWeights::issue_challenge(),
		),
		(
			"provider.submit_challenge_proof",
			StorageWeights::submit_challenge_proof(1),
			StorageWeights::submit_challenge_proof(crate::ProviderMaxProofNodes::get()),
		),
		(
			"provider.reconcile_bucket",
			StorageWeights::reconcile_bucket(2, 0),
			StorageWeights::reconcile_bucket(
				crate::ProviderMaxReplicas::get(),
				crate::ProviderMaxBucketAgreements::get(),
			),
		),
		(
			"provider.refresh_authority_valid",
			StorageWeights::refresh_bucket_authority_valid(2),
			StorageWeights::refresh_bucket_authority_valid(crate::ProviderMaxReplicas::get()),
		),
		(
			"provider.refresh_authority_failover",
			StorageWeights::refresh_bucket_authority_failover(2, 0),
			StorageWeights::refresh_bucket_authority_failover(
				crate::ProviderMaxReplicas::get(),
				crate::ProviderMaxBucketAgreements::get(),
			),
		),
		(
			"provider.register_manifest",
			StorageWeights::register_manifest(),
			StorageWeights::register_manifest(),
		),
		(
			"provider.publish_manifest",
			StorageWeights::publish_manifest(1),
			StorageWeights::publish_manifest(crate::ProviderMaxProofNodes::get()),
		),
		(
			"provider.tombstone_manifest",
			StorageWeights::tombstone_manifest(),
			StorageWeights::tombstone_manifest(),
		),
		(
			"provider.ack_deletion",
			StorageWeights::acknowledge_manifest_deletion(),
			StorageWeights::acknowledge_manifest_deletion(),
		),
		(
			"provider.replace_replica",
			StorageWeights::replace_bucket_replica(0),
			StorageWeights::replace_bucket_replica(crate::ProviderMaxBucketAgreements::get()),
		),
		(
			"provider.advance_finalized",
			StorageWeights::advance_finalized_checkpoint(),
			StorageWeights::advance_finalized_checkpoint(),
		),
	];
	let mut max_provider_dispatch = frame_support::weights::Weight::zero();
	for (name, base, limit) in provider_dispatches {
		assert!(base.all_lte(block), "{name} base weight {base:?} exceeds {block:?}");
		assert!(limit.all_lte(block), "{name} limit weight {limit:?} exceeds {block:?}");
		assert!(limit.all_gte(base), "{name} weight is not monotonic at its limits");
		max_provider_dispatch = max_provider_dispatch.max(limit);
	}
	for (phase, hook) in [("release", release), ("reconcile", reconcile), ("challenge", challenges)]
	{
		assert!(
			hook.saturating_add(max_provider_dispatch).all_lte(block),
			"{phase} hook leaves no maximum provider dispatch headroom"
		);
	}

	let drive_dispatches = [
		("drive.create_drive", DriveWeights::create_drive(1), DriveWeights::create_drive(256)),
		("drive.update_root", DriveWeights::update_root(0), DriveWeights::update_root(63)),
		("drive.set_grant", DriveWeights::set_grant(), DriveWeights::set_grant()),
		("drive.transfer_drive", DriveWeights::transfer_drive(), DriveWeights::transfer_drive()),
		("drive.archive_drive", DriveWeights::archive_drive(), DriveWeights::archive_drive()),
		(
			"drive.write_node_create",
			DriveWeights::write_node_create(1, 0),
			DriveWeights::write_node_create(4_096, 64),
		),
		(
			"drive.write_node_update_file",
			DriveWeights::write_node_update_file(1, 0),
			DriveWeights::write_node_update_file(4_096, 64),
		),
		("drive.remove_node", DriveWeights::remove_node(), DriveWeights::remove_node()),
	];
	let s3_dispatches = [
		("s3.create_bucket", S3Weights::create_bucket(3), S3Weights::create_bucket(63)),
		("s3.set_controller", S3Weights::set_controller(), S3Weights::set_controller()),
		("s3.transfer_bucket", S3Weights::transfer_bucket(), S3Weights::transfer_bucket()),
		("s3.set_archived", S3Weights::set_archived(), S3Weights::set_archived()),
		("s3.set_versioning", S3Weights::set_versioning(), S3Weights::set_versioning()),
		(
			"s3.put_object_create",
			S3Weights::put_object_create(1),
			S3Weights::put_object_create(1_024),
		),
		(
			"s3.put_object_update",
			S3Weights::put_object_update(1, 0),
			S3Weights::put_object_update(1_024, 63),
		),
		("s3.delete_object", S3Weights::delete_object(1, 0), S3Weights::delete_object(1_024, 63)),
		("s3.delete_bucket", S3Weights::delete_bucket(), S3Weights::delete_bucket()),
		("s3.prune_history", S3Weights::prune_history(1), S3Weights::prune_history(64)),
		("s3.purge_object", S3Weights::purge_object(1), S3Weights::purge_object(65)),
	];
	for (name, base, limit) in drive_dispatches.into_iter().chain(s3_dispatches) {
		assert!(base.all_lte(block), "{name} base weight {base:?} exceeds {block:?}");
		assert!(limit.all_lte(block), "{name} limit weight {limit:?} exceeds {block:?}");
		assert!(limit.all_gte(base), "{name} weight is not monotonic at its limits");
	}
}

#[test]
fn orbis_owned_origin_forks_preserve_indices_calls_and_storage_metadata() {
	use frame_support::traits::{PalletInfoAccess, StorageInfoTrait};
	use scale_info::{TypeDef, TypeInfo};

	fn call_variants<T: TypeInfo>() -> Vec<(u8, String)> {
		let TypeDef::Variant(variants) = T::type_info().type_def else {
			panic!("runtime call metadata must be a variant")
		};
		variants
			.variants
			.into_iter()
			.map(|variant| (variant.index, variant.name.into()))
			.collect()
	}

	fn storage_names<T: StorageInfoTrait>() -> Vec<String> {
		T::storage_info()
			.into_iter()
			.map(|info| {
				String::from_utf8(info.storage_name).expect("FRAME storage names are UTF-8")
			})
			.collect()
	}

	assert_eq!(pallet_origin_token::Pallet::<Runtime>::index(), 51);
	assert_eq!(pallet_origin_register::Pallet::<Runtime>::index(), 52);
	assert_eq!(pallet_origin_entity::Pallet::<Runtime>::index(), 53);
	assert_eq!(pallet_origin_feeless::Pallet::<Runtime>::index(), 54);
	assert_eq!(indiv_pallet_resources::Pallet::<Runtime>::index(), 96);
	assert_eq!(pallet_orbis_score::Pallet::<Runtime>::index(), 97);
	assert_eq!(pallet_orbis_honour::Pallet::<Runtime>::index(), 99);
	assert_eq!(crate::VERSION.spec_version, 30);
	assert_eq!(crate::VERSION.transaction_version, 8);

	assert_eq!(
		call_variants::<pallet_origin_register::Call<Runtime>>(),
		[
			"create_registry",
			"set_delegate_permissions",
			"remove_delegate_permissions",
			"update_registry_info",
			"revoke_registry",
			"restore_registry",
			"delete_registry",
			"create_packet",
			"update_packet",
			"revoke_packet",
			"restore_packet",
			"remove_packet",
		]
		.into_iter()
		.enumerate()
		.map(|(index, name)| (index as u8, name.into()))
		.collect::<Vec<_>>()
	);
	assert_eq!(
		call_variants::<pallet_origin_entity::Call<Runtime>>(),
		[
			"set_info",
			"rotate_attributes",
			"add_attributes",
			"remove_attribute",
			"rotate_attribute",
			"set_linked_account",
			"revoke_linked_account",
			"revoke_linked_account_for",
			"rotate_controller",
			"rotate_controller_for",
			"clear_everything",
			"clear_everything_for",
			"set_entity_nym",
			"remove_entity_nym",
		]
		.into_iter()
		.enumerate()
		.map(|(index, name)| (index as u8, name.into()))
		.collect::<Vec<_>>()
	);
	assert_eq!(
		call_variants::<pallet_origin_feeless::Call<Runtime>>(),
		vec![(0, "add_feeless_account".into()), (1, "remove_feeless_account".into())]
	);
	assert_eq!(
		call_variants::<indiv_pallet_resources::Call<Runtime>>(),
		[
			(0, "register_lite_person"),
			(1, "register_person"),
			(2, "touch_person_authorization"),
			(4, "update_identifier_key"),
			(7, "demote_auth_expired"),
			(8, "set_friend_request_statement_account_for_sequence"),
			(9, "clear_expired_friend_request_sequence"),
			(10, "set_statement_store_account"),
			(11, "clear_expired_stmt_store_allowances"),
			(12, "claim_long_term_storage"),
			(13, "clear_expired_long_term_storage_aliases"),
			(15, "cancel_long_term_storage_reservation"),
			(17, "expire_long_term_storage_reservations"),
		]
		.into_iter()
		.map(|(index, name)| (index, name.into()))
		.collect::<Vec<_>>()
	);

	assert_eq!(
		storage_names::<pallet_origin_token::Pallet<Runtime>>(),
		[
			"PalletIndex",
			"IndexToPallet",
			"NextPalletIndex",
			"GenesisNetworkId",
			"StateHistory",
			"StateVersion"
		]
	);
	assert_eq!(
		storage_names::<pallet_origin_register::Pallet<Runtime>>(),
		[
			"Registries",
			"RegistryDelegates",
			"PacketStates",
			"Packets",
			"LookupIndex",
			"RegistryQueryCounts"
		]
	);
	assert_eq!(
		storage_names::<pallet_origin_entity::Pallet<Runtime>>(),
		[
			"EntityInfoOf",
			"EntityTokenOfAccount",
			"LinkedAccounts",
			"ControllerAccountOf",
			"AccountUnbindHistory",
			"EntityNymOf",
			"EntityNymIndex",
			"AttributeVersionOf",
			"AttributeHistoryOf"
		]
	);
	assert_eq!(
		storage_names::<pallet_origin_feeless::Pallet<Runtime>>(),
		["FeelessAccountStore", "FeelessUsage"]
	);
}

/// Compile-time representation of ADR 0008's frozen policy slots. `NoPolicy` is deliberately not a
/// transaction extension: an unimplemented slot cannot authorize a call or mutate an origin.
mod transaction_policy_fixture {
	use super::Runtime;
	use core::marker::PhantomData;

	pub struct NoPolicy<const SLOT: u8>;
	pub struct Implemented<T>(PhantomData<T>);
	pub struct EnvelopeSignature;

	impl<const SLOT: u8> NoPolicy<SLOT> {
		pub fn passthrough_origin<O>(origin: O) -> O {
			origin
		}
	}

	pub type FrozenPolicySlots = (
		NoPolicy<0>, // AuthorizeValueTransfer
		EnvelopeSignature,
		indiv_pallet_people::extension::AsPerson<Runtime>,
		NoPolicy<3>, // AsProofOfInkParticipant
		pallet_orbis_score::ScoreAsParticipant<Runtime>,
		NoPolicy<5>, // GameAsInvited
		indiv_pallet_people_lite::extension::PeopleLiteAuth<Runtime>,
		NoPolicy<7>, // AsMember
		NoPolicy<8>, // AsCoinage
		indiv_pallet_resources::extension::AsResources<Runtime>,
		pallet_orbis_honour::extension::VoterAuth<Runtime>,
		frame_system::AuthorizeCall<Runtime>,
		NoPolicy<12>, // AsPgas
		NoPolicy<13>, // AsRingAlias
		NoPolicy<14>, // AsNamesGateway
	);

	pub type FrozenPayment = (
		Implemented<crate::ExplicitPayment<Runtime, crate::AssetPayment>>,
		Implemented<crate::AssetPayment>,
		NoPolicy<16>, // ChargePGAS
		Implemented<pallet_asset_conversion_tx_payment::ChargeAssetTxPayment<Runtime>>,
	);

	pub type FrozenPipeline = (
		Implemented<crate::TxExtensions>, // outer StorageWeightReclaim
		FrozenPolicySlots,
		NoPolicy<15>, // RestrictOrigin
		Implemented<(
			frame_system::CheckNonZeroSender<Runtime>,
			frame_system::CheckSpecVersion<Runtime>,
			frame_system::CheckTxVersion<Runtime>,
			frame_system::CheckGenesis<Runtime>,
			frame_system::CheckMortality<Runtime>,
			frame_system::CheckNonce<Runtime>,
			frame_system::CheckWeight<Runtime>,
		)>,
		FrozenPayment,
		Implemented<
			pallet_orbis_transaction_storage::extension::ValidateStorageCalls<
				Runtime,
				crate::OrbisStorageCallInspector,
			>,
		>,
		Implemented<frame_metadata_hash_extension::CheckMetadataHash<Runtime>>,
		Implemented<pallet_revive::evm::tx_extension::SetOrigin<Runtime>>,
	);
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn transaction_policy_construction_surfaces_share_the_frozen_slots() {
	use frame_system::offchain::CreateTransaction;
	use pallet_revive::evm::runtime::EthExtra;
	use sp_runtime::traits::TransactionExtension;
	use transaction_policy_fixture::*;
	assert_eq!(crate::VERSION.transaction_version, 8);

	fn assert_full_inner_projection(inner: crate::InnerTxExtensions) {
		let (
			policy,
			_nonzero,
			_spec,
			_tx,
			_genesis,
			_mortality,
			_nonce,
			_weight,
			_payment,
			_storage_policy,
			_metadata,
			_set_origin,
		) = inner;
		let (_as_person, score, _people_lite, as_resources, honour, _authorize_call) = policy;
		assert_eq!(as_resources.encode(), [0], "default surfaces cannot claim Resources origin");
		assert_eq!(score.encode(), [0], "default surfaces cannot claim Score participant origin");
		assert_eq!(honour.encode(), [0], "default surfaces cannot claim Honour voter origin");
	}

	fn assert_meta_projection(extension: crate::MetaTxExtension) {
		let (
			_verify,
			_consume,
			_marker,
			_nonzero,
			_spec,
			_tx,
			_genesis,
			_mortality,
			_nonce,
			identity_policies,
			_storage_policy,
			_metadata,
		) = extension;
		let (score, _policy, honour) = identity_policies;
		assert_eq!(score.encode(), [0], "MetaTx cannot claim Score without an explicit proof");
		assert_eq!(honour.encode(), [0], "MetaTx cannot claim Honour without an explicit proof");
	}

	let _: Option<FrozenPipeline> = None;
	assert_eq!(core::mem::size_of::<NoPolicy<0>>(), 0);
	assert_eq!(core::mem::size_of::<NoPolicy<15>>(), 0);
	assert_eq!(core::mem::size_of::<NoPolicy<16>>(), 0);
	let signed = RuntimeOrigin::signed(AccountId::from(ALICE));
	assert!(frame_system::ensure_signed(NoPolicy::<0>::passthrough_origin(signed)).is_ok());
	static_assertions::assert_type_eq_all!(
		crate::OriginPolicyExtensions,
		(
			indiv_pallet_people::extension::AsPerson<Runtime>,
			pallet_orbis_score::ScoreAsParticipant<Runtime>,
			indiv_pallet_people_lite::extension::PeopleLiteAuth<Runtime>,
			indiv_pallet_resources::extension::AsResources<Runtime>,
			pallet_orbis_honour::extension::VoterAuth<Runtime>,
			frame_system::AuthorizeCall<Runtime>,
		),
	);
	static_assertions::assert_type_eq_all!(
		crate::TxExtensions,
		<Runtime as CreateTransaction<RuntimeCall>>::Extension,
		<crate::EthExtraImpl as EthExtra>::ExtensionV0,
	);
	static_assertions::assert_type_eq_all!(
		crate::MetaTxExtension,
		<Runtime as pallet_meta_tx::Config>::Extension,
	);

	let ethereum = <crate::EthExtraImpl as EthExtra>::get_eth_extension(7, 11);
	assert_full_inner_projection(ethereum.0 .0);
	let authorized: crate::TxExtensions =
		<Runtime as frame_system::offchain::CreateAuthorizedTransaction<RuntimeCall>>::create_extension();
	let encoded_authorized_payment = authorized.0 .0 .8.encode();
	let decoded_network_payment =
		crate::AccountAwarePayment::decode(&mut &encoded_authorized_payment[..])
			.expect("payment policy encoding remains compatible with its inner asset payment");
	assert_eq!(decoded_network_payment, authorized.0 .0 .8.clone());
	assert_full_inner_projection(authorized.0 .0);

	let normal: crate::TxExtensions =
		crate::paid_tx_extensions(crate::default_inner_tx_extensions(
			3,
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(5, None),
			)
			.into(),
			Default::default(),
		));
	assert_full_inner_projection(normal.0 .0);

	let _actual_meta_projection: fn(crate::MetaTxExtension) = assert_meta_projection;

	let normal_metadata = crate::TxExtensions::metadata()
		.into_iter()
		.map(|entry| entry.identifier)
		.collect::<Vec<_>>();
	assert_eq!(
		normal_metadata,
		vec![
			"AsPerson",
			"ScoreAsParticipant",
			"PeopleLiteAuth",
			"AsResources",
			"HonourAuth",
			"AuthorizeCall",
			"CheckNonZeroSender",
			"CheckSpecVersion",
			"CheckTxVersion",
			"CheckGenesis",
			"CheckMortality",
			"CheckNonce",
			"CheckWeight",
			"ChargeAssetTxPayment",
			"ValidateStorageCalls",
			"CheckMetadataHash",
			"EthSetOrigin",
			"StorageWeightReclaim",
		]
	);
	let meta_metadata = crate::MetaTxExtension::metadata()
		.into_iter()
		.map(|entry| entry.identifier)
		.collect::<Vec<_>>();
	assert_eq!(
		meta_metadata,
		vec![
			"VerifyMultiSignature",
			"ConsumePaidMetaIngressV7",
			"MetaTxMarker",
			"CheckNonZeroSender",
			"CheckSpecVersion",
			"CheckTxVersion",
			"CheckGenesis",
			"CheckMortality",
			"CheckNonce",
			"ScoreAsParticipant",
			"MetaAccountBoundPoliciesV6",
			"HonourAuth",
			"ValidateStorageCalls",
			"CheckMetadataHash",
		]
	);
}

#[test]
fn score_normal_and_meta_signed_origins_share_active_participant_boundary() {
	use frame_support::{
		dispatch::GetDispatchInfo,
		traits::{BuildGenesisConfig, OriginTrait},
	};
	use pallet_orbis_score::{AccountOrPerson, Recognition, ScoreAsParticipantData};
	use sp_runtime::{traits::DispatchTransaction, transaction_validity::InvalidTransaction};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		pallet_orbis_score::GenesisConfig::<Runtime>::default().build();
		let normal = AccountId::new([0x31; 32]);
		let meta_inner = AccountId::new([0x32; 32]);
		for account in [&normal, &meta_inner] {
			crate::Score::onboard_for_recognition(account).unwrap();
		}
		let call = RuntimeCall::Score(pallet_orbis_score::Call::cash_out {});
		let info = call.get_dispatch_info();
		for account in [&normal, &meta_inner] {
			let extension = pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(Some(
				ScoreAsParticipantData { nonce: 0 },
			));
			extension
				.test_run(
					RuntimeOrigin::signed(account.clone()),
					&call,
					&info,
					0,
					0,
					|origin: RuntimeOrigin| {
						assert!(matches!(
							origin.into_caller().try_into(),
							Ok(pallet_orbis_score::Origin::AccountParticipant(who)) if who == *account
						));
						Ok(Default::default())
					},
				)
				.unwrap()
				.unwrap();
			// Score only validates its explicit policy nonce. The standard account-aware
			// CheckNonce in the concrete transaction pipeline owns the sole increment.
			assert_eq!(System::account(account).nonce, 0);
		}

		let suspended_key = AccountOrPerson::Account(meta_inner.clone());
		pallet_orbis_score::Participants::<Runtime>::mutate(&suspended_key, |participant| {
			participant.as_mut().unwrap().recognition = Recognition::Suspended(Default::default());
		});
		let before = pallet_orbis_score::Participants::<Runtime>::get(&suspended_key).unwrap();
		let nonce_before = System::account(&meta_inner).nonce;
		let extension =
			pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(Some(ScoreAsParticipantData {
				nonce: nonce_before,
			}));
		assert_eq!(
			extension
				.test_run(RuntimeOrigin::signed(meta_inner.clone()), &call, &info, 0, 0, |_| {
					Ok(Default::default())
				})
				.unwrap_err(),
			InvalidTransaction::Call.into()
		);
		assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&suspended_key), Some(before));
		assert_eq!(System::account(&meta_inner).nonce, nonce_before);

		let unknown = AccountId::new([0x33; 32]);
		let extension =
			pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(Some(ScoreAsParticipantData {
				nonce: 0,
			}));
		assert!(extension
			.test_run(RuntimeOrigin::signed(unknown.clone()), &call, &info, 0, 0, |_| {
				Ok(Default::default())
			})
			.is_err());
		assert_eq!(System::account(&unknown).nonce, 0);
	});
}

#[test]
fn ethereum_and_authorized_origins_cannot_activate_native_score_or_honour_policies() {
	use codec::DecodeAll;
	use frame_support::{dispatch::GetDispatchInfo, traits::BuildGenesisConfig};
	use sp_runtime::{
		traits::{DispatchTransaction, Dispatchable},
		transaction_validity::InvalidTransaction,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		let account = AccountId::new([0x66; 32]);
		let _ = <Balances as Mutate<AccountId>>::set_balance(&account, 100_000_000_000_000);
		let score_call = RuntimeCall::Score(pallet_orbis_score::Call::cash_out {});
		let score_extension = pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(Some(
			pallet_orbis_score::ScoreAsParticipantData { nonce: 0 },
		));
		let honour_bytes =
			include_bytes!("../vectors/transaction-policy-v8/honour-voter-meta.scale");
		let (honour_call, _, meta_extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut honour_bytes.as_slice()).unwrap();
		let honour_extension = meta_extension.9 .2;

		let before_root = sp_io::storage::root(sp_runtime::StateVersion::V1);
		let before_balance = Balances::free_balance(&account);
		let before_quota = pallet_origin_feeless::FeelessUsage::<Runtime>::get(&account);
		for (name, origin) in [
			(
				"Ethereum",
				RuntimeOrigin::from(pallet_revive::Origin::<Runtime>::EthTransaction(
					account.clone(),
				)),
			),
			("Authorized", RuntimeOrigin::from(frame_system::RawOrigin::Authorized)),
		] {
			assert_eq!(
				score_extension
					.clone()
					.test_run(
						origin.clone(),
						&score_call,
						&score_call.get_dispatch_info(),
						0,
						0,
						|_| Ok(Default::default()),
					)
					.unwrap_err(),
				InvalidTransaction::Call.into(),
				"{name} must not activate Score Some"
			);
			assert_eq!(
				honour_extension
					.clone()
					.test_run(
						origin.clone(),
						&honour_call,
						&honour_call.get_dispatch_info(),
						0,
						0,
						|_| Ok(Default::default()),
					)
					.unwrap_err(),
				InvalidTransaction::BadSigner.into(),
				"{name} must not activate Honour Some"
			);
			assert_eq!(
				score_call.clone().dispatch(origin.clone()).unwrap_err().error,
				pallet_orbis_score::Error::<Runtime>::BadOriginNotSignedNotAccountParticipant
					.into()
			);
			assert_eq!(
				honour_call.clone().dispatch(origin).unwrap_err().error,
				sp_runtime::DispatchError::BadOrigin
			);
			assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before_root);
			assert_eq!(System::account_nonce(&account), 0);
			assert_eq!(Balances::free_balance(&account), before_balance);
			assert!(pallet_orbis_score::Participants::<Runtime>::iter().next().is_none());
			assert!(pallet_orbis_honour::Votes::<Runtime>::iter().next().is_none());
			assert!(pallet_orbis_honour::Tally::<Runtime>::iter().next().is_none());
			assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&account), before_quota);
			assert!(crate::meta_v6::token().is_none());
		}
	});
}

#[test]
fn account_aware_resources_delegates_only_origin_payer() {
	use crate::Resources;
	use frame_support::{dispatch::GetDispatchInfo, traits::BuildGenesisConfig};
	use indiv_pallet_resources::types::MembershipCollection;
	use sp_runtime::traits::TransactionExtension;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		let payer = AccountId::from(ALICE);
		let other = AccountId::from([2u8; 32]);
		let _ =
			<Balances as Mutate<AccountId>>::set_balance(&payer, crate::ExistentialDeposit::get());
		let origin: RuntimeOrigin = indiv_pallet_resources::Origin::LongTermStorageClaim {
			alias: [7u8; 32],
			collection: MembershipCollection::People,
			payer: payer.clone(),
		}
		.into();
		let period = Resources::long_term_storage_period_from_timestamp(0);
		let mismatch =
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period,
				counter: 0,
				account_id: other,
			});
		let ext = crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(0));
		let implicit = ext.implicit().unwrap();
		assert!(ext
			.validate(
				origin.clone(),
				&mismatch,
				&mismatch.get_dispatch_info(),
				mismatch.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &mismatch)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.is_err());

		let matching =
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period,
				counter: 0,
				account_id: payer.clone(),
			});
		let ext = crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(0));
		let implicit = ext.implicit().unwrap();
		let (_, val, returned_origin) = ext
			.validate(
				origin.clone(),
				&matching,
				&matching.get_dispatch_info(),
				matching.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &matching)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.expect("origin payer should satisfy nonce validation");
		assert_eq!(
			frame_support::traits::OriginTrait::caller(&returned_origin),
			frame_support::traits::OriginTrait::caller(&origin)
		);
		let _pre = ext
			.prepare(
				val,
				&returned_origin,
				&matching,
				&matching.get_dispatch_info(),
				matching.encoded_size(),
			)
			.expect("prepare uses the same validated payer");
		assert_eq!(System::account_nonce(&payer), 1);

		let none_call = RuntimeCall::System(frame_system::Call::remark { remark: vec![0] });
		let none = indiv_pallet_resources::extension::AsResources::<Runtime>::new(None);
		let (_, none_val, none_origin) = none
			.validate(
				RuntimeOrigin::signed(payer.clone()),
				&none_call,
				&none_call.get_dispatch_info(),
				none_call.encoded_size(),
				(),
				&sp_runtime::traits::TxBaseImplication((0u8, &none_call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.expect("None Resources policy preserves an ordinary signed route");
		none.prepare(
			none_val,
			&none_origin,
			&none_call,
			&none_call.get_dispatch_info(),
			none_call.encoded_size(),
		)
		.expect("None Resources policy prepares without manufacturing a custom origin");
		assert_eq!(none_origin.as_system_origin_signer(), Some(&payer));
	});
}

#[test]
fn resources_people_and_lite_reservations_use_isolated_storage_capacity() {
	use crate::{Resources, Timestamp};
	use indiv_pallet_resources::types::{MembershipCollection, ReservationPurpose};
	use orbis_transaction_storage_primitives::ResourceReservationView;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		pallet_timestamp::Now::<Runtime>::put(3 * 24 * 60 * 60 * 1_000u64);
		let owner = AccountId::from(ALICE);
		let period = Resources::long_term_storage_period_from_timestamp(
			<Timestamp as frame_support::traits::UnixTime>::now().as_secs(),
		);
		let people_alias = [7u8; 32];
		let lite_alias = [8u8; 32];
		assert_ok!(Resources::claim_long_term_storage(
			indiv_pallet_resources::Origin::LongTermStorageClaim {
				alias: people_alias,
				collection: MembershipCollection::People,
				payer: owner.clone(),
			}
			.into(),
			period,
			0,
			owner.clone(),
		));
		assert_ok!(Resources::claim_long_term_storage(
			indiv_pallet_resources::Origin::LongTermStorageClaim {
				alias: lite_alias,
				collection: MembershipCollection::LitePeople,
				payer: owner.clone(),
			}
			.into(),
			period,
			0,
			owner.clone(),
		));

		let ResourceReservationView::Active(people) =
			TransactionStorage::resource_reservation(0).unwrap()
		else {
			panic!("people reservation must be active")
		};
		let ResourceReservationView::Active(lite) =
			TransactionStorage::resource_reservation(1).unwrap()
		else {
			panic!("lite reservation must be active")
		};
		assert_eq!(people.owner, owner);
		assert_eq!(people.bytes_remaining, 8 * 1024 * 1024);
		assert_eq!(people.transactions_remaining, 100);
		assert_eq!(lite.bytes_remaining, 4 * 1024 * 1024);
		assert_eq!(lite.transactions_remaining, 10);
		assert_eq!(
			pallet_orbis_transaction_storage::ReservedPermanentCapacity::<Runtime>::get(),
			people.bytes_remaining + lite.bytes_remaining
		);

		let duplicate = ReservationPurpose::Membership {
			period,
			alias: people_alias,
			counter: 0,
			collection: MembershipCollection::People,
		};
		assert_eq!(
			indiv_pallet_resources::StorageReservationByPurpose::<Runtime>::get(duplicate),
			Some(0)
		);
		assert_noop!(
			Resources::cancel_long_term_storage_reservation(
				RuntimeOrigin::signed(AccountId::from([2u8; 32])),
				0,
			),
			indiv_pallet_resources::Error::<Runtime>::NotReservationOwner
		);
		assert_ok!(Resources::cancel_long_term_storage_reservation(
			RuntimeOrigin::signed(owner),
			0,
		));
		assert!(matches!(
			TransactionStorage::resource_reservation(0),
			Some(ResourceReservationView::Tombstone(_))
		));
	});
}

#[test]
fn payment_skip_requires_authorized_origin_and_signed_origin_still_pays() {
	use frame_support::dispatch::GetDispatchInfo;
	use sp_runtime::traits::TransactionExtension;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let info = call.get_dispatch_info();
		let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into();
		let implicit = payment.implicit().unwrap();
		let result = payment.validate(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			&call,
			&info,
			call.encoded_size(),
			implicit,
			&sp_runtime::traits::TxBaseImplication((0u8, &call)),
			sp_runtime::transaction_validity::TransactionSource::External,
		);
		assert!(matches!(
			result,
			Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
				sp_runtime::transaction_validity::InvalidTransaction::Payment,
			))
		));
	});
}

#[test]
fn elastic_scaling_runtime_parameters_target_three_blocks_per_relay_slot() {
	assert_eq!(crate::RELAY_PARENT_OFFSET, 1);
	assert_eq!(crate::BLOCK_PROCESSING_VELOCITY, 3);
	assert_eq!(crate::SLOT_DURATION, 6_000);
	assert_eq!(crate::UNINCLUDED_SEGMENT_CAPACITY, 12);
	assert_eq!(
		<<Runtime as cumulus_pallet_parachain_system::Config>::RelayParentOffset as Get<u32>>::get(
		),
		1
	);
	assert!(<<Runtime as pallet_aura::Config>::AllowMultipleBlocksPerSlot as Get<bool>>::get());
}

#[test]
fn enterprise_asset_can_be_created_and_managed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let asset_id = 7u32;

		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			asset_id.into(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner),
			asset_id.into(),
			beneficiary.clone().into(),
			1_000,
		));

		assert_eq!(Assets::balance(asset_id, beneficiary), 1_000);
	});
}

#[test]
fn enterprise_assets_support_native_holds_and_freezes() {
	use frame_support::traits::tokens::fungibles::{
		freeze::{Inspect, Mutate},
		UnbalancedHold,
	};
	use pallet_assets::BalanceOnHold;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let asset_id = 8u32;
		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			asset_id.into(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner),
			asset_id.into(),
			beneficiary.clone().into(),
			1_000,
		));

		let hold_reason = crate::RuntimeHoldReason::TransactionStorage(
			pallet_orbis_transaction_storage::HoldReason::StorageFeeHold,
		);
		assert_ok!(AssetsHolder::set_balance_on_hold(asset_id, &hold_reason, &beneficiary, 400,));
		assert_eq!(AssetsHolder::balance_on_hold(asset_id, &beneficiary), Some(400));

		let freeze_reason =
			crate::RuntimeFreezeReason::Revive(pallet_revive::FreezeReason::PGasMinBalance);
		assert_ok!(AssetsFreezer::set_freeze(asset_id, &freeze_reason, &beneficiary, 700,));
		assert_eq!(AssetsFreezer::balance_frozen(asset_id, &freeze_reason, &beneficiary), 700);
	});
}

#[test]
fn foreign_and_pool_assets_are_native_and_sudo_administered() {
	use frame_support::traits::tokens::fungibles::freeze::Mutate;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let foreign_id = Location::parent();

		assert_noop!(
			ForeignAssets::create(
				RuntimeOrigin::signed(owner.clone()),
				foreign_id.clone(),
				owner.clone().into(),
				1,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(ForeignAssets::force_create(
			RuntimeOrigin::root(),
			foreign_id.clone(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(ForeignAssets::mint(
			RuntimeOrigin::signed(owner.clone()),
			foreign_id.clone(),
			beneficiary.clone().into(),
			500,
		));
		let freeze_reason =
			crate::RuntimeFreezeReason::Revive(pallet_revive::FreezeReason::PGasMinBalance);
		assert_ok!(
			ForeignAssetsFreezer::set_freeze(foreign_id, &freeze_reason, &beneficiary, 300,)
		);

		assert_ok!(PoolAssets::force_create(
			RuntimeOrigin::root(),
			7,
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(PoolAssets::mint(
			RuntimeOrigin::signed(owner),
			7,
			beneficiary.clone().into(),
			1_000,
		));
		assert_ok!(PoolAssetsFreezer::set_freeze(7, &freeze_reason, &beneficiary, 600,));
		assert_eq!(PoolAssets::balance(7, beneficiary), 1_000);
	});
}

#[test]
fn native_unique_and_nft_collections_mint_items() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		<Balances as Mutate<AccountId>>::set_balance(&owner, 1_000_000_000_000_000);

		assert_ok!(
			Uniques::create(RuntimeOrigin::signed(owner.clone()), 10, owner.clone().into(),)
		);
		assert_ok!(Uniques::mint(
			RuntimeOrigin::signed(owner.clone()),
			10,
			1,
			beneficiary.clone().into(),
		));
		assert_eq!(Uniques::owner(10, 1), Some(beneficiary.clone()));

		let config = pallet_nfts::CollectionConfig {
			settings: pallet_nfts::CollectionSettings::all_enabled(),
			max_supply: None,
			mint_settings: Default::default(),
		};
		assert_ok!(Nfts::create(
			RuntimeOrigin::signed(owner.clone()),
			owner.clone().into(),
			config,
		));
		assert_ok!(Nfts::mint(
			RuntimeOrigin::signed(owner),
			0,
			1,
			beneficiary.clone().into(),
			None,
		));
		assert_eq!(Nfts::owner(0, 1), Some(beneficiary));
	});
}

#[test]
fn asset_rates_are_location_based_and_sudo_administered() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let asset = Location::new(0, [PalletInstance(80), GeneralIndex(7)]);
		let initial = sp_runtime::FixedU128::from_rational(3, 2);
		let updated = sp_runtime::FixedU128::from_u32(2);

		assert_noop!(
			AssetRate::create(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				Box::new(asset.clone()),
				initial,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(AssetRate::create(RuntimeOrigin::root(), Box::new(asset.clone()), initial));
		assert_eq!(
			pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset),
			Some(initial)
		);
		assert_ok!(AssetRate::update(RuntimeOrigin::root(), Box::new(asset.clone()), updated));
		assert_eq!(
			pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset),
			Some(updated)
		);
		assert_ok!(AssetRate::remove(RuntimeOrigin::root(), Box::new(asset.clone())));
		assert!(pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset).is_none());
	});
}

#[test]
fn native_asset_conversion_pool_supports_liquidity_and_swaps() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let receiver = AccountId::from([2u8; 32]);
		let native = Location::parent();
		let local = Location::new(0, [PalletInstance(80), GeneralIndex(21)]);

		<Balances as Mutate<AccountId>>::set_balance(&owner, 10_000 * crate::UNITS);
		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			21u32.into(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner.clone()),
			21u32.into(),
			owner.clone().into(),
			10_000,
		));
		assert_ok!(AssetConversion::create_pool(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(native.clone()),
			Box::new(local.clone()),
		));
		assert_ok!(AssetConversion::add_liquidity(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(native.clone()),
			Box::new(local.clone()),
			1_000 * crate::UNITS,
			1_000,
			1,
			1,
			owner.clone(),
		));
		assert_ok!(AssetConversion::swap_exact_tokens_for_tokens(
			RuntimeOrigin::signed(owner),
			vec![Box::new(native), Box::new(local)],
			10 * crate::UNITS,
			1,
			receiver.clone(),
			true,
		));
		assert!(Assets::balance(21, receiver) > 0);
	});
}

#[test]
fn asset_fee_selector_is_preserved_inside_the_feeless_envelope() {
	type AssetCharge = pallet_asset_conversion_tx_payment::ChargeAssetTxPayment<Runtime>;
	type WrappedCharge = pallet_origin_feeless::ChargeOrSkipFeeless<Runtime, AssetCharge>;

	let asset = Location::new(0, [PalletInstance(80), GeneralIndex(21)]);
	let wrapped = WrappedCharge::from(AssetCharge::from(0, Some(asset)));
	let encoded = wrapped.encode();
	let decoded =
		WrappedCharge::decode(&mut encoded.as_slice()).expect("asset fee extension decodes");
	assert_eq!(decoded, wrapped);
}

#[test]
fn people_chunk_hashes_are_initialized_by_sudo_only() {
	use indiv_pallet_chunks_manager::{ChunkPageHashes, RingExponent};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let hashes: frame_support::BoundedVec<
			[u8; 32],
			frame_support::traits::ConstU32<{ indiv_pallet_chunks_manager::MAX_PAGE_COUNT }>,
		> = vec![[1u8; 32], [2u8; 32]].try_into().expect("two hashes are bounded");
		assert_noop!(
			ChunksManager::set_chunk_page_hashes(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				RingExponent::R2e9,
				hashes.clone(),
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			RingExponent::R2e9,
			hashes,
		));
		assert_eq!(ChunkPageHashes::<Runtime>::get(RingExponent::R2e9, 0), Some([1u8; 32]));
		assert_eq!(ChunkPageHashes::<Runtime>::get(RingExponent::R2e9, 1), Some([2u8; 32]));
	});
}

#[test]
fn people_membership_collections_are_native_and_sudo_managed() {
	use indiv_pallet_members::{Collections, OnboardingSize};
	use indiv_support::traits::{AppendOnlyMembers, RingExponent, RingMode};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let identifier = [7u8; 32];
		assert_ok!(<Members as AppendOnlyMembers>::create_collection(
			Location::here(),
			&identifier,
			5,
			RingMode::AppendOnly,
			RingExponent::R2e9,
			None,
		));
		assert!(Collections::<Runtime>::contains_key(identifier));
		assert_noop!(
			Members::set_onboarding_size(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				identifier,
				10,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(Members::set_onboarding_size(RuntimeOrigin::root(), identifier, 10));
		assert_eq!(OnboardingSize::<Runtime>::get(identifier), 10);
	});
}

#[test]
fn ring_root_changes_are_queued_and_subscriptions_are_sudo_managed() {
	use indiv_pallet_members_notifier::{PageState, PendingUpdates, Subscribers};
	use indiv_support::traits::{OnRingRootChange, RingExponent, RingRootOp};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		pallet_timestamp::Now::<Runtime>::put(1_000);
		let identifier = [8u8; 32];
		<MembersNotifier as OnRingRootChange<
			indiv_pallet_members_notifier::MembersOf<Runtime>,
		>>::on_ring_root_change(identifier, 3, RingRootOp::Deleted);
		assert!(PendingUpdates::<Runtime>::contains_key((
			PageState::<Runtime>::get().write_page,
			identifier,
			3,
		)));

		let collections: frame_support::BoundedVec<
			([u8; 32], RingExponent),
			frame_support::traits::ConstU32<3>,
		> = vec![(identifier, RingExponent::R2e9)].try_into().unwrap();
		assert_noop!(
			MembersNotifier::subscribe(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				ParaId::from(2_000u32),
				collections.clone(),
				60,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(MembersNotifier::subscribe(
			RuntimeOrigin::root(),
			ParaId::from(2_000u32),
			collections,
			60,
		));
		assert!(Subscribers::<Runtime>::contains_key(ParaId::from(2_000u32)));
	});
}

#[test]
fn people_lite_initializes_native_membership_and_uses_sudo_allowances() {
	use indiv_pallet_people_lite::{
		AttestationAllowance, LitePeopleCollectionCreated, LITE_PEOPLE_MEMBER_IDENTIFIER,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let verifier = AccountId::from([3u8; 32]);
		let mut meter = frame_support::weights::WeightMeter::with_limit(
			crate::RuntimeBlockWeights::get().max_block,
		);
		PeopleLite::on_poll(1, &mut meter);
		assert!(LitePeopleCollectionCreated::<Runtime>::get());
		assert!(indiv_pallet_members::Collections::<Runtime>::contains_key(
			LITE_PEOPLE_MEMBER_IDENTIFIER,
		));

		assert_noop!(
			PeopleLite::increase_attestation_allowance(
				RuntimeOrigin::signed(verifier.clone()),
				verifier.clone(),
				5,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(PeopleLite::increase_attestation_allowance(
			RuntimeOrigin::root(),
			verifier.clone(),
			5,
		));
		assert_eq!(AttestationAllowance::<Runtime>::get(&verifier), 5);
		assert_ok!(PeopleLite::clear_attestation_allowance(
			RuntimeOrigin::root(),
			verifier.clone(),
		));
		assert_eq!(AttestationAllowance::<Runtime>::get(&verifier), 0);
	});
}

#[test]
fn full_personhood_collection_and_recognition_are_native_and_sudo_managed() {
	use indiv_pallet_people::{PeopleCollectionCreated, PEOPLE_MEMBER_IDENTIFIER};
	use verifiable::GenerateVerifiable;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		pallet_timestamp::Now::<Runtime>::put(1_000);
		assert_ok!(Personhood::create_people_collection(
			frame_system::Origin::<Runtime>::Authorized.into(),
		));
		assert!(PeopleCollectionCreated::<Runtime>::get());
		assert!(indiv_pallet_members::Collections::<Runtime>::contains_key(
			PEOPLE_MEMBER_IDENTIFIER,
		));

		let secret = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([9; 32]);
		let member =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(&secret);
		assert_noop!(
			Personhood::force_recognize_personhood(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				vec![member.clone()],
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(Personhood::force_recognize_personhood(RuntimeOrigin::root(), vec![member],));
		assert!(indiv_pallet_people::People::<Runtime>::contains_key(0));
	});
}

#[test]
fn revive_uses_reserved_orbis_evm_chain_id() {
	assert_eq!(<<Runtime as pallet_revive::Config>::ChainId as Get<u64>>::get(), 420_001_006);
	assert!(<<Runtime as pallet_revive::Config>::AllowEVMBytecode as Get<bool>>::get());
	assert_eq!(<Assets as PalletInfoAccess>::index(), 80);
	assert_eq!(<AssetsFreezer as PalletInfoAccess>::index(), 81);
	assert_eq!(<AssetsHolder as PalletInfoAccess>::index(), 82);
	assert_eq!(<ForeignAssets as PalletInfoAccess>::index(), 83);
	assert_eq!(<PoolAssets as PalletInfoAccess>::index(), 84);
	assert_eq!(<ForeignAssetsFreezer as PalletInfoAccess>::index(), 85);
	assert_eq!(<PoolAssetsFreezer as PalletInfoAccess>::index(), 86);
	assert_eq!(<Uniques as PalletInfoAccess>::index(), 87);
	assert_eq!(<Nfts as PalletInfoAccess>::index(), 88);
	assert_eq!(<AssetRate as PalletInfoAccess>::index(), 89);
	assert_eq!(<AssetConversion as PalletInfoAccess>::index(), 200);
	assert_eq!(<AssetTxPayment as PalletInfoAccess>::index(), 201);
	assert_eq!(<Revive as PalletInfoAccess>::index(), 100);
	// The SDK relay Coretime pallet encodes callbacks to Broker at index 50.
	assert_eq!(<Broker as PalletInfoAccess>::index(), 50);
	assert_eq!(<Entity as PalletInfoAccess>::index(), 53);
	assert_eq!(<People as PalletInfoAccess>::index(), 90);
	assert_eq!(<TransactionStorage as PalletInfoAccess>::index(), 110);
	assert_eq!(<crate::HopPromotion as PalletInfoAccess>::index(), 111);
	assert_eq!(<crate::WeightReclaim as PalletInfoAccess>::index(), 4);
	assert_eq!(<ChunksManager as PalletInfoAccess>::index(), 91);
	assert_eq!(<Members as PalletInfoAccess>::index(), 92);
	assert_eq!(<MembersNotifier as PalletInfoAccess>::index(), 93);
	assert_eq!(<PeopleLite as PalletInfoAccess>::index(), 94);
	assert_eq!(<Personhood as PalletInfoAccess>::index(), 95);
	assert_eq!(<<Runtime as pallet_broker::Config>::MaxReservedCores as Get<u32>>::get(), 50);
}

#[test]
fn commons_drive_name_bound_is_exactly_256_bytes() {
	assert_eq!(crate::DriveMaxNameBytes::get(), 256);
	for length in [255usize, 256] {
		let name = pallet_orbis_drive::DriveNameOf::<Runtime>::try_from(vec![b'a'; length])
			.expect("255 and 256 byte Drive names are runtime-admitted");
		assert_ok!(Drive::validate_name(&name));
	}
	assert!(pallet_orbis_drive::DriveNameOf::<Runtime>::try_from(vec![b'a'; 257]).is_err());
}

pub(crate) struct CanonicalAdmission {
	pub provider_commitment: [u8; 32],
	pub bucket_id: sp_core::H256,
	pub primary: AccountId,
	pub replicas: Vec<AccountId>,
	pub organization_attestations: Vec<sp_core::H256>,
}

pub(crate) fn admit_canonical_manifest(
	owner: &AccountId,
	manifest: [u8; 32],
) -> CanonicalAdmission {
	use pallet_orbis_storage_provider::{
		CommitmentPayloadV2, CommitmentV1, MmrLeafV1, MmrProofV1, ProviderOrganizationRefV1,
		ReplicaSignature,
	};
	use sp_core::{ed25519, Pair};
	use sp_runtime::traits::{BlakeTwo256, Hash as HashT};
	if pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::get().is_none() {
		assert_ok!(crate::StorageProvider::advance_finalized_checkpoint(
			RuntimeOrigin::root(),
			System::block_number(),
		));
	}

	let definition: pallet_orbis_attestation::SchemaDefinitionOf<Runtime> =
		b"provider-sla-v1".to_vec().try_into().unwrap();
	let definition_commitment = BlakeTwo256::hash(definition.as_slice());
	let issuers: pallet_orbis_attestation::AuthorizedIssuersOf<Runtime> =
		vec![owner.clone()].try_into().unwrap();
	assert_ok!(Attestation::create_schema(
		RuntimeOrigin::signed(owner.clone()),
		definition,
		issuers,
		true,
		false,
		pallet_orbis_attestation::IndexPolicy::None,
	));
	let schema = Attestation::schema_id(
		owner,
		&definition_commitment,
		true,
		false,
		pallet_orbis_attestation::IndexPolicy::None,
	);
	let sla_commitment = sp_core::H256::repeat_byte(0x51);
	let provider_pairs = [10u8, 11, 12].map(|seed| ed25519::Pair::from_seed(&[seed; 32]));
	let mut provider_accounts = Vec::new();
	let mut organization_attestations = Vec::new();
	for (index, pair) in provider_pairs.iter().enumerate() {
		System::set_extrinsic_index(10 + index as u32);
		let provider = AccountId::from(pair.public().0);
		let mut entity_info = pallet_origin_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		let mut provider_display = b"Storage provider ".to_vec();
		provider_display.push(b'0' + index as u8);
		entity_info.display = origin_primitives::Element::Raw(provider_display.try_into().unwrap());
		assert_ok!(Entity::set_info(
			RuntimeOrigin::signed(provider.clone()),
			Box::new(entity_info),
		));
		let entity_id = pallet_origin_entity::EntityTokenOfAccount::<Runtime>::get(&provider)
			.expect("provider Entity exists");
		let input = pallet_orbis_attestation::AttestationInput::<Runtime> {
			schema,
			subject_commitment: BlakeTwo256::hash_of(&(entity_id.clone(), pair.public())),
			payload_commitment: sla_commitment,
			status_commitment: sp_core::H256::repeat_byte(1),
			parent: None,
			expiry: Some(1_000),
			uniqueness_commitment: None,
			revocable: true,
		};
		let nonce = pallet_orbis_attestation::NextIssuerAttestationNonce::<Runtime>::get(owner);
		let attestation = Attestation::attestation_id(owner, &input, nonce);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), input));
		organization_attestations.push(attestation);
		let organization = ProviderOrganizationRefV1 {
			entity_id: entity_id.as_ref().to_vec().try_into().unwrap(),
			attestation_id: attestation,
			schema_id: schema,
			sla_commitment,
			sla_version: 1,
			valid_from: 1,
			valid_until: 1_000,
			rotation_predecessor: None,
		};
		assert_ok!(crate::StorageProvider::register_provider(
			RuntimeOrigin::root(),
			provider.clone(),
			pair.public().0.to_vec().try_into().unwrap(),
			pair.public(),
			organization,
			1_000_000,
		));
		provider_accounts.push(provider);
	}
	let replicas = vec![provider_accounts[1].clone(), provider_accounts[2].clone()]
		.try_into()
		.unwrap();
	assert_ok!(crate::StorageProvider::create_bucket(
		RuntimeOrigin::signed(owner.clone()),
		sp_core::H256::repeat_byte(0x33),
		provider_accounts[0].clone(),
		replicas,
	));
	let bucket_id = pallet_orbis_storage_provider::BucketIds::<Runtime>::get()[0];
	let provider_commitment = [0xA5; 32];
	let leaf = MmrLeafV1 {
		data_root: sp_core::H256::from(provider_commitment),
		data_size: 1,
		total_size: 1,
	};
	let root = BlakeTwo256::hash_of(&leaf);
	System::set_block_number(101);
	assert_ok!(crate::StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 101,));
	assert_ok!(crate::StorageProvider::refresh_bucket_authority(
		RuntimeOrigin::signed(owner.clone()),
		bucket_id,
	));
	let payload = CommitmentPayloadV2 {
		version: 2,
		bucket_id,
		commitment: CommitmentV1 { mmr_root: root, start_seq: 0, leaf_count: 1 },
		nonce: 101,
	};
	let mut bytes = b"cord/storage/checkpoint/v2".to_vec();
	payload.encode_to(&mut bytes);
	let digest = sp_io::hashing::blake2_256(&bytes);
	let context = crate::StorageProvider::checkpoint_context_for(&payload).unwrap();
	let context_digest = crate::StorageProvider::checkpoint_context_digest(&context);
	let mut confirmations = vec![1usize, 2]
		.into_iter()
		.map(|index| ReplicaSignature {
			provider: provider_accounts[index].clone(),
			service_key: provider_pairs[index].public(),
			signature: provider_pairs[index].sign(&digest),
			context_signature: provider_pairs[index].sign(&context_digest),
		})
		.collect::<Vec<_>>();
	confirmations.sort_by(|left, right| left.provider.encode().cmp(&right.provider.encode()));
	let confirmations = confirmations.try_into().unwrap();
	assert_ok!(crate::StorageProvider::submit_checkpoint(
		RuntimeOrigin::signed(provider_accounts[0].clone()),
		b"cord/storage/checkpoint/v2".to_vec().try_into().unwrap(),
		payload,
		101,
		121,
		provider_pairs[0].public(),
		provider_pairs[0].sign(&digest),
		provider_pairs[0].sign(&context_digest),
		confirmations,
	));
	assert_ok!(crate::StorageProvider::register_manifest(
		RuntimeOrigin::signed(owner.clone()),
		bucket_id,
		1,
		manifest,
	));
	assert_ok!(crate::StorageProvider::publish_manifest(
		RuntimeOrigin::signed(provider_accounts[0].clone()),
		manifest,
		0,
		MmrProofV1 { peaks: vec![root], leaf, leaf_proof: Vec::new() },
	));
	CanonicalAdmission {
		provider_commitment,
		bucket_id,
		primary: provider_accounts[0].clone(),
		replicas: provider_accounts[1..].to_vec(),
		organization_attestations,
	}
}

#[test]
fn s3_runtime_api_uses_snapshot_cursor_raw_order_and_hides_tombstones() {
	use orbis_storage_runtime_api::runtime_decl_for_s3_registry_api::S3RegistryApi;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let owner = pallet_revive::test_utils::ALICE;
		let name: pallet_orbis_s3::BucketNameOf<Runtime> =
			b"cursor-api".to_vec().try_into().unwrap();
		let bucket = S3::bucket_id(&owner, &name);
		assert_ok!(S3::create_bucket(RuntimeOrigin::signed(owner.clone()), name));
		for (key, manifest) in [(b"z".as_slice(), [1; 32]), (b"A", [2; 32]), (b"a", [3; 32])] {
			pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::insert(
				manifest,
				pallet_orbis_storage_provider::CanonicalManifestRecord {
					bucket_id: bucket,
					provider_commitment: Some(manifest),
					state: pallet_orbis_storage_control_primitives::CommitmentState::Publishable,
					checkpoint: Some(1),
					tombstoned_at: None,
				},
			);
			assert_ok!(S3::put_object(
				RuntimeOrigin::signed(owner.clone()),
				bucket,
				key.to_vec().try_into().unwrap(),
				manifest,
				manifest,
				Default::default(),
				Default::default(),
				manifest,
				None,
				None,
			));
		}
		let page = Runtime::object_keys(bucket, None, None, 2).unwrap();
		assert_eq!(page.items, vec![b"A".to_vec(), b"a".to_vec()]);
		let cursor = page.next_cursor.expect("two-item page has a continuation");
		let added = [4; 32];
		pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::insert(
			added,
			pallet_orbis_storage_provider::CanonicalManifestRecord {
				bucket_id: bucket,
				provider_commitment: Some(added),
				state: pallet_orbis_storage_control_primitives::CommitmentState::Publishable,
				checkpoint: Some(1),
				tombstoned_at: None,
			},
		);
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(owner.clone()),
			bucket,
			b"b".to_vec().try_into().unwrap(),
			added,
			added,
			Default::default(),
			Default::default(),
			added,
			None,
			None,
		));
		assert_eq!(
			Runtime::object_keys(bucket, None, Some(cursor), 2),
			Err(orbis_storage_runtime_api::S3ListError::CursorStale)
		);
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(owner),
			bucket,
			b"A".to_vec().try_into().unwrap(),
			[9; 32],
			Some([2; 32]),
			1,
		));
		assert!(Runtime::object(bucket, b"A".to_vec()).value.is_none());
		let page = Runtime::object_keys(bucket, None, None, 100).unwrap();
		assert_eq!(page.items, vec![b"a".to_vec(), b"b".to_vec(), b"z".to_vec()]);
	});
}

#[test]
fn native_identity_attestation_name_asset_and_storage_journey() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let owner = pallet_revive::test_utils::ALICE;
		let recipient = pallet_revive::test_utils::BOB;
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);
		<Balances as Mutate<AccountId>>::set_balance(&recipient, crate::ExistentialDeposit::get());

		let mut identity = pallet_orbis_people::identity_info::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		identity.display =
			pallet_orbis_people::Data::Raw(b"Alice Orbis".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(RuntimeOrigin::signed(owner.clone()), Box::new(identity),));
		assert!(People::has_identity(&owner, 1));
		let mut entity_info = pallet_origin_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		entity_info.display =
			origin_primitives::Element::Raw(b"Alice Orbis".to_vec().try_into().unwrap());
		assert_ok!(Entity::set_info(RuntimeOrigin::signed(owner.clone()), Box::new(entity_info),));
		let subject_id = pallet_origin_entity::EntityTokenOfAccount::<Runtime>::get(&owner)
			.expect("Entity is the canonical SubjectId authority");
		let identity_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(subject_id.as_ref()));

		let asset_id = 7u32;
		assert_ok!(Assets::create(
			RuntimeOrigin::signed(owner.clone()),
			asset_id.into(),
			owner.clone().into(),
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner.clone()),
			asset_id.into(),
			owner.clone().into(),
			100,
		));
		assert_ok!(Assets::transfer(
			RuntimeOrigin::signed(owner.clone()),
			asset_id.into(),
			recipient.clone().into(),
			40,
		));
		assert_eq!(Assets::balance(asset_id, &owner), 60);
		assert_eq!(Assets::balance(asset_id, &recipient), 40);

		let audit = sp_io::hashing::blake2_256(b"alice:native-identity-asset-transfer:40");
		let admission = admit_canonical_manifest(&owner, audit);
		let provider_commitment = admission.provider_commitment;

		let definition: pallet_orbis_attestation::SchemaDefinitionOf<Runtime> =
			b"festival-pass-v1".to_vec().try_into().unwrap();
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
			subject_commitment: identity_commitment,
			payload_commitment: sp_core::H256::from(audit),
			status_commitment: sp_core::H256::from_low_u64_be(1),
			parent: None,
			expiry: Some(1_000),
			uniqueness_commitment: Some(sp_core::H256::from_low_u64_be(7)),
			revocable: true,
		};
		let attestation = Attestation::attestation_id(
			&owner,
			&input,
			pallet_orbis_attestation::NextIssuerAttestationNonce::<Runtime>::get(&owner),
		);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), input));
		assert!(Attestation::is_live(attestation));

		let label = Names::validate_label(b"alice".to_vec()).unwrap();
		let salt: pallet_orbis_names::SaltOf<Runtime> = b"festival".to_vec().try_into().unwrap();
		let commitment = Names::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Names::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(103);
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
		assert_ok!(Names::set_content(RuntimeOrigin::signed(owner.clone()), name, Some(audit),));

		let drive_name: pallet_orbis_drive::DriveNameOf<Runtime> =
			b"festival".to_vec().try_into().unwrap();
		assert_ok!(Drive::create_drive(RuntimeOrigin::signed(owner.clone()), drive_name));
		let drive_id = pallet_orbis_drive::OwnerDrives::<Runtime>::get(&owner)[0];
		assert_ok!(Drive::update_root(
			RuntimeOrigin::signed(owner.clone()),
			drive_id,
			1,
			None,
			audit,
			provider_commitment,
		));
		let drive = pallet_orbis_drive::Drives::<Runtime>::get(drive_id).unwrap();
		assert_eq!(drive.root_manifest, Some(audit));
		assert_eq!(drive.root_provider_commitment, Some(provider_commitment));

		let bucket_name: pallet_orbis_s3::BucketNameOf<Runtime> =
			b"festival-audit".to_vec().try_into().unwrap();
		let bucket = S3::bucket_id(&owner, &bucket_name);
		assert_ok!(S3::create_bucket(RuntimeOrigin::signed(owner.clone()), bucket_name));
		let key: pallet_orbis_s3::ObjectKeyOf<Runtime> =
			b"audit/transfer".to_vec().try_into().unwrap();
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(owner.clone()),
			bucket,
			key.clone(),
			audit,
			provider_commitment,
			Default::default(),
			Default::default(),
			[0x5A; 32],
			None,
			None,
		));
		assert_eq!(
			pallet_orbis_s3::Objects::<Runtime>::get(bucket, key).unwrap().content_hash,
			Some(audit)
		);
	});
}

#[test]
fn names_subjects_follow_entity_authority_not_opaque_attestation_subjects() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let owner = pallet_revive::test_utils::ALICE;
		let mut entity_info = pallet_origin_entity::entity::EntityInfo::<
			crate::entity::MaxRawDataLength,
			crate::entity::MaxAdditionalAttributes,
		>::default();
		entity_info.display =
			origin_primitives::Element::Raw(b"Canonical subject".to_vec().try_into().unwrap());
		assert_ok!(Entity::set_info(RuntimeOrigin::signed(owner.clone()), Box::new(entity_info),));
		let identity_subject = pallet_origin_entity::EntityTokenOfAccount::<Runtime>::get(&owner)
			.expect("identity subject exists without an attestation");

		let label = Names::validate_label(b"identity".to_vec()).unwrap();
		let salt: pallet_orbis_names::SaltOf<Runtime> = b"subject".to_vec().try_into().unwrap();
		let commitment = Names::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Names::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(3);
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
			Some(identity_subject),
		));

		let opaque_only =
			origin_primitives::identifier::Ss58Identifier::to_encoded([7u8; 32], 1006, 53, 1)
				.unwrap();
		let opaque_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(opaque_only.as_ref()));
		pallet_orbis_attestation::KnownSubjects::<Runtime>::insert(opaque_commitment, ());
		assert_noop!(
			Names::set_subject(RuntimeOrigin::signed(owner), name, Some(opaque_only),),
			pallet_orbis_names::Error::<Runtime>::InvalidSubjectReference
		);
	});
}

#[test]
fn names_attestation_resolution_fails_closed_after_revocation_and_expiry() {
	use pallet_orbis_names_runtime_api::runtime_decl_for_names_api::NamesApiV1;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let owner = pallet_revive::test_utils::ALICE;
		let definition: pallet_orbis_attestation::SchemaDefinitionOf<Runtime> =
			b"names-live-link-v1".to_vec().try_into().unwrap();
		let definition_commitment =
			sp_core::H256::from(sp_io::hashing::blake2_256(definition.as_slice()));
		let issuers: pallet_orbis_attestation::AuthorizedIssuersOf<Runtime> =
			vec![owner.clone()].try_into().unwrap();
		assert_ok!(Attestation::create_schema(
			RuntimeOrigin::signed(owner.clone()),
			definition,
			issuers,
			true,
			false,
			pallet_orbis_attestation::IndexPolicy::None,
		));
		let schema = Attestation::schema_id(
			&owner,
			&definition_commitment,
			true,
			false,
			pallet_orbis_attestation::IndexPolicy::None,
		);
		let input = |nonce: u64, expiry| pallet_orbis_attestation::AttestationInput::<Runtime> {
			schema,
			subject_commitment: sp_core::H256::from_low_u64_be(1),
			payload_commitment: sp_core::H256::from_low_u64_be(nonce),
			status_commitment: sp_core::H256::from_low_u64_be(nonce + 10),
			parent: None,
			expiry,
			uniqueness_commitment: None,
			revocable: true,
		};
		let first_input = input(1, None);
		let first = Attestation::attestation_id(&owner, &first_input, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), first_input));

		let label = Names::validate_label(b"live-link".to_vec()).unwrap();
		let salt: pallet_orbis_names::SaltOf<Runtime> = b"link-salt".to_vec().try_into().unwrap();
		let commitment = Names::registration_commitment(&owner, None, &label, &salt);
		assert_ok!(Names::commit(RuntimeOrigin::signed(owner.clone()), commitment));
		System::set_block_number(3);
		assert_ok!(Names::register(
			RuntimeOrigin::signed(owner.clone()),
			None,
			label.clone(),
			salt,
		));
		let name = Names::derive_name_id(None, &label);
		assert_ok!(
			Names::set_attestation(RuntimeOrigin::signed(owner.clone()), name, Some(first),)
		);
		assert_eq!(Runtime::resolve_attestation(name).value, Some(first));

		assert_ok!(Attestation::revoke(RuntimeOrigin::signed(owner.clone()), first));
		assert_eq!(Runtime::resolve_attestation(name).value, None);

		let expiring_input = input(2, Some(5));
		let expiring = Attestation::attestation_id(&owner, &expiring_input, 1);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(owner.clone()), expiring_input));
		assert_ok!(Names::set_attestation(RuntimeOrigin::signed(owner), name, Some(expiring),));
		assert_eq!(Runtime::resolve_attestation(name).value, Some(expiring));
		System::set_block_number(5);
		assert_eq!(Runtime::resolve_attestation(name).value, None);
	});
}

#[test]
fn people_identity_is_self_claimed_and_sudo_attested() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::from(ALICE);
		let registrar = AccountId::from([3u8; 32]);
		let mut info = pallet_orbis_people::identity_info::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		info.display = pallet_orbis_people::Data::Raw(b"Alice".to_vec().try_into().unwrap());

		assert_ok!(People::set_identity(RuntimeOrigin::signed(account.clone()), Box::new(info),));
		assert!(People::has_identity(&account, 1));

		assert_noop!(
			People::add_registrar(RuntimeOrigin::signed(account), registrar.clone().into()),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(People::add_registrar(RuntimeOrigin::root(), registrar.into()));
	});
}

#[test]
fn identity_personhood_runtime_api_returns_bounded_status_without_private_identity_data() {
	use crate::identity_personhood_api::runtime_decl_for_identity_personhood_api::IdentityPersonhoodApiV1;
	use sp_runtime::traits::{BlakeTwo256, Hash};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::from(ALICE);
		let registrar = AccountId::from([3u8; 32]);
		let empty = Runtime::identity_status(account.clone());
		assert_eq!(empty.version, crate::identity_personhood_api::RESPONSE_VERSION);
		assert!(!empty.value.registered);

		let mut info = pallet_orbis_people::identity_info::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		info.display = pallet_orbis_people::Data::Raw(b"Alice".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(
			RuntimeOrigin::signed(account.clone()),
			Box::new(info.clone()),
		));
		assert_ok!(People::add_registrar(RuntimeOrigin::root(), registrar.clone().into()));
		assert_ok!(People::provide_judgement(
			RuntimeOrigin::signed(registrar),
			account.clone().into(),
			pallet_orbis_people::Judgement::KnownGood,
			BlakeTwo256::hash_of(&info),
		));
		let identity = Runtime::identity_status(account.clone()).value;
		assert!(identity.registered);
		assert_eq!(identity.judgement_count, 1);
		assert_eq!(identity.known_good, 1);

		let personhood = Runtime::personhood_status(account.clone()).value;
		assert_eq!(personhood.full_personal_id, None);
		assert!(!personhood.full_recognized);
		assert!(!personhood.lite_recognized);
		assert_ok!(PeopleLite::increase_attestation_allowance(
			RuntimeOrigin::root(),
			account.clone(),
			7,
		));
		assert_eq!(Runtime::attestation_allowance(account).value.remaining, 7);
	});
}

#[test]
fn orbis_storage_is_authorized_indexed_and_content_addressed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let account = AccountId::from(ALICE);
		let data = b"identity-bound audit record".to_vec();

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			account.clone(),
			2,
			1024,
		));
		let authorization = TransactionStorage::account_authorization(account.clone()).unwrap();
		assert_eq!(authorization.transactions_allowance, 2);
		assert_eq!(authorization.bytes_allowance, 1024);
		assert!(TransactionStorage::can_store(&account, data.len() as u32));

		assert_ok!(TransactionStorage::store(RuntimeOrigin::root(), data.clone()));
		let content_hash = sp_io::hashing::blake2_256(&data);
		assert!(TransactionStorage::contains_transaction(content_hash));
		<TransactionStorage as Hooks<u32>>::on_finalize(1);
		let indexed = TransactionStorage::transactions_at(1).unwrap();
		assert_eq!(indexed.len(), 1);
		assert_eq!(indexed[0].content_hash, content_hash);
	});
}

#[test]
fn hop_promotion_accepts_authorized_signed_submit_intent() {
	use frame_support::traits::BuildGenesisConfig;
	use sp_core::{sr25519, Pair};
	use sp_runtime::{traits::IdentifyAccount, MultiSignature, MultiSigner};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let now = 1_750_000_000_000u64;
		pallet_timestamp::Now::<Runtime>::put(now);

		let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let signer = MultiSigner::from(pair.public());
		let account = signer.clone().into_account();
		<Balances as Mutate<AccountId>>::set_balance(&account, 1_000_000_000_000);
		assert_ok!(
			TransactionStorage::authorize_account(RuntimeOrigin::root(), account, 1, 1_024,)
		);

		let data = b"orbis hop promotion".to_vec();
		let hash = sp_io::hashing::blake2_256(&data);
		let payload = pallet_orbis_hop_promotion::signing_payload(&hash, now);
		let signature = MultiSignature::Sr25519(pair.sign(&payload));
		assert!(HopPromotion::authorize_promote(
			sp_runtime::transaction_validity::TransactionSource::Local,
			&signer,
			&signature,
			&now,
			&data,
		)
		.is_ok());
		assert!(!HopPromotion::is_promoted_on_chain(hash));
	});
}

#[test]
fn authorized_pipeline_retains_validation_and_explicitly_skips_payment_and_quota() {
	use frame_support::{dispatch::GetDispatchInfo, traits::BuildGenesisConfig};
	use sp_core::{sr25519, Pair};
	use sp_runtime::{
		traits::{AsTransactionAuthorizedOrigin, IdentifyAccount, TransactionExtension},
		MultiSignature, MultiSigner,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let now = 1_750_000_000_000u64;
		pallet_timestamp::Now::<Runtime>::put(now);

		let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let signer = MultiSigner::from(pair.public());
		let account = signer.clone().into_account();
		let initial_balance = 1_000_000_000_000;
		<Balances as Mutate<AccountId>>::set_balance(&account, initial_balance);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			account.clone(),
			1,
			1_024,
		));

		let data = b"authorized pipeline promotion".to_vec();
		let hash = sp_io::hashing::blake2_256(&data);
		let signature = MultiSignature::Sr25519(
			pair.sign(&pallet_orbis_hop_promotion::signing_payload(&hash, now)),
		);
		let call = RuntimeCall::HopPromotion(pallet_orbis_hop_promotion::Call::promote {
			signer: signer.clone(),
			signature: signature.clone(),
			submit_timestamp: now,
			data: data.clone(),
		});
		let encoded = <Runtime as frame_system::offchain::CreateAuthorizedTransaction<
			RuntimeCall,
		>>::create_authorized_transaction(call.clone())
		.encode();
		let decoded = crate::UncheckedExtrinsic::decode(&mut &encoded[..])
			.expect("authorized extrinsic round trips through its wire encoding");
		let decoded = decoded.0;
		assert_eq!(decoded.function, call);
		let extension = match decoded.preamble {
			sp_runtime::generic::Preamble::General(sp_runtime::traits::ExtensionVariant::V0(
				extension,
			)) => extension,
			_ => panic!("authorized calls use a version-zero general transaction"),
		};
		let call = decoded.function;
		let info = call.get_dispatch_info();
		let implicit = extension.implicit().unwrap();
		let (_, val, origin) = extension
			.validate(
				RuntimeOrigin::none(),
				&call,
				&info,
				call.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &call)),
				sp_runtime::transaction_validity::TransactionSource::Local,
			)
			.expect("the annotated call and its Orbis Storage authorization are valid");
		assert!(origin.is_transaction_authorized());
		let pre = extension
			.prepare(val, &origin, &call, &info, call.encoded_size())
			.expect("authorized preparation explicitly skips payment");
		assert_eq!(Balances::free_balance(&account), initial_balance);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&account), None);

		assert_ok!(HopPromotion::promote(origin, signer, signature, now, data,));
		assert!(TransactionStorage::contains_transaction(hash));
		assert_ok!(crate::TxExtensions::post_dispatch_details(
			pre,
			&info,
			&Default::default(),
			call.encoded_size(),
			&Ok(()),
		));
		assert_eq!(Balances::free_balance(&account), initial_balance);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&account), None);
	});
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn orbis_storage_mutations_are_rejected_when_wrapped_or_sent_by_xcm() {
	use codec::Encode;
	use frame_support::dispatch::GetDispatchInfo;
	use sp_runtime::traits::TransactionExtension;

	type XcmSafeCalls = <crate::xcm_config::XcmConfig as xcm_executor::Config>::SafeCallFilter;
	let store = RuntimeCall::TransactionStorage(pallet_orbis_transaction_storage::Call::store {
		data: b"audit".to_vec(),
	});
	assert!(crate::OrbisStorageCallInspector::contains(&store));
	assert!(!XcmSafeCalls::contains(&store));

	let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![store] });
	assert!(crate::OrbisStorageCallInspector::contains(&wrapped));
	assert!(!XcmSafeCalls::contains(&wrapped));
	let reserved_renew =
		RuntimeCall::TransactionStorage(pallet_orbis_transaction_storage::Call::renew_reserved {
			reservation_id: 7,
			content_hash: [9u8; 32],
		});
	assert!(crate::OrbisStorageCallInspector::contains(&reserved_renew));
	assert!(!XcmSafeCalls::contains(&reserved_renew));
	let wrapped_reserved =
		RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![reserved_renew] });
	assert!(crate::OrbisStorageCallInspector::contains(&wrapped_reserved));
	assert!(!XcmSafeCalls::contains(&wrapped_reserved));

	let reserved_store =
		RuntimeCall::TransactionStorage(pallet_orbis_transaction_storage::Call::store_reserved {
			reservation_id: 7,
			cid_config: orbis_transaction_storage_primitives::cids::CidConfig {
				codec: orbis_transaction_storage_primitives::cids::RAW_CODEC,
				hashing: orbis_transaction_storage_primitives::cids::HashingAlgorithm::Blake2b256,
			},
			data: b"reserved".to_vec(),
		});
	let proxy_any = RuntimeCall::Proxy(pallet_proxy::Call::proxy {
		real: AccountId::from([2u8; 32]).into(),
		force_proxy_type: Some(crate::ProxyType::Any),
		call: Box::new(reserved_store.clone()),
	});
	let multisig = RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
		other_signatories: vec![AccountId::from([2u8; 32])],
		call: Box::new(reserved_store.clone()),
	});
	let opaque_multisig = RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi {
		threshold: 2,
		other_signatories: vec![AccountId::from([2u8; 32])],
		maybe_timepoint: None,
		call_hash: [3u8; 32],
		max_weight: frame_support::weights::Weight::from_parts(1_000_000, 0),
	});
	let scheduled = RuntimeCall::Scheduler(pallet_scheduler::Call::schedule {
		when: 2,
		maybe_periodic: None,
		priority: 0,
		call: Box::new(reserved_store.clone()),
	});
	let revive = RuntimeCall::Revive(pallet_revive::Call::dispatch_as_fallback_account {
		call: Box::new(reserved_store.clone()),
	});
	let meta_extension: crate::MetaTxExtension = (
		pallet_verify_signature::VerifySignature::new_with_signature(
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
			AccountId::from(ALICE),
		),
		crate::meta_v6::ConsumePaidMetaIngress(crate::meta_v6::IntentPreimageV7 {
			domain: vec![],
			extension_version: 0,
			genesis_hash: Default::default(),
			spec_version: 0,
			transaction_version: 0,
			inner_signer: AccountId::from(ALICE),
			call_hash: Default::default(),
			mortality: sp_runtime::generic::Era::Immortal,
			nonce: 0,
			policy_proofs_hash: Default::default(),
			storage_extension_hash: Default::default(),
			metadata_extension_hash: Default::default(),
			metadata_implicit: None,
		}),
		pallet_meta_tx::MetaTxMarker::new(),
		frame_system::CheckNonZeroSender::new(),
		frame_system::CheckSpecVersion::new(),
		frame_system::CheckTxVersion::new(),
		frame_system::CheckGenesis::new(),
		frame_system::CheckMortality::from(sp_runtime::generic::Era::Immortal),
		frame_system::CheckNonce::from(0),
		(
			pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
			Default::default(),
			pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
		),
		Default::default(),
		frame_metadata_hash_extension::CheckMetadataHash::new(false),
	);
	let meta_tx = pallet_meta_tx::MetaTxFor::<Runtime>::new(reserved_store, 0, meta_extension);
	let meta_encoded_len = meta_tx.encoded_size() as u32;
	let meta = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
		meta_tx: Box::new(meta_tx),
		meta_tx_encoded_len: meta_encoded_len,
	});

	for (name, call) in [
		("proxy-any", proxy_any),
		("multisig", multisig),
		("multisig-opaque", opaque_multisig),
		("scheduler", scheduled),
		("meta-tx", meta),
		("revive", revive),
	] {
		assert!(crate::OrbisStorageCallInspector::contains(&call), "{name} bypassed inspection");
		assert!(!XcmSafeCalls::contains(&call), "{name} bypassed the XCM safe filter");
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let call = RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([2u8; 32]).into(),
			force_proxy_type: Some(crate::ProxyType::Any),
			call: Box::new(RuntimeCall::TransactionStorage(
				pallet_orbis_transaction_storage::Call::store_reserved {
					reservation_id: 7,
					cid_config: orbis_transaction_storage_primitives::cids::CidConfig {
						codec: orbis_transaction_storage_primitives::cids::RAW_CODEC,
						hashing:
							orbis_transaction_storage_primitives::cids::HashingAlgorithm::Blake2b256,
					},
					data: b"reserved".to_vec(),
				},
			)),
		});
		let extension = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let info = call.get_dispatch_info();
		let result = extension.validate(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			&call,
			&info,
			call.encoded_size(),
			(),
			&sp_runtime::traits::TxBaseImplication((0u8, &call)),
			sp_runtime::transaction_validity::TransactionSource::External,
		);
		assert_eq!(
			result.unwrap_err(),
			sp_runtime::transaction_validity::InvalidTransaction::Call.into()
		);
	});

	let ordinary = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
	assert!(!crate::OrbisStorageCallInspector::contains(&ordinary));
	assert!(XcmSafeCalls::contains(&ordinary));
}

fn full_core_task(task: u32) -> Schedule {
	Schedule::truncate_from(vec![ScheduleItem {
		mask: CoreMask::complete(),
		assignment: CoreAssignment::Task(task),
	}])
}

#[test]
fn orbis_sudo_can_reserve_multiple_full_cores_for_one_parachain() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let orbis = full_core_task(1006);
		let another_para = full_core_task(2000);

		for _ in 0..3 {
			assert_ok!(Broker::reserve(RuntimeOrigin::root(), orbis.clone()));
		}
		assert_ok!(Broker::reserve(RuntimeOrigin::root(), another_para.clone()));

		let reservations = Reservations::<Runtime>::get();
		assert_eq!(reservations.len(), 4);
		assert_eq!(reservations.iter().filter(|schedule| **schedule == orbis).count(), 3);
		assert_eq!(reservations[3], another_para);
	});
}

#[test]
fn broker_allocation_lifecycle_is_sudo_only() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let signed = RuntimeOrigin::signed(AccountId::from(ALICE));
		assert_noop!(
			Broker::reserve(signed.clone(), full_core_task(1006)),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			Broker::request_core_count(signed.clone(), 3),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(Broker::unreserve(signed, 0), sp_runtime::DispatchError::BadOrigin);

		assert_ok!(Broker::reserve(RuntimeOrigin::root(), full_core_task(1006)));
		assert_ok!(Broker::unreserve(RuntimeOrigin::root(), 0));
		assert!(Reservations::<Runtime>::get().is_empty());
	});
}

#[test]
fn fee_free_policy_is_call_scoped_quota_bounded_and_not_batchable() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let account = AccountId::from(ALICE);
		let origin = RuntimeOrigin::signed(account.clone());
		assert_ok!(Feeless::add_feeless_account(RuntimeOrigin::root(), account.clone()));

		let allowed =
			RuntimeCall::Entity(pallet_origin_entity::Call::rotate_attributes { ops: vec![] });
		assert!(allowed.is_feeless(&origin));

		let wrapped =
			RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![allowed.clone()] });
		assert!(!wrapped.is_feeless(&origin));

		let ordinary = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: AccountId::from([9u8; 32]).into(),
			value: 1,
		});
		assert!(!ordinary.is_feeless(&origin));

		for _ in 0..16 {
			assert_ok!(Feeless::consume_feeless_quota(&account));
		}
		assert!(!allowed.is_feeless(&origin));
		assert_noop!(
			Feeless::consume_feeless_quota(&account),
			pallet_origin_feeless::Error::<Runtime>::QuotaExhausted
		);
	});
}

#[test]
fn normal_pipeline_charges_nonce_owner_refunds_failure_and_consumes_prepared_quota() {
	use frame_support::{dispatch::GetDispatchInfo, traits::BuildGenesisConfig};
	use sp_runtime::traits::{Dispatchable, TransactionExtension};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let account = AccountId::from(ALICE);
		let initial_balance = 1_000_000_000_000u128;
		<Balances as Mutate<AccountId>>::set_balance(&account, initial_balance);

		let call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: AccountId::from([9u8; 32]).into(),
			value: initial_balance.saturating_mul(2),
		});
		let info = call.get_dispatch_info();
		let extension = crate::default_inner_tx_extensions(
			0,
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into(),
			Default::default(),
		);
		let implicit = extension.implicit().unwrap();
		let (_, val, origin) = extension
			.validate(
				RuntimeOrigin::signed(account.clone()),
				&call,
				&info,
				call.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		assert_eq!(frame_system::ensure_signed(origin.clone()).unwrap(), account);
		let pre = extension.prepare(val, &origin, &call, &info, call.encoded_size()).unwrap();
		assert_eq!(System::account_nonce(&account), 1);
		let after_withdrawal = Balances::free_balance(&account);
		assert!(after_withdrawal < initial_balance);

		let (post_info, dispatch_result) = match call.clone().dispatch(origin) {
			Ok(post_info) => (post_info, Ok(())),
			Err(error) => (error.post_info, Err(error.error)),
		};
		assert!(dispatch_result.is_err());
		assert_ok!(crate::InnerTxExtensions::post_dispatch_details(
			pre,
			&info,
			&post_info,
			call.encoded_size(),
			&dispatch_result,
		));
		assert!(Balances::free_balance(&account) >= after_withdrawal);
		assert!(Balances::free_balance(&account) < initial_balance);
		assert_eq!(System::account_nonce(&account), 1);

		assert_ok!(Feeless::add_feeless_account(RuntimeOrigin::root(), account.clone()));
		let feeless_call =
			RuntimeCall::Entity(pallet_origin_entity::Call::rotate_attributes { ops: vec![] });
		let feeless_info = feeless_call.get_dispatch_info();
		let extension = crate::default_inner_tx_extensions(
			1,
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into(),
			Default::default(),
		);
		let implicit = extension.implicit().unwrap();
		let (_, val, origin) = extension
			.validate(
				RuntimeOrigin::signed(account.clone()),
				&feeless_call,
				&feeless_info,
				feeless_call.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &feeless_call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		let balance_before_feeless = Balances::free_balance(&account);
		let _pre = extension
			.prepare(val, &origin, &feeless_call, &feeless_info, feeless_call.encoded_size())
			.unwrap();
		assert_eq!(Balances::free_balance(&account), balance_before_feeless);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&account), Some((1, 1)));
		assert_eq!(System::account_nonce(&account), 2);
	});
}

#[test]
fn ethereum_pipeline_uses_mapped_nonce_payer_and_only_terminal_revive_actor() {
	use frame_support::{
		dispatch::GetDispatchInfo,
		traits::{BuildGenesisConfig, OriginTrait},
	};
	use pallet_revive::evm::runtime::EthExtra;
	use sp_runtime::traits::{Dispatchable, TransactionExtension};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let mapped = AccountId::from([7u8; 32]);
		let initial_balance = 1_000_000_000_000u128;
		<Balances as Mutate<AccountId>>::set_balance(&mapped, initial_balance);
		let call = RuntimeCall::System(frame_system::Call::remark { remark: b"eth".to_vec() });
		let info = call.get_dispatch_info();
		let extension = <crate::EthExtraImpl as EthExtra>::get_eth_extension(0, 0).0 .0;
		let implicit = extension.implicit().unwrap();
		let (_, val, origin) = extension
			.validate(
				RuntimeOrigin::signed(mapped.clone()),
				&call,
				&info,
				call.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		assert!(matches!(
			origin.caller(),
			crate::OriginCaller::Revive(pallet_revive::Origin::EthTransaction(who)) if who == &mapped
		));
		let pre = extension.prepare(val, &origin, &call, &info, call.encoded_size()).unwrap();
		assert_eq!(System::account_nonce(&mapped), 1);
		let after_withdrawal = Balances::free_balance(&mapped);
		assert!(after_withdrawal < initial_balance);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&mapped), None);
		let failed = Err(sp_runtime::DispatchError::BadOrigin);
		assert_ok!(crate::InnerTxExtensions::post_dispatch_details(
			pre,
			&info,
			&Default::default(),
			call.encoded_size(),
			&failed,
		));
		assert!(Balances::free_balance(&mapped) >= after_withdrawal);
		assert!(Balances::free_balance(&mapped) < initial_balance);

		let resource_call =
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period: 0,
				counter: 0,
				account_id: mapped.clone(),
			});
		let resource_info = resource_call.get_dispatch_info();
		let eth_resource = <crate::EthExtraImpl as EthExtra>::get_eth_extension(1, 0).0 .0;
		let resource_implicit = eth_resource.implicit().unwrap();
		let (_, _, resource_origin) = eth_resource
			.validate(
				RuntimeOrigin::signed(mapped.clone()),
				&resource_call,
				&resource_info,
				resource_call.encoded_size(),
				resource_implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &resource_call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		assert!(matches!(
			resource_origin.caller(),
			crate::OriginCaller::Revive(pallet_revive::Origin::EthTransaction(who)) if who == &mapped
		));
		assert!(
			resource_call.dispatch(resource_origin).is_err(),
			"Ethereum cannot spoof Resources origin"
		);
	});
}

#[test]
fn direct_score_policy_executes_once_through_concrete_runtime_extensions() {
	use frame_support::traits::{BuildGenesisConfig, SignedTransactionBuilder};
	use sp_core::{sr25519, Pair};
	use sp_runtime::{
		generic::SignedPayload, traits::IdentifyAccount, MultiSignature, MultiSigner,
	};

	fn account(pair: &sr25519::Pair) -> AccountId {
		MultiSigner::from(pair.public()).into_account()
	}
	fn extensions(standard_nonce: u32, score_nonce: u32) -> crate::TxExtensions {
		let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into();
		crate::paid_tx_extensions((
			(
				indiv_pallet_people::extension::AsPerson::<Runtime>::new(None),
				pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(Some(
					pallet_orbis_score::ScoreAsParticipantData { nonce: score_nonce },
				)),
				indiv_pallet_people_lite::extension::PeopleLiteAuth::<Runtime>::new(None),
				indiv_pallet_resources::extension::AsResources::<Runtime>::new(None),
				pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
				frame_system::AuthorizeCall::<Runtime>::new(),
			),
			crate::AccountAwareResources::from(frame_system::CheckNonZeroSender::<Runtime>::new()),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(sp_runtime::generic::Era::Immortal),
			crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(
				standard_nonce,
			)),
			frame_system::CheckWeight::<Runtime>::new(),
			crate::AccountAwareResources::from(payment),
			Default::default(),
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
			Default::default(),
		))
	}
	fn signed(
		pair: &sr25519::Pair,
		who: &AccountId,
		call: RuntimeCall,
		extension: crate::TxExtensions,
	) -> crate::UncheckedExtrinsic {
		let payload = SignedPayload::new(call.clone(), extension.clone()).unwrap();
		let signature = payload.using_encoded(|bytes| pair.sign(bytes));
		<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
			call,
			who.clone().into(),
			MultiSignature::Sr25519(signature),
			extension,
		)
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let who = account(&pair);
		let initial = 100_000_000_000_000u128;
		let _ = <Balances as Mutate<AccountId>>::set_balance(&who, initial);
		assert_ok!(crate::Score::onboard_for_recognition(&who));
		let key = pallet_orbis_score::AccountOrPerson::Account(who.clone());
		pallet_orbis_score::Participants::<Runtime>::mutate(&key, |participant| {
			participant.as_mut().unwrap().score = 10
		});
		let call = RuntimeCall::Score(pallet_orbis_score::Call::cash_out {});

		for (standard, score, expected) in
			[(0, 1, sp_runtime::transaction_validity::InvalidTransaction::Future)]
		{
			let before = pallet_orbis_score::Participants::<Runtime>::get(&key).unwrap();
			let xt = signed(&pair, &who, call.clone(), extensions(standard, score));
			assert_eq!(
				crate::Executive::validate_transaction(
					sp_runtime::transaction_validity::TransactionSource::External,
					xt,
					System::block_hash(0),
				),
				Err(expected.into())
			);
			assert_eq!(System::account_nonce(&who), 0);
			assert_eq!(Balances::free_balance(&who), initial);
			assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&key), Some(before));
			assert!(crate::meta_v6::token().is_none());
		}

		let xt = signed(&pair, &who, call, extensions(0, 0));
		assert_ok!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			xt.clone(),
			System::block_hash(0),
		));
		assert_ok!(crate::Executive::apply_extrinsic(xt).unwrap());
		assert_eq!(System::account_nonce(&who), 1, "Score and CheckNonce increment exactly once");
		assert_eq!(Balances::free_balance(&who), initial, "Pays::No refunds the direct fee");
		let participant = pallet_orbis_score::Participants::<Runtime>::get(&key).unwrap();
		assert_eq!(participant.score, 5);
		assert!(participant.cashed_out);
		assert!(crate::meta_v6::token().is_none());
		let balance = Balances::free_balance(&who);
		let stale = signed(
			&pair,
			&who,
			RuntimeCall::Score(pallet_orbis_score::Call::cash_out {}),
			extensions(0, 0),
		);
		assert_eq!(
			crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				stale,
				System::block_hash(0),
			),
			Err(sp_runtime::transaction_validity::InvalidTransaction::Stale.into())
		);
		assert_eq!(System::account_nonce(&who), 1);
		assert_eq!(Balances::free_balance(&who), balance);
		assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&key), Some(participant));

		pallet_orbis_score::Participants::<Runtime>::mutate(&key, |value| {
			value.as_mut().unwrap().recognition = pallet_orbis_score::Recognition::Suspended(0)
		});
		let suspended_before = pallet_orbis_score::Participants::<Runtime>::get(&key);
		let suspended = signed(
			&pair,
			&who,
			RuntimeCall::Score(pallet_orbis_score::Call::cash_out {}),
			extensions(1, 1),
		);
		assert_eq!(
			crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				suspended,
				System::block_hash(0),
			),
			Err(sp_runtime::transaction_validity::InvalidTransaction::Call.into())
		);
		assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&key), suspended_before);
		assert_eq!(System::account_nonce(&who), 1);
		assert_eq!(Balances::free_balance(&who), balance);

		let unknown_pair = sr25519::Pair::from_string("//Charlie", None).unwrap();
		let unknown = account(&unknown_pair);
		let _ = <Balances as Mutate<AccountId>>::set_balance(&unknown, initial);
		let unknown_xt = signed(
			&unknown_pair,
			&unknown,
			RuntimeCall::Score(pallet_orbis_score::Call::cash_out {}),
			extensions(0, 0),
		);
		assert_eq!(
			crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				unknown_xt,
				System::block_hash(0),
			),
			Err(sp_runtime::transaction_validity::InvalidTransaction::Call.into())
		);
		assert_eq!(System::account_nonce(&unknown), 0);
		assert_eq!(Balances::free_balance(&unknown), initial);
		assert!(crate::meta_v6::token().is_none());
	});
}

#[test]
fn signed_direct_resources_claim_uses_validated_origin_payer_through_executive() {
	use codec::Encode;
	use frame_support::traits::{BuildGenesisConfig, SignedTransactionBuilder};
	use indiv_pallet_resources::types::{MembershipCollection, ReservationPurpose};
	use indiv_support::traits::{AppendOnlyMembers, MembershipProver, RingMode};
	use sp_core::{sr25519, Pair};
	use sp_runtime::{
		generic::SignedPayload,
		traits::{IdentifyAccount, TransactionExtension},
		MultiSignature, MultiSigner,
	};
	use verifiable::GenerateVerifiable;

	fn account(pair: &sr25519::Pair) -> AccountId {
		MultiSigner::from(pair.public()).into_account()
	}

	for (reserve_before_dispatch, declared_nonce) in [(true, 0u32), (false, 0), (false, 2)] {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			frame_system::GenesisConfig::<Runtime>::default().build();
			System::set_block_number(1);
			System::set_extrinsic_index(0);
			let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
			let payer = account(&pair);
			let initial_balance = 1_000_000_000_000u128;
			let _ = <Balances as Mutate<AccountId>>::set_balance(&payer, initial_balance);
			pallet_aura::CurrentSlot::<Runtime>::put(polkadot_primitives::Slot::from(43_200u64));
			assert_ok!(crate::Timestamp::set(RuntimeOrigin::none(), 3 * 24 * 60 * 60 * 1_000u64,));

			let identifier = *indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
			let domain: verifiable::ring::RingDomainSize =
				crate::MembersFlexibleRingExponent::get().try_into().unwrap();
			let chunks = indiv_support::genesis::ring_verifier_builder_params::<
				verifiable::ring::ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2,
			>(domain);
			for (page_index, page) in
				chunks.chunks(crate::PeopleChunkPageSize::get() as usize).enumerate()
			{
				let page: frame_support::BoundedVec<
					indiv_pallet_chunks_manager::UncheckedChunk<Runtime>,
					crate::PeopleChunkPageSize,
				> = page
					.iter()
					.cloned()
					.map(indiv_pallet_chunks_manager::UncheckedChunk::<Runtime>)
					.collect::<Vec<_>>()
					.try_into()
					.unwrap();
				indiv_pallet_chunks_manager::Chunks::<Runtime>::insert(
					crate::MembersFlexibleRingExponent::get(),
					page_index as u32,
					page,
				);
			}
			assert_ok!(<Members as AppendOnlyMembers>::create_collection(
				Location::here(),
				&identifier,
				1,
				RingMode::Flexible,
				crate::MembersFlexibleRingExponent::get(),
				None,
			));
			let member_secret =
				verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([92u8; 32]);
			let member =
				verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(
					&member_secret,
				);
			assert_ok!(<Members as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member.clone()]
			));
			assert_ok!(Members::onboard_members_authorized(
				frame_system::RawOrigin::Authorized.into(),
				identifier,
				0,
				0,
				Some(member.clone()),
				0,
			));
			assert_ok!(Members::build_ring_authorized(
				frame_system::RawOrigin::Authorized.into(),
				identifier,
				0,
				crate::MembersFlexibleRingExponent::get(),
				None,
				1,
				0,
			));
			let revision = <Members as MembershipProver>::ring_revision(&identifier, 0)
				.expect("the direct Resources test ring has a revision");
			let ring_members = <Members as AppendOnlyMembers>::ring_members(&identifier, 0);
			let capacity = crate::MembersFlexibleRingExponent::get().try_into().unwrap();
			let commitment = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::open(
				capacity,
				&member,
				ring_members.into_iter(),
			)
			.expect("the one-member ring opens");

			let period = crate::Resources::long_term_storage_period_from_timestamp(
				<crate::Timestamp as frame_support::traits::UnixTime>::now().as_secs(),
			);
			let counter = 0u8;
			let call =
				RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
					period,
					counter,
					account_id: payer.clone(),
				});
			let context = crate::Resources::long_term_storage_context(period, counter);
			let (_, alias) = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				commitment.clone(),
				&member_secret,
				&context,
				&[0u8; 32],
			)
			.expect("direct Resources alias preimage builds");
			let binding = indiv_support::traits::RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias { context, alias },
			};
			indiv_pallet_people::AccountToAlias::<Runtime>::insert(&payer, &binding);
			indiv_pallet_people::AliasToAccount::<Runtime>::insert(&binding.ca, &payer);

			let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into();
			let revive = pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default();
			let inner_tail = (
				crate::AccountAwareResources::from(
					frame_system::CheckNonZeroSender::<Runtime>::new(),
				),
				frame_system::CheckSpecVersion::<Runtime>::new(),
				frame_system::CheckTxVersion::<Runtime>::new(),
				frame_system::CheckGenesis::<Runtime>::new(),
				frame_system::CheckMortality::<Runtime>::from(sp_runtime::generic::Era::Immortal),
				crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(
					declared_nonce,
				)),
				frame_system::CheckWeight::<Runtime>::new(),
				crate::AccountAwareResources::from(payment),
				pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
					Runtime,
					crate::OrbisStorageCallInspector,
				>::default(),
				frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
				revive,
			);
			let origin_policy_tail = (
				pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
				frame_system::AuthorizeCall::<Runtime>::new(),
			);
			let inner_tail_implicit = inner_tail.implicit().unwrap();
			let origin_policy_tail_implicit = origin_policy_tail.implicit().unwrap();
			let inherited = sp_runtime::traits::ImplicationParts {
				base: sp_runtime::traits::TxBaseImplication((0u8, &call)),
				explicit: (&origin_policy_tail, (&inner_tail, ())),
				implicit: (&origin_policy_tail_implicit, (&inner_tail_implicit, ())),
			};
			let message = (
				indiv_pallet_resources::extension::DIRECT_LONG_TERM_STORAGE_DOMAIN,
				&payer,
				&payer,
				alias,
				&MembershipCollection::People,
				0u32,
				revision,
				context,
				&call,
				inherited,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, proof_alias) =
				verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
					commitment,
					&member_secret,
					&context,
					&message,
				)
				.expect("direct Resources proof builds against the exact inherited implication");
			assert_eq!(proof_alias, alias);

			let resources_extension =
				indiv_pallet_resources::extension::AsResources::<Runtime>::new(Some(
					indiv_pallet_resources::extension::AsResourcesInfo::ClaimLongTermStorage(
						proof,
						0,
						revision,
						MembershipCollection::People,
					),
				));
			let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into();
			let tx_ext = crate::paid_tx_extensions((
				(
					indiv_pallet_people::extension::AsPerson::<Runtime>::new(None),
					pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
					indiv_pallet_people_lite::extension::PeopleLiteAuth::<Runtime>::new(None),
					resources_extension,
					pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
					frame_system::AuthorizeCall::<Runtime>::new(),
				),
				crate::AccountAwareResources::from(
					frame_system::CheckNonZeroSender::<Runtime>::new(),
				),
				frame_system::CheckSpecVersion::<Runtime>::new(),
				frame_system::CheckTxVersion::<Runtime>::new(),
				frame_system::CheckGenesis::<Runtime>::new(),
				frame_system::CheckMortality::<Runtime>::from(sp_runtime::generic::Era::Immortal),
				crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(
					declared_nonce,
				)),
				frame_system::CheckWeight::<Runtime>::new(),
				crate::AccountAwareResources::from(payment),
				pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
					Runtime,
					crate::OrbisStorageCallInspector,
				>::default(),
				frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
				pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
			));
			let payload = SignedPayload::new(call.clone(), tx_ext.clone()).unwrap();
			let bad_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
			let bad_signature = payload.using_encoded(|bytes| bad_pair.sign(bytes));
			let bad_extrinsic =
				<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
					call.clone(),
					payer.clone().into(),
					MultiSignature::Sr25519(bad_signature),
					tx_ext.clone(),
				);
			let signature = payload.using_encoded(|bytes| pair.sign(bytes));
			let extrinsic =
				<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
					call.clone(),
					payer.clone().into(),
					MultiSignature::Sr25519(signature),
					tx_ext,
				);
			let encoded_len = extrinsic.encoded_size() as u32;
			let stale_extrinsic = extrinsic.clone();
			let purpose = ReservationPurpose::Membership {
				period,
				alias,
				counter,
				collection: MembershipCollection::People,
			};
			if reserve_before_dispatch {
				indiv_pallet_resources::StorageReservationByPurpose::<Runtime>::insert(
					&purpose, 99u64,
				);
			}

			assert!(crate::meta_v6::token().is_none());
			let balance_before_bad_signature = Balances::free_balance(&payer);
			assert!(crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				bad_extrinsic,
				System::block_hash(0),
			)
			.is_err());
			assert_eq!(System::account_nonce(&payer), 0);
			assert_eq!(Balances::free_balance(&payer), balance_before_bad_signature);
			let _ = <Balances as Mutate<AccountId>>::set_balance(&payer, 0);
			assert!(crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				extrinsic.clone(),
				System::block_hash(0),
			)
			.is_err());
			assert_eq!(System::account_nonce(&payer), 0);
			let _ = <Balances as Mutate<AccountId>>::set_balance(&payer, initial_balance);
			if declared_nonce > 0 {
				let future = crate::Executive::validate_transaction(
					sp_runtime::transaction_validity::TransactionSource::External,
					extrinsic.clone(),
					System::block_hash(0),
				)
				.expect("the pool retains a future nonce behind its dependency tag");
				assert!(!future.requires.is_empty());
				let in_block = crate::Executive::apply_extrinsic(extrinsic);
				assert!(format!("{in_block:?}").contains("Future"));
				assert_eq!(System::account_nonce(&payer), 0);
				assert_eq!(Balances::free_balance(&payer), initial_balance);
				return;
			}
			assert_ok!(crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				extrinsic.clone(),
				System::block_hash(0),
			));
			let balance_before_apply = Balances::free_balance(&payer);
			let declared_fee =
				crate::TransactionPayment::query_info(extrinsic.clone(), encoded_len).partial_fee;
			let apply_result = crate::Executive::apply_extrinsic(extrinsic);
			if reserve_before_dispatch {
				assert!(format!("{apply_result:?}").contains("ClaimAlreadyReserved"));
			} else {
				assert!(matches!(apply_result, Ok(Ok(_))));
			}
			assert_eq!(System::account_nonce(&payer), 1);
			let balance_after_apply = Balances::free_balance(&payer);
			let charged_fee = balance_before_apply - balance_after_apply;
			if reserve_before_dispatch {
				let dispatch_outcome = apply_result.expect("the signed extrinsic is valid");
				let _dispatch_error =
					dispatch_outcome.expect_err("the reserved claim dispatch fails");
				assert_eq!(charged_fee, declared_fee);
			} else {
				assert!(declared_fee > 0);
				assert_eq!(charged_fee, 0, "successful quota claim refunds the full declared fee");
			}
			if reserve_before_dispatch {
				assert!(balance_after_apply < initial_balance);
			} else {
				assert_eq!(balance_after_apply, initial_balance);
			}
			assert_eq!(
				indiv_pallet_resources::SpentLongTermStorageAliases::<Runtime>::contains_key(
					indiv_support::utils::BigEndianU32::from(period),
					alias,
				),
				!reserve_before_dispatch
			);
			assert!(crate::meta_v6::token().is_none());
			assert!(crate::Executive::validate_transaction(
				sp_runtime::transaction_validity::TransactionSource::External,
				stale_extrinsic,
				System::block_hash(0),
			)
			.is_err());
			assert_eq!(System::account_nonce(&payer), 1);
			cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
				cumulus_primitives_core::PersistedValidationData {
					parent_head: polkadot_parachain_primitives::primitives::HeadData(Vec::new()),
					relay_parent_number: 1,
					relay_parent_storage_root: Default::default(),
					max_pov_size: 1_000_000,
				},
			);
			cumulus_pallet_parachain_system::HostConfiguration::<Runtime>::put(
				cumulus_primitives_core::AbridgedHostConfiguration {
					max_code_size: 2 * 1024 * 1024,
					max_head_data_size: 1024 * 1024,
					max_upward_queue_count: 8,
					max_upward_queue_size: 1024,
					max_upward_message_size: 256,
					max_upward_message_num_per_candidate: 5,
					hrmp_max_message_num_per_candidate: 5,
					validation_upgrade_cooldown: 6,
					validation_upgrade_delay: 6,
					async_backing_params: polkadot_primitives::AsyncBackingParams {
						allowed_ancestry_len: 0,
						max_candidate_depth: 0,
					},
				},
			);
			cumulus_pallet_parachain_system::RelevantMessagingState::<Runtime>::put(
				cumulus_pallet_parachain_system::MessagingStateSnapshot {
					dmq_mqc_head: Default::default(),
					relay_dispatch_queue_remaining_capacity: Default::default(),
					ingress_channels: Vec::new(),
					egress_channels: Vec::new(),
				},
			);
			let header = crate::Executive::finalize_block();
			assert!(header.number > 0);
			assert!(encoded_len > 0);
		});
	}
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery_core(emit_v4: bool) {
	use codec::Encode;
	use frame_support::{
		dispatch::GetDispatchInfo,
		traits::{BuildGenesisConfig, SignedTransactionBuilder},
	};
	use indiv_support::traits::{AppendOnlyMembers, MembershipProver, RingMode};
	use sp_core::{sr25519, Pair};
	use sp_runtime::{
		generic::{Era, SignedPayload},
		traits::{IdentifyAccount, TransactionExtension},
		MultiSignature, MultiSigner,
	};
	use verifiable::GenerateVerifiable;
	const META_EXTENSION_VERSION: u8 = 0;

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

	fn account(pair: &sr25519::Pair) -> AccountId {
		MultiSigner::from(pair.public()).into_account()
	}

	fn signed_meta_tx(
		call: RuntimeCall,
		claimed: AccountId,
		signing_pair: &sr25519::Pair,
		proofs: crate::meta_v6::PolicyProofsV6,
	) -> pallet_meta_tx::MetaTxFor<Runtime> {
		signed_meta_tx_with_metadata(
			call,
			claimed,
			signing_pair,
			proofs,
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
			None,
		)
	}

	fn signed_meta_tx_with_metadata(
		call: RuntimeCall,
		claimed: AccountId,
		signing_pair: &sr25519::Pair,
		proofs: crate::meta_v6::PolicyProofsV6,
		metadata: frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		metadata_implicit: Option<[u8; 32]>,
	) -> pallet_meta_tx::MetaTxFor<Runtime> {
		let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
		let nonce = frame_system::CheckNonce::<Runtime>::from(System::account(&claimed).nonce);
		let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(proofs);
		let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let preimage = crate::meta_v6::IntentPreimageV7 {
			domain: crate::meta_v6::META_DOMAIN.to_vec(),
			extension_version: META_EXTENSION_VERSION,
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			inner_signer: claimed.clone(),
			call_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&call.encode())),
			mortality: Era::Immortal,
			nonce: System::account(&claimed).nonce,
			policy_proofs_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&policy.0.encode())),
			storage_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(
				&storage.encode(),
			)),
			metadata_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(
				&metadata.encode(),
			)),
			metadata_implicit,
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
		let implicit = bare.implicit().expect("test externalities provide implicit data");
		let signature = (META_EXTENSION_VERSION, call.clone(), bare.clone(), implicit)
			.using_encoded(|payload| signing_pair.sign(&sp_io::hashing::blake2_256(payload)));
		let verify = pallet_verify_signature::VerifySignature::new_with_signature(
			MultiSignature::Sr25519(signature),
			claimed,
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
		let extension = (
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
		);
		pallet_meta_tx::MetaTxFor::<Runtime>::new(call, META_EXTENSION_VERSION, extension)
	}

	fn signed_meta_tx_with_native_policies(
		call: RuntimeCall,
		claimed: AccountId,
		signing_pair: &sr25519::Pair,
		proofs: crate::meta_v6::PolicyProofsV6,
		metadata: frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		metadata_implicit: Option<[u8; 32]>,
		score: Option<pallet_orbis_score::ScoreAsParticipantData<u32>>,
		honour: Option<pallet_orbis_honour::extension::VoterAuthData<Runtime>>,
	) -> pallet_meta_tx::MetaTxFor<Runtime> {
		let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
		let nonce = frame_system::CheckNonce::<Runtime>::from(System::account(&claimed).nonce);
		let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(proofs);
		let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let preimage = crate::meta_v6::IntentPreimageV7 {
			domain: crate::meta_v6::META_DOMAIN.to_vec(),
			extension_version: META_EXTENSION_VERSION,
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			inner_signer: claimed.clone(),
			call_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&call.encode())),
			mortality: Era::Immortal,
			nonce: System::account(&claimed).nonce,
			policy_proofs_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&policy.0.encode())),
			storage_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(
				&storage.encode(),
			)),
			metadata_extension_hash: sp_core::H256::from(sp_io::hashing::blake2_256(
				&metadata.encode(),
			)),
			metadata_implicit,
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
				pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(score),
				policy,
				pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(honour),
			),
			storage,
			metadata,
		);
		let implicit = bare.implicit().expect("test externalities provide implicit data");
		let signature = (META_EXTENSION_VERSION, call.clone(), bare.clone(), implicit)
			.using_encoded(|payload| signing_pair.sign(&sp_io::hashing::blake2_256(payload)));
		let verify = pallet_verify_signature::VerifySignature::new_with_signature(
			MultiSignature::Sr25519(signature),
			claimed,
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
		let extension = (
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
		);
		pallet_meta_tx::MetaTxFor::<Runtime>::new(call, META_EXTENSION_VERSION, extension)
	}

	fn apply_meta_through_executive(
		meta: pallet_meta_tx::MetaTxFor<Runtime>,
		sponsor: &AccountId,
		sponsor_pair: &sr25519::Pair,
	) -> frame_support::dispatch::DispatchResultWithPostInfo {
		let encoded_meta = meta.encode();
		let (inner_call, _, inner_extension): (
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		) = Decode::decode(&mut encoded_meta.as_slice()).expect("the SDK Meta tuple decodes");
		let mut declared_info = inner_call.get_dispatch_info();
		declared_info.extension_weight = inner_extension.weight(&inner_call);
		let declared_inner_weight = declared_info.total_weight();
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV7 {
			payer: sponsor.clone(),
			intent_commitment: inner_extension.1 .0.commitment(),
			outer_nonce: System::account_nonce(sponsor),
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			consumed: false,
		});
		let inner_implicit = inner_extension.implicit().unwrap();
		let inner_validation = inner_extension.validate(
			RuntimeOrigin::none(),
			&inner_call,
			&declared_info,
			encoded_meta.len(),
			inner_implicit,
			&sp_runtime::traits::TxBaseImplication((0u8, &inner_call)),
			sp_runtime::transaction_validity::TransactionSource::External,
		);
		crate::meta_v6::clear_token();
		if let Err(error) = inner_validation {
			panic!("inner Meta extension validation failed for {inner_call:?}: {error:?}");
		}
		let meta_len = meta.encoded_size() as u32;
		let call = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(meta),
			meta_tx_encoded_len: meta_len,
		});
		let payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into();
		let extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
			System::account_nonce(sponsor),
			payment,
			Default::default(),
		));
		let payload = SignedPayload::new(call.clone(), extension.clone()).unwrap();
		let signature = payload.using_encoded(|bytes| sponsor_pair.sign(bytes));
		let extrinsic =
			<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
				call,
				sponsor.clone().into(),
				MultiSignature::Sr25519(signature),
				extension,
			);
		assert_ok!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			extrinsic.clone(),
			System::block_hash(0),
		));
		crate::Executive::apply_extrinsic(extrinsic)
			.expect("the signed outer Meta extrinsic is valid")
			.expect("the outer Meta dispatch succeeds");
		let result = System::events()
			.into_iter()
			.rev()
			.find_map(|record| match record.event {
				crate::RuntimeEvent::MetaTx(pallet_meta_tx::Event::Dispatched { result }) =>
					Some(result),
				_ => None,
			})
			.expect("MetaTx emits the inner dispatch result");
		let actual_inner_weight = result
			.as_ref()
			.map_or_else(|err| err.post_info.actual_weight, |post| post.actual_weight)
			.unwrap_or(declared_inner_weight);
		assert!(actual_inner_weight.all_lte(declared_inner_weight));
		assert!((meta_len as usize) <= crate::meta_v6::MAX_META_ENCODED_BYTES);
		result
	}

	fn resource_meta_message(call: &RuntimeCall, signer: &AccountId) -> [u8; 32] {
		let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let honour = pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None);
		let inherited = sp_runtime::traits::ImplicationParts {
			base: sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, call)),
			explicit: (&honour, (&storage, &metadata)),
			implicit: (
				honour.implicit().unwrap(),
				(storage.implicit().unwrap(), metadata.implicit().unwrap()),
			),
		};
		let RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
			period,
			counter,
			..
		}) = call
		else {
			panic!("resource transcript requires claim call")
		};
		let context = crate::Resources::long_term_storage_context(*period, *counter);
		let bound = indiv_pallet_people::AccountToAlias::<Runtime>::get(signer)
			.expect("resource signer has an authoritative People binding");
		(
			crate::meta_v6::RESOURCES_DOMAIN,
			signer,
			signer,
			bound.ca.alias,
			period,
			counter,
			indiv_pallet_resources::types::MembershipCollection::People,
			bound.ring,
			<Members as MembershipProver>::ring_revision(
				&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				0,
			)
			.unwrap(),
			context,
			call,
			inherited,
		)
			.using_encoded(sp_io::hashing::blake2_256)
	}

	fn revised_meta_message<const N: usize>(
		domain: &'static [u8; N],
		call: &RuntimeCall,
		signer: &AccountId,
	) -> [u8; 32] {
		let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let honour = pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None);
		let inherited = sp_runtime::traits::ImplicationParts {
			base: sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, call)),
			explicit: (&honour, (&storage, &metadata)),
			implicit: (
				honour.implicit().unwrap(),
				(storage.implicit().unwrap(), metadata.implicit().unwrap()),
			),
		};
		(domain, signer, signer, call, inherited).using_encoded(sp_io::hashing::blake2_256)
	}

	const COMPILED_METADATA_HASH: [u8; 32] = [0xabu8; 32];
	#[derive(Clone, Copy, Eq, PartialEq)]
	struct CompiledMetadataResolver;
	impl crate::meta_v6::MetadataImplicitResolver for CompiledMetadataResolver {
		fn resolve(
			_: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		) -> Result<Option<[u8; 32]>, sp_runtime::transaction_validity::TransactionValidityError>
		{
			Ok(Some(COMPILED_METADATA_HASH))
		}
	}
	#[derive(Clone, Copy, Eq, PartialEq)]
	struct WrongMetadataResolver;
	impl crate::meta_v6::MetadataImplicitResolver for WrongMetadataResolver {
		fn resolve(
			_: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		) -> Result<Option<[u8; 32]>, sp_runtime::transaction_validity::TransactionValidityError>
		{
			Ok(Some([0xacu8; 32]))
		}
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
		let alice = account(&alice_pair);
		let bob = account(&bob_pair);
		let metadata_inner = RuntimeCall::System(frame_system::Call::remark {
			remark: b"metadata-implicit-v7".to_vec(),
		});
		let enabled_meta = signed_meta_tx_with_metadata(
			metadata_inner,
			alice.clone(),
			&alice_pair,
			Default::default(),
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new_with_custom_hash(
				COMPILED_METADATA_HASH,
			),
			Some(COMPILED_METADATA_HASH),
		);
		let enabled_len = enabled_meta.encoded_size() as u32;
		let enabled_outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(enabled_meta),
			meta_tx_encoded_len: enabled_len,
		});
		let enabled_scope = crate::meta_v6::PaidMetaScope::<_, CompiledMetadataResolver>::from(
			frame_system::CheckSpecVersion::<Runtime>::new(),
		);
		let enabled_implicit = enabled_scope.implicit().unwrap();
		let business_hash = || {
			(
					System::account(&bob).nonce,
					Balances::free_balance(&bob),
					<Balances as frame_support::traits::fungible::InspectHold<AccountId>>::
						total_balance_on_hold(&bob),
					Assets::balance(1, &bob),
					<AssetsHolder as frame_support::traits::fungibles::InspectHold<AccountId>>::
						total_balance_on_hold(1, &bob),
				)
					.using_encoded(sp_io::hashing::blake2_256)
		};
		let business_before = business_hash();
		let token_before = crate::meta_v6::token();
		let (_, enabled_val, enabled_origin) = enabled_scope
			.validate(
				RuntimeOrigin::signed(bob.clone()),
				&enabled_outer,
				&enabled_outer.get_dispatch_info(),
				enabled_outer.encoded_size(),
				enabled_implicit,
				&sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, &enabled_outer)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		assert_eq!(enabled_val.core_count(), 1);
		assert_eq!(enabled_val.scope_count(), 1);
		let wrong_scope = crate::meta_v6::PaidMetaScope::<_, WrongMetadataResolver>::from(
			frame_system::CheckSpecVersion::<Runtime>::new(),
		);
		let wrong_implicit = wrong_scope.implicit().unwrap();
		assert!(wrong_scope
			.validate(
				RuntimeOrigin::signed(bob.clone()),
				&enabled_outer,
				&enabled_outer.get_dispatch_info(),
				enabled_outer.encoded_size(),
				wrong_implicit,
				&sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, &enabled_outer)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.is_err());
		let missing_scope = crate::meta_v6::PaidMetaScope::<_, CannotLookupMetadataResolver>::from(
			frame_system::CheckSpecVersion::<Runtime>::new(),
		);
		let missing_implicit = missing_scope.implicit().unwrap();
		assert!(matches!(
			missing_scope.validate(
				RuntimeOrigin::signed(bob.clone()),
				&enabled_outer,
				&enabled_outer.get_dispatch_info(),
				enabled_outer.encoded_size(),
				missing_implicit,
				&sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, &enabled_outer)),
				sp_runtime::transaction_validity::TransactionSource::External,
			),
			Err(sp_runtime::transaction_validity::TransactionValidityError::Unknown(
				sp_runtime::transaction_validity::UnknownTransaction::CannotLookup
			))
		));
		assert_eq!(crate::meta_v6::token(), token_before);
		assert_eq!(business_hash(), business_before);
		let enabled_pre = enabled_scope
			.prepare(
				enabled_val,
				&enabled_origin,
				&enabled_outer,
				&enabled_outer.get_dispatch_info(),
				enabled_outer.encoded_size(),
			)
			.unwrap();
		assert_eq!(enabled_pre.core_count(), 1);
		assert_eq!(enabled_pre.key_count(), 1);
		assert!(crate::meta_v6::token().is_some());
		assert_eq!(business_hash(), business_before);
		crate::meta_v6::clear_token();
		let production_metadata =
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true);
		if let Ok(Some(compiled_hash)) =
			<crate::meta_v6::ProductionMetadataImplicitResolver as crate::meta_v6::MetadataImplicitResolver>::resolve(&production_metadata)
		{
				let production_meta = signed_meta_tx_with_metadata(
					RuntimeCall::System(frame_system::Call::remark {
						remark: b"compiled-metadata-implicit-v7".to_vec(),
					}),
					alice.clone(),
					&alice_pair,
					Default::default(),
					production_metadata,
					Some(compiled_hash),
				);
				let production_len = production_meta.encoded_size() as u32;
				let production_outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
					meta_tx: Box::new(production_meta),
					meta_tx_encoded_len: production_len,
				});
				let production_scope = crate::meta_v6::PaidMetaScope::<_, crate::meta_v6::ProductionMetadataImplicitResolver>::from(
					frame_system::CheckSpecVersion::<Runtime>::new(),
				);
				let implicit = production_scope.implicit().unwrap();
				assert!(production_scope
					.validate(
						RuntimeOrigin::signed(bob.clone()),
						&production_outer,
						&production_outer.get_dispatch_info(),
						production_outer.encoded_size(),
						implicit,
						&sp_runtime::traits::TxBaseImplication((
							META_EXTENSION_VERSION,
							&production_outer,
						)),
						sp_runtime::transaction_validity::TransactionSource::External,
					)
					.is_ok());
		}
		let alice_balance =
			<Balances as Mutate<AccountId>>::set_balance(&alice, crate::ExistentialDeposit::get());
		let bob_balance = <Balances as Mutate<AccountId>>::set_balance(&bob, 100_000_000_000_000);
		pallet_aura::CurrentSlot::<Runtime>::put(polkadot_primitives::Slot::from(43_200u64));
		assert_ok!(crate::Timestamp::set(RuntimeOrigin::none(), 3 * 24 * 60 * 60 * 1_000u64,));
		let identifier = *indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
		let domain: verifiable::ring::RingDomainSize =
			crate::MembersFlexibleRingExponent::get().try_into().unwrap();
		let chunks = indiv_support::genesis::ring_verifier_builder_params::<
			verifiable::ring::ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2,
		>(domain);
		for (page_index, page) in
			chunks.chunks(crate::PeopleChunkPageSize::get() as usize).enumerate()
		{
			let page: frame_support::BoundedVec<
				indiv_pallet_chunks_manager::UncheckedChunk<Runtime>,
				crate::PeopleChunkPageSize,
			> = page
				.iter()
				.cloned()
				.map(indiv_pallet_chunks_manager::UncheckedChunk::<Runtime>)
				.collect::<Vec<_>>()
				.try_into()
				.unwrap();
			indiv_pallet_chunks_manager::Chunks::<Runtime>::insert(
				crate::MembersFlexibleRingExponent::get(),
				page_index as u32,
				page,
			);
		}
		assert_ok!(<Members as AppendOnlyMembers>::create_collection(
			Location::here(),
			&identifier,
			1,
			RingMode::Flexible,
			crate::MembersFlexibleRingExponent::get(),
			None,
		));
		let member_secret =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([91u8; 32]);
		let member = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(
			&member_secret,
		);
		assert_ok!(<Members as AppendOnlyMembers>::add_members(&identifier, vec![member.clone()],));
		assert_ok!(Members::onboard_members_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			0,
			Some(member.clone()),
			0,
		));
		assert_ok!(Members::build_ring_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			crate::MembersFlexibleRingExponent::get(),
			None,
			1,
			0,
		));
		let revision = <Members as MembershipProver>::ring_revision(&identifier, 0)
			.expect("the test ring has a revision");
		let ring_members = <Members as AppendOnlyMembers>::ring_members(&identifier, 0);
		let capacity = crate::MembersFlexibleRingExponent::get().try_into().unwrap();
		let commitment = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::open(
			capacity,
			&member,
			ring_members.into_iter(),
		)
		.expect("the one-member ring opens");

		{
		sp_io::storage::start_transaction();
		// Execute the native Score policy through a paid outer Meta extrinsic. The inner account
		// owns exactly one inner nonce and no fee; the sponsor owns the outer nonce/payment.
		assert_ok!(crate::Score::onboard_for_recognition(&alice));
		pallet_orbis_score::Participants::<Runtime>::mutate(
			pallet_orbis_score::AccountOrPerson::Account(alice.clone()),
			|participant| participant.as_mut().unwrap().score = 10,
		);
		let score_call = RuntimeCall::Score(pallet_orbis_score::Call::cash_out {});
		let alice_balance_before_score = Balances::free_balance(&alice);
		let bob_balance_before_score = Balances::free_balance(&bob);
		let score_meta = signed_meta_tx_with_native_policies(
			score_call,
			alice.clone(),
			&alice_pair,
			Default::default(),
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
			None,
			Some(pallet_orbis_score::ScoreAsParticipantData { nonce: 0 }),
			None,
		);
		assert_ok!(apply_meta_through_executive(score_meta, &bob, &bob_pair));
		assert_eq!(System::account_nonce(&alice), 1);
		assert_eq!(System::account_nonce(&bob), 1);
		assert_eq!(Balances::free_balance(&alice), alice_balance_before_score);
		assert!(Balances::free_balance(&bob) < bob_balance_before_score);
		let score = pallet_orbis_score::Participants::<Runtime>::get(
			pallet_orbis_score::AccountOrPerson::Account(alice.clone()),
		)
		.unwrap();
		assert_eq!(score.score, 5);
		assert!(score.cashed_out);
		assert!(crate::meta_v6::token().is_none());

		// Execute the native Honour policy against the exact active runtime ring.
		let now = <crate::Timestamp as frame_support::traits::UnixTime>::now().as_secs();
		let vote = pallet_orbis_honour::VoteData {
			subject: [0x71; 32],
			point: 7,
			direction: pallet_orbis_honour::Direction::Honourable,
		};
		let honour_call = RuntimeCall::Honour(pallet_orbis_honour::Call::bestow {
			vote: vote.clone(),
			call_valid_from: now,
		});
		let storage = pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::OrbisStorageCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let message = (META_EXTENSION_VERSION, &honour_call, &storage, &metadata, (), None::<[u8; 32]>, &alice)
			.using_encoded(sp_io::hashing::blake2_256);
		let contexts = vote.get_contexts();
		let contexts: Vec<&[u8]> = contexts.iter().map(|context| &context[..]).collect();
		let (honour_proof, _) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create_multi_context(
				commitment.clone(),
				&member_secret,
				&contexts,
				&message,
			)
			.unwrap();
		let alice_balance_before_honour = Balances::free_balance(&alice);
		let bob_balance_before_honour = Balances::free_balance(&bob);
		let honour_meta = signed_meta_tx_with_native_policies(
			honour_call,
			alice.clone(),
			&alice_pair,
			Default::default(),
			metadata,
			None,
			None,
			Some(pallet_orbis_honour::extension::VoterAuthData {
				account: alice.clone(),
				proof: honour_proof,
				ring_index: 0,
				revision,
			}),
		);
		assert_ok!(apply_meta_through_executive(honour_meta, &bob, &bob_pair));
		assert_eq!(System::account_nonce(&alice), 2);
		assert_eq!(System::account_nonce(&bob), 2);
		assert_eq!(Balances::free_balance(&alice), alice_balance_before_honour);
		let bob_balance_after_honour = Balances::free_balance(&bob);
		let sponsor_fee = bob_balance_before_honour
			.checked_sub(bob_balance_after_honour)
			.expect("the sponsor, not the inner Honour signer, owns the fee delta");
		assert!(sponsor_fee > 0, "the paid Meta Honour outer transaction charges its sponsor");
		assert!(pallet_orbis_honour::Votes::<Runtime>::iter().next().is_some());
		assert!(crate::meta_v6::token().is_none());

		// The direct Honour surface exercises the account-aware nonce/payment adapter after
		// VoterAuth has replaced Signed with the custom Voter origin.
		let direct_vote = pallet_orbis_honour::VoteData {
			subject: [0x72; 32],
			point: 8,
			direction: pallet_orbis_honour::Direction::Honourable,
		};
		let direct_call = RuntimeCall::Honour(pallet_orbis_honour::Call::bestow {
			vote: direct_vote.clone(),
			call_valid_from: now,
		});
		let _ = <Balances as Mutate<AccountId>>::set_balance(&alice, 100_000_000_000_000);
		let direct_payer_balance = Balances::free_balance(&alice);
		let build_direct_honour =
			|nonce_value: u32,
			 claimed: AccountId,
			 signer: AccountId,
			 signing_pair: &sr25519::Pair,
			 valid_proof: bool| {
				let direct_payment: crate::PaymentPolicy =
					pallet_origin_feeless::ChargeOrSkipFeeless::from(
						pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(
							0, None,
						),
					)
					.into();
				let direct_tail = (
					crate::AccountAwareResources::from(
						frame_system::CheckNonZeroSender::<Runtime>::new(),
					),
					frame_system::CheckSpecVersion::<Runtime>::new(),
					frame_system::CheckTxVersion::<Runtime>::new(),
					frame_system::CheckGenesis::<Runtime>::new(),
					frame_system::CheckMortality::<Runtime>::from(Era::Immortal),
					crate::AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(
						nonce_value,
					)),
					frame_system::CheckWeight::<Runtime>::new(),
					crate::AccountAwareResources::from(direct_payment),
					pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
						Runtime,
						crate::OrbisStorageCallInspector,
					>::default(),
					frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
					pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
				);
				let policy_tail = (frame_system::AuthorizeCall::<Runtime>::new(),);
				let direct_inherited = sp_runtime::traits::ImplicationParts {
					base: sp_runtime::traits::TxBaseImplication((
						META_EXTENSION_VERSION,
						&direct_call,
					)),
					explicit: (&policy_tail, (&direct_tail, ())),
					implicit: (
						policy_tail.implicit().unwrap(),
						(direct_tail.implicit().unwrap(), ()),
					),
				};
				let direct_message = if valid_proof {
					(&direct_inherited, &claimed).using_encoded(sp_io::hashing::blake2_256)
				} else {
					[0u8; 32]
				};
				let direct_contexts = direct_vote.get_contexts();
				let direct_contexts: Vec<&[u8]> =
					direct_contexts.iter().map(|context| &context[..]).collect();
				let (direct_proof, _) = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create_multi_context(
					commitment.clone(),
					&member_secret,
					&direct_contexts,
					&direct_message,
				)
				.unwrap();
				let (
					nonzero, spec, tx, genesis, mortality, nonce, weight, payment, storage,
					metadata, revive,
				) = direct_tail;
				let direct_extension = crate::paid_tx_extensions((
					(
						indiv_pallet_people::extension::AsPerson::<Runtime>::new(None),
						pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
						indiv_pallet_people_lite::extension::PeopleLiteAuth::<Runtime>::new(None),
						indiv_pallet_resources::extension::AsResources::<Runtime>::new(None),
						pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(Some(
							pallet_orbis_honour::extension::VoterAuthData {
								account: claimed,
								proof: direct_proof,
								ring_index: 0,
								revision,
							},
						)),
						frame_system::AuthorizeCall::<Runtime>::new(),
					),
					nonzero,
					spec,
					tx,
					genesis,
					mortality,
					nonce,
					weight,
					payment,
					storage,
					metadata,
					revive,
				));
				let payload = SignedPayload::new(direct_call.clone(), direct_extension.clone()).unwrap();
				let signature = payload.using_encoded(|bytes| signing_pair.sign(bytes));
				<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
					direct_call.clone(),
					signer.into(),
					MultiSignature::Sr25519(signature),
					direct_extension,
				)
			};

		let direct_votes = pallet_orbis_honour::Votes::<Runtime>::iter().collect::<Vec<_>>().encode();
		let direct_tally = pallet_orbis_honour::Tally::<Runtime>::iter().collect::<Vec<_>>().encode();
		let alice_quota = pallet_origin_feeless::FeelessUsage::<Runtime>::get(&alice);
		let bob_quota = pallet_origin_feeless::FeelessUsage::<Runtime>::get(&bob);
		for (name, xt, expected) in [
			(
				"mismatched account",
				build_direct_honour(2, alice.clone(), bob.clone(), &bob_pair, true),
				sp_runtime::transaction_validity::InvalidTransaction::BadSigner,
			),
			(
				"bad ring proof",
				build_direct_honour(2, alice.clone(), alice.clone(), &alice_pair, false),
				sp_runtime::transaction_validity::InvalidTransaction::BadProof,
			),
			(
				"stale nonce",
				build_direct_honour(1, alice.clone(), alice.clone(), &alice_pair, true),
				sp_runtime::transaction_validity::InvalidTransaction::Stale,
			),
		] {
			assert_eq!(
				crate::Executive::validate_transaction(
					sp_runtime::transaction_validity::TransactionSource::External,
					xt,
					System::block_hash(0),
				),
				Err(expected.into()),
				"{name} must fail at its exact pipeline boundary"
			);
			assert_eq!(System::account_nonce(&alice), 2);
			assert_eq!(System::account_nonce(&bob), 2);
			assert_eq!(Balances::free_balance(&alice), direct_payer_balance);
			assert_eq!(Balances::free_balance(&bob), bob_balance_after_honour);
			assert_eq!(
				pallet_orbis_honour::Votes::<Runtime>::iter().collect::<Vec<_>>().encode(),
				direct_votes
			);
			assert_eq!(
				pallet_orbis_honour::Tally::<Runtime>::iter().collect::<Vec<_>>().encode(),
				direct_tally
			);
			assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&alice), alice_quota);
			assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&bob), bob_quota);
			assert!(crate::meta_v6::token().is_none());
		}
		// Pool validation admits a future standard CheckNonce with an explicit dependency. Block
		// execution rejects it exactly as Future and must preserve all actor/business state.
		let future_xt = build_direct_honour(3, alice.clone(), alice.clone(), &alice_pair, true);
		assert!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			future_xt.clone(),
			System::block_hash(0),
		)
		.is_ok());
		assert_eq!(
			crate::Executive::apply_extrinsic(future_xt),
			Err(sp_runtime::transaction_validity::InvalidTransaction::Future.into())
		);
		assert_eq!(System::account_nonce(&alice), 2);
		assert_eq!(System::account_nonce(&bob), 2);
		assert_eq!(Balances::free_balance(&alice), direct_payer_balance);
		assert_eq!(Balances::free_balance(&bob), bob_balance_after_honour);
		assert_eq!(
			pallet_orbis_honour::Votes::<Runtime>::iter().collect::<Vec<_>>().encode(),
			direct_votes
		);
		assert_eq!(
			pallet_orbis_honour::Tally::<Runtime>::iter().collect::<Vec<_>>().encode(),
			direct_tally
		);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&alice), alice_quota);
		assert_eq!(pallet_origin_feeless::FeelessUsage::<Runtime>::get(&bob), bob_quota);
		assert!(crate::meta_v6::token().is_none());

		let direct_xt = build_direct_honour(2, alice.clone(), alice.clone(), &alice_pair, true);
		let direct_balance = Balances::free_balance(&alice);
		assert_ok!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			direct_xt.clone(),
			System::block_hash(0),
		));
		assert_ok!(crate::Executive::apply_extrinsic(direct_xt).unwrap());
		assert_eq!(System::account_nonce(&alice), 3);
		assert!(Balances::free_balance(&alice) < direct_balance);
		assert!(crate::meta_v6::token().is_none());
		let _ = <Balances as Mutate<AccountId>>::set_balance(&alice, alice_balance);
		sp_io::storage::rollback_transaction();
		}
		let period = crate::Resources::long_term_storage_period_from_timestamp(
			<crate::Timestamp as frame_support::traits::UnixTime>::now().as_secs(),
		);
		let inner = RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
			period,
			counter: 0,
			account_id: alice.clone(),
		});
		let context = crate::Resources::long_term_storage_context(period, 0);
		let (_, resource_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				commitment.clone(),
				&member_secret,
				&context,
				&[0u8; 32],
			)
			.expect("the MetaTx Resources alias builds");
		let account_binding = indiv_support::traits::RevisedContextualAlias {
			revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: crate::ORBIS_PERSON_CONTEXT,
				alias: resource_alias,
			},
		};
		indiv_pallet_people::AccountToAlias::<Runtime>::insert(&alice, &account_binding);
		indiv_pallet_people::AliasToAccount::<Runtime>::insert(&account_binding.ca, &alice);
		let message = resource_meta_message(&inner, &alice);
		let (proof, proof_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				commitment.clone(),
				&member_secret,
				&context,
				&message,
			)
			.expect("the MetaTx Resources proof builds");
		assert_eq!(proof_alias, resource_alias);
		let resource_info = crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(
			proof,
			0,
			revision,
			indiv_pallet_resources::types::MembershipCollection::People,
		);
		let (invalid_proof, _) = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
			commitment.clone(),
			&member_secret,
			&context,
			&[0u8; 32],
		)
		.expect("the negative MetaTx Resources proof builds");
		let invalid_meta = signed_meta_tx(
			inner.clone(),
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				resources: Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(
					invalid_proof,
					0,
					revision,
					indiv_pallet_resources::types::MembershipCollection::People,
				)),
				..Default::default()
			},
		);
		let (_, _, invalid_extension): (
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		) = Decode::decode(&mut invalid_meta.encode().as_slice()).unwrap();
		let (_, invalid_consume, ..) = invalid_extension;
		let invalid_token = crate::meta_v6::PaidMetaTokenV7 {
			payer: bob.clone(),
			intent_commitment: invalid_consume.0.commitment(),
			outer_nonce: 0,
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			consumed: false,
		};
		crate::meta_v6::put_token(&invalid_token);
		let invalid_len = invalid_meta.encoded_size() as u32;
		assert_noop!(
			crate::MetaTx::dispatch(
				RuntimeOrigin::signed(bob.clone()),
				Box::new(invalid_meta),
				invalid_len,
			),
			pallet_meta_tx::Error::<Runtime>::BadProof,
		);
		assert!(crate::meta_v6::token().is_some());
		crate::meta_v6::clear_token();

		let meta = signed_meta_tx(
			inner.clone(),
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				resources: Some(resource_info.clone()),
				..Default::default()
			},
		);
		let (v5_call, v5_version, mut v5_extension): (
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		) = Decode::decode(&mut meta.encode().as_slice()).unwrap();
		v5_extension.1 .0.transaction_version = 5;
		let v5_meta = pallet_meta_tx::MetaTxFor::<Runtime>::new(v5_call, v5_version, v5_extension);
		let v5_len = v5_meta.encoded_size() as u32;
		let v5_outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(v5_meta),
			meta_tx_encoded_len: v5_len,
		});
		assert!(crate::meta_v6::inspect_paid_meta::<
			crate::meta_v6::ProductionMetadataImplicitResolver,
		>(&v5_outer, 0)
		.is_err());
		let (spec26_call, spec26_version, mut spec26_extension): (
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		) = Decode::decode(&mut meta.encode().as_slice()).unwrap();
		spec26_extension.1 .0.spec_version = 26;
		let spec26_meta = pallet_meta_tx::MetaTxFor::<Runtime>::new(
			spec26_call,
			spec26_version,
			spec26_extension,
		);
		let spec26_len = spec26_meta.encoded_size() as u32;
		assert!(crate::meta_v6::inspect_paid_meta::<
			crate::meta_v6::ProductionMetadataImplicitResolver,
		>(
			&RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
				meta_tx: Box::new(spec26_meta),
				meta_tx_encoded_len: spec26_len,
			}),
			0,
		)
		.is_err());
		assert!(<(
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		)>::decode(&mut meta.encode().as_slice())
		.is_ok(), "SDK MetaTx encoding must match the Orbis inspection mirror");
		let encoded_len = meta.encoded_size() as u32;
		let outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(meta.clone()),
			meta_tx_encoded_len: encoded_len,
		});
		let authorized_meta = <Runtime as frame_system::offchain::CreateAuthorizedTransaction<
			RuntimeCall,
		>>::create_authorized_transaction(outer.clone());
		assert!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::Local,
			authorized_meta,
			System::block_hash(0),
		)
		.is_err());
		let authorized_defense =
			crate::meta_v6::PaidMetaScope::new(frame_system::CheckSpecVersion::<Runtime>::new());
		let authorized_implicit = authorized_defense.implicit().unwrap();
		assert!(authorized_defense
			.validate(
				frame_system::RawOrigin::Authorized.into(),
				&outer,
				&outer.get_dispatch_info(),
				outer.encoded_size(),
				authorized_implicit,
				&sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, &outer)),
				sp_runtime::transaction_validity::TransactionSource::Local,
			)
			.is_err());
		let utility_meta = RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![
				RuntimeCall::System(frame_system::Call::remark { remark: vec![] }),
				outer.clone(),
			],
		});
		let proxy_meta = RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: alice.clone().into(),
			force_proxy_type: Some(crate::ProxyType::Any),
			call: Box::new(outer.clone()),
		});
		let multisig_meta = RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
			other_signatories: vec![alice.clone()],
			call: Box::new(outer.clone()),
		});
		for allowed in [&outer, &utility_meta, &proxy_meta, &multisig_meta] {
			assert!(crate::meta_v6::inspect_paid_meta::<
				crate::meta_v6::ProductionMetadataImplicitResolver,
			>(allowed, 0)
			.unwrap()
			.is_some());
			assert!(
				!<crate::xcm_config::OrbisXcmSafeCallFilter as frame_support::traits::Contains<RuntimeCall>>::contains(
					allowed
				),
				"XCM/sovereign ingress cannot carry a paid Meta envelope"
			);
		}
		let denied_sudo =
			RuntimeCall::Sudo(pallet_sudo::Call::sudo { call: Box::new(outer.clone()) });
		let denied_scheduler = RuntimeCall::Scheduler(pallet_scheduler::Call::schedule {
			when: 2,
			maybe_periodic: None,
			priority: 0,
			call: Box::new(outer.clone()),
		});
		let approval_only = RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi {
			threshold: 2,
			other_signatories: vec![alice.clone()],
			maybe_timepoint: None,
			call_hash: [0u8; 32],
			max_weight: frame_support::weights::Weight::from_parts(1, 0),
		});
		for denied in [&denied_sudo, &denied_scheduler] {
			assert!(crate::meta_v6::inspect_paid_meta::<
				crate::meta_v6::ProductionMetadataImplicitResolver,
			>(denied, 0)
			.is_err());
		}
		let denial_payment: crate::PaymentPolicy = pallet_origin_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into();
		let denial_extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
			System::account_nonce(&bob),
			denial_payment,
			Default::default(),
		));
		let denial_payload = SignedPayload::new(denied_sudo.clone(), denial_extension.clone())
			.expect("the signed Sudo bypass payload encodes");
		let denial_signature = denial_payload.using_encoded(|bytes| bob_pair.sign(bytes));
		let denial_extrinsic =
			<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
				denied_sudo.clone(),
				bob.clone().into(),
				MultiSignature::Sr25519(denial_signature),
				denial_extension,
			);
		assert!(crate::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			denial_extrinsic,
			System::block_hash(0),
		)
		.is_err());
		assert!(crate::meta_v6::token().is_none());
		pallet_sudo::Key::<Runtime>::put(&bob);
		assert_ok!(crate::Sudo::sudo(RuntimeOrigin::signed(bob.clone()), Box::new(outer.clone())));
		assert!(System::events().iter().rev().any(|record| matches!(
			&record.event,
			crate::RuntimeEvent::Sudo(pallet_sudo::Event::Sudid { sudo_result: Err(_) })
		)));
		assert!(crate::meta_v6::token().is_none());
		assert_eq!(
			crate::meta_v6::inspect_paid_meta::<crate::meta_v6::ProductionMetadataImplicitResolver>(
				&approval_only,
				0
			),
			Ok(None)
		);
		assert!(
			!<crate::xcm_config::OrbisXcmSafeCallFilter as frame_support::traits::Contains<RuntimeCall>>::contains(
				&approval_only
			),
			"XCM rejects opaque approval even though signed pool ingress remains ordinary paid",
		);
		assert!(
			!outer.is_feeless(&RuntimeOrigin::signed(bob.clone())),
			"the sponsor's outer meta transaction must follow ordinary fee accounting"
		);
		let outer_info = outer.get_dispatch_info();
		let outer_extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
			System::account_nonce(&bob),
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into(),
			Default::default(),
		));
		let implicit = outer_extension.implicit().unwrap();
		let (_, val, outer_origin) = outer_extension
			.validate(
				RuntimeOrigin::signed(bob.clone()),
				&outer,
				&outer_info,
				outer.encoded_size(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &outer)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.unwrap();
		let pre = outer_extension
			.prepare(val, &outer_origin, &outer, &outer_info, outer.encoded_size())
			.unwrap();
		let bob_after_withdrawal = Balances::free_balance(&bob);
		assert!(bob_after_withdrawal < bob_balance);
		assert_eq!(System::account_nonce(&bob), 1);
		let post_info = crate::MetaTx::dispatch(outer_origin, Box::new(meta.clone()), encoded_len)
			.expect("the signed inner intent dispatches");
		assert_ok!(crate::TxExtensions::post_dispatch_details(
			pre,
			&outer_info,
			&post_info,
			outer.encoded_size(),
			&Ok(()),
		));
		assert!(Balances::free_balance(&bob) >= bob_after_withdrawal);
		assert!(Balances::free_balance(&bob) < bob_balance);
		assert!(matches!(
			crate::TransactionStorage::resource_reservation(0),
			Some(orbis_transaction_storage_primitives::ResourceReservationView::Active(_))
		));
		assert_eq!(System::account_nonce(&alice), 1);
		assert_eq!(Balances::free_balance(&alice), alice_balance);

		assert_noop!(
			crate::MetaTx::dispatch(
				RuntimeOrigin::signed(bob.clone()),
				Box::new(meta),
				encoded_len,
			),
			pallet_meta_tx::Error::<Runtime>::Invalid,
		);

		indiv_pallet_people::AccountToPersonalId::<Runtime>::insert(&alice, 7u64);
		indiv_pallet_people::People::<Runtime>::insert(
			7u64,
			indiv_pallet_people::types::PersonRecord {
				key: member.clone(),
				account: Some(alice.clone()),
			},
		);

		let lite_identifier = *indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER;
		assert_ok!(<Members as AppendOnlyMembers>::create_collection(
			Location::here(),
			&lite_identifier,
			1,
			RingMode::Flexible,
			crate::MembersFlexibleRingExponent::get(),
			None,
		));
		let lite_secret =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([93u8; 32]);
		let lite_member =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(
				&lite_secret,
			);
		assert_ok!(<Members as AppendOnlyMembers>::add_members(
			&lite_identifier,
			vec![lite_member.clone()],
		));
		assert_ok!(Members::onboard_members_authorized(
			frame_system::RawOrigin::Authorized.into(),
			lite_identifier,
			0,
			0,
			Some(lite_member.clone()),
			0,
		));
		assert_ok!(Members::build_ring_authorized(
			frame_system::RawOrigin::Authorized.into(),
			lite_identifier,
			0,
			crate::MembersFlexibleRingExponent::get(),
			None,
			1,
			0,
		));
		let lite_revision = <Members as MembershipProver>::ring_revision(&lite_identifier, 0)
			.expect("the lite test ring has a revision");
		indiv_pallet_people_lite::LitePeople::<Runtime>::insert(
			&alice,
			indiv_pallet_people_lite::types::LitePersonInfo {
				ring_vrf_key: lite_member.clone(),
				method: indiv_pallet_people_lite::types::RecognitionMethod::UniqueDevice(
					alice.clone(),
				),
			},
		);
		let lite_binding = indiv_support::traits::RevisedContextualAlias {
			revision: lite_revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: *indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				alias: [94u8; 32],
			},
		};
		indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&alice, &lite_binding);
		indiv_pallet_people_lite::AliasToAccount::<Runtime>::insert(&lite_binding.ca, &alice);

		let account_routes = [
			(
				crate::meta_v6::PolicyProofsV6 {
					personhood: Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccount),
					..Default::default()
				},
				RuntimeCall::Personhood(indiv_pallet_people::Call::unset_alias_account {}),
			),
			(
				crate::meta_v6::PolicyProofsV6 {
					personhood: Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalIdentityAccount),
					..Default::default()
				},
				RuntimeCall::Personhood(indiv_pallet_people::Call::unset_personal_id_account {}),
			),
			(
				crate::meta_v6::PolicyProofsV6 {
					people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LitePerson),
					..Default::default()
				},
				RuntimeCall::PeopleLite(indiv_pallet_people_lite::Call::dispatch_as_signer {
					call: Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
						remark: vec![2],
					})),
				}),
			),
			(
				crate::meta_v6::PolicyProofsV6 {
					people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccount),
					..Default::default()
				},
				RuntimeCall::PeopleLite(indiv_pallet_people_lite::Call::unset_alias_account {}),
			),
		];
		for (route, (proofs, success_call)) in account_routes.into_iter().enumerate() {
			let inner_nonce = System::account_nonce(&alice);
			let sponsor_nonce = System::account_nonce(&bob);
			let error_call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
				dest: bob.clone().into(),
				value: route as u128 + 1,
			});
			let error_meta = signed_meta_tx(error_call, alice.clone(), &alice_pair, proofs.clone());
			assert!(apply_meta_through_executive(error_meta, &bob, &bob_pair).is_err());
			assert_eq!(System::account_nonce(&alice), inner_nonce + 1);
			assert_eq!(System::account_nonce(&bob), sponsor_nonce + 1);
			assert!(crate::meta_v6::token().is_none());

			let success_inner_nonce = System::account_nonce(&alice);
			let success_sponsor_nonce = System::account_nonce(&bob);
			let route_meta = signed_meta_tx(success_call, alice.clone(), &alice_pair, proofs);
			assert_ok!(apply_meta_through_executive(route_meta, &bob, &bob_pair));
			assert_eq!(System::account_nonce(&alice), success_inner_nonce + 1);
			assert_eq!(System::account_nonce(&bob), success_sponsor_nonce + 1);
			match route {
				0 => assert!(!indiv_pallet_people::AccountToAlias::<Runtime>::contains_key(&alice)),
				1 => assert!(!indiv_pallet_people::AccountToPersonalId::<Runtime>::contains_key(
					&alice
				)),
				2 => assert!(System::events().iter().any(|record| matches!(
					record.event,
					crate::RuntimeEvent::System(frame_system::Event::Remarked { ref sender, .. })
						if sender == &alice
				))),
				3 => assert!(!indiv_pallet_people_lite::AccountToAlias::<Runtime>::contains_key(
					&alice
				)),
				_ => unreachable!(),
			}
			assert!(crate::meta_v6::token().is_none());
		}

		let (_, person_alias) = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
			commitment.clone(),
			&member_secret,
			&crate::ORBIS_PERSON_CONTEXT,
			&[0u8; 32],
		)
		.expect("the old person alias builds");
		let old_person_binding = indiv_support::traits::RevisedContextualAlias {
			revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: crate::ORBIS_PERSON_CONTEXT,
				alias: person_alias,
			},
		};
		indiv_pallet_people::AccountToAlias::<Runtime>::insert(&alice, &old_person_binding);
		indiv_pallet_people::AliasToAccount::<Runtime>::insert(&old_person_binding.ca, &alice);
		let second_secret =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([95u8; 32]);
		let second_member =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(
				&second_secret,
			);
		assert_ok!(<Members as AppendOnlyMembers>::add_members(
			&identifier,
			vec![second_member.clone()],
		));
		assert_ok!(Members::onboard_members_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			1,
			Some(second_member),
			0,
		));
		assert_ok!(Members::build_ring_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			crate::MembersFlexibleRingExponent::get(),
			Some(revision),
			1,
			1,
		));
		let revised_person_revision = <Members as MembershipProver>::ring_revision(&identifier, 0)
			.expect("the rebuilt person ring has a revision");
		assert!(revised_person_revision > revision);
		let revised_person_commitment =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::open(
				capacity,
				&member,
				<Members as AppendOnlyMembers>::ring_members(&identifier, 0).into_iter(),
			)
			.expect("the rebuilt person ring opens");
		let revised_person_target = AccountId::from([77u8; 32]);
		let revised_person_call =
			RuntimeCall::Personhood(indiv_pallet_people::Call::set_alias_account {
				account: revised_person_target.clone(),
				call_valid_at: 1,
			});
		let revised_person_message = revised_meta_message(
			b"orbis/meta/v6/personhood/alias-revised",
			&revised_person_call,
			&alice,
		);
		let (revised_person_proof, revised_person_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment.clone(),
				&member_secret,
				&crate::ORBIS_PERSON_CONTEXT,
				&revised_person_message,
			)
			.expect("the revised person proof builds");
		assert_eq!(revised_person_alias, person_alias);
		let verified_person = <Members as MembershipProver>::verify_membership(
			&identifier,
			&revised_person_proof,
			0,
			crate::ORBIS_PERSON_CONTEXT,
			&revised_person_message,
		)
		.expect("the revised person proof verifies before Meta construction");
		assert_eq!(verified_person.revision, revised_person_revision);
		assert_eq!(verified_person.ca, old_person_binding.ca);
		let inner_nonce = System::account_nonce(&alice);
		let sponsor_nonce = System::account_nonce(&bob);
		let revised_person_meta = signed_meta_tx(
			revised_person_call.clone(),
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				personhood: Some(
					crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
						revised_person_proof.clone(),
						0,
						crate::ORBIS_PERSON_CONTEXT,
					),
				),
				..Default::default()
			},
		);
		assert_ok!(apply_meta_through_executive(revised_person_meta, &bob, &bob_pair));
		assert_eq!(System::account_nonce(&alice), inner_nonce + 1);
		assert_eq!(System::account_nonce(&bob), sponsor_nonce + 1);
		assert_eq!(
			indiv_pallet_people::AccountToAlias::<Runtime>::get(&revised_person_target)
				.expect("the revised binding is stored")
				.revision,
			revised_person_revision,
		);
		indiv_pallet_people::AccountToAlias::<Runtime>::insert(&alice, &old_person_binding);
		indiv_pallet_people::AliasToAccount::<Runtime>::insert(&old_person_binding.ca, &alice);
		let revised_person_error_call =
			RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
				dest: bob.clone().into(),
				value: 1,
			});
		let revised_person_error_message = revised_meta_message(
			b"orbis/meta/v6/personhood/alias-revised",
			&revised_person_error_call,
			&alice,
		);
		let (revised_person_error_proof, _) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment.clone(),
				&member_secret,
				&crate::ORBIS_PERSON_CONTEXT,
				&revised_person_error_message,
			)
			.expect("the revised person dispatch-error proof builds");
		let revised_person_error_meta = signed_meta_tx(
			revised_person_error_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				personhood: Some(
					crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
						revised_person_error_proof,
						0,
						crate::ORBIS_PERSON_CONTEXT,
					),
				),
				..Default::default()
			},
		);
		assert!(apply_meta_through_executive(revised_person_error_meta, &bob, &bob_pair).is_err());
		assert_eq!(
			indiv_pallet_people::AccountToAlias::<Runtime>::get(&alice)
				.expect("prepare writes the revised binding before dispatch error")
				.revision,
			revised_person_revision,
		);
		let stale_revised_meta = signed_meta_tx(
			revised_person_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				personhood: Some(
					crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
						revised_person_proof,
						0,
						crate::ORBIS_PERSON_CONTEXT,
					),
				),
				..Default::default()
			},
		);
		let (_, _, stale_extension): (
			RuntimeCall,
			sp_runtime::generic::ExtensionVersion,
			crate::MetaTxExtension,
		) = Decode::decode(&mut stale_revised_meta.encode().as_slice()).unwrap();
		let (_, stale_consume, ..) = stale_extension;
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV7 {
			payer: bob.clone(),
			intent_commitment: stale_consume.0.commitment(),
			outer_nonce: System::account_nonce(&bob),
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			consumed: false,
		});
		let stale_len = stale_revised_meta.encoded_size() as u32;
		assert!(crate::MetaTx::dispatch(
			RuntimeOrigin::signed(bob.clone()),
			Box::new(stale_revised_meta),
			stale_len,
		)
		.is_err());
		assert_eq!(
			indiv_pallet_people::AccountToAlias::<Runtime>::get(&alice)
				.expect("stale revised proof cannot rewrite the binding")
				.revision,
			revised_person_revision,
		);
		crate::meta_v6::clear_token();

		let lite_commitment = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::open(
			capacity,
			&lite_member,
			<Members as AppendOnlyMembers>::ring_members(&lite_identifier, 0).into_iter(),
		)
		.expect("the old lite ring opens");
		let (_, lite_alias) = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
			lite_commitment,
			&lite_secret,
			&*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
			&[0u8; 32],
		)
		.expect("the old lite alias builds");
		let old_lite_binding = indiv_support::traits::RevisedContextualAlias {
			revision: lite_revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: *indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				alias: lite_alias,
			},
		};
		indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&alice, &old_lite_binding);
		indiv_pallet_people_lite::AliasToAccount::<Runtime>::insert(&old_lite_binding.ca, &alice);
		let second_lite_secret =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::new_secret([96u8; 32]);
		let second_lite_member =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::member_from_secret(
				&second_lite_secret,
			);
		assert_ok!(<Members as AppendOnlyMembers>::add_members(
			&lite_identifier,
			vec![second_lite_member.clone()],
		));
		assert_ok!(Members::onboard_members_authorized(
			frame_system::RawOrigin::Authorized.into(),
			lite_identifier,
			0,
			1,
			Some(second_lite_member),
			0,
		));
		assert_ok!(Members::build_ring_authorized(
			frame_system::RawOrigin::Authorized.into(),
			lite_identifier,
			0,
			crate::MembersFlexibleRingExponent::get(),
			Some(lite_revision),
			1,
			1,
		));
		let revised_lite_revision =
			<Members as MembershipProver>::ring_revision(&lite_identifier, 0)
				.expect("the rebuilt lite ring has a revision");
		assert!(revised_lite_revision > lite_revision);
		let revised_lite_commitment =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::open(
				capacity,
				&lite_member,
				<Members as AppendOnlyMembers>::ring_members(&lite_identifier, 0).into_iter(),
			)
			.expect("the rebuilt lite ring opens");
		let revised_lite_target = AccountId::from([78u8; 32]);
		let revised_lite_call =
			RuntimeCall::PeopleLite(indiv_pallet_people_lite::Call::set_alias_account {
				account: revised_lite_target.clone(),
				valid_at_block: 1,
			});
		let revised_lite_message = revised_meta_message(
			b"orbis/meta/v6/people-lite/alias-revised",
			&revised_lite_call,
			&alice,
		);
		let (revised_lite_proof, revised_lite_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_lite_commitment.clone(),
				&lite_secret,
				&*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				&revised_lite_message,
			)
			.expect("the revised lite proof builds");
		assert_eq!(revised_lite_alias, lite_alias);
		let verified_lite = <Members as MembershipProver>::verify_membership(
			&lite_identifier,
			&revised_lite_proof,
			0,
			*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
			&revised_lite_message,
		)
		.expect("the revised lite proof verifies before Meta construction");
		assert_eq!(verified_lite.revision, revised_lite_revision);
		assert_eq!(verified_lite.ca, old_lite_binding.ca);
		let inner_nonce = System::account_nonce(&alice);
		let sponsor_nonce = System::account_nonce(&bob);
		let revised_lite_meta = signed_meta_tx(
			revised_lite_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(
					revised_lite_proof,
					0,
					*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				)),
				..Default::default()
			},
		);
		assert_ok!(apply_meta_through_executive(revised_lite_meta, &bob, &bob_pair));
		assert_eq!(System::account_nonce(&alice), inner_nonce + 1);
		assert_eq!(System::account_nonce(&bob), sponsor_nonce + 1);
		assert_eq!(
			indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&revised_lite_target)
				.expect("the revised lite binding is stored")
				.revision,
			revised_lite_revision,
		);
		indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&alice, &old_lite_binding);
		indiv_pallet_people_lite::AliasToAccount::<Runtime>::insert(&old_lite_binding.ca, &alice);
		let revised_lite_error_call =
			RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
				dest: bob.clone().into(),
				value: 1,
			});
		let revised_lite_error_message = revised_meta_message(
			b"orbis/meta/v6/people-lite/alias-revised",
			&revised_lite_error_call,
			&alice,
		);
		let (revised_lite_error_proof, _) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_lite_commitment,
				&lite_secret,
				&*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				&revised_lite_error_message,
			)
			.expect("the revised lite dispatch-error proof builds");
		let revised_lite_error_meta = signed_meta_tx(
			revised_lite_error_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(
					revised_lite_error_proof,
					0,
					*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
				)),
				..Default::default()
			},
		);
		assert!(apply_meta_through_executive(revised_lite_error_meta, &bob, &bob_pair).is_err());
		assert_eq!(
			indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&alice)
				.expect("lite prepare writes revision before dispatch error")
				.revision,
			revised_lite_revision,
		);

		let resource_call =
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period,
				counter: 1,
				account_id: alice.clone(),
			});
		let resource_context = crate::Resources::long_term_storage_context(period, 1);
		let (_, resource_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment.clone(),
				&member_secret,
				&resource_context,
				&[0u8; 32],
			)
			.expect("the Executive Resources alias builds");
		let resource_binding = indiv_support::traits::RevisedContextualAlias {
			revision: revised_person_revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: crate::ORBIS_PERSON_CONTEXT,
				alias: resource_alias,
			},
		};
		indiv_pallet_people::AccountToAlias::<Runtime>::insert(&alice, &resource_binding);
		indiv_pallet_people::AliasToAccount::<Runtime>::insert(&resource_binding.ca, &alice);
		let resource_message = resource_meta_message(&resource_call, &alice);
		let (resource_proof, proof_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment.clone(),
				&member_secret,
				&resource_context,
				&resource_message,
			)
			.expect("the Executive Resources route proof builds");
		assert_eq!(proof_alias, resource_alias);
		let inner_nonce = System::account_nonce(&alice);
		let sponsor_nonce = System::account_nonce(&bob);
		let resource_meta = signed_meta_tx(
			resource_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				resources: Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(
					resource_proof,
					0,
					revised_person_revision,
					indiv_pallet_resources::types::MembershipCollection::People,
				)),
				..Default::default()
			},
		);
		assert_ok!(apply_meta_through_executive(resource_meta, &bob, &bob_pair));
		assert_eq!(System::account_nonce(&alice), inner_nonce + 1);
		assert_eq!(System::account_nonce(&bob), sponsor_nonce + 1);
		assert!(indiv_pallet_resources::SpentLongTermStorageAliases::<Runtime>::contains_key(
			indiv_support::utils::BigEndianU32::from(period),
			resource_alias,
		));
		assert!(crate::meta_v6::token().is_none());

		let resource_error_call =
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period,
				counter: 2,
				account_id: alice.clone(),
			});
		let resource_error_context = crate::Resources::long_term_storage_context(period, 2);
		let (_, resource_error_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment.clone(),
				&member_secret,
				&resource_error_context,
				&[0u8; 32],
			)
			.expect("the Resources dispatch-error alias builds");
		let resource_error_binding = indiv_support::traits::RevisedContextualAlias {
			revision: revised_person_revision,
			ring: 0,
			ca: indiv_support::traits::ContextualAlias {
				context: crate::ORBIS_PERSON_CONTEXT,
				alias: resource_error_alias,
			},
		};
		indiv_pallet_people::AccountToAlias::<Runtime>::insert(&alice, &resource_error_binding);
		indiv_pallet_people::AliasToAccount::<Runtime>::insert(&resource_error_binding.ca, &alice);
		let resource_error_message = resource_meta_message(&resource_error_call, &alice);
		let (resource_error_proof, proof_alias) =
			verifiable::ring::bandersnatch::BandersnatchVrfVerifiable::create(
				revised_person_commitment,
				&member_secret,
				&resource_error_context,
				&resource_error_message,
			)
			.expect("the Resources dispatch-error proof builds");
		assert_eq!(proof_alias, resource_error_alias);
		let resource_error_purpose =
			indiv_pallet_resources::types::ReservationPurpose::Membership {
				period,
				alias: resource_error_alias,
				counter: 2,
				collection: indiv_pallet_resources::types::MembershipCollection::People,
			};
		indiv_pallet_resources::StorageReservationByPurpose::<Runtime>::insert(
			&resource_error_purpose,
			999u64,
		);
		let resource_error_meta = signed_meta_tx(
			resource_error_call,
			alice.clone(),
			&alice_pair,
			crate::meta_v6::PolicyProofsV6 {
				resources: Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(
					resource_error_proof,
					0,
					revised_person_revision,
					indiv_pallet_resources::types::MembershipCollection::People,
				)),
				..Default::default()
			},
		);
		assert!(apply_meta_through_executive(resource_error_meta, &bob, &bob_pair).is_err());
		assert!(!indiv_pallet_resources::SpentLongTermStorageAliases::<Runtime>::contains_key(
			indiv_support::utils::BigEndianU32::from(period),
			resource_error_alias,
		));

		let forged = signed_meta_tx(
			inner,
			alice,
			&bob_pair,
			crate::meta_v6::PolicyProofsV6 { resources: Some(resource_info), ..Default::default() },
		);
		let forged_len = forged.encoded_size() as u32;
		assert_noop!(
			crate::MetaTx::dispatch(RuntimeOrigin::signed(bob), Box::new(forged), forged_len),
			pallet_meta_tx::Error::<Runtime>::BadProof,
		);
		cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
			cumulus_primitives_core::PersistedValidationData {
				parent_head: polkadot_parachain_primitives::primitives::HeadData(Vec::new()),
				relay_parent_number: 1,
				relay_parent_storage_root: Default::default(),
				max_pov_size: 1_000_000,
			},
		);
		cumulus_pallet_parachain_system::HostConfiguration::<Runtime>::put(
			cumulus_primitives_core::AbridgedHostConfiguration {
				max_code_size: 2 * 1024 * 1024,
				max_head_data_size: 1024 * 1024,
				max_upward_queue_count: 8,
				max_upward_queue_size: 1024,
				max_upward_message_size: 256,
				max_upward_message_num_per_candidate: 5,
				hrmp_max_message_num_per_candidate: 5,
				validation_upgrade_cooldown: 6,
				validation_upgrade_delay: 6,
				async_backing_params: polkadot_primitives::AsyncBackingParams {
					allowed_ancestry_len: 0,
					max_candidate_depth: 0,
				},
			},
		);
		cumulus_pallet_parachain_system::RelevantMessagingState::<Runtime>::put(
			cumulus_pallet_parachain_system::MessagingStateSnapshot {
				dmq_mqc_head: Default::default(),
				relay_dispatch_queue_remaining_capacity: Default::default(),
				ingress_channels: Vec::new(),
				egress_channels: Vec::new(),
			},
		);
		let header = crate::Executive::finalize_block();
		assert!(header.number > 0);
		assert!(crate::meta_v6::token().is_none());
	});
	if emit_v4 {}
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery() {
	sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery_core(true);
}

#[test]
fn location_conversion_works() {
	let alice_32 = AccountId32 { network: None, id: AccountId::from(ALICE).into() };
	let bob_20 = AccountKey20 { network: None, key: [123u8; 20] };

	// the purpose of hardcoded values is to catch an unintended location conversion logic change.
	struct TestCase {
		description: &'static str,
		location: Location,
		expected_account_id_str: &'static str,
	}

	let test_cases = vec![
		// DescribeTerminus
		TestCase {
			description: "DescribeTerminus Parent",
			location: Location::new(1, Here),
			expected_account_id_str: "5Dt6dpkWPwLaH4BBCKJwjiWrFVAGyYk3tLUabvyn4v7KtESG",
		},
		TestCase {
			description: "DescribeTerminus Sibling",
			location: Location::new(1, [Parachain(1111)]),
			expected_account_id_str: "5Eg2fnssmmJnF3z1iZ1NouAuzciDaaDQH7qURAy3w15jULDk",
		},
		// DescribePalletTerminal
		TestCase {
			description: "DescribePalletTerminal Parent",
			location: Location::new(1, [PalletInstance(50)]),
			expected_account_id_str: "5CnwemvaAXkWFVwibiCvf2EjqwiqBi29S5cLLydZLEaEw6jZ",
		},
		TestCase {
			description: "DescribePalletTerminal Sibling",
			location: Location::new(1, [Parachain(1111), PalletInstance(50)]),
			expected_account_id_str: "5GFBgPjpEQPdaxEnFirUoa51u5erVx84twYxJVuBRAT2UP2g",
		},
		// DescribeAccountId32Terminal
		TestCase {
			description: "DescribeAccountId32Terminal Parent",
			location: Location::new(1, [alice_32]),
			expected_account_id_str: "5DN5SGsuUG7PAqFL47J9meViwdnk9AdeSWKFkcHC45hEzVz4",
		},
		TestCase {
			description: "DescribeAccountId32Terminal Sibling",
			location: Location::new(1, [Parachain(1111), alice_32]),
			expected_account_id_str: "5DGRXLYwWGce7wvm14vX1Ms4Vf118FSWQbJkyQigY2pfm6bg",
		},
		// DescribeAccountKey20Terminal
		TestCase {
			description: "DescribeAccountKey20Terminal Parent",
			location: Location::new(1, [bob_20]),
			expected_account_id_str: "5CJeW9bdeos6EmaEofTUiNrvyVobMBfWbdQvhTe6UciGjH2n",
		},
		TestCase {
			description: "DescribeAccountKey20Terminal Sibling",
			location: Location::new(1, [Parachain(1111), bob_20]),
			expected_account_id_str: "5CE6V5AKH8H4rg2aq5KMbvaVUDMumHKVPPQEEDMHPy3GmJQp",
		},
		// DescribeTreasuryVoiceTerminal
		TestCase {
			description: "DescribeTreasuryVoiceTerminal Parent",
			location: Location::new(1, [Plurality { id: BodyId::Treasury, part: BodyPart::Voice }]),
			expected_account_id_str: "5CUjnE2vgcUCuhxPwFoQ5r7p1DkhujgvMNDHaF2bLqRp4D5F",
		},
		TestCase {
			description: "DescribeTreasuryVoiceTerminal Sibling",
			location: Location::new(
				1,
				[Parachain(1111), Plurality { id: BodyId::Treasury, part: BodyPart::Voice }],
			),
			expected_account_id_str: "5G6TDwaVgbWmhqRUKjBhRRnH4ry9L9cjRymUEmiRsLbSE4gB",
		},
		// DescribeBodyTerminal
		TestCase {
			description: "DescribeBodyTerminal Parent",
			location: Location::new(1, [Plurality { id: BodyId::Unit, part: BodyPart::Voice }]),
			expected_account_id_str: "5EBRMTBkDisEXsaN283SRbzx9Xf2PXwUxxFCJohSGo4jYe6B",
		},
		TestCase {
			description: "DescribeBodyTerminal Sibling",
			location: Location::new(
				1,
				[Parachain(1111), Plurality { id: BodyId::Unit, part: BodyPart::Voice }],
			),
			expected_account_id_str: "5DBoExvojy8tYnHgLL97phNH975CyT45PWTZEeGoBZfAyRMH",
		},
	];

	for tc in test_cases {
		let expected =
			AccountId::from_string(tc.expected_account_id_str).expect("Invalid AccountId string");

		let got = LocationToAccountHelper::<AccountId, LocationToAccountId>::convert_location(
			tc.location.into(),
		)
		.unwrap();

		assert_eq!(got, expected, "{}", tc.description);
	}
}

#[test]
fn xcm_payment_api_works() {
	use crate::{Block, Runtime, RuntimeCall, RuntimeOrigin, WeightToFee};
	parachains_runtimes_test_utils::test_cases::xcm_payment_api_with_native_token_works::<
		Runtime,
		RuntimeCall,
		RuntimeOrigin,
		Block,
		WeightToFee,
	>();
}

#[test]
#[cfg(feature = "runtime-benchmarks")]
fn native_benchmark_api_executes_all_meta_policy_targets() {
	use frame_benchmarking::{runtime_decl_for_benchmark::BenchmarkV2, BenchmarkConfig};

	let names = [
		"meta_policy_personal_alias",
		"meta_policy_personal_identity",
		"meta_policy_personal_alias_revised",
		"meta_policy_lite_person",
		"meta_policy_lite_alias",
		"meta_policy_lite_alias_revised",
		"meta_policy_resources_claim",
		"meta_policy_malformed",
		"meta_policy_mapping_miss",
		"meta_policy_revised_write",
		"meta_policy_max_proof",
		"meta_policy_envelope",
		"meta_policy_metadata_enabled",
		"meta_policy_metadata_disabled",
		"meta_policy_metadata_cannot_lookup",
		"meta_policy_metadata_max",
	];
	for name in names {
		let state = sc_client_db::BenchmarkingState::<sp_runtime::traits::BlakeTwo256>::new(
			Default::default(),
			None,
			false,
			false,
		)
		.expect("benchmark state opens");
		let mut overlay = Default::default();
		let mut ext = sp_state_machine::Ext::new(&mut overlay, &state, None);
		sp_externalities::set_and_run_with_externalities(&mut ext, || {
			System::set_block_number(1);
			let batches = Runtime::dispatch_benchmark(BenchmarkConfig {
				pallet: b"indiv_pallet_resources".to_vec(),
				instance: b"Resources".to_vec(),
				benchmark: name.as_bytes().to_vec(),
				selected_components: Vec::new(),
				verify: true,
				internal_repeats: 1,
			})
			.unwrap_or_else(|error| panic!("native {name} benchmark failed: {error}"));
			assert_eq!(batches.len(), 1, "{name} must yield one native batch");
			assert_eq!(batches[0].benchmark, name.as_bytes());
			assert_eq!(batches[0].results.len(), 1);
			assert!(
				batches[0].results[0].components.is_empty(),
				"fixed workload has no dimensions"
			);
		});
	}
}

#[test]
#[cfg(feature = "runtime-benchmarks")]
fn native_benchmark_api_discovers_and_executes_score_and_honour() {
	use frame_benchmarking::{
		runtime_decl_for_benchmark::BenchmarkV2, BenchmarkConfig, Benchmarking,
	};

	// Query these pallets directly: aggregate metadata also evaluates every registered pallet's
	// component bounds, and upstream collator-selection underflows when this enterprise runtime
	// deliberately configures zero invulnerables.
	let score_metadata = pallet_orbis_score::Pallet::<Runtime>::benchmarks(false);
	let honour_metadata = pallet_orbis_honour::Pallet::<Runtime>::benchmarks(false);
	assert!(score_metadata.iter().any(|entry| entry.name == b"set_payout_account"));
	assert!(score_metadata.iter().any(|entry| entry.name == b"as_participant_tx_ext"));
	assert!(honour_metadata.iter().any(|entry| entry.name == b"extension_validate"));
	for (pallet, instance, benchmark) in [
		(b"pallet_orbis_score".as_slice(), b"Score".as_slice(), b"set_payout_account".as_slice()),
		(
			b"pallet_orbis_score".as_slice(),
			b"Score".as_slice(),
			b"as_participant_tx_ext".as_slice(),
		),
		(b"pallet_orbis_honour".as_slice(), b"Honour".as_slice(), b"extension_validate".as_slice()),
	] {
		let state = sc_client_db::BenchmarkingState::<sp_runtime::traits::BlakeTwo256>::new(
			Default::default(),
			None,
			false,
			false,
		)
		.unwrap();
		let mut overlay = Default::default();
		let mut ext = sp_state_machine::Ext::new(&mut overlay, &state, None);
		sp_externalities::set_and_run_with_externalities(&mut ext, || {
			System::set_block_number(1);
			let batches = Runtime::dispatch_benchmark(BenchmarkConfig {
				pallet: pallet.to_vec(),
				instance: instance.to_vec(),
				benchmark: benchmark.to_vec(),
				selected_components: Vec::new(),
				verify: true,
				internal_repeats: 1,
			})
			.unwrap_or_else(|error| panic!("benchmark failed: {error}"));
			assert_eq!(batches.len(), 1);
			assert_eq!(batches[0].benchmark, benchmark);
			assert_eq!(batches[0].results.len(), 1);
		});
	}
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct CannotLookupMetadataResolver;

impl crate::meta_v6::MetadataImplicitResolver for CannotLookupMetadataResolver {
	fn resolve(
		_: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	) -> Result<Option<[u8; 32]>, sp_runtime::transaction_validity::TransactionValidityError> {
		Err(sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into())
	}
}

#[test]
fn paid_meta_scope_implicit_is_wire_transparent_and_only_delegates_core() {
	use codec::{DecodeAll, Encode};
	use frame_support::traits::BuildGenesisConfig;
	use sp_runtime::traits::TransactionExtension;
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
	let production = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
		0,
		pallet_origin_feeless::ChargeOrSkipFeeless::from(
			pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
		)
		.into(),
		pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
	));
	fn assert_production_default(_: &crate::TxExtensions) {}
	assert_production_default(&production);
	let injected = crate::meta_v6::PaidMetaScope::<_, CannotLookupMetadataResolver>::from(
		production.0.clone(),
	);
	let weight_call = RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
	assert!(injected
		.weight(&weight_call)
		.all_gte(crate::weights::meta_v6::metadata_outer_implicit()));
	assert!(injected.weight(&weight_call).all_gte(
		<<Runtime as indiv_pallet_resources::Config>::WeightInfo as
			indiv_pallet_resources::weights::WeightInfo>::meta_policy_metadata_max(),
	));
	assert_eq!(injected.implicit(), production.implicit());
	assert_eq!(injected.encode(), production.encode());
	let decoded = crate::meta_v6::PaidMetaScope::<
		crate::OuterCoreExtensions,
		CannotLookupMetadataResolver,
	>::decode_all(&mut injected.encode().as_slice())
	.unwrap();
	assert_eq!(decoded.encode(), production.encode());
	assert_eq!(
		<crate::meta_v6::PaidMetaScope<crate::OuterCoreExtensions, CannotLookupMetadataResolver> as scale_info::TypeInfo>::type_info(),
		<crate::OuterCoreExtensions as scale_info::TypeInfo>::type_info(),
	);
	});
}

#[test]
fn metadata_custom_hash_loss_is_detected_after_wire_roundtrip() {
	use codec::{DecodeAll, Encode};
	use sp_runtime::traits::TransactionExtension;
	let custom = [0x6du8; 32];
	let other = [0x7eu8; 32];
	let extension =
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new_with_custom_hash(custom);
	assert_eq!(
		orbis_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(&extension).unwrap(),
		Some(custom),
	);
	let decoded = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::decode_all(
		&mut extension.encode().as_slice(),
	)
	.unwrap();
	assert_eq!(decoded.encode(), extension.encode());
	assert_eq!(
		extension.encode(),
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new_with_custom_hash(other)
			.encode(),
	);
	assert_eq!(
		extension.encode(),
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true).encode(),
	);
	let decoded_implicit =
		orbis_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(&decoded);
	assert_eq!(
		decoded_implicit,
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true).implicit(),
	);
	use sp_core::Pair;
	let pair = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
	let signature = pair.sign(&(extension.encode(), Some(custom)).encode());
	assert!(sp_core::sr25519::Pair::verify(
		&signature,
		&(extension.encode(), Some(custom)).encode(),
		&pair.public(),
	));
	assert!(!sp_core::sr25519::Pair::verify(
		&signature,
		&(extension.encode(), Some(other)).encode(),
		&pair.public(),
	));
}

#[test]
fn checkpoint_duty_runtime_api_has_exact_128_snapshot_paging_contract() {
	use orbis_storage_runtime_api as storage_api;
	use pallet_orbis_storage_provider::{
		BucketIds, BucketRecord, Buckets, CheckpointDutyCurrent, CheckpointDutyPending,
		CheckpointDutyRecord, GovernedFinalizedCheckpoint, ReplicasOf,
	};

	fn duty(index: u32) -> pallet_orbis_storage_provider::CheckpointDutyRecordOf<Runtime> {
		let replicas: ReplicasOf<Runtime> =
			vec![AccountId::new([2; 32]), AccountId::new([3; 32])].try_into().unwrap();
		CheckpointDutyRecord {
			bucket_id: sp_core::H256::from_low_u64_be(index as u64 + 1),
			primary: AccountId::new([1; 32]),
			replicas,
			previous_checkpoint: 42,
			previous_commitment: None,
			expected_next_start_seq: 0,
			due_at: 100 + index,
			grace_until: 120 + index,
			scheduled_at: 40,
			mode: pallet_orbis_storage_provider::CheckpointDutyMode::Standard,
			promotion_predecessor: None,
		}
	}
	fn put_duties(count: u32) {
		let mut ids = Vec::new();
		for index in 0..count {
			let record = duty(index);
			ids.push(record.bucket_id);
			Buckets::<Runtime>::insert(
				record.bucket_id,
				BucketRecord {
					owner: AccountId::new([9; 32]),
					version: 1,
					policy: sp_core::H256::repeat_byte(10),
					primary: record.primary.clone(),
					replicas: record.replicas.clone(),
					grants: Default::default(),
					created_at: 1,
				},
			);
			CheckpointDutyCurrent::<Runtime>::insert(record.bucket_id, record);
		}
		let ids: frame_support::BoundedVec<crate::Hash, crate::ProviderMaxBuckets> =
			ids.try_into().expect("bounded runtime bucket index");
		BucketIds::<Runtime>::put(ids);
	}
	fn page(
		cursor: Option<storage_api::CheckpointDutyCursor<crate::BlockNumber>>,
		limit: u32,
	) -> Result<
		storage_api::CheckpointDutyPage<
			storage_api::CheckpointDutyInfo<AccountId, crate::Hash, crate::BlockNumber>,
			crate::BlockNumber,
		>,
		storage_api::CheckpointDutyPageError,
	> {
		crate::checkpoint_duty_page(AccountId::new([1; 32]), cursor, limit)
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		GovernedFinalizedCheckpoint::<Runtime>::put(42);
		for count in [0u32, 1, 127, 128, 129] {
			put_duties(count);
			let first = page(None, 128).unwrap();
			assert_eq!(first.items.len(), core::cmp::min(count, 128) as usize);
			assert_eq!(first.snapshot_checkpoint, 42);
			if count <= 128 {
				assert!(first.next_cursor.is_none());
			} else {
				let second = page(first.next_cursor.clone(), 128).unwrap();
				assert_eq!(second.items.len(), 1);
				assert!(second.next_cursor.is_none());
			}
		}
		put_duties(1);
		let bucket = duty(0).bucket_id;
		let mut changed = duty(0);
		changed.primary = AccountId::new([4; 32]);
		changed.scheduled_at = 42;
		CheckpointDutyPending::<Runtime>::insert(bucket, changed);
		assert_eq!(page(None, 128).unwrap().items.len(), 1);
		assert!(crate::checkpoint_duty_page(AccountId::new([4; 32]), None, 128)
			.unwrap()
			.items
			.is_empty());
		GovernedFinalizedCheckpoint::<Runtime>::put(43);
		assert!(page(None, 128).unwrap().items.is_empty());
		assert_eq!(
			crate::checkpoint_duty_page(AccountId::new([4; 32]), None, 128)
				.unwrap()
				.items
				.len(),
			1
		);
		CheckpointDutyPending::<Runtime>::remove(bucket);
		GovernedFinalizedCheckpoint::<Runtime>::put(42);
		assert_eq!(page(None, 0), Err(storage_api::CheckpointDutyPageError::PageLimitInvalid));
		assert_eq!(page(None, 129), Err(storage_api::CheckpointDutyPageError::PageLimitInvalid));

		put_duties(129);
		let first = page(None, 128).unwrap();
		let cursor = first.next_cursor.clone().unwrap();
		let mut unfinalized = duty(0);
		unfinalized.due_at = 98;
		unfinalized.scheduled_at = 43;
		CheckpointDutyPending::<Runtime>::insert(unfinalized.bucket_id, unfinalized);
		assert_eq!(page(Some(cursor.clone()), 128).unwrap().items.len(), 1);
		let invalid =
			storage_api::CheckpointDutyCursor { snapshot_checkpoint: 42, last_key: vec![0xff] };
		assert_eq!(
			page(Some(invalid), 128),
			Err(storage_api::CheckpointDutyPageError::CursorKeyInvalid)
		);

		let mut added = duty(129);
		added.bucket_id = sp_core::H256::zero();
		added.due_at = 99;
		added.grace_until = 119;
		added.previous_checkpoint = 43;
		added.scheduled_at = 43;
		CheckpointDutyPending::<Runtime>::insert(added.bucket_id, added);
		BucketIds::<Runtime>::try_mutate(|ids| ids.try_push(sp_core::H256::zero())).unwrap();
		GovernedFinalizedCheckpoint::<Runtime>::put(44);
		assert_eq!(
			page(Some(cursor), 128),
			Err(storage_api::CheckpointDutyPageError::CursorSnapshotStale)
		);
		let restarted = page(None, 128).unwrap();
		assert_eq!(restarted.items[0].bucket_id, sp_core::H256::zero());
		assert_eq!(restarted.items[0].due_at, 99);
		assert_eq!(restarted.items[1].due_at, 98);
		let tail = page(restarted.next_cursor, 128).unwrap();
		assert_eq!(restarted.items.len() + tail.items.len(), 130);
	});
}

#[test]
fn commons_checkpoint_wire_vector_production_profile_is_stable() {
	use pallet_orbis_storage_provider::{
		CheckpointDutyCurrent, CheckpointDutyMode, CheckpointDutyRecord,
		CheckpointFallbackPromotionV1, CommitmentPayloadV2, CommitmentV1, ReplicasOf,
	};
	use sp_core::Pair;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(101);
		frame_system::BlockHash::<Runtime>::insert(0, sp_core::H256::repeat_byte(0x11));
		frame_system::BlockHash::<Runtime>::insert(101, sp_core::H256::repeat_byte(0x33));
		let bucket_id = sp_core::H256::repeat_byte(0x44);
		let primary = AccountId::new([1; 32]);
		let replicas: ReplicasOf<Runtime> =
			vec![AccountId::new([2; 32]), AccountId::new([3; 32])].try_into().unwrap();
		let duty = CheckpointDutyRecord {
			bucket_id,
			primary: primary.clone(),
			replicas,
			previous_checkpoint: 100,
			previous_commitment: None,
			expected_next_start_seq: 0,
			due_at: 101,
			grace_until: 121,
			scheduled_at: 100,
			mode: CheckpointDutyMode::Standard,
			promotion_predecessor: None,
		};
		CheckpointDutyCurrent::<Runtime>::insert(bucket_id, &duty);
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id,
			commitment: CommitmentV1 {
				mmr_root: sp_core::H256::repeat_byte(0x55),
				start_seq: 0,
				leaf_count: 1,
			},
			nonce: 101,
		};
		let duty_preimage =
			pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_duty_preimage(&duty, 101);
		let duty_id =
			pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_duty_id(&duty, 101);
		let context =
			pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_context_for(&payload)
				.unwrap();
		let context_scale = context.encode();
		let mut context_message = b"cord/storage/checkpoint-context/v1".to_vec();
		context_message.extend_from_slice(&context_scale);
		let context_digest =
			pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_context_digest(&context);
		let pair = sp_core::ed25519::Pair::from_seed(&[1; 32]);
		let context_signature = pair.sign(&context_digest);
		let promotion =
			CheckpointFallbackPromotionV1 { version: 1, bucket_id, snapshot_nonce: 101, duty_id };
		assert_eq!((101u32).encode().len(), 4);
		assert_eq!(primary.encode().len(), 32);
		assert_eq!(
			hex::encode(&duty_preimage),
			"636f72642f73746f726167652f636865636b706f696e742d647574792f763211111111111111111111111111111111111111111111111111111111111111111f0000000800000000000000000000000000000000000000000000000000000000000000000000006500000033333333333333333333333333333333333333333333333333333333333333334444444444444444444444444444444444444444444444444444444444444444010101010101010101010101010101010101010101010101010101010101010108020202020202020202020202020202020202020202020202020202020202020203030303030303030303030303030303030303030303030303030303030303036400000000000000000000000065000000790000000000"
		);
		assert_eq!(
			hex::encode(duty_id.as_bytes()),
			"e3224e65a6802bcb25dd90842c5af86353bcec0c296263497f55e33e31d14f79"
		);
		assert_eq!(
			hex::encode(&context_scale),
			"0111111111111111111111111111111111111111111111111111111111111111111f0000000800000000000000000000000000000000000000000000000000000000000000000000003333333333333333333333333333333333333333333333333333333333333333e3224e65a6802bcb25dd90842c5af86353bcec0c296263497f55e33e31d14f79e900e784b0698b7a8ea9a84e96bc50196cbc2fc1e1354955c1a9f0b0d781cb1d"
		);
		assert_eq!(
			hex::encode(&context_message),
			"636f72642f73746f726167652f636865636b706f696e742d636f6e746578742f76310111111111111111111111111111111111111111111111111111111111111111111f0000000800000000000000000000000000000000000000000000000000000000000000000000003333333333333333333333333333333333333333333333333333333333333333e3224e65a6802bcb25dd90842c5af86353bcec0c296263497f55e33e31d14f79e900e784b0698b7a8ea9a84e96bc50196cbc2fc1e1354955c1a9f0b0d781cb1d"
		);
		assert_eq!(
			hex::encode(context_digest),
			"3740d134e048c20317e41178a4cb6badb1628a1cc2bf9a8760fbc1edf0442868"
		);
		assert_eq!(
			hex::encode(pair.public().0),
			"8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"
		);
		assert_eq!(
			hex::encode(context_signature.0),
			"8d70c432862088031d9376e584445337d4293b22cae8197ce2cab31c4420f557a8db69e27a0e503a08816f584a4eac6b76214407e484a8f6c69ae608a6b78b03"
		);
		assert!(sp_io::crypto::ed25519_verify(
			&context_signature,
			&context_digest,
			&pair.public()
		));
		assert_eq!(
			hex::encode(promotion.encode()),
			"01444444444444444444444444444444444444444444444444444444444444444465000000e3224e65a6802bcb25dd90842c5af86353bcec0c296263497f55e33e31d14f79"
		);
		assert_eq!(
			hex::encode(
				pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_promotion_digest(
					&promotion,
				)
			),
			"e01c29220d047b914f0d43b34ed27fd04723097bb41d11a325d14e66460d0a7e"
		);
	});
}

#[test]
fn checkpoint_duty_runtime_api_filters_members_and_preserves_typed_ineligible_views() {
	use orbis_storage_runtime_api::{
		CheckpointDutyMode as ApiDutyMode, CheckpointDutyPhase, ProviderDutyExclusion,
		ProviderDutyRole,
	};
	use pallet_orbis_storage_provider::{
		CheckpointDutyCurrent, CheckpointDutyMode, CheckpointDutyPending,
		GovernedFinalizedCheckpoint, ProviderStatus, Providers,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let owner = AccountId::new([9; 32]);
		let admission = admit_canonical_manifest(&owner, [0xA1; 32]);
		let primary = crate::checkpoint_duty_page(admission.primary.clone(), None, 128).unwrap();
		assert_eq!(primary.items.len(), 1);
		assert_eq!(primary.items[0].bucket_id, admission.bucket_id);
		assert_eq!(primary.items[0].initiator, Some(admission.primary.clone()));

		let eligible_replica = admission.replicas[0].clone();
		let replica_page =
			crate::checkpoint_duty_page(eligible_replica.clone(), None, 128).unwrap();
		assert_eq!(replica_page.items.len(), 1);
		let replica_view = replica_page.items[0]
			.authorities
			.iter()
			.find(|view| view.provider == eligible_replica)
			.unwrap();
		assert_eq!(replica_view.role, ProviderDutyRole::Replica);
		assert!(replica_view.eligible);
		assert!(replica_view.may_sign);
		assert!(!replica_view.may_initiate);

		let suspended = admission.replicas[1].clone();
		Providers::<Runtime>::mutate(&suspended, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		let suspended_page = crate::checkpoint_duty_page(suspended.clone(), None, 128).unwrap();
		assert_eq!(suspended_page.items.len(), 1);
		let suspended_view = suspended_page.items[0]
			.authorities
			.iter()
			.find(|view| view.provider == suspended)
			.unwrap();
		assert!(!suspended_view.eligible);
		assert!(!suspended_view.may_sign);
		assert!(!suspended_view.may_initiate);
		assert_eq!(suspended_view.exclusion, Some(ProviderDutyExclusion::Inactive));

		let mut fallback = CheckpointDutyCurrent::<Runtime>::get(admission.bucket_id).unwrap();
		CheckpointDutyPending::<Runtime>::remove(admission.bucket_id);
		fallback.due_at = 101;
		fallback.grace_until = 121;
		fallback.mode = CheckpointDutyMode::Standard;
		CheckpointDutyCurrent::<Runtime>::insert(admission.bucket_id, &fallback);
		GovernedFinalizedCheckpoint::<Runtime>::put(121);
		let fallback_page =
			crate::checkpoint_duty_page(eligible_replica.clone(), None, 128).unwrap();
		assert_eq!(fallback_page.items[0].phase, CheckpointDutyPhase::ReplicaFallbackPromotion);
		assert_eq!(fallback_page.items[0].mode, ApiDutyMode::Standard);
		assert_eq!(fallback_page.items[0].initiator, Some(eligible_replica.clone()));

		fallback.primary = eligible_replica.clone();
		fallback.replicas = vec![admission.primary.clone(), suspended.clone()].try_into().unwrap();
		fallback.due_at = 121;
		fallback.grace_until = 141;
		fallback.mode = CheckpointDutyMode::PromotionPending;
		fallback.promotion_predecessor = Some(admission.primary.clone());
		CheckpointDutyCurrent::<Runtime>::insert(admission.bucket_id, &fallback);
		let blocked = crate::checkpoint_duty_page(eligible_replica.clone(), None, 128).unwrap();
		assert_eq!(blocked.items[0].phase, CheckpointDutyPhase::BlockedInsufficientFallbackQuorum);
		assert_eq!(blocked.items[0].mode, ApiDutyMode::PromotionPending);
		assert_eq!(blocked.items[0].initiator, None);
		Providers::<Runtime>::mutate(&suspended, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Active
		});
		let repaired = crate::checkpoint_duty_page(eligible_replica.clone(), None, 128).unwrap();
		assert_eq!(repaired.items[0].phase, CheckpointDutyPhase::Primary);
		assert_eq!(repaired.items[0].initiator, Some(eligible_replica));

		assert!(crate::checkpoint_duty_page(AccountId::new([0xEE; 32]), None, 128)
			.unwrap()
			.items
			.is_empty());
	});
}

#[test]
fn runtime_checkpoint_duty_admission_is_exactly_255_256_257() {
	use pallet_orbis_storage_provider::{
		DutyAdmissionCount, Error as StorageError, OrganizationRefOf, ProviderOrganizationRefV1,
		ProviderRecord, ProviderStatus, Providers, ReplicasOf, ServiceKeyRecord,
	};
	use sp_core::ed25519;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::put(1);
		let provider = |byte: u8| AccountId::new([byte; 32]);
		for byte in 1..=3 {
			let key = ed25519::Public::from_raw([byte; 32]);
			let organization: OrganizationRefOf<Runtime> = ProviderOrganizationRefV1 {
				entity_id: vec![byte].try_into().unwrap(),
				attestation_id: sp_core::H256::repeat_byte(byte),
				schema_id: sp_core::H256::repeat_byte(10),
				sla_commitment: sp_core::H256::repeat_byte(11),
				sla_version: 1,
				valid_from: 1,
				valid_until: 1_000,
				rotation_predecessor: None,
			};
			Providers::<Runtime>::insert(
				provider(byte),
				ProviderRecord {
					endpoint: vec![byte].try_into().unwrap(),
					organization,
					service_key: ServiceKeyRecord {
						active: key,
						active_version: 1,
						previous: None,
						pending: None,
						pending_version: None,
						pending_effective_at: None,
					},
					capacity_bytes: u64::MAX,
					allocated_bytes: 0,
					pending_bytes: 0,
					status: ProviderStatus::Active,
					last_heartbeat: 1,
					authority_validated_at: Some(1),
				},
			);
		}
		let replicas: ReplicasOf<Runtime> = vec![provider(2), provider(3)].try_into().unwrap();
		let owner = AccountId::new([9; 32]);
		assert!(crate::checkpoint_duty_page(provider(1), None, 128).unwrap().items.is_empty());
		for index in 0..255u32 {
			assert_ok!(crate::StorageProvider::create_bucket(
				crate::RuntimeOrigin::signed(owner.clone()),
				sp_core::H256::from_low_u64_be(index as u64 + 1),
				provider(1),
				replicas.clone(),
			));
			let count = index + 1;
			if matches!(count, 1 | 127 | 128 | 129) {
				pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::put(2);
				let first = crate::checkpoint_duty_page(provider(1), None, 128).unwrap();
				assert_eq!(first.items.len(), core::cmp::min(count, 128) as usize);
				assert!(first
					.items
					.iter()
					.all(|duty| duty.authorities.len() == 3 &&
						duty.required_replica_confirmations == 2));
				if count == 129 {
					let second =
						crate::checkpoint_duty_page(provider(1), first.next_cursor, 128).unwrap();
					assert_eq!(second.items.len(), 1);
				} else {
					assert!(first.next_cursor.is_none());
				}
				pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::put(1);
			}
		}
		assert!(crate::checkpoint_duty_page(owner.clone(), None, 128).unwrap().items.is_empty());
		assert_eq!(DutyAdmissionCount::<Runtime>::get(), 255);
		let bucket = pallet_orbis_storage_provider::BucketIds::<Runtime>::get()[0];
		pallet_orbis_storage_provider::BucketSnapshots::<Runtime>::insert(
			bucket,
			pallet_orbis_storage_provider::BucketSnapshot {
				commitment: pallet_orbis_storage_provider::CommitmentV1 {
					mmr_root: sp_core::H256::repeat_byte(1),
					start_seq: 0,
					leaf_count: 1,
				},
				checkpoint_block: 1,
				primary_signers: 1,
				commitment_nonce: 1,
				replica_confirmations: replicas.clone(),
			},
		);
		assert_ok!(crate::StorageProvider::issue_challenge(
			crate::RuntimeOrigin::root(),
			bucket,
			provider(1),
			pallet_orbis_storage_provider::ChunkLocationV1 { leaf_index: 0, chunk_index: 0 },
			2,
		));
		assert_eq!(DutyAdmissionCount::<Runtime>::get(), 256);
		assert_noop!(
			crate::StorageProvider::create_bucket(
				crate::RuntimeOrigin::signed(owner.clone()),
				sp_core::H256::from_low_u64_be(256),
				provider(1),
				replicas.clone(),
			),
			StorageError::<Runtime>::CheckpointDutyLimit
		);
		assert_noop!(
			crate::StorageProvider::issue_challenge(
				crate::RuntimeOrigin::root(),
				bucket,
				provider(1),
				pallet_orbis_storage_provider::ChunkLocationV1 { leaf_index: 1, chunk_index: 0 },
				2,
			),
			StorageError::<Runtime>::ChallengeDutyLimit
		);
		assert_eq!(DutyAdmissionCount::<Runtime>::get(), 256);
		assert_eq!(pallet_orbis_storage_provider::BucketIds::<Runtime>::get().len(), 255);
		assert_eq!(
			pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(bucket).len(),
			0
		);
		for index in 0..31 {
			assert_ok!(crate::StorageProvider::propose_agreement(
				crate::RuntimeOrigin::signed(AccountId::new([9; 32])),
				bucket,
				1,
				1,
				1_000,
			));
			if index == 0 {
				assert_eq!(
					pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(bucket).len(),
					1
				);
			}
		}
		assert_eq!(
			pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(bucket).len(),
			31
		);
		assert_ok!(crate::StorageProvider::propose_agreement(
			crate::RuntimeOrigin::signed(AccountId::new([9; 32])),
			bucket,
			1,
			1,
			1_000,
		));
		assert_eq!(
			pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(bucket).len(),
			32
		);
		assert_noop!(
			crate::StorageProvider::propose_agreement(
				crate::RuntimeOrigin::signed(AccountId::new([9; 32])),
				bucket,
				1,
				1,
				1_000,
			),
			StorageError::<Runtime>::AgreementIndexFull
		);

		System::set_block_number(2);
		let provider_full_bucket_hash = sp_core::H256::from_low_u64_be(10_000);
		assert_ok!(crate::StorageProvider::create_bucket(
			crate::RuntimeOrigin::signed(owner.clone()),
			provider_full_bucket_hash,
			provider(1),
			replicas,
		));
		let provider_full_bucket = pallet_orbis_storage_provider::BucketIds::<Runtime>::get()
			.last()
			.copied()
			.unwrap();
		let full_provider_index: frame_support::BoundedVec<
			sp_core::H256,
			crate::ProviderMaxAgreements,
		> = (0..1_024u64)
			.map(|index| sp_core::H256::from_low_u64_be(20_000 + index))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		pallet_orbis_storage_provider::ProviderAgreements::<Runtime>::insert(
			provider(1),
			full_provider_index,
		);
		let bucket_index_before =
			pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(provider_full_bucket);
		let provider_indexes_before = (1..=3)
			.map(|byte| {
				pallet_orbis_storage_provider::ProviderAgreements::<Runtime>::get(provider(byte))
			})
			.collect::<Vec<_>>();
		let providers_before = (1..=3)
			.map(|byte| Providers::<Runtime>::get(provider(byte)))
			.collect::<Vec<_>>();
		let agreement_count = pallet_orbis_storage_provider::Agreements::<Runtime>::iter().count();
		let nonce = pallet_orbis_storage_provider::AgreementNonce::<Runtime>::get(&owner);
		let events = System::events().len();
		assert_noop!(
			crate::StorageProvider::propose_agreement(
				crate::RuntimeOrigin::signed(owner.clone()),
				provider_full_bucket,
				1,
				1,
				1_000,
			),
			StorageError::<Runtime>::AgreementIndexFull
		);
		assert_eq!(
			pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(provider_full_bucket),
			bucket_index_before
		);
		assert_eq!(
			(1..=3)
				.map(|byte| {
					pallet_orbis_storage_provider::ProviderAgreements::<Runtime>::get(provider(
						byte,
					))
				})
				.collect::<Vec<_>>(),
			provider_indexes_before
		);
		assert_eq!(
			(1..=3)
				.map(|byte| Providers::<Runtime>::get(provider(byte)))
				.collect::<Vec<_>>(),
			providers_before
		);
		assert_eq!(
			pallet_orbis_storage_provider::Agreements::<Runtime>::iter().count(),
			agreement_count
		);
		assert_eq!(pallet_orbis_storage_provider::AgreementNonce::<Runtime>::get(&owner), nonce);
		assert_eq!(System::events().len(), events);

		System::set_block_number(3);
		let backlog_255: frame_support::BoundedVec<
			sp_core::H256,
			crate::ProviderMaxChallengeBacklog,
		> = (0..255u64)
			.map(|index| sp_core::H256::from_low_u64_be(30_000 + index))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		pallet_orbis_storage_provider::ChallengeBacklog::<Runtime>::put(backlog_255);
		assert_eq!(pallet_orbis_storage_provider::ChallengeBacklog::<Runtime>::get().len(), 255);
		assert_ok!(crate::StorageProvider::issue_challenge(
			crate::RuntimeOrigin::root(),
			bucket,
			provider(1),
			pallet_orbis_storage_provider::ChunkLocationV1 { leaf_index: 255, chunk_index: 0 },
			4,
		));
		assert_eq!(pallet_orbis_storage_provider::ChallengeBacklog::<Runtime>::get().len(), 256);
		let challenges_before =
			pallet_orbis_storage_provider::Challenges::<Runtime>::iter().count();
		let events = System::events().len();
		assert_noop!(
			crate::StorageProvider::issue_challenge(
				crate::RuntimeOrigin::root(),
				bucket,
				provider(1),
				pallet_orbis_storage_provider::ChunkLocationV1 { leaf_index: 256, chunk_index: 0 },
				4,
			),
			StorageError::<Runtime>::ChallengeDutyLimit
		);
		assert_eq!(pallet_orbis_storage_provider::ChallengeBacklog::<Runtime>::get().len(), 256);
		assert_eq!(
			pallet_orbis_storage_provider::Challenges::<Runtime>::iter().count(),
			challenges_before
		);
		assert_eq!(DutyAdmissionCount::<Runtime>::get(), 1);
		assert_eq!(System::events().len(), events);

		let releases_255: frame_support::BoundedVec<
			sp_core::H256,
			crate::ProviderMaxCapacityReleasesPerBlock,
		> = (0..255u64)
			.map(|index| sp_core::H256::from_low_u64_be(40_000 + index))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		let earliest = System::block_number().saturating_add(crate::ProviderEvidenceWindow::get());
		let release_at = earliest.saturating_add((3 - earliest % 3) % 3);
		pallet_orbis_storage_provider::CapacityReleases::<Runtime>::insert(
			release_at,
			releases_255,
		);
		let agreements = pallet_orbis_storage_provider::BucketAgreements::<Runtime>::get(bucket);
		let accepted = agreements[0];
		let rejected = agreements[1];
		assert_ok!(crate::StorageProvider::terminate_agreement(
			crate::RuntimeOrigin::signed(owner.clone()),
			accepted,
			1,
		));
		assert_eq!(
			pallet_orbis_storage_provider::CapacityReleases::<Runtime>::get(release_at).len(),
			256
		);
		let accepted_record =
			pallet_orbis_storage_provider::Agreements::<Runtime>::get(accepted).unwrap();
		assert_eq!(
			accepted_record.status,
			pallet_orbis_storage_provider::AgreementStatus::Cancelled
		);
		assert_eq!(accepted_record.release_at, Some(release_at));
		let rejected_before =
			pallet_orbis_storage_provider::Agreements::<Runtime>::get(rejected).unwrap();
		let queue_before =
			pallet_orbis_storage_provider::CapacityReleases::<Runtime>::get(release_at);
		let events = System::events().len();
		assert_noop!(
			crate::StorageProvider::terminate_agreement(
				crate::RuntimeOrigin::signed(owner),
				rejected,
				1,
			),
			StorageError::<Runtime>::CapacityReleaseQueueFull
		);
		assert_eq!(
			pallet_orbis_storage_provider::Agreements::<Runtime>::get(rejected).unwrap(),
			rejected_before
		);
		assert_eq!(
			pallet_orbis_storage_provider::CapacityReleases::<Runtime>::get(release_at),
			queue_before
		);
		assert_eq!(System::events().len(), events);
	});
}

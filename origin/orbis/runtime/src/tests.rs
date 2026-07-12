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
	AssetsFreezer, AssetsHolder, Balances, Broker, ChunksManager, Entity, Feeless, ForeignAssets,
	ForeignAssetsFreezer, HopPromotion, Members, MembersNotifier, Nfts, People, PeopleLite,
	Personhood, PoolAssets, PoolAssetsFreezer, Revive, Runtime, RuntimeCall, RuntimeOrigin, System,
	TransactionStorage, Uniques,
};
use codec::{Decode, Encode};
use cumulus_primitives_core::ParaId;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::CheckIfFeeless,
	traits::{fungible::Mutate, Contains, Get, Hooks, PalletInfoAccess},
};
use pallet_broker::{CoreAssignment, CoreMask, Reservations, Schedule, ScheduleItem};
use polkadot_primitives::AccountId;
use sp_core::crypto::Ss58Codec;
use sp_runtime::traits::AsSystemOriginSigner;
use xcm::prelude::*;
use xcm_runtime_apis::conversions::LocationToAccountHelper;

#[path = "remediation_v3.rs"]
mod remediation_v3;

const ALICE: [u8; 32] = [1u8; 32];

#[test]
#[should_panic(expected = "Orbis paid Meta token leaked")]
fn post_transactions_rejects_a_leaked_paid_meta_token() {
	use frame_support::traits::PostTransactions;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV6 {
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
fn completion_manifest_is_parseable_finite_and_uniquely_indexed() {
	use std::collections::BTreeSet;

	let manifest: toml::Value =
		toml::from_str(include_str!("../../../../docs/orbis-completion-manifest.toml"))
			.expect("the frozen completion manifest must be valid TOML");
	assert_eq!(manifest["manifest_version"].as_integer(), Some(3));
	assert_eq!(manifest["replanning"]["iteration"].as_integer(), Some(4));
	assert_eq!(
		manifest["replanning"]["decision"].as_str(),
		Some("approved-verify-consume-bounded-remediation")
	);
	for (table, expected) in [
		("source", 7),
		("runtime_pallet", 72),
		("runtime_api", 109),
		("benchmark", 57),
		("migration", 22),
		("migration_pipeline", 1),
		("node_surface", 45),
		("acceptance", 26),
		("exclusion", 10),
		("package_provenance", 41),
		("protocol_call", 7),
		("protocol_storage", 14),
		("protocol_type", 13),
		("protocol_internal", 9),
		("protocol_view", 3),
		("protocol_event", 10),
		("protocol_error", 18),
		("protocol_benchmark", 9),
		("protocol_migration", 2),
		("protocol_invariant", 12),
		("protocol_acceptance", 20),
		("protocol_obligation", 10),
		("protocol_dependency", 3),
		("protocol_constant", 5),
		("meta_contract", 38),
		("meta_router_variant", 7),
		("meta_vector", 18),
		("meta_ingress", 20),
		("bulletin_v7_rehearsal", 13),
		("bulletin_v7_contract", 10),
		("provider_v8_contract", 4),
		("remediation_gate", 5),
	] {
		assert_eq!(manifest[table].as_array().map(Vec::len), Some(expected), "{table}");
	}
	let pallets = manifest["runtime_pallet"].as_array().unwrap();
	let indices = pallets
		.iter()
		.map(|row| row["index"].as_integer().unwrap())
		.collect::<BTreeSet<_>>();
	assert_eq!(indices.len(), pallets.len(), "pallet indices are unique");
	for table in [
		"source",
		"runtime_pallet",
		"runtime_api",
		"benchmark",
		"migration",
		"node_surface",
		"acceptance",
		"exclusion",
		"protocol_call",
		"protocol_storage",
		"protocol_type",
		"protocol_internal",
		"protocol_view",
		"protocol_event",
		"protocol_error",
		"protocol_benchmark",
		"protocol_migration",
		"protocol_invariant",
		"protocol_acceptance",
		"protocol_obligation",
		"protocol_dependency",
		"protocol_constant",
		"meta_contract",
		"meta_router_variant",
		"meta_vector",
		"meta_ingress",
		"bulletin_v7_rehearsal",
		"bulletin_v7_contract",
		"provider_v8_contract",
		"remediation_gate",
	] {
		let rows = manifest[table].as_array().unwrap();
		let ids = rows.iter().map(|row| row["id"].as_str().unwrap()).collect::<BTreeSet<_>>();
		assert_eq!(ids.len(), rows.len(), "{table} ids are unique");
	}
	let filesystem = manifest["package_provenance"]
		.as_array()
		.unwrap()
		.iter()
		.find(|row| row["package"].as_str() == Some("file-system-primitives"))
		.unwrap();
	assert_eq!(filesystem["license"].as_str(), Some("UNDECLARED-REQUIRES-LEGAL-CLEARANCE"));

	let provenance = manifest["package_provenance"].as_array().unwrap();
	let provenance_packages = provenance
		.iter()
		.map(|row| row["package"].as_str().unwrap())
		.collect::<BTreeSet<_>>();
	assert_eq!(provenance_packages.len(), provenance.len(), "package provenance is unique");
	for pallet in pallets {
		let package = pallet["package"].as_str().unwrap();
		let source = pallet["source"].as_str().unwrap();
		if source != "dhiway-sdk" ||
			matches!(package, "pallet-pgas-allowance" | "pallet-vesting" | "pallet-claims")
		{
			assert!(
				provenance_packages.contains(package),
				"retained local/adapted/planned package lacks provenance: {package}"
			);
		}
	}
	for (id, owner) in [
		("PAL-114", "slice-8"),
		("PAL-115", "slice-8"),
		("PAL-116", "slice-8"),
		("PAL-123", "slice-9"),
		("PAL-124", "slice-9"),
		("PAL-120", "slice-10"),
		("PAL-121", "slice-11"),
		("PAL-122", "slice-11"),
	] {
		let pallet = pallets.iter().find(|row| row["id"].as_str() == Some(id)).unwrap();
		assert_eq!(pallet["evidence"].as_str(), Some(owner), "{id} owner");
	}
	let node_surfaces = manifest["node_surface"].as_array().unwrap();
	for (owner, packages) in [
		("slice-10", &["storage-primitives", "pallet-storage-provider"][..]),
		(
			"slice-11",
			&[
				"file-system-primitives",
				"pallet-drive-registry",
				"s3-primitives",
				"pallet-s3-registry",
			][..],
		),
		("slice-12", &["storage-client", "file-system-client", "s3-client"][..]),
	] {
		for package in packages {
			let row = node_surfaces
				.iter()
				.find(|row| {
					row["kind"].as_str() == Some("source-crate") &&
						row["name"].as_str() == Some(*package)
				})
				.unwrap();
			assert_eq!(row["owner"].as_str(), Some(owner), "{package} owner");
			assert_eq!(row["evidence"].as_str(), Some(owner), "{package} evidence");
		}
	}
	for row in node_surfaces.iter().filter(|row| {
		matches!(
			row["kind"].as_str(),
			Some("provider-module" | "provider-http-method" | "retained-worker")
		)
	}) {
		assert_eq!(row["owner"].as_str(), Some("slice-12"));
		assert_eq!(row["evidence"].as_str(), Some("slice-12"));
	}

	let api_rows = manifest["runtime_api"].as_array().unwrap();
	for api in ["StorageProviderApi", "DriveRegistryApi", "S3RegistryApi"] {
		let rows = api_rows.iter().filter(|row| row["api"].as_str() == Some(api));
		let mut count = 0;
		for row in rows {
			assert_eq!(row["evidence"].as_str(), Some("slice-12"), "{api} owner");
			count += 1;
		}
		assert!(count > 0, "{api} must remain inventoried");
	}
	for row in api_rows {
		let method = row["method"].as_str().unwrap();
		let return_type = method.split_once("->").map(|(_, result)| result).unwrap_or("()");
		let max_results = row["max_results"].as_integer().unwrap();
		if return_type.contains("BoundedVec<") {
			assert_eq!(max_results, 100, "bounded API cardinality: {method}");
		} else if !return_type.contains("Vec<") {
			assert_eq!(max_results, 1, "scalar API cardinality: {method}");
		}
	}

	let revive = api_rows
		.iter()
		.filter(|row| row["api"].as_str() == Some("ReviveApi"))
		.map(|row| row["method"].as_str().unwrap())
		.collect::<Vec<_>>();
	assert_eq!(
		revive,
		[
			"eth_block() -> EthBlock",
			"eth_block_hash(number: U256) -> Option<H256>",
			"eth_receipt_data() -> Vec<ReceiptGasInfo>",
			"block_gas_limit() -> U256",
			"max_extrinsic_weight_in_gas() -> U256",
			"balance(address: H160) -> U256",
			"gas_price() -> U256",
			"nonce(address: H160) -> Nonce",
			"call(origin: AccountId, dest: H160, value: Balance, gas_limit: Option<Weight>, storage_deposit_limit: Option<Balance>, input_data: Vec<u8>) -> ContractResult<ExecReturnValue, Balance>",
			"instantiate(origin: AccountId, value: Balance, gas_limit: Option<Weight>, storage_deposit_limit: Option<Balance>, code: Code, data: Vec<u8>, salt: Option<[u8; 32]>) -> ContractResult<InstantiateReturnValue, Balance>",
			"eth_transact(tx: GenericTransaction) -> Result<EthTransactInfo<Balance>, EthTransactError>",
			"eth_transact_with_config(tx: GenericTransaction, config: DryRunConfig<Moment>) -> Result<EthTransactInfo<Balance>, EthTransactError>",
			"eth_estimate_gas(tx: GenericTransaction, config: DryRunConfig<Moment>) -> Result<U256, EthTransactError>",
			"eth_pre_dispatch_weight(tx: Vec<u8>) -> Result<Weight, EthTransactError>",
			"upload_code(origin: AccountId, code: Vec<u8>, storage_deposit_limit: Option<Balance>) -> CodeUploadResult<Balance>",
			"get_storage(address: H160, key: [u8; 32]) -> GetStorageResult",
			"get_storage_var_key(address: H160, key: Vec<u8>) -> GetStorageResult",
			"trace_block(block: Block, config: TracerType) -> Vec<(u32, Trace)>",
			"trace_tx(block: Block, tx_index: u32, config: TracerType) -> Option<Trace>",
			"trace_call(tx: GenericTransaction, config: TracerType) -> Result<Trace, EthTransactError>",
			"trace_call_with_config(tx: GenericTransaction, tracer_type: TracerType, config: TracingConfig) -> Result<Trace, EthTransactError>",
			"block_author() -> H160",
			"address(account_id: AccountId) -> H160",
			"account_id(address: H160) -> AccountId",
			"runtime_pallets_address() -> H160",
			"code(address: H160) -> Vec<u8>",
			"new_balance_with_dust(balance: U256) -> Result<(Balance, u32), BalanceConversionError>",
		]
	);
	assert!(
		manifest["node_surface"]
			.as_array()
			.unwrap()
			.iter()
			.any(|row| row["id"].as_str() == Some("NODE-bulletin-proof-provider") &&
				row["state"].as_str() == Some("planned") &&
				row["owner"].as_str() == Some("slice-12") &&
				row["evidence"]
					.as_str()
					.is_some_and(|evidence| evidence.starts_with("slice-12:"))),
		"Bulletin's node proof provider remains a truthful Slice 12 deliverable"
	);
	for package in ["pallet-orbis-entity", "pallet-orbis-feeless"] {
		let row = provenance.iter().find(|row| row["package"].as_str() == Some(package)).unwrap();
		assert_eq!(row["upstream_declared_license"].as_str(), Some("Apache-2.0"));
		assert_eq!(row["fork_license"].as_str(), Some("GPL-3.0-or-later"));
		assert!(row["legal_note"].as_str().is_some_and(|note| note.contains("SPDX")));
	}
	assert_eq!(manifest["migration_pipeline"][0]["state"].as_str(), Some("present-empty-pipeline"));
}

#[test]
fn remediation_manifest_v3_is_exact_and_semantically_frozen() {
	use std::collections::BTreeSet;

	let manifest: toml::Value =
		toml::from_str(include_str!("../../../../docs/orbis-completion-manifest.toml")).unwrap();
	let tables = [
		"meta_contract",
		"meta_router_variant",
		"meta_vector",
		"meta_ingress",
		"bulletin_v7_rehearsal",
		"bulletin_v7_contract",
		"provider_v8_contract",
		"remediation_gate",
	];
	for table in tables {
		for row in manifest[table].as_array().unwrap() {
			let row_id = row["id"].as_str().unwrap();
			for field in [
				"id",
				"source_paths",
				"test_or_command",
				"expected_assertion",
				"artifact_path",
				"artifact_sha256",
				"source_commit",
				"status",
			] {
				assert!(
					row[field].as_str().is_some_and(|value| !value.is_empty()),
					"{table}.{field}"
				);
			}
			let status = row["status"].as_str().unwrap();
			if status == "present" {
				let artifact_path = row["artifact_path"].as_str().unwrap();
				let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
				assert!(repo_root.join(artifact_path).is_file(), "{table} artifact exists");
				assert_ne!(
					row["artifact_sha256"].as_str(),
					Some("0000000000000000000000000000000000000000000000000000000000000000"),
					"present evidence has a nonzero digest"
				);
			} else {
				let artifact_path = format!("target/orbis-remediation/{row_id}.json");
				assert_eq!(
					row["artifact_path"].as_str(),
					Some(artifact_path.as_str()),
					"{table} artifact path"
				);
				assert_eq!(
					row["artifact_sha256"].as_str(),
					Some("0000000000000000000000000000000000000000000000000000000000000000"),
					"pending evidence has a zero digest"
				);
			}
			assert_eq!(
				row["source_commit"].as_str(),
				Some("d75ff22af02120daccdb5e017cfddc925e622ac5"),
				"{table} source boundary"
			);
			let expected_status = if status == "present" {
				"present"
			} else if matches!(table, "meta_vector" | "provider_v8_contract") ||
				(table == "remediation_gate" && row_id == "GATE-5-EVIDENCE")
			{
				"planned"
			} else {
				"implemented-pending-evidence"
			};
			assert_eq!(row["status"].as_str(), Some(expected_status), "{table} Gate-4 state");
		}
	}

	let ids = |table: &str| {
		manifest[table]
			.as_array()
			.unwrap()
			.iter()
			.map(|row| row["id"].as_str().unwrap())
			.collect::<Vec<_>>()
	};
	assert_eq!(
		ids("meta_router_variant"),
		[
			"ROUTER-PERSON-ALIAS",
			"ROUTER-PERSON-IDENTITY",
			"ROUTER-PERSON-ALIAS-REVISED",
			"ROUTER-LITE-PERSON",
			"ROUTER-LITE-ALIAS",
			"ROUTER-LITE-ALIAS-REVISED",
			"ROUTER-RESOURCES-CLAIM",
		]
	);
	assert_eq!(
		ids("bulletin_v7_rehearsal"),
		[
			"REF-MISSING",
			"REF-PARTIAL",
			"REF-STALE",
			"REF-DUPLICATE",
			"HASH-MISSING",
			"HASH-PARTIAL",
			"HASH-STALE",
			"HASH-DUPLICATE",
			"BOTH-PARTIAL-BAD-COUNTER",
			"EMPTY",
			"HISTORICAL-4C",
			"HISTORICAL-640",
			"MAX-VALID",
		]
	);
	assert_eq!(
		ids("remediation_gate"),
		[
			"GATE-1-DOCS-V3",
			"GATE-2-DORMANT",
			"GATE-3-BULLETIN-DORMANT",
			"GATE-4-SINGLE-INTEGRATION",
			"GATE-5-EVIDENCE",
		]
	);
	let ingress = manifest["meta_ingress"].as_array().unwrap();
	let allowed = ingress
		.iter()
		.filter(|row| row["decision"].as_str() == Some("allowed"))
		.map(|row| row["shape"].as_str().unwrap())
		.collect::<BTreeSet<_>>();
	assert_eq!(
		allowed,
		BTreeSet::from([
			"direct leaf",
			"Utility::batch",
			"Utility::batch_all",
			"Utility::force_batch",
			"Proxy::proxy",
			"Proxy::proxy_announced",
			"Multisig::as_multi concrete call",
			"Multisig::as_multi_threshold_1 concrete call",
		])
	);

	let contract = manifest["meta_contract"].as_array().unwrap();
	let value = |id: &str| {
		contract.iter().find(|row| row["id"].as_str() == Some(id)).unwrap()["value"]
			.as_str()
			.unwrap()
	};
	assert_eq!(
		value("META-COMPAT-SPEC"),
		"canonical positive spec_version = 27; canonical spec-26 literal is negative"
	);
	assert_eq!(value("META-COMPAT-TX"), "transaction_version = 6");
	assert_eq!(value("META-DIRECT-PAYER"), "Signed(call.account_id)");
	assert_eq!(value("META-WEIGHT-PAID"), "PaidMetaScope = 2R + 2W");
	assert_eq!(value("META-WEIGHT-BASE"), "Base leaf inspection = 1R");
	assert_eq!(value("META-WEIGHT-CONSUME"), "ConsumePaidMetaIngress = 1R + 1W");
	assert_eq!(
		value("META-ALIAS-VERIFY-CONSUME"),
		"(VerifySignature, ConsumePaidMetaIngress, MetaTxMarker, CheckNonZeroSender, CheckSpecVersion, CheckTxVersion, CheckGenesis, CheckMortality, CheckNonce, MetaAccountBoundPoliciesV6, ValidateStorageCalls, CheckMetadataHash)"
	);
	let bulletin = manifest["bulletin_v7_contract"].as_array().unwrap();
	let bulletin_value = |id: &str| {
		bulletin.iter().find(|row| row["id"].as_str() == Some(id)).unwrap()["value"]
			.as_str()
			.unwrap()
	};
	assert_eq!(bulletin_value("BUL-V7-READS"), "reads = A + T + L + I_ref + I_hash + 3L + 3");
	assert_eq!(bulletin_value("BUL-V7-WRITES"), "writes = I_ref + I_hash + 2L + 2 + 1");

	fn canonical_semantics(value: &toml::Value, output: &mut String) {
		match value {
			toml::Value::String(value) => {
				output.push_str("s");
				output.push_str(&value.len().to_string());
				output.push(':');
				output.push_str(value);
			},
			toml::Value::Integer(value) => output.push_str(&format!("i{value};")),
			toml::Value::Float(value) => output.push_str(&format!("f{:016x};", value.to_bits())),
			toml::Value::Boolean(value) => output.push_str(if *value { "b1;" } else { "b0;" }),
			toml::Value::Datetime(value) => output.push_str(&format!("d{value};")),
			toml::Value::Array(values) => {
				output.push('[');
				for value in values {
					canonical_semantics(value, output);
				}
				output.push(']');
			},
			toml::Value::Table(values) => {
				output.push('{');
				let mut keys = values
					.keys()
					.filter(|key| {
						!matches!(key.as_str(), "artifact_sha256" | "source_commit" | "status")
					})
					.collect::<Vec<_>>();
				keys.sort();
				for key in keys {
					canonical_semantics(&toml::Value::String(key.clone()), output);
					canonical_semantics(&values[key], output);
				}
				output.push('}');
			},
		}
	}
	let mut canonical = String::new();
	for table in tables {
		canonical.push_str(table);
		canonical_semantics(&manifest[table], &mut canonical);
	}
	let hash = sp_io::hashing::blake2_256(canonical.as_bytes())
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect::<String>();
	assert_eq!(hash, "47e0a570a3de291cd2e07d42dde5b84f6b6087ad754db2c77cfe742b1fa453d2");
}

#[test]
fn resources_bulletin_iteration_two_manifest_is_exact() {
	let manifest: toml::Value =
		toml::from_str(include_str!("../../../../docs/orbis-completion-manifest.toml")).unwrap();
	let rows = |table: &str| manifest[table].as_array().unwrap();
	let ids =
		|table: &str| rows(table).iter().map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>();

	fn canonical_semantics(value: &toml::Value, output: &mut String) {
		match value {
			toml::Value::String(value) => {
				output.push_str("s");
				output.push_str(&value.len().to_string());
				output.push(':');
				output.push_str(value);
			},
			toml::Value::Integer(value) => output.push_str(&format!("i{value};")),
			toml::Value::Float(value) => output.push_str(&format!("f{:016x};", value.to_bits())),
			toml::Value::Boolean(value) => output.push_str(if *value { "b1;" } else { "b0;" }),
			toml::Value::Datetime(value) => output.push_str(&format!("d{value};")),
			toml::Value::Array(values) => {
				output.push('[');
				for value in values {
					canonical_semantics(value, output);
				}
				output.push(']');
			},
			toml::Value::Table(values) => {
				output.push('{');
				let mut keys = values
					.keys()
					.filter(|key| !matches!(key.as_str(), "state" | "evidence"))
					.collect::<Vec<_>>();
				keys.sort();
				for key in keys {
					canonical_semantics(&toml::Value::String(key.clone()), output);
					canonical_semantics(&values[key], output);
				}
				output.push('}');
			},
		}
	}

	let mut canonical = String::new();
	for table in [
		"protocol_call",
		"protocol_storage",
		"protocol_type",
		"protocol_internal",
		"protocol_view",
		"protocol_event",
		"protocol_error",
		"protocol_benchmark",
		"protocol_migration",
		"protocol_invariant",
		"protocol_acceptance",
		"protocol_obligation",
		"protocol_dependency",
		"protocol_constant",
	] {
		canonical.push_str(table);
		canonical_semantics(&manifest[table], &mut canonical);
	}
	let api_semantics = manifest["runtime_api"]
		.as_array()
		.unwrap()
		.iter()
		.filter(|row| {
			matches!(
				row["id"].as_str(),
				Some(
					"API-BulletinTransactionStorageApi-04" |
						"API-BulletinTransactionStorageApi-05" |
						"API-BulletinTransactionStorageApi-06"
				)
			)
		})
		.cloned()
		.collect::<Vec<_>>();
	canonical.push_str("runtime_api:ResourcesBulletinIteration2");
	canonical_semantics(&toml::Value::Array(api_semantics), &mut canonical);
	let semantic_hash = sp_io::hashing::blake2_256(canonical.as_bytes());
	let semantic_hash = semantic_hash.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
	assert_eq!(
		semantic_hash, "f8c59405ae1b15d9e24d8e16ed5d452c7915e6b65e388188d1b0fbfef802af4d",
		"iteration-2 semantic rows changed; mutable state/evidence are deliberately excluded"
	);

	assert_eq!(
		ids("protocol_call"),
		[
			"CALL-Resources-12",
			"CALL-Resources-14",
			"CALL-Resources-15",
			"CALL-Resources-16",
			"CALL-Resources-17",
			"CALL-BulletinTransactionStorage-10",
			"CALL-BulletinTransactionStorage-11",
		]
	);
	let calls = rows("protocol_call");
	let call_shape = calls
		.iter()
		.map(|row| {
			(
				row["pallet"].as_str().unwrap(),
				row["index"].as_integer().unwrap(),
				row["name"].as_str().unwrap(),
				row["owner"].as_str().unwrap(),
			)
		})
		.collect::<Vec<_>>();
	assert_eq!(
		call_shape,
		[
			("Resources", 12, "claim_long_term_storage", "slice-1"),
			("Resources", 14, "reserved-unused", "compatibility"),
			("Resources", 15, "cancel_long_term_storage_reservation", "slice-1"),
			("Resources", 16, "reserved-unused", "compatibility"),
			("Resources", 17, "expire_long_term_storage_reservations", "slice-1"),
			("BulletinTransactionStorage", 10, "store_reserved", "slice-1"),
			("BulletinTransactionStorage", 11, "renew_reserved", "slice-1"),
		]
	);
	assert!(calls
		.iter()
		.filter(|row| row["status"].as_str() == Some("reserved-unused"))
		.all(|row| matches!(row["index"].as_integer(), Some(14 | 16))));

	assert_eq!(
		ids("protocol_storage"),
		[
			"STORE-Resources-NextStorageReservationId",
			"STORE-Resources-StorageClaims",
			"STORE-Resources-StorageReservationByPurpose",
			"STORE-BulletinV6-StoredBy",
			"STORE-BulletinV6-ResourceReservations",
			"STORE-BulletinV6-ResourceReservationExpiryBlocks",
			"STORE-BulletinV6-ResourceReservationExpiryBuckets",
			"STORE-BulletinV6-ResourceReservationExpiryCursor",
			"STORE-BulletinV6-ResourceReservationLinks",
			"STORE-BulletinV6-ResourceLinkByRef",
			"STORE-BulletinV6-ResourceReservationTombstones",
			"STORE-BulletinV6-TombstonePruneQueue",
			"STORE-BulletinV6-TombstonePruneCursor",
			"STORE-BulletinV6-ReservedPermanentCapacity",
		]
	);
	for row in rows("protocol_storage") {
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert_eq!(row["state"].as_str(), Some("present"));
	}

	assert_eq!(
		ids("protocol_type"),
		[
			"TYPE-ReservationId",
			"TYPE-ReservationPurpose",
			"TYPE-TwoPhaseStorage",
			"TYPE-BulletinRef",
			"TYPE-StorageActor",
			"TYPE-ResourceReservation",
			"TYPE-ResourceReservationLink",
			"TYPE-ResourceReservationTombstone",
			"TYPE-ResourceClaimLifecycle",
			"TYPE-ClaimCleanupOutcome",
			"TYPE-ResourceReservationView",
			"TYPE-PreparedReservedStore",
			"TYPE-PreparedReservedRenew",
		]
	);
	assert_eq!(
		ids("protocol_internal"),
		[
			"INTERNAL-prepare-reserved-store",
			"INTERNAL-commit-reserved-store",
			"INTERNAL-prepare-reserved-renew",
			"INTERNAL-commit-reserved-renew",
			"INTERNAL-reserve-resource-capacity",
			"INTERNAL-cancel-resource-capacity",
			"INTERNAL-expire-due-resource-capacity",
			"INTERNAL-prune-resource-tombstones",
			"INTERNAL-resource-claim-lifecycle",
		]
	);
	assert_eq!(
		ids("protocol_event"),
		[
			"EVENT-Resources-LongTermStorageReserved",
			"EVENT-Resources-LongTermStorageReservationCancelled",
			"EVENT-Resources-LongTermStorageReservationExpired",
			"EVENT-Bulletin-StoredContentProvenanceRecorded",
			"EVENT-Bulletin-ResourceCapacityReserved",
			"EVENT-Bulletin-ReservedContentStored",
			"EVENT-Bulletin-ReservedContentRenewed",
			"EVENT-Bulletin-ResourceCapacityReleased",
			"EVENT-Bulletin-ResourceReservationExpired",
			"EVENT-Bulletin-ResourceTombstonePruned",
		]
	);
	assert_eq!(
		ids("protocol_error"),
		[
			"ERROR-ReservationBackendFailed",
			"ERROR-ReservationIdOverflow",
			"ERROR-ReservationNotFound",
			"ERROR-NotReservationOwner",
			"ERROR-ClaimAlreadyReserved",
			"ERROR-ReservationNotActive",
			"ERROR-ReservationExpired",
			"ERROR-ContentNotFound",
			"ERROR-ContentAlreadyLinked",
			"ERROR-ContentTooLarge",
			"ERROR-TransactionAllowanceExhausted",
			"ERROR-BytesAllowanceExhausted",
			"ERROR-StoredContentOwnerMismatch",
			"ERROR-LegacyContentUnrenewable",
			"ERROR-BulletinRefHashMismatch",
			"ERROR-ExpiryBucketFull",
			"ERROR-ExpiryBlockSetFull",
			"ERROR-CleanupLimitExceeded",
		]
	);
	assert_eq!(
		ids("protocol_benchmark"),
		[
			"PBENCH-Resources-claim-long-term-storage",
			"PBENCH-Resources-cancel-long-term-storage-reservation",
			"PBENCH-Resources-expire-long-term-storage-reservations",
			"PBENCH-Bulletin-store-reserved",
			"PBENCH-Bulletin-renew-reserved",
			"PBENCH-Bulletin-provenance-actor-paths",
			"PBENCH-Bulletin-worst-expiry-cursor",
			"PBENCH-Bulletin-full-tombstone-scan",
			"PBENCH-Bulletin-cross-pallet-prune",
		]
	);

	let migrations = rows("protocol_migration");
	assert_eq!(ids("protocol_migration"), ["PMIG-Bulletin-V5-to-V6", "PMIG-Bulletin-V6-to-V7"]);
	assert_eq!(migrations[0]["from_version"].as_integer(), Some(5));
	assert_eq!(migrations[0]["to_version"].as_integer(), Some(6));
	assert_eq!(migrations[0]["owner"].as_str(), Some("slice-1"));
	assert_eq!(migrations[1]["from_version"].as_integer(), Some(6));
	assert_eq!(migrations[1]["to_version"].as_integer(), Some(7));
	assert_eq!(migrations[1]["owner"].as_str(), Some("slice-10"));
	for row in migrations {
		assert!(!row["pre_invariant"].as_str().unwrap().is_empty());
		assert!(!row["post_invariant"].as_str().unwrap().is_empty());
	}

	assert_eq!(
		ids("protocol_acceptance"),
		(1..=20).map(|n| format!("RES-BUL-{n:02}")).collect::<Vec<_>>()
	);
	assert_eq!(
		ids("protocol_obligation"),
		[
			"P0-AsResources-slot",
			"P0-AsResources-order",
			"P0-AsResources-defaults",
			"P0-AsResources-version",
			"P0-AsResources-metadata",
			"P0-AsResources-signing",
			"P0-AsResources-direct-meta",
			"P0-AsResources-ethereum",
			"P0-AsResources-authorized",
			"P0-ReservedStorage-validation",
		]
	);
	for row in rows("protocol_obligation") {
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
	}
	assert_eq!(
		ids("protocol_dependency"),
		["DEP-Slice3-ProofOfInk", "DEP-Slice10-Provider", "DEP-Slice14-UnifiedApp"]
	);

	for (row, name) in rows("protocol_type").iter().zip([
		"ReservationId",
		"ReservationPurpose",
		"TwoPhaseStorage",
		"BulletinRef",
		"StorageActor",
		"ResourceReservation",
		"ResourceReservationLink",
		"ResourceReservationTombstone",
		"ResourceClaimLifecycle",
		"ClaimCleanupOutcome",
		"ResourceReservationView",
		"PreparedReservedStore",
		"PreparedReservedRenew",
	]) {
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert!(!row["shape"].as_str().unwrap().is_empty(), "{name} shape");
		assert!(!row["contract"].as_str().unwrap().is_empty(), "{name} contract");
	}
	for (row, name) in rows("protocol_internal").iter().zip([
		"prepare_reserved_store",
		"commit_reserved_store",
		"prepare_reserved_renew",
		"commit_reserved_renew",
		"reserve_resource_capacity",
		"cancel_resource_capacity",
		"expire_due_resource_capacity",
		"prune_resource_tombstones",
		"ResourceClaimLifecycle::prune_claim",
	]) {
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert!(!row["contract"].as_str().unwrap().is_empty(), "{name} contract");
	}
	for (row, (name, shape)) in rows("protocol_view").iter().zip([
		("stored_content_provenance", "StorageActor<AccountId>"),
		(
			"resource_reservation",
			"Option<Active(ResourceReservation) | Tombstone(ResourceReservationTombstone)>",
		),
		("resource_reservation_link", "Option<ResourceReservationLink>"),
	]) {
		assert_eq!(row["api"].as_str(), Some("BulletinTransactionStorageApi"));
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["shape"].as_str(), Some(shape));
		assert_eq!(row["max_results"].as_integer(), Some(1));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
	}
	for (row, name) in rows("protocol_event").iter().zip([
		"LongTermStorageReserved",
		"LongTermStorageReservationCancelled",
		"LongTermStorageReservationExpired",
		"StoredContentProvenanceRecorded",
		"ResourceCapacityReserved",
		"ReservedContentStored",
		"ReservedContentRenewed",
		"ResourceCapacityReleased",
		"ResourceReservationExpired",
		"ResourceTombstonePruned",
	]) {
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
	}
	for (row, name) in rows("protocol_error").iter().zip([
		"ReservationBackendFailed",
		"ReservationIdOverflow",
		"ReservationNotFound",
		"NotReservationOwner",
		"ClaimAlreadyReserved",
		"ReservationNotActive",
		"ReservationExpired",
		"ContentNotFound",
		"ContentAlreadyLinked",
		"ContentTooLarge",
		"TransactionAllowanceExhausted",
		"BytesAllowanceExhausted",
		"StoredContentOwnerMismatch",
		"LegacyContentUnrenewable",
		"BulletinRefHashMismatch",
		"ExpiryBucketFull",
		"ExpiryBlockSetFull",
		"CleanupLimitExceeded",
	]) {
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
	}
	for (row, target) in rows("protocol_benchmark").iter().zip([
		"Resources::claim_long_term_storage",
		"Resources::cancel_long_term_storage_reservation",
		"Resources::expire_long_term_storage_reservations",
		"BulletinTransactionStorage::store_reserved",
		"BulletinTransactionStorage::renew_reserved",
		"BulletinTransactionStorage::all_provenance_actor_paths",
		"BulletinTransactionStorage::expire_due_resource_capacity",
		"BulletinTransactionStorage::prune_resource_tombstones",
		"BulletinTransactionStorage::ResourceClaimLifecycle",
	]) {
		assert_eq!(row["target"].as_str(), Some(target));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert_eq!(row["state"].as_str(), Some("present"));
	}
	for (row, keyword) in rows("protocol_invariant").iter().zip([
		"sum(active",
		"MaxPermanentStorageSize",
		"MaxReservations",
		"StorageClaims",
		"StorageReservationByPurpose",
		"strictly ascending",
		"TombstonePruneQueue",
		"current ResourceReservationLinks ref",
		"StoredBy[BulletinRef]",
		"only its ReservationId",
		"both count in PermanentStorageUsed",
		"host call",
	]) {
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert!(row["formula"].as_str().unwrap().contains(keyword));
	}
	for (row, keyword) in rows("protocol_acceptance").iter().zip([
		"reserve then store_reserved",
		"owner and collision",
		"person and lite-person",
		"partial bytes",
		"manual renew_reserved",
		"owner cancellation",
		"numeric expiry",
		"failure injection",
		"full expiry bucket",
		"active-plus-tombstone",
		"match and mismatch",
		"signed, root, preimage",
		"V5-to-V6",
		"direct AsResources",
		"MetaTx AsResources",
		"Utility, Proxy, Multisig",
		"Ethereum",
		"authorized/offchain",
		"person/lite quota exhaustion",
		"ordinary paid",
	]) {
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		assert!(row["scenario"].as_str().unwrap().contains(keyword));
	}
	for (row, (owner, consumer, required)) in rows("protocol_dependency").iter().zip([
		("slice-3", "slice-3", "ReservationPurpose::ProofOfInk"),
		("slice-10", "slice-10", "V6-to-V7 provider_ref migration"),
		("slice-14", "slice-14", "complete Resources reservation"),
	]) {
		assert_eq!(row["owner"].as_str(), Some(owner));
		assert_eq!(row["consumer"].as_str(), Some(consumer));
		assert!(row["requires"].as_str().unwrap().contains(required));
		assert!(!row["contract"].as_str().unwrap().is_empty());
	}

	let constants = rows("protocol_constant");
	assert_eq!(
		ids("protocol_constant"),
		[
			"CONST-MaxReservations",
			"CONST-MaxReservationExpiryBlocks",
			"CONST-MaxReservationsPerExpiryBlock",
			"CONST-MaxReservationLinks",
			"CONST-TombstoneRetention",
		]
	);
	for (row, (name, value, unit)) in constants.iter().zip([
		("MaxReservations", 256, "reservations"),
		("MaxReservationExpiryBlocks", 256, "distinct-blocks"),
		("MaxReservationsPerExpiryBlock", 256, "reservations-per-block"),
		("MaxReservationLinks", 1024, "links"),
		("TombstoneRetention", 100, "blocks"),
	]) {
		assert_eq!(row["name"].as_str(), Some(name));
		assert_eq!(row["value"].as_integer(), Some(value));
		assert_eq!(row["unit"].as_str(), Some(unit));
		assert_eq!(row["owner"].as_str(), Some("slice-1"));
		let policy = row["policy"].as_str().unwrap();
		assert!(policy.contains("production tuning requires a runtime upgrade"));
		assert!(policy.contains("replan"));
	}

	let resources = manifest["runtime_pallet"]
		.as_array()
		.unwrap()
		.iter()
		.find(|row| row["id"].as_str() == Some("PAL-096"))
		.unwrap();
	assert_eq!(resources["index"].as_integer(), Some(96));
	assert_eq!(resources["state"].as_str(), Some("present"));
	assert_eq!(resources["evidence"].as_str(), Some("origin/orbis/runtime/src/lib.rs:index-96"));
	let new_api = manifest["runtime_api"]
		.as_array()
		.unwrap()
		.iter()
		.filter(|row| {
			matches!(
				row["id"].as_str(),
				Some(
					"API-BulletinTransactionStorageApi-04" |
						"API-BulletinTransactionStorageApi-05" |
						"API-BulletinTransactionStorageApi-06"
				)
			)
		})
		.collect::<Vec<_>>();
	for (row, (id, signature)) in new_api.iter().zip([
		(
			"API-BulletinTransactionStorageApi-04",
			"stored_content_provenance(reference: BulletinRef<BlockNumber>) -> StorageActor<AccountId>",
		),
		(
			"API-BulletinTransactionStorageApi-05",
			"resource_reservation(reservation_id: ReservationId) -> Option<ResourceReservationView<AccountId, BlockNumber>>",
		),
		(
			"API-BulletinTransactionStorageApi-06",
			"resource_reservation_link(reservation_id: ReservationId, content_hash: ContentHash) -> Option<ResourceReservationLink<AccountId, BlockNumber>>",
		),
	]) {
		assert_eq!(row["id"].as_str(), Some(id));
		assert_eq!(row["method"].as_str(), Some(signature));
		assert_eq!(row["max_results"].as_integer(), Some(1));
		assert_eq!(row["state"].as_str(), Some("present"));
		assert_eq!(
			row["evidence"].as_str(),
			Some(
				"6401424b"
			)
		);
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

	assert_eq!(pallet_orbis_token::Pallet::<Runtime>::index(), 51);
	assert_eq!(pallet_orbis_register::Pallet::<Runtime>::index(), 52);
	assert_eq!(pallet_orbis_entity::Pallet::<Runtime>::index(), 53);
	assert_eq!(pallet_orbis_feeless::Pallet::<Runtime>::index(), 54);
	assert_eq!(indiv_pallet_resources::Pallet::<Runtime>::index(), 96);
	assert_eq!(crate::VERSION.spec_version, 27);
	assert_eq!(crate::VERSION.transaction_version, 6);

	assert_eq!(
		call_variants::<pallet_orbis_register::Call<Runtime>>(),
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
		call_variants::<pallet_orbis_entity::Call<Runtime>>(),
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
		call_variants::<pallet_orbis_feeless::Call<Runtime>>(),
		vec![(0, "add_feeless_account".into()), (1, "remove_feeless_account".into())]
	);
	assert_eq!(
		call_variants::<indiv_pallet_resources::Call<Runtime>>(),
		[
			(0, "register_lite_person"),
			(1, "register_person"),
			(2, "touch_person_authorization"),
			(3, "remove_expired_username_reservation"),
			(4, "update_identifier_key"),
			(5, "set_username_reservation_duration"),
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
		storage_names::<pallet_orbis_token::Pallet<Runtime>>(),
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
		storage_names::<pallet_orbis_register::Pallet<Runtime>>(),
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
		storage_names::<pallet_orbis_entity::Pallet<Runtime>>(),
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
		storage_names::<pallet_orbis_feeless::Pallet<Runtime>>(),
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
		NoPolicy<4>, // ScoreAsParticipant
		NoPolicy<5>, // GameAsInvited
		indiv_pallet_people_lite::extension::PeopleLiteAuth<Runtime>,
		NoPolicy<7>, // AsMember
		NoPolicy<8>, // AsCoinage
		indiv_pallet_resources::extension::AsResources<Runtime>,
		NoPolicy<10>, // VoterAuth
		frame_system::AuthorizeCall<Runtime>,
		NoPolicy<12>, // AsPgas
		NoPolicy<13>, // AsRingAlias
		NoPolicy<14>, // AsDotnsGateway
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
			pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
				Runtime,
				crate::BulletinCallInspector,
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
	assert_eq!(crate::VERSION.transaction_version, 6);

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
			_bulletin,
			_metadata,
			_set_origin,
		) = inner;
		let (_as_person, _people_lite, as_resources, _authorize_call) = policy;
		assert_eq!(as_resources.encode(), [0], "default surfaces cannot claim Resources origin");
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
			_policy,
			_bulletin,
			_metadata,
		) = extension;
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
			indiv_pallet_people_lite::extension::PeopleLiteAuth<Runtime>,
			indiv_pallet_resources::extension::AsResources<Runtime>,
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
			pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
			"PeopleLiteAuth",
			"AsResources",
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
			"ConsumePaidMetaIngressV6",
			"MetaTxMarker",
			"CheckNonZeroSender",
			"CheckSpecVersion",
			"CheckTxVersion",
			"CheckGenesis",
			"CheckMortality",
			"CheckNonce",
			"MetaAccountBoundPoliciesV6",
			"ValidateStorageCalls",
			"CheckMetadataHash",
		]
	);
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
fn resources_people_and_lite_reservations_use_isolated_bulletin_capacity() {
	use crate::{Resources, Timestamp};
	use bulletin_transaction_storage_primitives::ResourceReservationView;
	use indiv_pallet_resources::types::{MembershipCollection, ReservationPurpose};

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
			pallet_bulletin_transaction_storage::ReservedPermanentCapacity::<Runtime>::get(),
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
		let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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

fn decode_hex(input: &str) -> Vec<u8> {
	let input = input.trim();
	assert_eq!(input.len() % 2, 0);
	input
		.as_bytes()
		.chunks_exact(2)
		.map(|pair| {
			let digit = |byte: u8| match byte {
				b'0'..=b'9' => byte - b'0',
				b'a'..=b'f' => byte - b'a' + 10,
				b'A'..=b'F' => byte - b'A' + 10,
				_ => panic!("invalid fixture hex"),
			};
			(digit(pair[0]) << 4) | digit(pair[1])
		})
		.collect()
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
			pallet_bulletin_transaction_storage::HoldReason::StorageFeeHold,
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
	type WrappedCharge = pallet_orbis_feeless::ChargeOrSkipFeeless<Runtime, AssetCharge>;

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
fn solidity_evm_fixture_deploys_and_executes_through_revive() {
	use pallet_revive::{
		test_utils::builder::{BareCallBuilder, BareInstantiateBuilder},
		Code, TransactionLimits,
	};
	let limits = || TransactionLimits::WeightAndDeposit {
		weight_limit: frame_support::weights::Weight::from_parts(500_000_000_000, 10 * 1024 * 1024),
		deposit_limit: 50_000_000_000_000_000,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let account = pallet_revive::test_utils::ALICE;
		let funded =
			<Balances as Mutate<AccountId>>::set_balance(&account, 100_000_000_000_000_000);
		assert_eq!(funded, 100_000_000_000_000_000);
		assert_eq!(Balances::free_balance(&account), funded);
		let revive_account = Revive::account_id();
		<Balances as Mutate<AccountId>>::set_balance(
			&revive_account,
			crate::ExistentialDeposit::get(),
		);
		let code = decode_hex(include_str!("../fixtures/build/Counter.bin"));

		let instantiate = BareInstantiateBuilder::<Runtime>::bare_instantiate(
			RuntimeOrigin::signed(account.clone()),
			Code::Upload(code),
		)
		.transaction_limits(limits())
		.salt(Some([7u8; 32]))
		.build();
		let instantiated = instantiate.result.unwrap();
		assert!(!instantiated.result.did_revert());
		let contract_addr = instantiated.addr;

		let increment = sp_io::hashing::keccak_256(b"increment()")[..4].to_vec();
		let increment_result = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(account.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(increment)
		.build_and_unwrap_result();
		assert!(!increment_result.did_revert());

		let value = sp_io::hashing::keccak_256(b"value()")[..4].to_vec();
		let value_result =
			BareCallBuilder::<Runtime>::bare_call(RuntimeOrigin::signed(account), contract_addr)
				.transaction_limits(limits())
				.data(value)
				.build_and_unwrap_result();
		assert!(!value_result.did_revert());
		assert_eq!(value_result.data.len(), 32);
		assert_eq!(value_result.data[31], 42);
	});
}

#[test]
fn identity_bound_contract_moves_assets_and_persists_its_audit() {
	use pallet_revive::{
		test_utils::builder::{BareCallBuilder, BareInstantiateBuilder},
		AddressMapper, Code, TransactionLimits,
	};
	let limits = || TransactionLimits::WeightAndDeposit {
		weight_limit: frame_support::weights::Weight::from_parts(500_000_000_000, 10 * 1024 * 1024),
		deposit_limit: 50_000_000_000_000_000,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let owner = pallet_revive::test_utils::ALICE;
		let recipient = pallet_revive::test_utils::BOB;
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);
		<Balances as Mutate<AccountId>>::set_balance(&recipient, crate::ExistentialDeposit::get());
		<Balances as Mutate<AccountId>>::set_balance(
			&Revive::account_id(),
			crate::ExistentialDeposit::get(),
		);

		let mut identity = pallet_orbis_people::legacy::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		identity.display =
			pallet_orbis_people::Data::Raw(b"Alice Orbis".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(RuntimeOrigin::signed(owner.clone()), Box::new(identity),));
		assert!(People::has_identity(&owner, 1));
		let identity_commitment = sp_io::hashing::blake2_256(owner.as_ref());

		let code = decode_hex(include_str!("../fixtures/build/IdentityAssetAudit.bin"));
		let instantiated = BareInstantiateBuilder::<Runtime>::bare_instantiate(
			RuntimeOrigin::signed(owner.clone()),
			Code::Upload(code),
		)
		.transaction_limits(limits())
		.constructor_data(identity_commitment.to_vec())
		.salt(Some([8u8; 32]))
		.build_and_unwrap_result();
		assert!(!instantiated.result.did_revert());
		let contract_addr = instantiated.addr;
		let contract_account = <pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<
			Runtime,
		>>::to_fallback_account_id(&contract_addr);

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
			contract_account.clone().into(),
			100,
		));

		let audit_record = b"alice:identity-asset-transfer:40".to_vec();
		let audit = sp_io::hashing::blake2_256(&audit_record);
		let mut asset_addr = [0u8; 20];
		asset_addr[..4].copy_from_slice(&asset_id.to_be_bytes());
		asset_addr[16..18].copy_from_slice(&0x0120u16.to_be_bytes());
		let recipient_addr = <pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<
			Runtime,
		>>::to_address(&recipient);
		let mut transfer =
			sp_io::hashing::keccak_256(b"transferAndAudit(address,address,uint256,bytes32)")[..4]
				.to_vec();
		for address in [asset_addr, recipient_addr.0] {
			transfer.extend_from_slice(&[0u8; 12]);
			transfer.extend_from_slice(&address);
		}
		transfer.extend_from_slice(&[0u8; 31]);
		transfer.push(40);
		transfer.extend_from_slice(&audit);
		let transferred = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(owner.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(transfer)
		.build_and_unwrap_result();
		assert!(!transferred.did_revert(), "contract call reverted: {transferred:?}");
		assert_eq!(Assets::balance(asset_id, &contract_account), 60);
		assert_eq!(Assets::balance(asset_id, &recipient), 40);

		let last_audit = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(owner.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(sp_io::hashing::keccak_256(b"lastAudit()")[..4].to_vec())
		.build_and_unwrap_result();
		assert_eq!(last_audit.data, audit);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner.clone(),
			1,
			1024,
		));
		let storage_call = pallet_bulletin_transaction_storage::Call::<Runtime>::store {
			data: audit_record.clone(),
		};
		let (_, scope) = TransactionStorage::validate_signed(&owner, &storage_call).unwrap();
		let scope = scope.expect("store calls carry their validated authorization scope");
		assert_ok!(TransactionStorage::pre_dispatch_signed(&owner, &storage_call));
		let authorized = pallet_bulletin_transaction_storage::Origin::<Runtime>::Authorized {
			who: owner,
			scope,
		};
		assert_ok!(TransactionStorage::store(RuntimeOrigin::from(authorized), audit_record,));
		assert!(TransactionStorage::contains_transaction(audit));
		<TransactionStorage as Hooks<u32>>::on_finalize(1);
		assert_eq!(TransactionStorage::transactions_at(1).unwrap()[0].content_hash, audit);
	});
}

#[test]
fn people_identity_is_self_claimed_and_sudo_attested() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::from(ALICE);
		let registrar = AccountId::from([3u8; 32]);
		let mut info =
			pallet_orbis_people::legacy::IdentityInfo::<crate::PeopleMaxAdditionalFields>::default(
			);
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
fn bulletin_storage_is_authorized_indexed_and_content_addressed() {
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
		let payload = pallet_bulletin_hop_promotion::signing_payload(&hash, now);
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
			pair.sign(&pallet_bulletin_hop_promotion::signing_payload(&hash, now)),
		);
		let call = RuntimeCall::HopPromotion(pallet_bulletin_hop_promotion::Call::promote {
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
			.expect("the annotated call and its Bulletin authorization are valid");
		assert!(origin.is_transaction_authorized());
		let pre = extension
			.prepare(val, &origin, &call, &info, call.encoded_size())
			.expect("authorized preparation explicitly skips payment");
		assert_eq!(Balances::free_balance(&account), initial_balance);
		assert_eq!(pallet_orbis_feeless::FeelessUsage::<Runtime>::get(&account), None);

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
		assert_eq!(pallet_orbis_feeless::FeelessUsage::<Runtime>::get(&account), None);
	});
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn bulletin_storage_mutations_are_rejected_when_wrapped_or_sent_by_xcm() {
	use codec::Encode;
	use frame_support::dispatch::GetDispatchInfo;
	use sp_runtime::traits::TransactionExtension;

	type XcmSafeCalls = <crate::xcm_config::XcmConfig as xcm_executor::Config>::SafeCallFilter;
	let store = RuntimeCall::TransactionStorage(pallet_bulletin_transaction_storage::Call::store {
		data: b"audit".to_vec(),
	});
	assert!(crate::BulletinCallInspector::contains(&store));
	assert!(!XcmSafeCalls::contains(&store));

	let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![store] });
	assert!(crate::BulletinCallInspector::contains(&wrapped));
	assert!(!XcmSafeCalls::contains(&wrapped));
	let reserved_renew = RuntimeCall::TransactionStorage(
		pallet_bulletin_transaction_storage::Call::renew_reserved {
			reservation_id: 7,
			content_hash: [9u8; 32],
		},
	);
	assert!(crate::BulletinCallInspector::contains(&reserved_renew));
	assert!(!XcmSafeCalls::contains(&reserved_renew));
	let wrapped_reserved =
		RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![reserved_renew] });
	assert!(crate::BulletinCallInspector::contains(&wrapped_reserved));
	assert!(!XcmSafeCalls::contains(&wrapped_reserved));

	let reserved_store = RuntimeCall::TransactionStorage(
		pallet_bulletin_transaction_storage::Call::store_reserved {
			reservation_id: 7,
			cid_config: bulletin_transaction_storage_primitives::cids::CidConfig {
				codec: bulletin_transaction_storage_primitives::cids::RAW_CODEC,
				hashing:
					bulletin_transaction_storage_primitives::cids::HashingAlgorithm::Blake2b256,
			},
			data: b"reserved".to_vec(),
		},
	);
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
		crate::meta_v6::ConsumePaidMetaIngress(crate::meta_v6::IntentPreimageV6 {
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
		}),
		pallet_meta_tx::MetaTxMarker::new(),
		frame_system::CheckNonZeroSender::new(),
		frame_system::CheckSpecVersion::new(),
		frame_system::CheckTxVersion::new(),
		frame_system::CheckGenesis::new(),
		frame_system::CheckMortality::from(sp_runtime::generic::Era::Immortal),
		frame_system::CheckNonce::from(0),
		Default::default(),
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
		assert!(crate::BulletinCallInspector::contains(&call), "{name} bypassed inspection");
		assert!(!XcmSafeCalls::contains(&call), "{name} bypassed the XCM safe filter");
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let call = RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([2u8; 32]).into(),
			force_proxy_type: Some(crate::ProxyType::Any),
			call: Box::new(RuntimeCall::TransactionStorage(
				pallet_bulletin_transaction_storage::Call::store_reserved {
					reservation_id: 7,
					cid_config: bulletin_transaction_storage_primitives::cids::CidConfig {
				codec: bulletin_transaction_storage_primitives::cids::RAW_CODEC,
				hashing: bulletin_transaction_storage_primitives::cids::HashingAlgorithm::Blake2b256,
			},
					data: b"reserved".to_vec(),
				},
			)),
		});
		let extension = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::BulletinCallInspector,
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
	assert!(!crate::BulletinCallInspector::contains(&ordinary));
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
			RuntimeCall::Entity(pallet_orbis_entity::Call::rotate_attributes { ops: vec![] });
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
			pallet_orbis_feeless::Error::<Runtime>::QuotaExhausted
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
			pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
			RuntimeCall::Entity(pallet_orbis_entity::Call::rotate_attributes { ops: vec![] });
		let feeless_info = feeless_call.get_dispatch_info();
		let extension = crate::default_inner_tx_extensions(
			1,
			pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
		assert_eq!(pallet_orbis_feeless::FeelessUsage::<Runtime>::get(&account), Some((1, 1)));
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
		assert_eq!(pallet_orbis_feeless::FeelessUsage::<Runtime>::get(&mapped), None);
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

			let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
				pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
					Runtime,
					crate::BulletinCallInspector,
				>::default(),
				frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
				revive,
			);
			let origin_policy_tail = (frame_system::AuthorizeCall::<Runtime>::new(),);
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
			let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into();
			let tx_ext = crate::paid_tx_extensions((
				(
					indiv_pallet_people::extension::AsPerson::<Runtime>::new(None),
					indiv_pallet_people_lite::extension::PeopleLiteAuth::<Runtime>::new(None),
					resources_extension,
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
				pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
					Runtime,
					crate::BulletinCallInspector,
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

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery() {
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
		crate::meta_v6::MetaAccountBoundPoliciesV6,
		pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
			Runtime,
			crate::BulletinCallInspector,
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
		let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
		let nonce = frame_system::CheckNonce::<Runtime>::from(System::account(&claimed).nonce);
		let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(proofs);
		let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::BulletinCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let preimage = crate::meta_v6::IntentPreimageV6 {
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
			policy,
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
			policy,
			storage,
			metadata,
		) = bare;
		let extension = (
			verify, consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage,
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
		let meta_len = meta.encoded_size() as u32;
		let call = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(meta),
			meta_tx_encoded_len: meta_len,
		});
		let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
		let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::BulletinCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let inherited = sp_runtime::traits::ImplicationParts {
			base: sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, call)),
			explicit: (&storage, &metadata),
			implicit: (storage.implicit().unwrap(), metadata.implicit().unwrap()),
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
		let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			crate::BulletinCallInspector,
		>::default();
		let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
		let inherited = sp_runtime::traits::ImplicationParts {
			base: sp_runtime::traits::TxBaseImplication((META_EXTENSION_VERSION, call)),
			explicit: (&storage, &metadata),
			implicit: (storage.implicit().unwrap(), metadata.implicit().unwrap()),
		};
		(domain, signer, signer, call, inherited).using_encoded(sp_io::hashing::blake2_256)
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
		let alice = account(&alice_pair);
		let bob = account(&bob_pair);
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
		let invalid_token = crate::meta_v6::PaidMetaTokenV6 {
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
		assert!(crate::meta_v6::inspect_paid_meta(&v5_outer, 0).is_err());
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
		assert!(crate::meta_v6::inspect_paid_meta(
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
			crate::meta_v6::PaidMetaScope::from(frame_system::CheckSpecVersion::<Runtime>::new());
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
			assert!(crate::meta_v6::inspect_paid_meta(allowed, 0).unwrap().is_some());
			assert!(
				!<crate::xcm_config::OrbisXcmSafeCallFilter as Contains<RuntimeCall>>::contains(
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
			assert!(crate::meta_v6::inspect_paid_meta(denied, 0).is_err());
		}
		let denial_payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
		assert_eq!(crate::meta_v6::inspect_paid_meta(&approval_only, 0), Ok(None));
		assert!(
			!<crate::xcm_config::OrbisXcmSafeCallFilter as Contains<RuntimeCall>>::contains(
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
			0,
			pallet_orbis_feeless::ChargeOrSkipFeeless::from(
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
			Some(bulletin_transaction_storage_primitives::ResourceReservationView::Active(_))
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
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV6 {
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
	use frame_benchmarking::runtime_decl_for_benchmark::BenchmarkV2;
	use frame_benchmarking::BenchmarkConfig;

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

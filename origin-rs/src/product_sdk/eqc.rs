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

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
	contract::{
		NativeError, NativeErrorCode, NATIVE_SDK_RATIFICATION_PAYLOAD_SHA256,
		ORBIS_CANDIDATE_GENESIS_HEADER_HASH, ORBIS_DESCRIPTOR_CONTRACT_SHA256, ORBIS_METADATA_HASH,
		ORBIS_PARA_ID, ORBIS_SPEC_VERSION, ORBIS_TRANSACTION_VERSION,
	},
	version::ORBIS_COMPACT_WASM_SHA256,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloManifest {
	pub manifest_version: u8,
	pub status: String,
	pub network: SloNetwork,
	pub campaign: SloCampaign,
	#[serde(rename = "E")]
	pub e: SloE,
	#[serde(rename = "Q")]
	pub q: SloQ,
	#[serde(rename = "C")]
	pub c: SloC,
	pub ratification: RatificationReference,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloNetwork {
	pub aura_slot_ms: u64,
	pub orbis_collators: u32,
	pub orbis_cores: Vec<u32>,
	pub origin_validators: u32,
	pub para_id: u32,
	pub relay_parent_offset: u32,
	pub target_block_rate: u32,
	pub unincluded_segment_capacity: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloCampaign {
	pub cooldown_seconds: u64,
	pub interleaved_runs: u32,
	pub max_clock_offset_ms: u64,
	pub max_cv_percent: f64,
	pub max_finality_lag_blocks: u64,
	pub measurement_seconds: u64,
	pub seed: u64,
	pub warmup_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloE {
	pub mix_percent: BTreeMap<String, f64>,
	pub observed_resource_utilization_max_percent: f64,
	pub p95_finality_ms: f64,
	pub p95_inclusion_ms: f64,
	pub p95_resource_utilization_max_percent: f64,
	pub sponsored_intent_success_percent: f64,
	pub storage_headroom_min_percent: f64,
	pub target_finalized_successful_extrinsics_per_second: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloQ {
	pub finalized_hash_required: bool,
	pub mix_percent: BTreeMap<String, f64>,
	pub reconnect_catchup_p95_ms: f64,
	pub request_p95_ms: f64,
	pub request_success_percent: f64,
	pub subscription_completeness_percent: f64,
	pub subscription_delivery_p95_ms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SloC {
	pub cid_integrity_percent: f64,
	pub completion_p95_ms: f64,
	pub concurrency: u64,
	pub content_buckets: Vec<ContentBucket>,
	pub failover_success_percent: f64,
	pub mix_percent: BTreeMap<String, f64>,
	pub network: ContentNetwork,
	pub provider_count: u64,
	pub provider_topology: String,
	pub request_rate_per_second: u64,
	pub retries: u64,
	pub success_percent: f64,
	pub throughput_floor_mib_per_second: f64,
	pub timeout_ms: u64,
	pub ttfb_p95_ms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBucket {
	pub proportion_percent: f64,
	pub range: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentNetwork {
	pub bandwidth_mbps: f64,
	pub latency_ms: f64,
	pub loss_percent: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RatificationReference {
	pub canonical_envelope: String,
	pub payload_sha256: String,
	pub sdk_freeze_status: String,
	pub production_activation_status: String,
}

pub fn validate_slo_manifest(
	manifest: &SloManifest,
	ratification_payload: &Value,
) -> Result<(), NativeError> {
	if manifest.manifest_version != 1 || manifest.status != "proposed-unratified" {
		return Err(invalid("unsupported E/Q/C manifest version or status"));
	}
	let expected_network = SloNetwork {
		aura_slot_ms: 6_000,
		orbis_collators: 2,
		orbis_cores: vec![1, 3],
		origin_validators: 6,
		para_id: ORBIS_PARA_ID,
		relay_parent_offset: 1,
		target_block_rate: 3,
		unincluded_segment_capacity: 12,
	};
	if manifest.network != expected_network {
		return Err(invalid("E/Q/C topology drift"));
	}
	if manifest.campaign.seed != 20_260_713
		|| manifest.campaign.interleaved_runs < 5
		|| manifest.campaign.warmup_seconds < 600
		|| manifest.campaign.measurement_seconds < 1_800
		|| manifest.campaign.cooldown_seconds < 300
		|| manifest.campaign.max_clock_offset_ms > 50
		|| !finite_range(manifest.campaign.max_cv_percent, 0.0, 10.0)
		|| manifest.campaign.max_finality_lag_blocks > 15
	{
		return Err(invalid("E/Q/C campaign bounds were weakened"));
	}
	validate_mix(
		&manifest.e.mix_percent,
		&["assets", "attestation", "names", "identity_personhood", "sponsored_meta_tx", "storage"],
		"E",
	)?;
	validate_mix(
		&manifest.q.mix_percent,
		&[
			"assets",
			"attestation",
			"names",
			"identity_personhood",
			"storage",
			"subscriptions",
			"system",
		],
		"Q",
	)?;
	validate_mix(&manifest.c.mix_percent, &["failover", "multi_chunk", "small_single_block"], "C")?;
	for value in [
		manifest.e.observed_resource_utilization_max_percent,
		manifest.e.p95_resource_utilization_max_percent,
		manifest.e.sponsored_intent_success_percent,
		manifest.e.storage_headroom_min_percent,
		manifest.q.request_success_percent,
		manifest.q.subscription_completeness_percent,
		manifest.c.cid_integrity_percent,
		manifest.c.failover_success_percent,
		manifest.c.success_percent,
		manifest.c.network.loss_percent,
	] {
		if !finite_range(value, 0.0, 100.0) {
			return Err(invalid("E/Q/C percentage is outside 0..=100"));
		}
	}
	for value in [
		manifest.e.p95_finality_ms,
		manifest.e.p95_inclusion_ms,
		manifest.e.target_finalized_successful_extrinsics_per_second,
		manifest.q.reconnect_catchup_p95_ms,
		manifest.q.request_p95_ms,
		manifest.q.subscription_delivery_p95_ms,
		manifest.c.completion_p95_ms,
		manifest.c.throughput_floor_mib_per_second,
		manifest.c.ttfb_p95_ms,
		manifest.c.network.latency_ms,
	] {
		if !finite_range(value, 0.0, f64::MAX) {
			return Err(invalid("E/Q/C metric must be finite and non-negative"));
		}
	}
	if !manifest.q.finalized_hash_required
		|| manifest.c.provider_topology.is_empty()
		|| manifest.c.network.bandwidth_mbps < 1.0
		|| manifest.c.content_buckets.len() != 4
		|| !approximately_100(
			manifest.c.content_buckets.iter().map(|bucket| bucket.proportion_percent).sum(),
		) || manifest.c.content_buckets.iter().any(|bucket| {
		bucket.range.is_empty() || !finite_range(bucket.proportion_percent, 0.0, 100.0)
	}) {
		return Err(invalid("invalid E/Q/C content or finalized-read target"));
	}
	validate_sdk_freeze_payload(ratification_payload)?;
	if manifest.ratification.canonical_envelope
		!= "docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json"
		|| manifest.ratification.payload_sha256 != NATIVE_SDK_RATIFICATION_PAYLOAD_SHA256
		|| manifest.ratification.sdk_freeze_status != "PENDING"
		|| manifest.ratification.production_activation_status != "BLOCKED"
	{
		return Err(invalid("E/Q/C ratification reference drift"));
	}
	Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EqcClass {
	E,
	Q,
	C,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EqcResult {
	pub schema_version: u8,
	pub scope: String,
	pub campaign_executed: bool,
	pub class: EqcClass,
	pub manifest_sha256: String,
	pub runtime: EqcRuntime,
	pub environment: EqcEnvironment,
	pub raw_uri: String,
	pub raw_sha256: String,
	pub samples: u64,
	pub observations: BTreeMap<String, f64>,
	pub verdict: EqcVerdict,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EqcRuntime {
	pub genesis_identity: String,
	pub genesis_status: String,
	pub wasm_sha256: String,
	pub metadata_hash: String,
	pub descriptor_contract_sha256: String,
	pub spec_version: u32,
	pub transaction_version: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EqcEnvironment {
	pub topology_sha256: String,
	pub origin_validators: u32,
	pub orbis_collators: u32,
	pub orbis_cores: u32,
	pub clock_offset_ms: i64,
	pub seed: u64,
	pub warmup_seconds: u64,
	pub measurement_seconds: u64,
	pub cooldown_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EqcVerdict {
	pub pass: bool,
	pub failures: Vec<String>,
	pub bottleneck_attribution: Vec<String>,
	pub recomputed: bool,
}

/// Validate the candidate-network schema-only result contract.
///
/// Network campaign validation intentionally fails closed here. Production campaign evidence is
/// authorized only after final genesis and a separately approved production campaign.
pub fn validate_eqc_result(result: &EqcResult, manifest_sha256: &str) -> Result<(), NativeError> {
	if result.schema_version != 1
		|| result.manifest_sha256 != manifest_sha256
		|| !is_hash(&result.manifest_sha256)
		|| !is_hash(&result.raw_sha256)
		|| !is_hash(&result.runtime.wasm_sha256)
		|| !is_hash(&result.runtime.descriptor_contract_sha256)
		|| !is_hash(&result.environment.topology_sha256)
		|| result.raw_uri.is_empty()
	{
		return Err(invalid("E/Q/C result identity or hash is invalid"));
	}
	if result.runtime.genesis_identity != ORBIS_CANDIDATE_GENESIS_HEADER_HASH
		|| result.runtime.metadata_hash != ORBIS_METADATA_HASH
		|| result.runtime.wasm_sha256 != ORBIS_COMPACT_WASM_SHA256
		|| result.runtime.descriptor_contract_sha256 != ORBIS_DESCRIPTOR_CONTRACT_SHA256
		|| result.runtime.spec_version != ORBIS_SPEC_VERSION
		|| result.runtime.transaction_version != ORBIS_TRANSACTION_VERSION
	{
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"E/Q/C runtime binding mismatch",
		));
	}
	if result.environment.origin_validators != 6
		|| result.environment.orbis_collators != 2
		|| ![1, 3].contains(&result.environment.orbis_cores)
		|| !(-50..=50).contains(&result.environment.clock_offset_ms)
		|| result.environment.seed != 20_260_713
		|| !result.verdict.recomputed
		|| result.verdict.failures.iter().any(String::is_empty)
		|| result.verdict.bottleneck_attribution.iter().any(String::is_empty)
	{
		return Err(invalid("E/Q/C result environment or verdict is invalid"));
	}
	if result.campaign_executed {
		return Err(invalid("candidate result cannot claim a network campaign"));
	}
	if result.scope != "schema-validation-only"
		|| result.samples != 0
		|| !result.observations.is_empty()
		|| result.verdict.pass
		|| result.verdict.failures != ["schema-only-no-network-campaign"]
		|| result.runtime.genesis_status
			!= "deterministic-clean-break-candidate-not-production-approved"
		|| !result.raw_uri.starts_with("schema-only:")
	{
		return Err(invalid("schema-only result made a campaign or production claim"));
	}
	Ok(())
}

fn validate_sdk_freeze_payload(payload: &Value) -> Result<(), NativeError> {
	let object = payload
		.as_object()
		.ok_or_else(|| invalid("ratification payload is not an object"))?;
	let required: BTreeSet<_> = [
		"approval_policy",
		"contract_digests",
		"fixture_identity",
		"network_launch_approval",
		"performance_claim",
		"production_activation",
		"runtime",
		"scope",
	]
	.into_iter()
	.collect();
	if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != required
		|| object.get("scope").and_then(Value::as_str)
			!= Some("p5-first-supported-clean-break-native-sdk-freeze-requires-fresh-ratifier-signatures")
		|| object.get("performance_claim").and_then(Value::as_bool) != Some(false)
		|| object.get("network_launch_approval").and_then(Value::as_bool) != Some(false)
	{
		return Err(invalid("ratification payload is not the native SDK freeze"));
	}
	let runtime = object
		.get("runtime")
		.and_then(Value::as_object)
		.ok_or_else(|| invalid("runtime missing"))?;
	if runtime.get("name").and_then(Value::as_str) != Some("orbis")
		|| runtime.get("para_id").and_then(Value::as_u64) != Some(ORBIS_PARA_ID.into())
		|| runtime.get("spec_version").and_then(Value::as_u64) != Some(ORBIS_SPEC_VERSION.into())
		|| runtime.get("transaction_version").and_then(Value::as_u64)
			!= Some(ORBIS_TRANSACTION_VERSION.into())
		|| runtime.get("metadata_hash").and_then(Value::as_str) != Some(ORBIS_METADATA_HASH)
	{
		return Err(invalid("ratification runtime binding drift"));
	}
	let activation = object
		.get("production_activation")
		.and_then(Value::as_object)
		.ok_or_else(|| invalid("production activation missing"))?;
	if activation.get("campaign_authorized").and_then(Value::as_bool) != Some(false)
		|| activation.get("final_genesis_status").and_then(Value::as_str) != Some("PENDING")
		|| !activation.get("final_genesis_hash").is_some_and(Value::is_null)
	{
		return Err(invalid("SDK freeze payload claimed production activation"));
	}
	Ok(())
}

fn validate_mix(
	mix: &BTreeMap<String, f64>,
	expected: &[&str],
	class: &str,
) -> Result<(), NativeError> {
	let actual: BTreeSet<_> = mix.keys().map(String::as_str).collect();
	let expected: BTreeSet<_> = expected.iter().copied().collect();
	if actual != expected
		|| mix.values().any(|value| !finite_range(*value, 0.0, 100.0))
		|| !approximately_100(mix.values().sum())
	{
		return Err(invalid(format!("{class} workload mix is invalid")));
	}
	Ok(())
}

fn finite_range(value: f64, min: f64, max: f64) -> bool {
	value.is_finite() && value >= min && value <= max
}

fn approximately_100(value: f64) -> bool {
	(value - 100.0).abs() < f64::EPSILON
}

fn is_hash(value: &str) -> bool {
	value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn invalid(message: impl Into<String>) -> NativeError {
	NativeError::new(NativeErrorCode::InvalidInput, message)
}

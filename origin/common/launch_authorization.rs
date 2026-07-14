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

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::DateTime;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sp_core::{ed25519, Pair};
use std::collections::BTreeSet;

const LAUNCH_ENVELOPE: &[u8] =
	include_bytes!("../../docs/genesis/origin-orbis-production-launch-approval.json");
const LAUNCH_PAYLOAD: &[u8] =
	include_bytes!("../../docs/genesis/origin-orbis-production-launch-approval.payload.json");
const P5_ENVELOPE: &[u8] =
	include_bytes!("../../docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json");
const P5_PAYLOAD: &[u8] =
	include_bytes!("../../docs/evidence/verification/p5/sdk-freeze-ratification.payload.json");
const METADATA_IDENTITY: &[u8] =
	include_bytes!("../orbis/runtime/vectors/transaction-policy-v8/metadata-hash.json");

const REQUIRED_ROLES: [&str; 5] =
	["runtime-owner", "sdk-owner", "security-owner", "performance-owner", "architecture-owner"];

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum LaunchChain {
	Origin,
	Orbis,
}

impl LaunchChain {
	fn name(self) -> &'static str {
		match self {
			Self::Origin => "origin",
			Self::Orbis => "orbis",
		}
	}
}

fn parse(bytes: &[u8], label: &str) -> Result<Value, String> {
	serde_json::from_slice(bytes).map_err(|error| format!("invalid {label}: {error}"))
}

fn object_field<'a>(value: &'a Value, field: &str, label: &str) -> Result<&'a Value, String> {
	value.get(field).ok_or_else(|| format!("{label} is missing {field}"))
}

fn has_exact_fields(value: &Value, expected: &[&str]) -> bool {
	let Some(object) = value.as_object() else { return false };
	let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
	actual == expected.iter().copied().collect()
}

fn string_field<'a>(value: &'a Value, field: &str, label: &str) -> Result<&'a str, String> {
	object_field(value, field, label)?
		.as_str()
		.ok_or_else(|| format!("{label}.{field} must be a string"))
}

fn true_field(value: &Value, field: &str, label: &str) -> Result<(), String> {
	if object_field(value, field, label)?.as_bool() == Some(true) {
		Ok(())
	} else {
		Err(format!("{label}.{field} must be true for production launch"))
	}
}

fn sha256_hex(bytes: &[u8]) -> String {
	hex::encode(Sha256::digest(bytes))
}

fn require_sha256(value: &str, expected: &[u8], label: &str) -> Result<(), String> {
	if value == sha256_hex(expected) {
		Ok(())
	} else {
		Err(format!("{label} SHA-256 does not match the embedded release artifact"))
	}
}

#[derive(Clone, Copy)]
struct ApprovalTiming {
	finalized_at: i64,
	launch_epoch: i64,
	max_signature_age_seconds: i64,
}

fn rfc3339_epoch(value: &str, label: &str) -> Result<i64, String> {
	if value.len() != 20 || !value.ends_with('Z') {
		return Err(format!("{label} must use exact RFC3339 UTC seconds (YYYY-MM-DDTHH:MM:SSZ)"));
	}
	DateTime::parse_from_rfc3339(value)
		.map(|time| time.timestamp())
		.map_err(|_| format!("{label} must be a valid RFC3339 timestamp"))
}

fn approval_timing(payload: &Value) -> Result<ApprovalTiming, String> {
	let timing = object_field(payload, "approval_timing", "launch payload")?;
	let finalized_at = rfc3339_epoch(
		string_field(timing, "envelope_finalized_at", "approval_timing")?,
		"approval_timing.envelope_finalized_at",
	)?;
	let launch_epoch = rfc3339_epoch(
		string_field(timing, "launch_epoch", "approval_timing")?,
		"approval_timing.launch_epoch",
	)?;
	let launch_not_after = rfc3339_epoch(
		string_field(timing, "launch_not_after", "approval_timing")?,
		"approval_timing.launch_not_after",
	)?;
	let max_signature_age_seconds =
		object_field(timing, "max_signature_age_seconds", "approval_timing")?
			.as_i64()
			.ok_or("approval_timing.max_signature_age_seconds must be an integer")?;
	let max_launch_window_seconds =
		object_field(timing, "max_launch_window_seconds", "approval_timing")?
			.as_i64()
			.ok_or("approval_timing.max_launch_window_seconds must be an integer")?;
	if max_signature_age_seconds != 7 * 24 * 60 * 60
		|| max_launch_window_seconds != 24 * 60 * 60
		|| finalized_at > launch_epoch
		|| launch_epoch > launch_not_after
		|| launch_not_after - launch_epoch > max_launch_window_seconds
	{
		return Err("approval_timing finalization/launch epoch window is invalid".into());
	}
	Ok(ApprovalTiming { finalized_at, launch_epoch, max_signature_age_seconds })
}

fn verify_signature_time(
	key: &Value,
	signature: &Value,
	timing: ApprovalTiming,
) -> Result<(), String> {
	let valid_from = rfc3339_epoch(
		string_field(key, "valid_from", "authorized key")?,
		"authorized key valid_from",
	)?;
	let valid_until = rfc3339_epoch(
		string_field(key, "valid_until", "authorized key")?,
		"authorized key valid_until",
	)?;
	if valid_from >= valid_until {
		return Err("authorized key validity interval is invalid".into());
	}
	let signed_at = rfc3339_epoch(
		string_field(signature, "signed_at", "launch signature")?,
		"launch signature signed_at",
	)?;
	if signed_at < valid_from || signed_at > valid_until {
		return Err("signature key is expired or not yet valid at signed_at".into());
	}
	if timing.finalized_at < valid_from
		|| timing.finalized_at > valid_until
		|| timing.launch_epoch < valid_from
		|| timing.launch_epoch > valid_until
	{
		return Err(
			"key is not valid at the deterministic envelope finalization/launch epoch".into()
		);
	}
	if signed_at > timing.finalized_at
		|| timing.finalized_at - signed_at > timing.max_signature_age_seconds
	{
		return Err("signature is outside the deterministic finalization freshness window".into());
	}
	if let Some(revoked_at) = key.get("revoked_at") {
		if !revoked_at.is_null() {
			let revoked_at = rfc3339_epoch(
				revoked_at
					.as_str()
					.ok_or("authorized key revoked_at must be null or an RFC3339 string")?,
				"authorized key revoked_at",
			)?;
			if revoked_at <= signed_at {
				return Err("signature key was revoked at or before signed_at".into());
			}
		}
	} else {
		return Err("authorized key is missing revoked_at".into());
	}
	Ok(())
}

fn collect_input_authority_plane(
	chain: LaunchChain,
	input: &Value,
) -> Result<BTreeSet<String>, String> {
	let mut plane = BTreeSet::new();
	plane.insert(string_field(input, "root_key", "genesis input")?.to_owned());
	let members = match chain {
		LaunchChain::Origin => object_field(input, "validators", "Origin genesis input")?,
		LaunchChain::Orbis => object_field(input, "collators", "Orbis genesis input")?,
	}
	.as_array()
	.ok_or("authority members must be an array")?;
	for member in members {
		let fields: &[&str] = match chain {
			LaunchChain::Origin => &[
				"account_id",
				"babe",
				"grandpa",
				"para_validator",
				"para_assignment",
				"authority_discovery",
				"beefy",
			],
			LaunchChain::Orbis => &["account_id", "aura_id"],
		};
		for field in fields {
			plane.insert(string_field(member, field, "authority member")?.to_owned());
		}
	}
	Ok(plane)
}

fn verify_authority_planes(
	payload: &Value,
	chain: LaunchChain,
	genesis_input_bytes: &[u8],
) -> Result<(), String> {
	let planes = object_field(payload, "authority_planes", "launch payload")?;
	let mut decoded_planes = Vec::new();
	for name in ["origin", "orbis"] {
		let values = object_field(planes, name, "authority_planes")?
			.as_array()
			.ok_or_else(|| format!("authority_planes.{name} must be an array"))?;
		let mut plane = BTreeSet::new();
		for value in values {
			let encoded = value
				.as_str()
				.ok_or_else(|| format!("authority_planes.{name} values must be strings"))?;
			let raw = encoded
				.strip_prefix("0x")
				.ok_or_else(|| format!("authority_planes.{name} values must be 0x-prefixed"))?;
			let bytes = hex::decode(raw)
				.map_err(|_| format!("authority_planes.{name} contains invalid hex"))?;
			if raw.bytes().any(|byte| byte.is_ascii_uppercase())
				|| !matches!(bytes.len(), 32 | 33)
				|| bytes.iter().all(|byte| *byte == bytes[0])
			{
				return Err(format!(
					"authority_planes.{name} contains placeholder or malformed authority material"
				));
			}
			if !plane.insert(encoded.to_owned()) {
				return Err(format!(
					"authority_planes.{name} contains duplicate authority material"
				));
			}
		}
		decoded_planes.push(plane);
	}
	if !decoded_planes[0].is_disjoint(&decoded_planes[1]) {
		return Err("Origin and Orbis production authority planes must be distinct".into());
	}
	let input = parse(genesis_input_bytes, &format!("{} genesis input", chain.name()))?;
	if collect_input_authority_plane(chain, &input)?
		!= decoded_planes[if matches!(chain, LaunchChain::Origin) { 0 } else { 1 }]
	{
		return Err(format!(
			"{} genesis authority plane differs from the signed launch payload",
			chain.name()
		));
	}
	Ok(())
}

fn verify_role_signatures(
	envelope: &Value,
	payload: &Value,
	payload_bytes: &[u8],
	timing: ApprovalTiming,
) -> Result<(), String> {
	let policy = object_field(payload, "approval_policy", "launch payload")?;
	let required = object_field(policy, "required_roles", "approval_policy")?
		.as_array()
		.ok_or("approval_policy.required_roles must be an array")?;
	let roles = required
		.iter()
		.map(|role| role.as_str().ok_or("required role must be a string"))
		.collect::<Result<BTreeSet<_>, _>>()?;
	if roles != REQUIRED_ROLES.into_iter().collect() {
		return Err("launch approval policy must require the five frozen owner roles".into());
	}
	let slots = object_field(policy, "approver_slots", "approval_policy")?
		.as_array()
		.ok_or("approval_policy.approver_slots must be an array")?;
	let keys = object_field(policy, "authorized_keys", "approval_policy")?
		.as_array()
		.ok_or("approval_policy.authorized_keys must be an array")?;
	let signatures = object_field(envelope, "signatures", "launch envelope")?
		.as_array()
		.ok_or("launch envelope signatures must be an array")?;
	let mut verified_public_keys = BTreeSet::new();

	for role in REQUIRED_ROLES {
		let slot = slots
			.iter()
			.find(|slot| slot.get("role").and_then(Value::as_str) == Some(role))
			.ok_or_else(|| format!("missing approval slot for {role}"))?;
		if slot.get("status").and_then(Value::as_str) != Some("READY") {
			return Err(format!("approval slot {role} is not READY"));
		}
		let fingerprint = string_field(slot, "authorized_key_fingerprint_sha256", "approver slot")?;
		let key = keys
			.iter()
			.find(|key| {
				key.get("role").and_then(Value::as_str) == Some(role)
					&& key.get("fingerprint_sha256").and_then(Value::as_str) == Some(fingerprint)
			})
			.ok_or_else(|| format!("approved key for {role} is not in the checked-in registry"))?;
		if !has_exact_fields(
			key,
			&[
				"role",
				"fingerprint_sha256",
				"public_key_spki_der_base64",
				"valid_from",
				"valid_until",
				"revoked_at",
			],
		) {
			return Err(format!("authorized key for {role} has an invalid policy shape"));
		}
		let der = BASE64
			.decode(string_field(key, "public_key_spki_der_base64", "authorized key")?)
			.map_err(|_| format!("invalid Ed25519 SPKI encoding for {role}"))?;
		const ED25519_SPKI_PREFIX: [u8; 12] =
			[0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
		if der.len() != 44 || der[..12] != ED25519_SPKI_PREFIX {
			return Err(format!("authorized key for {role} is not an Ed25519 SPKI key"));
		}
		require_sha256(fingerprint, &der, &format!("authorized key fingerprint for {role}"))?;
		let public_bytes: [u8; 32] = der[12..]
			.try_into()
			.map_err(|_| format!("invalid Ed25519 public key length for {role}"))?;
		if !verified_public_keys.insert(public_bytes) {
			return Err("production approval roles must use distinct public keys".into());
		}
		let signature = signatures
			.iter()
			.find(|signature| {
				signature.get("role").and_then(Value::as_str) == Some(role)
					&& signature.get("key_fingerprint_sha256").and_then(Value::as_str)
						== Some(fingerprint)
			})
			.ok_or_else(|| format!("missing production launch signature for {role}"))?;
		if !has_exact_fields(
			signature,
			&[
				"role",
				"key_fingerprint_sha256",
				"payload_sha256",
				"public_key_spki_der_base64",
				"signature_base64",
				"signed_at",
			],
		) || string_field(signature, "payload_sha256", "launch signature")?
			!= sha256_hex(payload_bytes)
			|| string_field(signature, "public_key_spki_der_base64", "launch signature")?
				!= string_field(key, "public_key_spki_der_base64", "authorized key")?
		{
			return Err(format!("production launch signature for {role} is misbound"));
		}
		verify_signature_time(key, signature, timing)?;
		let signature_bytes = BASE64
			.decode(string_field(signature, "signature_base64", "launch signature")?)
			.map_err(|_| format!("invalid Ed25519 signature encoding for {role}"))?;
		let signature_raw: [u8; 64] = signature_bytes
			.try_into()
			.map_err(|_| format!("invalid Ed25519 signature length for {role}"))?;
		if !ed25519::Pair::verify(
			&ed25519::Signature::from_raw(signature_raw),
			payload_bytes,
			&ed25519::Public::from_raw(public_bytes),
		) {
			return Err(format!("invalid production launch signature for {role}"));
		}
	}
	if signatures.len() != REQUIRED_ROLES.len() {
		return Err(
			"production launch envelope must contain exactly one signature per required role"
				.into(),
		);
	}
	Ok(())
}

/// Verify the compile-time launch ceremony before a `Live` chain spec can be constructed.
pub fn authorize_production(
	chain: LaunchChain,
	genesis_input_bytes: &[u8],
	chain_spec_source: &[u8],
) -> Result<(), String> {
	let envelope = parse(LAUNCH_ENVELOPE, "production launch envelope")?;
	let payload = parse(LAUNCH_PAYLOAD, "production launch payload")?;
	if object_field(&envelope, "payload", "launch envelope")? != &payload {
		return Err(
			"production launch envelope payload differs from the canonical checked-in payload"
				.into(),
		);
	}
	require_sha256(
		string_field(&envelope, "payload_sha256", "launch envelope")?,
		LAUNCH_PAYLOAD,
		"production launch payload",
	)?;
	if string_field(&payload, "activation_state", "launch payload")? != "production-approved" {
		return Err(
			"production launch is blocked: activation_state is not production-approved".into()
		);
	}
	true_field(&payload, "production_activation", "launch payload")?;
	true_field(&payload, "network_launch_approval", "launch payload")?;
	true_field(&payload, "campaign_authorized", "launch payload")?;
	if string_field(&payload, "final_genesis_status", "launch payload")? != "FINAL" {
		return Err("production launch is blocked: final_genesis_status is not FINAL".into());
	}
	let timing = approval_timing(&payload)?;

	let p5_envelope = parse(P5_ENVELOPE, "P5 ratification envelope")?;
	let p5_payload = parse(P5_PAYLOAD, "P5 ratification payload")?;
	if object_field(&p5_envelope, "payload", "P5 envelope")? != &p5_payload {
		return Err("P5 envelope payload differs from its canonical checked-in payload".into());
	}
	require_sha256(
		string_field(&p5_envelope, "payload_sha256", "P5 envelope")?,
		P5_PAYLOAD,
		"P5 envelope payload",
	)?;
	require_sha256(
		string_field(&payload, "p5_payload_sha256", "launch payload")?,
		P5_PAYLOAD,
		"P5 payload",
	)?;
	true_field(&p5_payload, "network_launch_approval", "P5 payload")?;
	let p5_activation = object_field(&p5_payload, "production_activation", "P5 payload")?;
	true_field(p5_activation, "campaign_authorized", "P5 production_activation")?;
	if string_field(p5_activation, "final_genesis_status", "P5 production_activation")? != "FINAL" {
		return Err("P5 final genesis is not FINAL".into());
	}
	let final_hash = string_field(p5_activation, "final_genesis_hash", "P5 production_activation")?;
	if string_field(&payload, "final_genesis_hash", "launch payload")? != final_hash
		|| string_field(&payload, "genesis_header_hash", "launch payload")? != final_hash
	{
		return Err("final genesis and header identities are not exactly bound".into());
	}

	let fixture = object_field(&p5_payload, "fixture_identity", "P5 payload")?;
	for field in ["genesis_state_root", "chain_spec_source_sha256"] {
		if string_field(&payload, field, "launch payload")?
			!= string_field(fixture, field, "P5 fixture")?
		{
			return Err(format!("launch payload {field} differs from the P5 identity"));
		}
	}
	let runtime = object_field(&p5_payload, "runtime", "P5 payload")?;
	let metadata = parse(METADATA_IDENTITY, "metadata identity")?;
	let metadata_hash = string_field(runtime, "metadata_hash", "P5 runtime")?;
	if string_field(&payload, "metadata_hash", "launch payload")? != metadata_hash
		|| string_field(&metadata, "metadata_hash", "metadata identity")? != metadata_hash
	{
		return Err("production metadata identity is not exactly bound".into());
	}

	let inputs = object_field(&payload, "genesis_inputs", "launch payload")?;
	let chain_input = object_field(inputs, chain.name(), "genesis_inputs")?;
	require_sha256(
		string_field(chain_input, "sha256", "genesis input")?,
		genesis_input_bytes,
		&format!("{} genesis input", chain.name()),
	)?;
	let sources = object_field(&payload, "chain_spec_sources", "launch payload")?;
	require_sha256(
		string_field(
			object_field(sources, chain.name(), "chain_spec_sources")?,
			"sha256",
			"chain-spec source",
		)?,
		chain_spec_source,
		&format!("{} chain-spec source", chain.name()),
	)?;
	verify_authority_planes(&payload, chain, genesis_input_bytes)?;

	verify_role_signatures(&p5_envelope, &p5_payload, P5_PAYLOAD, timing)?;
	verify_role_signatures(&envelope, &payload, LAUNCH_PAYLOAD, timing)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn key(valid_from: &str, valid_until: &str, revoked_at: Value) -> Value {
		json!({ "valid_from": valid_from, "valid_until": valid_until, "revoked_at": revoked_at })
	}

	fn signature(signed_at: Option<&str>) -> Value {
		match signed_at {
			Some(value) => json!({ "signed_at": value }),
			None => json!({}),
		}
	}

	fn timing(finalized_at: &str, launch_epoch: &str, max_age: i64) -> ApprovalTiming {
		ApprovalTiming {
			finalized_at: rfc3339_epoch(finalized_at, "test finalized_at").unwrap(),
			launch_epoch: rfc3339_epoch(launch_epoch, "test launch_epoch").unwrap(),
			max_signature_age_seconds: max_age,
		}
	}

	#[test]
	fn signature_time_accepts_inclusive_key_boundaries() {
		let lower = key("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", Value::Null);
		assert!(verify_signature_time(
			&lower,
			&signature(Some("2026-01-01T00:00:00Z")),
			timing("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z", 1),
		)
		.is_ok());
		assert!(verify_signature_time(
			&lower,
			&signature(Some("2026-01-02T00:00:00Z")),
			timing("2026-01-02T00:00:00Z", "2026-01-02T00:00:00Z", 1),
		)
		.is_ok());
	}

	#[test]
	fn signature_time_rejects_before_valid_and_after_expiry() {
		let value = key("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", Value::Null);
		let at_start = timing("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z", 86_400);
		assert!(verify_signature_time(&value, &signature(Some("2025-12-31T23:59:59Z")), at_start,)
			.unwrap_err()
			.contains("expired or not yet valid"));
		let at_end = timing("2026-01-02T00:00:00Z", "2026-01-02T00:00:00Z", 86_400);
		assert!(verify_signature_time(&value, &signature(Some("2026-01-02T00:00:01Z")), at_end,)
			.unwrap_err()
			.contains("expired or not yet valid"));
	}

	#[test]
	fn signature_time_rejects_revocation_at_or_before_signature() {
		let timing = timing("2026-01-01T12:00:00Z", "2026-01-01T12:00:00Z", 86_400);
		for revoked_at in ["2026-01-01T09:59:59Z", "2026-01-01T10:00:00Z"] {
			let value = key("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", json!(revoked_at));
			assert!(verify_signature_time(
				&value,
				&signature(Some("2026-01-01T10:00:00Z")),
				timing,
			)
			.unwrap_err()
			.contains("revoked at or before"));
		}
		let revoked_after =
			key("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", json!("2026-01-01T10:00:01Z"));
		assert!(verify_signature_time(
			&revoked_after,
			&signature(Some("2026-01-01T10:00:00Z")),
			timing,
		)
		.is_ok());
	}

	#[test]
	fn signature_time_rejects_missing_or_malformed_timestamp() {
		let value = key("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", Value::Null);
		let timing = timing("2026-01-01T12:00:00Z", "2026-01-01T12:00:00Z", 86_400);
		assert!(verify_signature_time(&value, &signature(None), timing)
			.unwrap_err()
			.contains("missing signed_at"));
		assert!(verify_signature_time(&value, &signature(Some("not-a-time")), timing)
			.unwrap_err()
			.contains("exact RFC3339"));
		let missing_key_time = json!({
			"valid_until": "2026-01-02T00:00:00Z",
			"revoked_at": null
		});
		assert!(verify_signature_time(
			&missing_key_time,
			&signature(Some("2026-01-01T12:00:00Z")),
			timing,
		)
		.unwrap_err()
		.contains("missing valid_from"));
	}

	#[test]
	fn signature_time_enforces_finalization_freshness_without_wall_clock() {
		let value = key("2026-01-01T00:00:00Z", "2026-01-10T00:00:00Z", Value::Null);
		let error = verify_signature_time(
			&value,
			&signature(Some("2026-01-01T00:00:00Z")),
			timing("2026-01-03T00:00:01Z", "2026-01-03T00:00:01Z", 172_800),
		)
		.unwrap_err();
		assert!(error.contains("freshness window"));
	}
}

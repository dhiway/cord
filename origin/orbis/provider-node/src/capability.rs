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

//! Deterministic provider-capability decoding and finalized authority verification.

use std::array;

use ciborium::value::Value;
use orbis_storage_runtime_api::AgreementStatus;
use sha2::{Digest, Sha256};
use sp_core::{ed25519, Pair as _};
use unicode_normalization::UnicodeNormalization;

use crate::{CanonicalCid, CapabilityAuthoritySnapshot};

/// SHA-256 of the normative `cord.provider/1` registry consumed by the provider.
pub(crate) const NORMATIVE_REGISTRY_SHA256: [u8; 32] = [
	0xd1, 0x7c, 0x24, 0x59, 0x6f, 0xba, 0xe3, 0x0c, 0x30, 0x0d, 0x57, 0xae, 0x8e, 0x51, 0xbc, 0x0c,
	0x7b, 0x14, 0x9a, 0xb2, 0xe9, 0x1c, 0x2b, 0x9c, 0x75, 0x1b, 0xed, 0xd3, 0xfb, 0xc1, 0xee, 0xba,
];

const CAPABILITY_DOMAIN: &[u8] = b"cord.provider.capability.v1";
const MAX_CAPABILITY_LIFETIME: u64 = 128;
const MAX_PRODUCT_BYTES: usize = 128;
const MAX_METHODS: usize = 64;
const MAX_CID_BYTES: usize = 128;

/// Closed numeric capability-verification failures from the host protocol registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[repr(u16)]
pub(crate) enum CapabilityError {
	/// Input does not match the closed `ProviderCapabilityV1` schema.
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid = 100,
	/// Input is valid CBOR but not the one deterministic encoding of the capability.
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical = 101,
	/// Capability version is not version one.
	#[error("WIRE_VERSION_MISMATCH")]
	WireVersionMismatch = 102,
	/// Capability targets another chain genesis.
	#[error("WIRE_GENESIS_MISMATCH")]
	WireGenesisMismatch = 103,
	/// Capability or authority uses another normative registry.
	#[error("WIRE_DESCRIPTOR_MISMATCH")]
	WireDescriptorMismatch = 104,
	/// Finalized host delegation does not exist.
	#[error("GRANT_REQUIRED")]
	GrantRequired = 109,
	/// Capability exceeds or disagrees with the finalized delegation scope.
	#[error("GRANT_SCOPE_DENIED")]
	GrantScopeDenied = 110,
	/// Finalized host delegation is not live at the snapshot.
	#[error("GRANT_EXPIRED")]
	GrantExpired = 111,
	/// Capability signature does not verify with the finalized delegation key.
	#[error("CAPABILITY_SIGNATURE_INVALID")]
	CapabilitySignatureInvalid = 226,
	/// Capability targets another local provider.
	#[error("CAPABILITY_AUDIENCE_INVALID")]
	CapabilityAudienceInvalid = 227,
	/// Capability content scope does not match the requested content.
	#[error("CAPABILITY_CONTENT_INVALID")]
	CapabilityContentInvalid = 228,
	/// Capability nonce is not fresh; exact retry still requires durable recovery support.
	#[error("CAPABILITY_NONCE_REPLAY")]
	CapabilityNonceReplay = 229,
	/// Capability is not live or exceeds its 128-block lifetime.
	#[error("CAPABILITY_EXPIRED")]
	CapabilityExpired = 230,
	/// Finalized delegation was revoked or its signing key was rotated.
	#[error("CAPABILITY_ISSUER_REVOKED")]
	CapabilityIssuerRevoked = 231,
	/// Required agreement is missing or not active at the snapshot.
	#[error("AGREEMENT_INVALID_STATE")]
	AgreementInvalidState = 253,
	/// Capability or request exceeds the finalized agreement byte limit.
	#[error("AGREEMENT_CAPACITY_EXCEEDED")]
	AgreementCapacityExceeded = 254,
}

impl CapabilityError {
	/// Stable numeric error code from the normative host registry.
	pub(crate) const fn code(self) -> u16 {
		self as u16
	}
}

/// Exact closed-map model decoded from `ProviderCapabilityV1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderCapabilityV1 {
	/// Wire version; verification requires one.
	pub(crate) version: u8,
	/// Normative provider-registry SHA-256.
	pub(crate) registry_sha256: [u8; 32],
	/// Target chain genesis hash.
	pub(crate) genesis_hash: [u8; 32],
	/// Finalized host-delegation identifier.
	pub(crate) grant_id: [u8; 32],
	/// Finalized active host-delegation key identifier.
	pub(crate) issuer_key_id: [u8; 32],
	/// UTF-8 product identifier.
	pub(crate) product_id: String,
	/// Target control-plane bucket.
	pub(crate) bucket_id: [u8; 32],
	/// Optional storage agreement required by the capability.
	pub(crate) agreement_id: Option<[u8; 32]>,
	/// Exact provider audience.
	pub(crate) provider: [u8; 32],
	/// Bounded host method identifiers.
	pub(crate) methods: Vec<u16>,
	/// Optional canonical stored-content identifier.
	pub(crate) cid: Option<CanonicalCid>,
	/// Maximum bytes authorized by this capability.
	pub(crate) max_bytes: u64,
	/// First finalized block at which the capability may be used.
	pub(crate) issued_at: u64,
	/// Exclusive finalized expiry block.
	pub(crate) expires_at: u64,
	/// Provider-local replay nonce.
	pub(crate) nonce: [u8; 16],
	/// Detached Ed25519 signature over keys zero through fourteen.
	pub(crate) signature: [u8; 64],
}

impl ProviderCapabilityV1 {
	/// Decode the exact deterministic CBOR representation of the closed capability map.
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CapabilityError> {
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| CapabilityError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(CapabilityError::WireSchemaInvalid) };
		let mut fields: [Option<Value>; 16] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key = value_u64(key)?;
			let index: usize = key.try_into().map_err(|_| CapabilityError::WireSchemaInvalid)?;
			if index > 15 || fields[index].replace(value).is_some() {
				return Err(CapabilityError::WireSchemaInvalid);
			}
		}
		for key in [0usize, 1, 2, 3, 4, 5, 6, 8, 9, 11, 12, 13, 14, 15] {
			if fields[key].is_none() {
				return Err(CapabilityError::WireSchemaInvalid);
			}
		}

		let version = value_u64(take(&mut fields, 0)?)?
			.try_into()
			.map_err(|_| CapabilityError::WireSchemaInvalid)?;
		let registry_sha256 = fixed_bytes(take(&mut fields, 1)?)?;
		let genesis_hash = fixed_bytes(take(&mut fields, 2)?)?;
		let grant_id = fixed_bytes(take(&mut fields, 3)?)?;
		let issuer_key_id = fixed_bytes(take(&mut fields, 4)?)?;
		let product_id = bounded_text(take(&mut fields, 5)?, MAX_PRODUCT_BYTES)?;
		let bucket_id = fixed_bytes(take(&mut fields, 6)?)?;
		let agreement_id = fields[7].take().map(fixed_bytes).transpose()?;
		let provider = fixed_bytes(take(&mut fields, 8)?)?;
		let methods = methods(take(&mut fields, 9)?)?;
		let cid = fields[10]
			.take()
			.map(|value| {
				let text = bounded_text(value, MAX_CID_BYTES)?;
				CanonicalCid::parse(&text).map_err(|_| CapabilityError::WireSchemaInvalid)
			})
			.transpose()?;
		let max_bytes = value_u64(take(&mut fields, 11)?)?;
		let issued_at = value_u64(take(&mut fields, 12)?)?;
		let expires_at = value_u64(take(&mut fields, 13)?)?;
		let nonce = fixed_bytes(take(&mut fields, 14)?)?;
		let signature = fixed_bytes(take(&mut fields, 15)?)?;
		let capability = Self {
			version,
			registry_sha256,
			genesis_hash,
			grant_id,
			issuer_key_id,
			product_id,
			bucket_id,
			agreement_id,
			provider,
			methods,
			cid,
			max_bytes,
			issued_at,
			expires_at,
			nonce,
			signature,
		};
		if capability.canonical_bytes() != bytes {
			return Err(CapabilityError::WireNonCanonical);
		}
		Ok(capability)
	}

	/// Return the deterministic full capability encoding, including detached signature key 15.
	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		encode_map(self, true)
	}

	/// Return the exact domain-separated Ed25519 message for capability keys zero through fourteen.
	pub(crate) fn signed_preimage(&self) -> Vec<u8> {
		let unsigned = encode_map(self, false);
		let mut preimage = Vec::with_capacity(CAPABILITY_DOMAIN.len() + unsigned.len());
		preimage.extend_from_slice(CAPABILITY_DOMAIN);
		preimage.extend_from_slice(&unsigned);
		preimage
	}

	/// Return SHA-256 of the deterministic full capability bytes.
	pub(crate) fn canonical_capability_sha256(&self) -> [u8; 32] {
		Sha256::digest(self.canonical_bytes()).into()
	}

	/// Verify the detached signature with one exact finalized Ed25519 public key.
	pub(crate) fn verify_signature(&self, public_key: [u8; 32]) -> Result<(), CapabilityError> {
		if ed25519::Pair::verify(
			&ed25519::Signature::from_raw(self.signature),
			&self.signed_preimage(),
			&ed25519::Public::from_raw(public_key),
		) {
			Ok(())
		} else {
			Err(CapabilityError::CapabilitySignatureInvalid)
		}
	}
}

/// Requested provider operation that must be contained by a capability.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CapabilityRequest<'a> {
	/// Exact product identifier carried by the request.
	pub(crate) product_id: &'a str,
	/// Exact control-plane bucket carried by the request.
	pub(crate) bucket_id: [u8; 32],
	/// Exact optional storage agreement carried by the request.
	pub(crate) agreement_id: Option<[u8; 32]>,
	/// Exact host method being requested.
	pub(crate) method: u16,
	/// Exact optional canonical CID used by the request.
	pub(crate) cid: Option<&'a CanonicalCid>,
	/// Bytes the operation may store or return.
	pub(crate) bytes: u64,
	/// Whether this operation requires an active finalized agreement.
	pub(crate) requires_agreement: bool,
}

/// Provider-local replay-table observation; this type performs no persistence or mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityReplayInspection {
	/// The `(grant, nonce)` has not been observed.
	Fresh,
	/// The nonce and capability digest match, but durable request recovery is not implemented.
	ExactRetry,
	/// The nonce was observed with a different canonical capability digest.
	Conflict,
}

/// Read-only seam for a future provider-local durable capability replay table.
pub(crate) trait CapabilityReplayInspector {
	/// Inspect one grant/nonce/capability-digest tuple without consuming or persisting it.
	fn inspect(
		&self,
		grant_id: &[u8; 32],
		nonce: &[u8; 16],
		canonical_capability_sha256: &[u8; 32],
	) -> CapabilityReplayInspection;
}

/// Successful finalized capability decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedCapability {
	/// Finalized state hash that fixed every mutable authority record.
	pub(crate) finalized_hash: String,
	/// Finalized state number used for validity checks.
	pub(crate) finalized_number: u32,
	/// SHA-256 of only the canonical capability bytes, not a future request recovery fingerprint.
	pub(crate) canonical_capability_sha256: [u8; 32],
}

/// Verify a capability only against its singular finalized host delegation and optional agreement.
pub(crate) fn verify_capability<I: CapabilityReplayInspector>(
	capability: &ProviderCapabilityV1,
	snapshot: &CapabilityAuthoritySnapshot,
	request: CapabilityRequest<'_>,
	replay: &I,
) -> Result<VerifiedCapability, CapabilityError> {
	if capability.version != 1 {
		return Err(CapabilityError::WireVersionMismatch);
	}
	if capability.provider != snapshot.local_provider {
		return Err(CapabilityError::CapabilityAudienceInvalid);
	}
	if snapshot.registry_sha256 != NORMATIVE_REGISTRY_SHA256
		|| capability.registry_sha256 != NORMATIVE_REGISTRY_SHA256
	{
		return Err(CapabilityError::WireDescriptorMismatch);
	}
	if capability.genesis_hash != snapshot.genesis_hash {
		return Err(CapabilityError::WireGenesisMismatch);
	}
	let delegation = &snapshot.delegation;
	if capability.grant_id != delegation.grant_id.0 {
		return Err(CapabilityError::GrantRequired);
	}
	if capability.issuer_key_id != delegation.issuer_key_id.0 {
		return Err(CapabilityError::CapabilityIssuerRevoked);
	}
	if capability.product_id.as_bytes() != delegation.product_id.as_slice()
		|| capability.bucket_id != delegation.bucket_id.0
		|| capability.bucket_id != snapshot.bucket.bucket_id.0
	{
		return Err(CapabilityError::GrantScopeDenied);
	}
	if capability.product_id != request.product_id || capability.bucket_id != request.bucket_id {
		return Err(CapabilityError::GrantScopeDenied);
	}
	if capability.agreement_id != request.agreement_id
		|| (request.requires_agreement && request.agreement_id.is_none())
	{
		return Err(CapabilityError::AgreementInvalidState);
	}
	if !provider_is_assigned(
		&snapshot.local_provider,
		&snapshot.bucket.primary,
		&snapshot.bucket.replicas,
	) {
		return Err(CapabilityError::CapabilityAudienceInvalid);
	}
	if capability.methods.is_empty()
		|| capability.methods.iter().any(|method| !delegation.methods.contains(method))
		|| !capability.methods.contains(&request.method)
	{
		return Err(CapabilityError::GrantScopeDenied);
	}
	let delegated_cid = delegation
		.cid
		.as_ref()
		.map(|bytes| {
			let text =
				std::str::from_utf8(bytes).map_err(|_| CapabilityError::WireDescriptorMismatch)?;
			CanonicalCid::parse(text).map_err(|_| CapabilityError::WireDescriptorMismatch)
		})
		.transpose()?;
	if delegated_cid
		.as_ref()
		.is_some_and(|delegated| capability.cid.as_ref() != Some(delegated))
		|| capability.cid.as_ref() != request.cid
	{
		return Err(CapabilityError::CapabilityContentInvalid);
	}
	if capability.max_bytes > delegation.max_bytes || request.bytes > capability.max_bytes {
		return Err(CapabilityError::GrantScopeDenied);
	}

	let now = u64::from(snapshot.finalized_number);
	if delegation.revoked_at.is_some()
		|| capability.issued_at < u64::from(delegation.key_activated_at)
	{
		return Err(CapabilityError::CapabilityIssuerRevoked);
	}
	if now < u64::from(delegation.issued_at) || now >= u64::from(delegation.expires_at) {
		return Err(CapabilityError::GrantExpired);
	}
	if capability.expires_at <= capability.issued_at
		|| capability.expires_at - capability.issued_at > MAX_CAPABILITY_LIFETIME
		|| now < capability.issued_at
		|| now >= capability.expires_at
		|| capability.expires_at > u64::from(delegation.expires_at)
	{
		return Err(CapabilityError::CapabilityExpired);
	}

	match (capability.agreement_id, snapshot.agreement.as_ref()) {
		(None, None) if !request.requires_agreement => {},
		(Some(expected), Some(agreement)) => {
			if agreement.agreement_id.0 != expected
				|| agreement.bucket_id.0 != capability.bucket_id
				|| agreement.status != AgreementStatus::Active
				|| now >= u64::from(agreement.expires_at)
				|| !provider_is_assigned(
					&snapshot.local_provider,
					&agreement.primary,
					&agreement.replicas,
				) {
				return Err(CapabilityError::AgreementInvalidState);
			}
			if capability.max_bytes > agreement.bytes || request.bytes > agreement.bytes {
				return Err(CapabilityError::AgreementCapacityExceeded);
			}
		},
		_ => return Err(CapabilityError::AgreementInvalidState),
	}

	capability.verify_signature(delegation.issuer_public_key)?;
	// Recovery later hashes canonical RequestV2 followed by canonical authority. This narrower
	// digest is deliberately named as capability-only so it cannot be mistaken for that fingerprint.
	let canonical_capability_sha256 = capability.canonical_capability_sha256();
	if replay.inspect(&capability.grant_id, &capability.nonce, &canonical_capability_sha256)
		!= CapabilityReplayInspection::Fresh
	{
		return Err(CapabilityError::CapabilityNonceReplay);
	}
	Ok(VerifiedCapability {
		finalized_hash: snapshot.finalized_hash.clone(),
		finalized_number: snapshot.finalized_number,
		canonical_capability_sha256,
	})
}

fn provider_is_assigned(
	local_provider: &[u8; 32],
	primary: &sp_core::crypto::AccountId32,
	replicas: &[sp_core::crypto::AccountId32],
) -> bool {
	account_bytes(primary) == *local_provider
		|| replicas.iter().any(|provider| account_bytes(provider) == *local_provider)
}

fn account_bytes(account: &sp_core::crypto::AccountId32) -> [u8; 32] {
	let bytes: &[u8] = account.as_ref();
	bytes.try_into().expect("AccountId32 always contains 32 bytes")
}

fn take(fields: &mut [Option<Value>; 16], key: usize) -> Result<Value, CapabilityError> {
	fields[key].take().ok_or(CapabilityError::WireSchemaInvalid)
}

fn value_u64(value: Value) -> Result<u64, CapabilityError> {
	let Value::Integer(value) = value else { return Err(CapabilityError::WireSchemaInvalid) };
	value.try_into().map_err(|_| CapabilityError::WireSchemaInvalid)
}

fn fixed_bytes<const N: usize>(value: Value) -> Result<[u8; N], CapabilityError> {
	let Value::Bytes(bytes) = value else { return Err(CapabilityError::WireSchemaInvalid) };
	bytes.try_into().map_err(|_| CapabilityError::WireSchemaInvalid)
}

fn bounded_text(value: Value, max_bytes: usize) -> Result<String, CapabilityError> {
	let Value::Text(text) = value else { return Err(CapabilityError::WireSchemaInvalid) };
	if text.is_empty() || text.len() > max_bytes || !text.nfc().eq(text.chars()) {
		return Err(CapabilityError::WireSchemaInvalid);
	}
	Ok(text)
}

fn methods(value: Value) -> Result<Vec<u16>, CapabilityError> {
	let Value::Array(values) = value else { return Err(CapabilityError::WireSchemaInvalid) };
	if values.is_empty() || values.len() > MAX_METHODS {
		return Err(CapabilityError::WireSchemaInvalid);
	}
	values
		.into_iter()
		.map(|value| value_u64(value)?.try_into().map_err(|_| CapabilityError::WireSchemaInvalid))
		.collect()
}

fn encode_map(capability: &ProviderCapabilityV1, include_signature: bool) -> Vec<u8> {
	let mut entries = Vec::with_capacity(if include_signature { 16 } else { 15 });
	let mut push = |key: u8, value: Value| {
		entries.push((Value::Integer(key.into()), value));
	};
	push(0, Value::Integer(capability.version.into()));
	push(1, Value::Bytes(capability.registry_sha256.to_vec()));
	push(2, Value::Bytes(capability.genesis_hash.to_vec()));
	push(3, Value::Bytes(capability.grant_id.to_vec()));
	push(4, Value::Bytes(capability.issuer_key_id.to_vec()));
	push(5, Value::Text(capability.product_id.clone()));
	push(6, Value::Bytes(capability.bucket_id.to_vec()));
	if let Some(agreement_id) = capability.agreement_id {
		push(7, Value::Bytes(agreement_id.to_vec()));
	}
	push(8, Value::Bytes(capability.provider.to_vec()));
	push(
		9,
		Value::Array(
			capability.methods.iter().map(|value| Value::Integer((*value).into())).collect(),
		),
	);
	if let Some(cid) = &capability.cid {
		push(10, Value::Text(cid.as_str().to_owned()));
	}
	push(11, Value::Integer(capability.max_bytes.into()));
	push(12, Value::Integer(capability.issued_at.into()));
	push(13, Value::Integer(capability.expires_at.into()));
	push(14, Value::Bytes(capability.nonce.to_vec()));
	if include_signature {
		push(15, Value::Bytes(capability.signature.to_vec()));
	}
	let mut encoded = Vec::new();
	ciborium::ser::into_writer(&Value::Map(entries), &mut encoded)
		.expect("the in-memory deterministic capability model is serializable");
	encoded
}

#[cfg(test)]
mod tests {
	use super::*;
	use orbis_storage_runtime_api::{
		AgreementInfo, BucketGrantInfo, BucketRole, ControlBucketInfo, HostDelegationInfo,
	};
	use serde_json::Value as JsonValue;
	use sp_core::{crypto::AccountId32, H256};

	const VECTORS: &str =
		include_str!("../../../../docs/specs/protocol-executable-v2.vectors.json");

	struct Replay(CapabilityReplayInspection);

	impl CapabilityReplayInspector for Replay {
		fn inspect(
			&self,
			_grant_id: &[u8; 32],
			_nonce: &[u8; 16],
			_canonical_capability_sha256: &[u8; 32],
		) -> CapabilityReplayInspection {
			self.0
		}
	}

	fn vector() -> JsonValue {
		serde_json::from_str::<JsonValue>(VECTORS).expect("vectors are JSON")["vectors"]
			.as_array()
			.expect("vectors array")
			.iter()
			.find(|value| value["id"] == "provider-capability-v1")
			.expect("provider capability vector")
			.clone()
	}

	fn hex_field(value: &JsonValue, field: &str) -> Vec<u8> {
		hex::decode(value[field].as_str().expect("hex field")).expect("valid hex")
	}

	#[test]
	fn executable_vector_fixes_canonical_signature_and_fingerprint_bytes() {
		let vector = vector();
		let canonical = hex_field(&vector, "canonical_cbor_hex");
		let capability = ProviderCapabilityV1::decode(&canonical).expect("canonical vector");
		assert_eq!(capability.canonical_bytes(), canonical);
		assert_eq!(
			hex::encode(capability.signed_preimage()),
			vector["crypto"]["signed_bytes_hex"].as_str().unwrap()
		);
		assert_eq!(
			hex::encode(capability.canonical_capability_sha256()),
			vector["crypto"]["fingerprint_sha256"].as_str().unwrap()
		);
		let public_key: [u8; 32] =
			hex_field(&vector["crypto"], "public_key_hex").try_into().unwrap();
		assert_eq!(capability.verify_signature(public_key), Ok(()));

		let noncanonical = hex_field(&vector, "noncanonical_cbor_hex");
		assert_eq!(
			ProviderCapabilityV1::decode(&noncanonical),
			Err(CapabilityError::WireNonCanonical)
		);
		let bitflip =
			hex::decode(vector["negative_vectors"][0]["canonical_cbor_hex"].as_str().unwrap())
				.unwrap();
		let bitflip = ProviderCapabilityV1::decode(&bitflip).expect("canonical bitflip vector");
		assert_eq!(
			bitflip.verify_signature(public_key),
			Err(CapabilityError::CapabilitySignatureInvalid)
		);
		assert_eq!(CapabilityError::WireNonCanonical.code(), 101);
		assert_eq!(CapabilityError::CapabilitySignatureInvalid.code(), 226);
	}

	#[test]
	fn decoder_rejects_closed_map_and_scalar_ambiguity() {
		let canonical = hex_field(&vector(), "canonical_cbor_hex");
		let malformed = [
			[canonical.clone(), vec![0]].concat(),
			{
				let mut value = canonical.clone();
				value[0] = 0xbf;
				value.push(0xff);
				value
			},
			{
				let mut value = canonical.clone();
				value[1] = 0x18;
				value.insert(2, 0);
				value
			},
		];
		for value in malformed {
			assert_eq!(
				ProviderCapabilityV1::decode(&value),
				Err(CapabilityError::WireNonCanonical)
			);
		}
		let mut missing = canonical;
		missing[0] = 0xaf;
		missing.drain(1..3);
		assert_eq!(ProviderCapabilityV1::decode(&missing), Err(CapabilityError::WireSchemaInvalid));

		let canonical = hex_field(&vector(), "canonical_cbor_hex");
		let Value::Map(entries) = ciborium::de::from_reader(canonical.as_slice()).unwrap() else {
			unreachable!()
		};
		let encode = |entries: Vec<(Value, Value)>| {
			let mut bytes = Vec::new();
			ciborium::ser::into_writer(&Value::Map(entries), &mut bytes).unwrap();
			bytes
		};
		let mut out_of_order = entries.clone();
		out_of_order.reverse();
		assert_eq!(
			ProviderCapabilityV1::decode(&encode(out_of_order)),
			Err(CapabilityError::WireNonCanonical)
		);
		let mut duplicate = entries.clone();
		duplicate.push(entries[0].clone());
		assert_eq!(
			ProviderCapabilityV1::decode(&encode(duplicate)),
			Err(CapabilityError::WireSchemaInvalid)
		);
		let mut unknown = entries.clone();
		unknown.push((Value::Integer(16.into()), Value::Bool(false)));
		assert_eq!(
			ProviderCapabilityV1::decode(&encode(unknown)),
			Err(CapabilityError::WireSchemaInvalid)
		);
		let mut empty_product = entries;
		empty_product
			.iter_mut()
			.find(|(key, _)| key == &Value::Integer(5.into()))
			.unwrap()
			.1 = Value::Text(String::new());
		assert_eq!(
			ProviderCapabilityV1::decode(&encode(empty_product)),
			Err(CapabilityError::WireSchemaInvalid)
		);

		let canonical = hex_field(&vector(), "canonical_cbor_hex");
		let Value::Map(mut decomposed_product) =
			ciborium::de::from_reader(canonical.as_slice()).unwrap()
		else {
			unreachable!()
		};
		decomposed_product
			.iter_mut()
			.find(|(key, _)| key == &Value::Integer(5.into()))
			.unwrap()
			.1 = Value::Text("fe\u{301}stival".into());
		assert_eq!(
			ProviderCapabilityV1::decode(&encode(decomposed_product)),
			Err(CapabilityError::WireSchemaInvalid)
		);
	}

	fn signed_fixture() -> (ProviderCapabilityV1, CapabilityAuthoritySnapshot, CanonicalCid) {
		let pair = ed25519::Pair::from_seed(&[9; 32]);
		let cid = CanonicalCid::from_digest([7; 32]);
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			genesis_hash: [2; 32],
			grant_id: [3; 32],
			issuer_key_id: [4; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([6; 32]),
			provider: [7; 32],
			methods: vec![1010, 1011],
			cid: Some(cid.clone()),
			max_bytes: 4096,
			issued_at: 100,
			expires_at: 128,
			nonce: [8; 16],
			signature: [0; 64],
		};
		capability.signature = pair.sign(&capability.signed_preimage()).0;
		let local = AccountId32::new([7; 32]);
		let snapshot = CapabilityAuthoritySnapshot {
			finalized_hash: format!("0x{}", hex::encode([10; 32])),
			finalized_number: 110,
			genesis_hash: [2; 32],
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			local_provider: [7; 32],
			delegation: HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				issuance_nonce: 0,
				issuer_key_id: H256([4; 32]),
				issuer_public_key: pair.public().0,
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![1010, 1011, 1012],
				cid: Some(cid.as_str().as_bytes().to_vec()),
				max_bytes: 8192,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			},
			bucket: ControlBucketInfo {
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				version: 1,
				policy: H256([11; 32]),
				primary: local.clone(),
				replicas: vec![],
				grants: vec![BucketGrantInfo {
					account: AccountId32::new([99; 32]),
					role: BucketRole::Admin,
				}],
				created_at: 1,
			},
			agreement: Some(AgreementInfo {
				agreement_id: H256([6; 32]),
				owner: AccountId32::new([1; 32]),
				bucket_id: H256([5; 32]),
				primary: local,
				replicas: vec![],
				bytes: 8192,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}),
		};
		(capability, snapshot, cid)
	}

	fn request(cid: &CanonicalCid) -> CapabilityRequest<'_> {
		CapabilityRequest {
			product_id: "festival",
			bucket_id: [5; 32],
			agreement_id: Some([6; 32]),
			method: 1010,
			cid: Some(cid),
			bytes: 1024,
			requires_agreement: true,
		}
	}

	fn invalid_signature_fixture(
	) -> (ProviderCapabilityV1, CapabilityAuthoritySnapshot, CanonicalCid) {
		let (mut capability, snapshot, cid) = signed_fixture();
		capability.signature = [0; 64];
		(capability, snapshot, cid)
	}

	#[test]
	fn request_product_must_match_capability_before_signature_verification() {
		let (capability, snapshot, cid) = invalid_signature_fixture();
		let mut product = request(&cid);
		product.product_id = "levity";
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				product,
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::GrantScopeDenied)
		);
	}

	#[test]
	fn request_bucket_must_match_capability_before_signature_verification() {
		let (capability, snapshot, cid) = invalid_signature_fixture();
		let mut bucket = request(&cid);
		bucket.bucket_id = [55; 32];
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				bucket,
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::GrantScopeDenied)
		);
	}

	#[test]
	fn request_agreement_must_match_capability_before_signature_verification() {
		let (capability, snapshot, cid) = invalid_signature_fixture();
		let mut agreement = request(&cid);
		agreement.agreement_id = Some([66; 32]);
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				agreement,
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::AgreementInvalidState)
		);
	}

	#[test]
	fn finalized_verifier_is_scope_exact_and_has_no_acl_fallback() {
		let (capability, mut snapshot, cid) = signed_fixture();
		let verified = verify_capability(
			&capability,
			&snapshot,
			request(&cid),
			&Replay(CapabilityReplayInspection::Fresh),
		)
		.expect("finalized authority accepts exact capability");
		assert_eq!(verified.canonical_capability_sha256, capability.canonical_capability_sha256());
		snapshot.delegation.cid = None;
		verify_capability(
			&capability,
			&snapshot,
			request(&cid),
			&Replay(CapabilityReplayInspection::Fresh),
		)
		.expect("an unscoped delegation permits a narrower per-CID capability");

		let vector = vector();
		let cross_provider =
			hex::decode(vector["negative_vectors"][1]["canonical_cbor_hex"].as_str().unwrap())
				.unwrap();
		let cross_provider =
			ProviderCapabilityV1::decode(&cross_provider).expect("canonical cross-provider vector");
		assert_eq!(
			verify_capability(
				&cross_provider,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::CapabilityAudienceInvalid)
		);
		assert_eq!(CapabilityError::CapabilityAudienceInvalid.code(), 227);

		snapshot.delegation.revoked_at = Some(110);
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::CapabilityIssuerRevoked)
		);
		// The unrelated ACL administrator in `bucket.grants` is intentionally never consulted.
		snapshot.delegation.revoked_at = None;
		snapshot.delegation.issuer_key_id = H256([44; 32]);
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::CapabilityIssuerRevoked)
		);
	}

	#[test]
	fn agreement_lifetime_and_replay_checks_fail_closed() {
		let (mut capability, mut snapshot, cid) = signed_fixture();
		let pair = ed25519::Pair::from_seed(&[9; 32]);
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::ExactRetry),
			),
			Err(CapabilityError::CapabilityNonceReplay)
		);
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Conflict),
			),
			Err(CapabilityError::CapabilityNonceReplay)
		);

		snapshot.agreement.as_mut().unwrap().status = AgreementStatus::Suspended;
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::AgreementInvalidState)
		);
		snapshot.agreement.as_mut().unwrap().status = AgreementStatus::Active;
		snapshot.agreement.as_mut().unwrap().bytes = 512;
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::AgreementCapacityExceeded)
		);

		snapshot.agreement.as_mut().unwrap().bytes = 8192;
		capability.expires_at = capability.issued_at + 129;
		capability.signature = pair.sign(&capability.signed_preimage()).0;
		assert_eq!(
			verify_capability(
				&capability,
				&snapshot,
				request(&cid),
				&Replay(CapabilityReplayInspection::Fresh),
			),
			Err(CapabilityError::CapabilityExpired)
		);
	}
}

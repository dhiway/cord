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

//! CORD Origin SDK-backed finality lane for native checkpoint submissions.

use async_trait::async_trait;
use oc::{
	product_sdk::{NativeLifecycle, NativeLifecycleState, OrbisNativeClient, OrbisTxPipeline},
	OriginSigner,
};
use sp_core::{ed25519, Pair as _};
use subxt::Metadata;

use super::{
	checkpoint_outbox::{
		finality_attestation_digest, FINALITY_ATTESTATION_VERSION, FINALIZED_STATE,
	},
	checkpoint_promotion::{promotion_finality_attestation_digest, FallbackPromotionIntentV2},
	checkpoint_promotion_submitter::{PromotionFinalityLane, NATIVE_INTENT_PREFIX},
	checkpoint_submitter::{CheckpointFinalityLane, FinalizedEvidence},
};
use crate::ContentError;

const CHECKPOINT_INTENT_PREFIX: &str = "orbis-checkpoint-v2-";

/// Live CORD SDK lane with distinct extrinsic and finality-attestation signers.
pub(crate) struct OriginRsCheckpointFinalityLane {
	client: OrbisNativeClient,
	pipeline: OrbisTxPipeline,
	signer: OriginSigner,
	signer_account: [u8; 32],
	service_key: ed25519::Pair,
	metadata: Metadata,
}

impl OriginRsCheckpointFinalityLane {
	/// Capture one metadata snapshot and retain one long-lived account-serialized Orbis pipeline.
	pub(crate) fn new(
		client: OrbisNativeClient,
		signer: OriginSigner,
		service_key: ed25519::Pair,
	) -> Result<Self, ContentError> {
		let signer_account = signer.account_id().into();
		let metadata = client.online().metadata();
		let pipeline = OrbisTxPipeline::new(client.online().clone());
		Ok(Self { client, pipeline, signer, signer_account, service_key, metadata })
	}

	async fn finalize_native(
		&self,
		intent_id: &str,
		payload: subxt::tx::DynamicPayload,
	) -> Result<NativeFinalizedEvidence, ContentError> {
		let lifecycle = self
			.pipeline
			.submit_and_finalize(&self.signer, intent_id, payload)
			.await
			.map_err(|error| ContentError::Io(error.to_string()))?;
		let (block_hash, extrinsic_hash) = finalized_hashes(intent_id, &lifecycle)?;
		let block = self
			.client
			.online()
			.blocks()
			.at(subxt::config::substrate::H256::from(block_hash))
			.await
			.map_err(|error| ContentError::Io(error.to_string()))?;
		let observed_hash = parse_hash(&format!("{:#x}", block.hash()))?;
		if observed_hash != block_hash {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(NativeFinalizedEvidence {
			block_hash,
			block_number: checked_block_number(u64::from(block.number()))?,
			extrinsic_hash,
		})
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFinalizedEvidence {
	block_hash: [u8; 32],
	block_number: u32,
	extrinsic_hash: [u8; 32],
}

#[async_trait]
impl CheckpointFinalityLane for OriginRsCheckpointFinalityLane {
	fn metadata(&self) -> &Metadata {
		&self.metadata
	}

	fn signer_account(&self) -> [u8; 32] {
		self.signer_account
	}

	fn service_key(&self) -> [u8; 32] {
		self.service_key.public().0
	}

	async fn submit_and_finalize(
		&self,
		intent_id: &str,
		payload: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError> {
		let submission_id = checkpoint_submission_id(intent_id)?;
		let evidence = self.finalize_native(intent_id, payload).await?;
		signed_evidence(
			&self.service_key,
			submission_id,
			evidence.block_hash,
			evidence.block_number,
			evidence.extrinsic_hash,
		)
	}
}

#[async_trait]
impl PromotionFinalityLane for OriginRsCheckpointFinalityLane {
	fn metadata(&self) -> &Metadata {
		&self.metadata
	}

	fn signer_account(&self) -> [u8; 32] {
		self.signer_account
	}

	fn service_key(&self) -> [u8; 32] {
		self.service_key.public().0
	}

	async fn submit_and_finalize(
		&self,
		native_intent_id: &str,
		intent: &FallbackPromotionIntentV2,
		payload: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError> {
		let intent_id = promotion_intent_id(native_intent_id)?;
		if intent_id != intent.intent_id {
			return Err(ContentError::IntegrityFailed);
		}
		let evidence = self.finalize_native(native_intent_id, payload).await?;
		signed_promotion_evidence(&self.service_key, intent, evidence)
	}
}

fn finalized_hashes(
	expected_intent_id: &str,
	lifecycle: &NativeLifecycle,
) -> Result<([u8; 32], [u8; 32]), ContentError> {
	lifecycle.validate().map_err(|_| ContentError::IntegrityFailed)?;
	if lifecycle.state != NativeLifecycleState::Finalized ||
		lifecycle.intent_id != expected_intent_id
	{
		return Err(ContentError::IntegrityFailed);
	}
	let block_hash = lifecycle.block_hash.as_deref().ok_or(ContentError::IntegrityFailed)?;
	let extrinsic_hash =
		lifecycle.extrinsic_hash.as_deref().ok_or(ContentError::IntegrityFailed)?;
	Ok((parse_hash(block_hash)?, parse_hash(extrinsic_hash)?))
}

fn checkpoint_submission_id(intent_id: &str) -> Result<&str, ContentError> {
	canonical_intent_suffix(intent_id, CHECKPOINT_INTENT_PREFIX)
}

fn promotion_intent_id(intent_id: &str) -> Result<&str, ContentError> {
	canonical_intent_suffix(intent_id, NATIVE_INTENT_PREFIX)
}

fn canonical_intent_suffix<'a>(intent_id: &'a str, prefix: &str) -> Result<&'a str, ContentError> {
	let value = intent_id.strip_prefix(prefix).ok_or(ContentError::IntegrityFailed)?;
	if value.len() != 64 ||
		value.bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(value)
}

fn signed_promotion_evidence(
	service_key: &ed25519::Pair,
	intent: &FallbackPromotionIntentV2,
	evidence: NativeFinalizedEvidence,
) -> Result<FinalizedEvidence, ContentError> {
	let digest = promotion_finality_attestation_digest(
		FINALITY_ATTESTATION_VERSION,
		&intent.intent_id,
		&intent.record_hash,
		&intent.tuple_key,
		evidence.block_hash,
		evidence.block_number,
		evidence.extrinsic_hash,
		FINALIZED_STATE,
	)?;
	Ok(FinalizedEvidence {
		block_hash: evidence.block_hash,
		block_number: evidence.block_number,
		extrinsic_hash: evidence.extrinsic_hash,
		finality_attestation_version: FINALITY_ATTESTATION_VERSION,
		finality_signature: service_key.sign(&digest).0,
	})
}

fn parse_hash(value: &str) -> Result<[u8; 32], ContentError> {
	let encoded = value.strip_prefix("0x").ok_or(ContentError::IntegrityFailed)?;
	if encoded.len() != 64 ||
		encoded
			.bytes()
			.any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
	{
		return Err(ContentError::IntegrityFailed);
	}
	hex::decode(encoded)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn checked_block_number(number: u64) -> Result<u32, ContentError> {
	number.try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn signed_evidence(
	service_key: &ed25519::Pair,
	submission_id: &str,
	block_hash: [u8; 32],
	block_number: u32,
	extrinsic_hash: [u8; 32],
) -> Result<FinalizedEvidence, ContentError> {
	let digest = finality_attestation_digest(
		FINALITY_ATTESTATION_VERSION,
		submission_id,
		block_hash,
		block_number,
		extrinsic_hash,
		FINALIZED_STATE,
	)?;
	Ok(FinalizedEvidence {
		block_hash,
		block_number,
		extrinsic_hash,
		finality_attestation_version: FINALITY_ATTESTATION_VERSION,
		finality_signature: service_key.sign(&digest).0,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn lifecycle(intent_id: &str) -> NativeLifecycle {
		NativeLifecycle {
			version: 1,
			intent_id: intent_id.into(),
			state: NativeLifecycleState::Finalized,
			block_hash: Some(format!("0x{}", "08".repeat(32))),
			extrinsic_hash: Some(format!("0x{}", "09".repeat(32))),
			error: None,
		}
	}

	#[test]
	fn lifecycle_requires_exact_finalized_hash_evidence() {
		let intent = format!("{CHECKPOINT_INTENT_PREFIX}{}", "01".repeat(32));
		assert_eq!(finalized_hashes(&intent, &lifecycle(&intent)).unwrap(), ([8; 32], [9; 32]));
		let mut included = lifecycle(&intent);
		included.state = NativeLifecycleState::Included;
		included.extrinsic_hash = None;
		assert!(matches!(finalized_hashes(&intent, &included), Err(ContentError::IntegrityFailed)));
		let mut noncanonical = lifecycle(&intent);
		noncanonical.block_hash = Some(format!("0x{}", "AA".repeat(32)));
		assert!(matches!(
			finalized_hashes(&intent, &noncanonical),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn lifecycle_intent_mismatch_fails_closed() {
		let expected = format!("{CHECKPOINT_INTENT_PREFIX}{}", "01".repeat(32));
		let returned = format!("{CHECKPOINT_INTENT_PREFIX}{}", "02".repeat(32));
		assert!(matches!(
			finalized_hashes(&expected, &lifecycle(&returned)),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn block_number_overflow_fails_closed() {
		assert_eq!(checked_block_number(u64::from(u32::MAX)).unwrap(), u32::MAX);
		assert!(matches!(
			checked_block_number(u64::from(u32::MAX) + 1),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn finality_signature_binds_every_evidence_field() {
		let service_key = ed25519::Pair::from_seed(&[7; 32]);
		let submission_id = "01".repeat(32);
		let evidence = signed_evidence(&service_key, &submission_id, [8; 32], 44, [9; 32]).unwrap();
		let digest = finality_attestation_digest(
			evidence.finality_attestation_version,
			&submission_id,
			evidence.block_hash,
			evidence.block_number,
			evidence.extrinsic_hash,
			FINALIZED_STATE,
		)
		.unwrap();
		assert!(ed25519::Pair::verify(
			&ed25519::Signature::from_raw(evidence.finality_signature),
			&digest,
			&service_key.public()
		));
		let changed = finality_attestation_digest(
			evidence.finality_attestation_version,
			&submission_id,
			[10; 32],
			evidence.block_number,
			evidence.extrinsic_hash,
			FINALIZED_STATE,
		)
		.unwrap();
		assert!(!ed25519::Pair::verify(
			&ed25519::Signature::from_raw(evidence.finality_signature),
			&changed,
			&service_key.public()
		));
	}

	#[test]
	fn promotion_signature_uses_the_same_native_lane_but_its_own_domain() {
		let service_key = ed25519::Pair::from_seed(&[7; 32]);
		let intent = FallbackPromotionIntentV2 {
			version: 2,
			intent_id: "01".repeat(32),
			tuple_key: "02".repeat(32),
			provider: "03".repeat(32),
			duty_scale: String::new(),
			duty_fingerprint: "04".repeat(32),
			inventory_finalized_hash: "05".repeat(32),
			inventory_finalized_number: 40,
			snapshot_checkpoint: 40,
			bucket_id: "06".repeat(32),
			duty_id: "07".repeat(32),
			service_key_version: 1,
			payload_scale: String::new(),
			service_key: hex::encode(service_key.public().0),
			signature: String::new(),
			call_args_scale: String::new(),
			state: "authorized".into(),
			record_hash: "08".repeat(32),
		};
		let native = NativeFinalizedEvidence {
			block_hash: [9; 32],
			block_number: 44,
			extrinsic_hash: [10; 32],
		};
		let evidence = signed_promotion_evidence(&service_key, &intent, native).unwrap();
		let digest = promotion_finality_attestation_digest(
			evidence.finality_attestation_version,
			&intent.intent_id,
			&intent.record_hash,
			&intent.tuple_key,
			evidence.block_hash,
			evidence.block_number,
			evidence.extrinsic_hash,
			FINALIZED_STATE,
		)
		.unwrap();
		assert!(ed25519::Pair::verify(
			&ed25519::Signature::from_raw(evidence.finality_signature),
			&digest,
			&service_key.public()
		));
		assert_eq!(
			promotion_intent_id(&format!("{NATIVE_INTENT_PREFIX}{}", intent.intent_id)).unwrap(),
			intent.intent_id
		);
		assert!(matches!(
			promotion_intent_id(&format!("{CHECKPOINT_INTENT_PREFIX}{}", intent.intent_id)),
			Err(ContentError::IntegrityFailed)
		));
	}
}

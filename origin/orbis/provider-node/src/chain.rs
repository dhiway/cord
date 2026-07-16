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

//! Finalized Commons runtime authority binding.

use async_trait::async_trait;
use codec::{Decode, Encode};
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use orbis_storage_runtime_api::{
	AgreementInfo, AgreementStatus, CheckpointDutyCursor, CheckpointDutyInfo,
	CheckpointDutyMode as RuntimeCheckpointDutyMode, CheckpointDutyPage, CheckpointDutyPageError,
	CheckpointDutyPhase as RuntimeCheckpointDutyPhase, ControlBucketInfo, HostDelegationInfo,
	ProviderDutyRole, ProviderInfo, ProviderStatus, Versioned, MAX_CHECKPOINT_DUTY_PAGE_SIZE,
	RESPONSE_VERSION,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};

use crate::capability::NORMATIVE_REGISTRY_SHA256;

/// Authorization returned after checking a storage agreement at a finalized block.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgreementAuthorization {
	/// Finalized block used for the decision.
	pub finalized_hash: String,
	/// Agreement identifier.
	pub agreement_id: String,
	/// Provider account authorized by the agreement.
	pub provider: String,
	/// Canonical container reference.
	pub container_ref: String,
	/// Maximum byte length authorized by the agreement.
	pub bytes: u64,
	/// Expiry block.
	pub expires_at: u32,
}

/// Singular capability authority loaded from one exact finalized Commons state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityAuthoritySnapshot {
	/// Finalized block hash used for every mutable runtime-API read.
	pub finalized_hash: String,
	/// Finalized block number corresponding to `finalized_hash`.
	pub finalized_number: u32,
	/// Immutable chain block-zero hash.
	pub genesis_hash: [u8; 32],
	/// Normative provider-registry SHA-256 supported by this provider build.
	pub registry_sha256: [u8; 32],
	/// Provider account configured locally.
	pub local_provider: [u8; 32],
	/// Required finalized host delegation. Missing delegation is a terminal chain error.
	pub delegation: HostDelegationInfo<AccountId32, H256, u32>,
	/// Exact control bucket referenced by the delegation; ACL grants are never capability fallback.
	pub bucket: ControlBucketInfo<AccountId32, H256, u32>,
	/// Optional exact agreement requested by the capability.
	pub agreement: Option<AgreementInfo<AccountId32, H256, u32>>,
}

/// Open proof duty discovered from a finalized `StorageProviderApi::challenges_at` query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChallengeDuty {
	/// Challenge identifier passed to `submit_checkpoint`.
	pub challenge_id: String,
	/// Agreement being challenged.
	pub agreement_id: String,
	/// Agreement's canonical raw-content commitment.
	pub content_commitment: String,
	/// Provider root committed by the challenge issuer.
	pub expected_commitment: String,
	/// Inclusive due block.
	pub due_at: u32,
	/// Finalized hash at which the duty was observed.
	pub observed_at: String,
}

/// Bounded finalized challenge scan result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChallengeBatch {
	/// Highest finalized block scanned.
	pub finalized_number: u32,
	/// Hash of that finalized block.
	pub finalized_hash: String,
	/// Highest challenge due-block inspected in the finalized state.
	pub scanned_through: u32,
	/// Open duties belonging to this provider.
	pub duties: Vec<ChallengeDuty>,
}

/// Provider role assigned by one finalized checkpoint duty.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointDutyRole {
	/// The provider is the duty's primary.
	Primary,
	/// The provider is one of the duty's replicas.
	Replica,
}

/// Finalized scheduling phase reported for a checkpoint duty.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointDutyPhase {
	/// Duty is retained but is not due yet.
	NotDue,
	/// Primary provider may initiate during its grace window.
	Primary,
	/// Deterministic replica fallback may initiate.
	ReplicaFallback,
	/// Replica fallback must additionally promote the checkpoint.
	ReplicaFallbackPromotion,
	/// Fallback quorum is insufficient.
	BlockedInsufficientFallbackQuorum,
	/// No eligible initiator exists.
	Unavailable,
}

/// Runtime mode attached to a finalized checkpoint duty.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointDutyMode {
	/// Normal checkpoint flow.
	Standard,
	/// Checkpoint promotion is pending.
	PromotionPending,
}

/// Exact runtime cursor retained as the durable end of a fully scanned duty snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDutyScanCursor {
	/// Governed finalized checkpoint which fixes the duty snapshot.
	pub snapshot_checkpoint: u32,
	/// SCALE-encoded bucket key of the last duty in the snapshot.
	pub last_key: String,
}

/// Durable request used to resume one fixed finalized checkpoint-duty snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDutyPageRequest {
	/// Fixed finalized block hash used for every page and retry.
	pub finalized_hash: String,
	/// Block number corresponding to `finalized_hash`.
	pub finalized_number: u32,
	/// Provider account which owns the scan.
	pub provider: String,
	/// Governed finalized checkpoint shared by all staged pages.
	pub snapshot_checkpoint: u32,
	/// Exact opaque cursor returned by the preceding runtime page.
	pub cursor: CheckpointDutyScanCursor,
}

/// One finalized checkpoint duty addressed to this provider.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDuty {
	/// Runtime-assigned duty identifier.
	pub duty_id: String,
	/// Runtime bucket identifier.
	pub bucket_id: String,
	/// Local provider account to which the duty is addressed.
	pub provider: String,
	/// Local provider role in the duty.
	pub role: CheckpointDutyRole,
	/// Active finalized service-key version.
	pub service_key_version: u64,
	/// Active finalized service key.
	pub service_key: String,
	/// Governed finalized checkpoint which fixes the duty.
	pub snapshot_checkpoint: u32,
	/// Hash of the governed finalized checkpoint.
	pub snapshot_hash: String,
	/// First block at which the duty is due.
	pub due_at: u32,
	/// Last primary grace block.
	pub grace_until: u32,
	/// Finalized scheduling phase; blocked and not-due duties remain durable.
	pub phase: CheckpointDutyPhase,
	/// Finalized checkpoint mode.
	pub mode: CheckpointDutyMode,
	/// Whether the local finalized authority may sign this duty.
	pub may_sign: bool,
	/// Whether the local finalized authority may initiate this duty.
	pub may_initiate: bool,
	/// Exact SCALE encoding of `CheckpointDutyInfo` for later protocol stages.
	pub encoded_duty: String,
	/// Blake2-256 digest of `encoded_duty`, used to reject changed-payload replay.
	pub duty_fingerprint: String,
}

/// One validated checkpoint-duty page read at a fixed finalized block hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDutyBatch {
	/// Finalized block hash used for every runtime API page.
	pub finalized_hash: String,
	/// Finalized block number corresponding to `finalized_hash`.
	pub finalized_number: u32,
	/// Provider account which owns the scan.
	pub provider: String,
	/// Governed finalized checkpoint shared by every returned page and duty.
	pub snapshot_checkpoint: u32,
	/// Cursor used to request this page; `None` only for the first page.
	pub requested_cursor: Option<CheckpointDutyScanCursor>,
	/// Exact opaque cursor for the next page; `None` marks the terminal page.
	pub next_cursor: Option<CheckpointDutyScanCursor>,
	/// Duties addressed to the configured provider in this page.
	pub duties: Vec<CheckpointDuty>,
}

/// Errors from the finalized chain authority.
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
	/// Transport or JSON-RPC failure.
	#[error("Orbis RPC failed: {0}")]
	Rpc(String),
	/// Runtime API returned malformed SCALE.
	#[error("Commons runtime API response is invalid: {0}")]
	Decode(String),
	/// Agreement/provider/challenge state rejected the operation.
	#[error("storage authority rejected: {0}")]
	Rejected(String),
	/// Finalized checkpoint-duty paging or audience contract was violated.
	#[error("checkpoint duty protocol rejected: {0}")]
	DutyProtocol(String),
}

/// Canonical authority seam for all content mutations and proof duties.
#[async_trait]
pub trait ChainAuthority: Send + Sync + 'static {
	/// Load the singular host-delegation capability authority at one finalized state hash.
	///
	/// The default rejects rather than consulting bucket ACLs, bearer credentials, DIDs or any
	/// other fallback. Implementations that support the internal capability protocol override it.
	async fn capability_authority_snapshot(
		&self,
		_grant_id: [u8; 32],
		_agreement_id: Option<[u8; 32]>,
	) -> Result<CapabilityAuthoritySnapshot, ChainError> {
		Err(ChainError::Rejected(
			"finalized host-delegation capability authority unavailable".into(),
		))
	}

	/// Validate a content write against one exact finalized Orbis state.
	async fn authorize_commit(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
		bytes: u64,
	) -> Result<AgreementAuthorization, ChainError>;

	/// Validate deletion against a cancelled or expired finalized agreement.
	async fn authorize_delete(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
	) -> Result<AgreementAuthorization, ChainError>;

	/// Scan a bounded future due-block window in finalized state. The returned safe cursor never
	/// advances past the current finalized block, so newly-created future duties cannot be skipped.
	async fn challenge_duties(
		&self,
		after_block: Option<u32>,
	) -> Result<ChallengeBatch, ChainError>;

	/// Read and validate one page, starting or resuming one fixed finalized duty snapshot.
	async fn checkpoint_duties(
		&self,
		request: Option<CheckpointDutyPageRequest>,
	) -> Result<CheckpointDutyBatch, ChainError>;
}

/// Runtime-API client that validates decisions at `chain_getFinalizedHead`.
pub struct FinalizedRuntimeAuthority {
	client: HttpClient,
	provider: AccountId32,
	service_key: [u8; 32],
}

impl FinalizedRuntimeAuthority {
	/// Connect to an Orbis HTTP JSON-RPC endpoint and bind one provider/service-key pair.
	pub fn connect(
		endpoint: &str,
		provider: AccountId32,
		service_key: [u8; 32],
	) -> Result<Self, ChainError> {
		let client = jsonrpsee::http_client::HttpClientBuilder::default()
			.build(endpoint)
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		Ok(Self { client, provider, service_key })
	}

	async fn finalized_context(&self) -> Result<(String, u32), ChainError> {
		let hash: String = self
			.client
			.request("chain_getFinalizedHead", rpc_params![])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let header: RpcHeader = self
			.client
			.request("chain_getHeader", rpc_params![hash.clone()])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let number = u32::from_str_radix(header.number.trim_start_matches("0x"), 16)
			.map_err(|error| ChainError::Decode(error.to_string()))?;
		self.ensure_provider(&hash, number).await?;
		Ok((hash, number))
	}

	async fn ensure_provider(&self, hash: &str, finalized_number: u32) -> Result<(), ChainError> {
		let response: Versioned<ProviderInfo<H256, u32>> = self
			.runtime_call("StorageProviderApi_provider", self.provider.encode(), hash)
			.await?;
		ensure_version(response.version)?;
		let provider = response
			.value
			.ok_or_else(|| ChainError::Rejected("provider is not registered".into()))?;
		if provider.status != ProviderStatus::Active {
			return Err(ChainError::Rejected("provider is suspended".into()));
		}
		let finalized_service_key = if provider
			.service_key
			.pending_effective_at
			.is_some_and(|at| at <= finalized_number)
		{
			provider.service_key.pending.ok_or_else(|| {
				ChainError::DutyProtocol(
					"finalized provider key activation has no pending key".into(),
				)
			})?
		} else {
			provider.service_key.active
		};
		if finalized_service_key != self.service_key {
			return Err(ChainError::Rejected(
				"local service key does not match finalized provider record".into(),
			));
		}
		Ok(())
	}

	async fn agreement(
		&self,
		agreement_id: [u8; 32],
		hash: &str,
	) -> Result<AgreementInfo<AccountId32, H256, u32>, ChainError> {
		let response: Versioned<AgreementInfo<AccountId32, H256, u32>> = self
			.runtime_call("StorageProviderApi_agreement", H256::from(agreement_id).encode(), hash)
			.await?;
		ensure_version(response.version)?;
		let agreement = response
			.value
			.ok_or_else(|| ChainError::Rejected("agreement not found".into()))?;
		if agreement.primary != self.provider && !agreement.replicas.contains(&self.provider) {
			return Err(ChainError::Rejected("agreement belongs to other providers".into()));
		}
		Ok(agreement)
	}

	async fn capability_delegation(
		&self,
		grant_id: [u8; 32],
		hash: &str,
	) -> Result<HostDelegationInfo<AccountId32, H256, u32>, ChainError> {
		let response: Versioned<HostDelegationInfo<AccountId32, H256, u32>> = self
			.runtime_call(
				"StorageProviderApi_capability_authority",
				H256::from(grant_id).encode(),
				hash,
			)
			.await?;
		ensure_version(response.version)?;
		response.value.ok_or_else(|| {
			ChainError::Rejected(
				"finalized host delegation not found; no authority fallback".into(),
			)
		})
	}

	async fn control_bucket(
		&self,
		bucket_id: H256,
		hash: &str,
	) -> Result<ControlBucketInfo<AccountId32, H256, u32>, ChainError> {
		let response: Versioned<ControlBucketInfo<AccountId32, H256, u32>> = self
			.runtime_call("StorageProviderApi_control_bucket", bucket_id.encode(), hash)
			.await?;
		ensure_version(response.version)?;
		response
			.value
			.ok_or_else(|| ChainError::Rejected("host delegation control bucket not found".into()))
	}

	async fn genesis_hash(&self) -> Result<[u8; 32], ChainError> {
		let hash: Option<String> = self
			.client
			.request("chain_getBlockHash", rpc_params![0u32])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let hash =
			hash.ok_or_else(|| ChainError::Rejected("chain genesis hash not found".into()))?;
		decode_hash(&hash, "genesis hash")
	}

	async fn runtime_call<T: Decode>(
		&self,
		method: &str,
		params: Vec<u8>,
		hash: &str,
	) -> Result<T, ChainError> {
		let encoded: String = self
			.client
			.request("state_call", rpc_params![method, format!("0x{}", hex::encode(params)), hash])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let raw = hex::decode(encoded.trim_start_matches("0x"))
			.map_err(|error| ChainError::Decode(error.to_string()))?;
		T::decode(&mut &raw[..]).map_err(|error| ChainError::Decode(error.to_string()))
	}

	fn authorization(
		&self,
		agreement: AgreementInfo<AccountId32, H256, u32>,
		finalized_hash: String,
		authorized_bytes: u64,
	) -> AgreementAuthorization {
		let provider_bytes: &[u8] = self.provider.as_ref();
		AgreementAuthorization {
			finalized_hash,
			agreement_id: format!("{:#x}", agreement.agreement_id),
			provider: format!("0x{}", hex::encode(provider_bytes)),
			container_ref: format!("{:#x}", agreement.bucket_id),
			bytes: authorized_bytes,
			expires_at: agreement.expires_at,
		}
	}
}

#[async_trait]
impl ChainAuthority for FinalizedRuntimeAuthority {
	async fn capability_authority_snapshot(
		&self,
		grant_id: [u8; 32],
		agreement_id: Option<[u8; 32]>,
	) -> Result<CapabilityAuthoritySnapshot, ChainError> {
		let (finalized_hash, finalized_number) = self.finalized_context().await?;
		let delegation = self.capability_delegation(grant_id, &finalized_hash).await?;
		let bucket = self.control_bucket(delegation.bucket_id, &finalized_hash).await?;
		let agreement = match agreement_id {
			Some(agreement_id) => Some(self.agreement(agreement_id, &finalized_hash).await?),
			None => None,
		};
		let provider: &[u8] = self.provider.as_ref();
		Ok(CapabilityAuthoritySnapshot {
			finalized_hash,
			finalized_number,
			genesis_hash: self.genesis_hash().await?,
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			local_provider: provider
				.try_into()
				.expect("AccountId32 always contains exactly 32 bytes"),
			delegation,
			bucket,
			agreement,
		})
	}

	async fn authorize_commit(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
		bytes: u64,
	) -> Result<AgreementAuthorization, ChainError> {
		let (hash, number) = self.finalized_context().await?;
		let agreement = self.agreement(agreement_id, &hash).await?;
		if agreement.status != AgreementStatus::Active || number >= agreement.expires_at {
			return Err(ChainError::Rejected("agreement is not live and active".into()));
		}
		let _ = content_commitment;
		if bytes > agreement.bytes {
			return Err(ChainError::Rejected(format!(
				"content length {bytes} exceeds agreement capacity {}",
				agreement.bytes
			)));
		}
		Ok(self.authorization(agreement, hash, bytes))
	}

	async fn authorize_delete(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
	) -> Result<AgreementAuthorization, ChainError> {
		let (hash, _number) = self.finalized_context().await?;
		let agreement = self.agreement(agreement_id, &hash).await?;
		let _ = content_commitment;
		let terminal =
			matches!(agreement.status, AgreementStatus::Cancelled | AgreementStatus::Expired);
		if !terminal {
			return Err(ChainError::Rejected(
				"agreement must be finalized as cancelled or expired before content deletion"
					.into(),
			));
		}
		let bytes = agreement.bytes;
		Ok(self.authorization(agreement, hash, bytes))
	}

	async fn challenge_duties(
		&self,
		_after_block: Option<u32>,
	) -> Result<ChallengeBatch, ChainError> {
		Err(ChainError::Rejected(
			"legacy content-challenge intake is not valid for checkpoint duty API v9".into(),
		))
	}

	async fn checkpoint_duties(
		&self,
		request: Option<CheckpointDutyPageRequest>,
	) -> Result<CheckpointDutyBatch, ChainError> {
		let provider_bytes: &[u8] = self.provider.as_ref();
		let provider = format!("0x{}", hex::encode(provider_bytes));
		let (finalized_hash, finalized_number, expected_snapshot, requested_cursor) = match request
		{
			None => {
				let (hash, number) = self.finalized_context().await?;
				canonical_hash(&hash)?;
				(hash, number, None, None)
			},
			Some(request) => {
				if request.provider != provider {
					return Err(ChainError::DutyProtocol(
						"resume request belongs to another provider".into(),
					));
				}
				canonical_hash(&request.finalized_hash)?;
				if request.cursor.snapshot_checkpoint != request.snapshot_checkpoint {
					return Err(ChainError::DutyProtocol(
						"resume cursor belongs to another snapshot".into(),
					));
				}
				let cursor = decode_checkpoint_cursor(&request.cursor)?;
				let header: RpcHeader = self
					.client
					.request("chain_getHeader", rpc_params![request.finalized_hash.clone()])
					.await
					.map_err(|error| ChainError::Rpc(error.to_string()))?;
				let number = u32::from_str_radix(header.number.trim_start_matches("0x"), 16)
					.map_err(|error| ChainError::Decode(error.to_string()))?;
				if number != request.finalized_number {
					return Err(ChainError::DutyProtocol(
						"resume finalized hash and number disagree".into(),
					));
				}
				self.ensure_provider(&request.finalized_hash, number).await?;
				(request.finalized_hash, number, Some(request.snapshot_checkpoint), Some(cursor))
			},
		};
		let params =
			(self.provider.clone(), requested_cursor.clone(), MAX_CHECKPOINT_DUTY_PAGE_SIZE)
				.encode();
		let result: Result<
			CheckpointDutyPage<CheckpointDutyInfo<AccountId32, H256, u32>, u32>,
			CheckpointDutyPageError,
		> = self
			.runtime_call("StorageProviderApi_checkpoint_duties", params, &finalized_hash)
			.await?;
		let page = result.map_err(|error| {
			ChainError::DutyProtocol(format!("runtime rejected checkpoint duty page: {error:?}"))
		})?;
		if page.version != RESPONSE_VERSION {
			return Err(ChainError::DutyProtocol(format!(
				"checkpoint duty page response version {} is not {RESPONSE_VERSION}",
				page.version
			)));
		}
		if expected_snapshot.is_some_and(|expected| expected != page.snapshot_checkpoint) {
			return Err(ChainError::DutyProtocol(
				"checkpoint duty snapshot changed while resuming fixed finalized state".into(),
			));
		}
		let snapshot_checkpoint = page.snapshot_checkpoint;
		let mut duties = Vec::with_capacity(page.items.len());
		let mut last_key = None;
		for duty in page.items {
			last_key = Some(duty.bucket_id.encode());
			duties.push(validate_checkpoint_duty(
				duty,
				&self.provider,
				self.service_key,
				snapshot_checkpoint,
			)?);
		}
		let next_cursor = match page.next_cursor {
			Some(cursor) => {
				if cursor.snapshot_checkpoint != snapshot_checkpoint {
					return Err(ChainError::DutyProtocol(
						"checkpoint duty cursor has the wrong snapshot".into(),
					));
				}
				if last_key.as_ref() != Some(&cursor.last_key) {
					return Err(ChainError::DutyProtocol(
						"checkpoint duty cursor does not bind the page tail".into(),
					));
				}
				if requested_cursor.as_ref() == Some(&cursor) {
					return Err(ChainError::DutyProtocol(
						"checkpoint duty cursor did not advance".into(),
					));
				}
				Some(encode_checkpoint_cursor(cursor))
			},
			None => None,
		};
		Ok(CheckpointDutyBatch {
			finalized_hash,
			finalized_number,
			provider,
			snapshot_checkpoint,
			requested_cursor: requested_cursor.map(encode_checkpoint_cursor),
			next_cursor,
			duties,
		})
	}
}

fn canonical_hash(value: &str) -> Result<(), ChainError> {
	let raw = hex::decode(value.strip_prefix("0x").ok_or_else(|| {
		ChainError::DutyProtocol("checkpoint duty hash is not 0x-prefixed".into())
	})?)
	.map_err(|_| ChainError::DutyProtocol("checkpoint duty hash is not hexadecimal".into()))?;
	if raw.len() != 32 || format!("0x{}", hex::encode(raw)) != value {
		return Err(ChainError::DutyProtocol(
			"checkpoint duty hash is not canonical 32-byte lowercase hex".into(),
		));
	}
	Ok(())
}

fn decode_hash(value: &str, label: &str) -> Result<[u8; 32], ChainError> {
	let raw = hex::decode(
		value
			.strip_prefix("0x")
			.ok_or_else(|| ChainError::Decode(format!("{label} is not 0x-prefixed")))?,
	)
	.map_err(|_| ChainError::Decode(format!("{label} is not hexadecimal")))?;
	if format!("0x{}", hex::encode(&raw)) != value {
		return Err(ChainError::Decode(format!("{label} is not canonical lowercase hex")));
	}
	raw.try_into()
		.map_err(|_| ChainError::Decode(format!("{label} is not 32 bytes")))
}

fn decode_checkpoint_cursor(
	cursor: &CheckpointDutyScanCursor,
) -> Result<CheckpointDutyCursor<u32>, ChainError> {
	canonical_hash(&cursor.last_key)?;
	Ok(CheckpointDutyCursor {
		snapshot_checkpoint: cursor.snapshot_checkpoint,
		last_key: hex::decode(cursor.last_key.trim_start_matches("0x"))
			.expect("canonical_hash checked hexadecimal"),
	})
}

fn encode_checkpoint_cursor(cursor: CheckpointDutyCursor<u32>) -> CheckpointDutyScanCursor {
	CheckpointDutyScanCursor {
		snapshot_checkpoint: cursor.snapshot_checkpoint,
		last_key: format!("0x{}", hex::encode(cursor.last_key)),
	}
}

pub(crate) fn validate_checkpoint_duty(
	duty: CheckpointDutyInfo<AccountId32, H256, u32>,
	provider: &AccountId32,
	service_key: [u8; 32],
	snapshot_checkpoint: u32,
) -> Result<CheckpointDuty, ChainError> {
	if duty.response_version != RESPONSE_VERSION {
		return Err(ChainError::DutyProtocol(format!(
			"checkpoint duty response version {} is not {RESPONSE_VERSION}",
			duty.response_version
		)));
	}
	if duty.snapshot_checkpoint != snapshot_checkpoint {
		return Err(ChainError::DutyProtocol("checkpoint duty has the wrong snapshot".into()));
	}
	let mut expected = Vec::with_capacity(duty.replicas.len().saturating_add(1));
	expected.push((&duty.primary, ProviderDutyRole::Primary, 0u8));
	for (index, replica) in duty.replicas.iter().enumerate() {
		expected.push((replica, ProviderDutyRole::Replica, index.saturating_add(1) as u8));
	}
	if duty.authorities.len() != expected.len() {
		return Err(ChainError::DutyProtocol(
			"checkpoint duty authority set does not match its provider audience".into(),
		));
	}
	let mut local = None;
	for ((expected_provider, expected_role, expected_order), authority) in
		expected.iter().zip(&duty.authorities)
	{
		if authority.provider != **expected_provider
			|| authority.role != *expected_role
			|| authority.order != *expected_order
		{
			return Err(ChainError::DutyProtocol(
				"checkpoint duty authority ordering does not match its provider audience".into(),
			));
		}
		if &authority.provider == provider {
			if local.is_some() {
				return Err(ChainError::DutyProtocol(
					"checkpoint duty addresses the local provider more than once".into(),
				));
			}
			local = Some(authority);
		}
	}
	let local = local.ok_or_else(|| {
		ChainError::DutyProtocol("checkpoint duty is addressed to another provider".into())
	})?;
	if local.active_service_key != service_key {
		return Err(ChainError::DutyProtocol(
			"checkpoint duty service key does not match the local finalized key".into(),
		));
	}
	if let Some(initiator) = duty.initiator.as_ref() {
		let Some(authority) = duty.authorities.iter().find(|item| &item.provider == initiator)
		else {
			return Err(ChainError::DutyProtocol(
				"checkpoint duty initiator is outside its provider audience".into(),
			));
		};
		if !authority.may_initiate {
			return Err(ChainError::DutyProtocol(
				"checkpoint duty initiator is not authorized to initiate".into(),
			));
		}
	}
	let role = match local.role {
		ProviderDutyRole::Primary => CheckpointDutyRole::Primary,
		ProviderDutyRole::Replica => CheckpointDutyRole::Replica,
	};
	let phase = match duty.phase {
		RuntimeCheckpointDutyPhase::NotDue => CheckpointDutyPhase::NotDue,
		RuntimeCheckpointDutyPhase::Primary => CheckpointDutyPhase::Primary,
		RuntimeCheckpointDutyPhase::ReplicaFallback => CheckpointDutyPhase::ReplicaFallback,
		RuntimeCheckpointDutyPhase::ReplicaFallbackPromotion => {
			CheckpointDutyPhase::ReplicaFallbackPromotion
		},
		RuntimeCheckpointDutyPhase::BlockedInsufficientFallbackQuorum => {
			CheckpointDutyPhase::BlockedInsufficientFallbackQuorum
		},
		RuntimeCheckpointDutyPhase::Unavailable => CheckpointDutyPhase::Unavailable,
	};
	let mode = match duty.mode {
		RuntimeCheckpointDutyMode::Standard => CheckpointDutyMode::Standard,
		RuntimeCheckpointDutyMode::PromotionPending => CheckpointDutyMode::PromotionPending,
	};
	let encoded = duty.encode();
	let provider_bytes: &[u8] = provider.as_ref();
	Ok(CheckpointDuty {
		duty_id: format!("{:#x}", duty.duty_id),
		bucket_id: format!("{:#x}", duty.bucket_id),
		provider: format!("0x{}", hex::encode(provider_bytes)),
		role,
		service_key_version: local.active_service_key_version,
		service_key: format!("0x{}", hex::encode(local.active_service_key)),
		snapshot_checkpoint,
		snapshot_hash: format!("{:#x}", duty.snapshot_hash),
		due_at: duty.due_at,
		grace_until: duty.grace_until,
		phase,
		mode,
		may_sign: local.may_sign,
		may_initiate: local.may_initiate,
		encoded_duty: format!("0x{}", hex::encode(&encoded)),
		duty_fingerprint: format!("0x{}", hex::encode(sp_crypto_hashing::blake2_256(&encoded))),
	})
}

fn ensure_version(version: u16) -> Result<(), ChainError> {
	if version == RESPONSE_VERSION {
		Ok(())
	} else {
		Err(ChainError::Rejected(format!(
			"unsupported StorageProviderApi response version {version}"
		)))
	}
}

#[derive(Deserialize)]
struct RpcHeader {
	number: String,
}

#[cfg(test)]
mod tests {
	use std::{convert::Infallible, sync::Arc};

	use http_body_util::{BodyExt, Full};
	use hyper::{body::Bytes, server::conn::http1, service::service_fn, Request, Response};
	use hyper_util::rt::TokioIo;
	use orbis_storage_runtime_api::{
		BucketGrantInfo, BucketRole, OrganizationInfo, ServiceKeyInfo,
	};
	use serde_json::{json, Value};
	use tokio::{net::TcpListener, sync::Mutex};

	use super::*;

	const FINALIZED_HASH: &str =
		"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
	const GENESIS_HASH: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

	fn runtime_response(method: &str) -> String {
		let provider = AccountId32::new([7; 32]);
		let owner = AccountId32::new([1; 32]);
		let encoded = match method {
			"StorageProviderApi_provider" => Versioned::new(Some(ProviderInfo {
				endpoint: b"https://provider.invalid".to_vec(),
				organization: OrganizationInfo {
					entity_id: b"enterprise".to_vec(),
					attestation_id: H256([1; 32]),
					schema_id: H256([2; 32]),
					sla_commitment: H256([3; 32]),
					sla_version: 1,
					valid_from: 1,
					valid_until: 1_000,
					rotation_predecessor: None,
				},
				service_key: ServiceKeyInfo {
					active: [9; 32],
					active_version: 1,
					previous: None,
					pending: None,
					pending_version: None,
					pending_effective_at: None,
				},
				capacity_bytes: 1_000_000,
				allocated_bytes: 0,
				pending_bytes: 0,
				status: ProviderStatus::Active,
				last_heartbeat: 109,
				overdue_challenges: 0,
				authority_validated_at: Some(100),
			}))
			.encode(),
			"StorageProviderApi_capability_authority" => Versioned::new(Some(HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256([5; 32]),
				owner: owner.clone(),
				issuance_nonce: 0,
				issuer_key_id: H256([4; 32]),
				issuer_public_key: [6; 32],
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![1010],
				cid: None,
				max_bytes: 4096,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			}))
			.encode(),
			"StorageProviderApi_control_bucket" => Versioned::new(Some(ControlBucketInfo {
				bucket_id: H256([5; 32]),
				owner: owner.clone(),
				version: 1,
				policy: H256([8; 32]),
				primary: provider.clone(),
				replicas: vec![],
				grants: vec![BucketGrantInfo { account: owner.clone(), role: BucketRole::Admin }],
				created_at: 1,
			}))
			.encode(),
			"StorageProviderApi_agreement" => Versioned::new(Some(AgreementInfo {
				agreement_id: H256([6; 32]),
				owner,
				bucket_id: H256([5; 32]),
				primary: provider,
				replicas: vec![],
				bytes: 4096,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}))
			.encode(),
			other => panic!("unexpected runtime API method: {other}"),
		};
		format!("0x{}", hex::encode(encoded))
	}

	async fn rpc_response(
		request: Request<hyper::body::Incoming>,
		reads: Arc<Mutex<Vec<(String, String)>>>,
	) -> Result<Response<Full<Bytes>>, Infallible> {
		let body = request.into_body().collect().await.unwrap().to_bytes();
		let request: Value = serde_json::from_slice(&body).unwrap();
		let method = request["method"].as_str().unwrap();
		let result = match method {
			"chain_getFinalizedHead" => json!(FINALIZED_HASH),
			"chain_getHeader" => json!({ "number": "0x6e" }),
			"chain_getBlockHash" => json!(GENESIS_HASH),
			"state_call" => {
				let params = request["params"].as_array().unwrap();
				let runtime_method = params[0].as_str().unwrap().to_owned();
				let at = params[2].as_str().unwrap().to_owned();
				reads.lock().await.push((runtime_method.clone(), at));
				json!(runtime_response(&runtime_method))
			},
			other => panic!("unexpected JSON-RPC method: {other}"),
		};
		let body = serde_json::to_vec(&json!({
			"jsonrpc": "2.0",
			"id": request["id"],
			"result": result,
		}))
		.unwrap();
		Ok(Response::new(Full::new(Bytes::from(body))))
	}

	#[tokio::test]
	async fn capability_snapshot_pins_every_mutable_authority_read_to_one_finalized_hash() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let reads = Arc::new(Mutex::new(Vec::new()));
		let server_reads = reads.clone();
		let server = tokio::spawn(async move {
			loop {
				let (stream, _) = listener.accept().await.unwrap();
				let connection_reads = server_reads.clone();
				tokio::spawn(async move {
					http1::Builder::new()
						.serve_connection(
							TokioIo::new(stream),
							service_fn(move |request| {
								rpc_response(request, connection_reads.clone())
							}),
						)
						.await
						.unwrap();
				});
			}
		});

		let authority = FinalizedRuntimeAuthority::connect(
			&format!("http://{address}"),
			AccountId32::new([7; 32]),
			[9; 32],
		)
		.unwrap();
		let snapshot =
			authority.capability_authority_snapshot([3; 32], Some([6; 32])).await.unwrap();
		assert_eq!(snapshot.finalized_hash, FINALIZED_HASH);
		assert_eq!(snapshot.finalized_number, 110);
		assert_eq!(snapshot.genesis_hash, [0xbb; 32]);
		let reads = reads.lock().await.clone();
		assert_eq!(
			reads.iter().map(|(method, _)| method.as_str()).collect::<Vec<_>>(),
			[
				"StorageProviderApi_provider",
				"StorageProviderApi_capability_authority",
				"StorageProviderApi_control_bucket",
				"StorageProviderApi_agreement",
			]
		);
		assert!(reads.iter().all(|(_, at)| at == FINALIZED_HASH));
		server.abort();
	}
}

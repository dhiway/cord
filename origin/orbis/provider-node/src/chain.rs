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
	CheckpointDutyPhase as RuntimeCheckpointDutyPhase, CheckpointInfo, ControlBucketInfo,
	HostDelegationInfo, ProviderDutyRole, ProviderInfo, ProviderStatus, Versioned,
	MAX_CHECKPOINT_DUTY_PAGE_SIZE, RESPONSE_VERSION,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};
use sp_crypto_hashing::blake2_256;

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

/// One reason an ordered replication member cannot currently be used.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) enum ReplicationProviderExclusion {
	MissingProvider,
	Inactive,
	GovernedCheckpointUnavailable,
	OrgInvalid,
	AuthorityUnvalidated,
	OverdueChallenge,
	RuntimeIneligible,
	InvalidEndpoint,
	InvalidServiceKey,
	ConfirmationInvalid,
}

impl ReplicationProviderExclusion {
	fn prevents_local_participation(self) -> bool {
		!matches!(self, Self::ConfirmationInvalid)
	}
}

/// One evidence-preserving provider slot in finalized bucket membership order.
#[allow(dead_code)]
#[derive(Clone, Debug, Encode, Eq, PartialEq)]
pub(crate) struct ReplicationProviderSnapshot {
	pub(crate) provider: [u8; 32],
	pub(crate) order: u8,
	pub(crate) primary: bool,
	pub(crate) record_present: bool,
	pub(crate) endpoint: Option<Vec<u8>>,
	pub(crate) endpoint_hash: Option<[u8; 32]>,
	pub(crate) active_service_key: Option<[u8; 32]>,
	pub(crate) active_service_key_version: Option<u64>,
	pub(crate) status_active: bool,
	pub(crate) organization_valid: bool,
	pub(crate) authority_validated_at: Option<u32>,
	pub(crate) overdue_challenges: u32,
	/// Exact answer returned by `provider_is_eligible` at the pinned state hash.
	pub(crate) eligible: bool,
	/// True only when the complete retained evidence has no exclusion.
	pub(crate) usable: bool,
	pub(crate) exclusions: Vec<ReplicationProviderExclusion>,
	pub(crate) confirmed_checkpoint: Option<u32>,
}

/// Exact finalized replication topology for one control bucket.
#[allow(dead_code)]
#[derive(Clone, Debug, Encode, Eq, PartialEq)]
pub(crate) struct ReplicationTopologySnapshot {
	pub(crate) genesis_hash: [u8; 32],
	pub(crate) finalized_hash: [u8; 32],
	/// Header number retained solely as the pinned transport-state identity.
	pub(crate) finalized_number: u32,
	/// Runtime-governed checkpoint used for authority and confirmation semantics.
	pub(crate) governed_finalized_checkpoint: Option<u32>,
	pub(crate) bucket_id: [u8; 32],
	pub(crate) bucket_version: u64,
	pub(crate) primary: [u8; 32],
	pub(crate) replicas: Vec<[u8; 32]>,
	pub(crate) providers: Vec<ReplicationProviderSnapshot>,
	pub(crate) current_checkpoint: Option<CheckpointInfo<AccountId32, H256, u32>>,
	pub(crate) snapshot_hash: [u8; 32],
}

/// Private authority seam for a replication session pinned to one finalized Commons state.
#[async_trait]
#[allow(dead_code)]
pub(crate) trait ReplicationAuthority: Send + Sync {
	async fn replication_topology(
		&self,
		bucket_id: [u8; 32],
	) -> Result<ReplicationTopologySnapshot, ChainError>;

	async fn replication_topology_at(
		&self,
		bucket_id: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
	) -> Result<ReplicationTopologySnapshot, ChainError>;
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
		let mut input = &raw[..];
		let decoded =
			T::decode(&mut input).map_err(|error| ChainError::Decode(error.to_string()))?;
		if !input.is_empty() {
			return Err(ChainError::Decode("runtime API response contains trailing bytes".into()));
		}
		Ok(decoded)
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
	async fn replication_topology_at_hash(
		&self,
		bucket_id: [u8; 32],
		finalized_hash_text: String,
		expected_number: Option<u32>,
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		let finalized_hash = decode_hash(&finalized_hash_text, "finalized hash")?;
		let header: RpcHeader = self
			.client
			.request("chain_getHeader", rpc_params![finalized_hash_text.clone()])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let finalized_number = u32::from_str_radix(header.number.trim_start_matches("0x"), 16)
			.map_err(|error| ChainError::Decode(error.to_string()))?;
		if expected_number.is_some_and(|expected| expected != finalized_number) {
			return Err(ChainError::Rejected(
				"replication finalized hash and number disagree".into(),
			));
		}
		let governed_finalized_checkpoint: Option<u32> = self
			.runtime_call(
				"StorageProviderApi_governed_finalized_checkpoint",
				().encode(),
				&finalized_hash_text,
			)
			.await?;
		let bucket_hash = H256::from(bucket_id);
		let bucket_response: Versioned<ControlBucketInfo<AccountId32, H256, u32>> = self
			.runtime_call(
				"StorageProviderApi_control_bucket",
				bucket_hash.encode(),
				&finalized_hash_text,
			)
			.await?;
		ensure_version(bucket_response.version)?;
		let bucket = bucket_response
			.value
			.ok_or_else(|| ChainError::Rejected("replication control bucket not found".into()))?;
		if bucket.bucket_id != bucket_hash {
			return Err(ChainError::Rejected(
				"replication control bucket response has the wrong bucket id".into(),
			));
		}
		let membership = replication_members(&bucket, &self.provider)?;
		let checkpoint_response: Versioned<CheckpointInfo<AccountId32, H256, u32>> = self
			.runtime_call(
				"StorageProviderApi_checkpoint",
				bucket_hash.encode(),
				&finalized_hash_text,
			)
			.await?;
		ensure_version(checkpoint_response.version)?;
		let current_checkpoint = checkpoint_response.value;
		if current_checkpoint
			.as_ref()
			.is_some_and(|checkpoint| checkpoint.bucket_id != bucket_hash)
		{
			return Err(ChainError::Rejected(
				"replication checkpoint response has the wrong bucket id".into(),
			));
		}

		let mut providers = Vec::with_capacity(membership.len());
		for (index, provider) in membership.iter().enumerate() {
			let response: Versioned<ProviderInfo<H256, u32>> = self
				.runtime_call(
					"StorageProviderApi_provider",
					provider.encode(),
					&finalized_hash_text,
				)
				.await?;
			ensure_version(response.version)?;
			let eligible: bool = self
				.runtime_call(
					"StorageProviderApi_provider_is_eligible",
					provider.encode(),
					&finalized_hash_text,
				)
				.await?;
			let confirmed_checkpoint = if index == 0 {
				current_checkpoint.as_ref().map(|checkpoint| checkpoint.checkpoint_block)
			} else {
				self.runtime_call(
					"StorageProviderApi_replica_checkpoint",
					(bucket_hash, provider.clone()).encode(),
					&finalized_hash_text,
				)
				.await?
			};
			providers.push(replication_provider(
				provider,
				index,
				response.value,
				governed_finalized_checkpoint,
				eligible,
				confirmed_checkpoint,
			)?);
		}
		apply_confirmation_evidence(
			&mut providers,
			&current_checkpoint,
			governed_finalized_checkpoint,
			&bucket.replicas,
		)?;

		let primary = account_bytes(&bucket.primary);
		let replicas = bucket.replicas.iter().map(account_bytes).collect();
		let mut snapshot = ReplicationTopologySnapshot {
			genesis_hash: self.genesis_hash().await?,
			finalized_hash,
			finalized_number,
			governed_finalized_checkpoint,
			bucket_id,
			bucket_version: bucket.version,
			primary,
			replicas,
			providers,
			current_checkpoint,
			snapshot_hash: [0; 32],
		};
		snapshot.snapshot_hash = snapshot.calculated_hash();
		snapshot.validate(account_bytes(&self.provider), self.service_key)?;
		Ok(snapshot)
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

#[async_trait]
impl ReplicationAuthority for FinalizedRuntimeAuthority {
	async fn replication_topology(
		&self,
		bucket_id: [u8; 32],
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		let finalized_hash: String = self
			.client
			.request("chain_getFinalizedHead", rpc_params![])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		self.replication_topology_at_hash(bucket_id, finalized_hash, None).await
	}

	async fn replication_topology_at(
		&self,
		bucket_id: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		let canonical_hash: Option<String> = self
			.client
			.request("chain_getBlockHash", rpc_params![finalized_number])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let canonical_hash = canonical_hash
			.ok_or_else(|| ChainError::Rejected("replication finalized block not found".into()))?;
		if decode_hash(&canonical_hash, "replication finalized hash")? != finalized_hash {
			return Err(ChainError::Rejected(
				"replication finalized hash is not canonical at its claimed number".into(),
			));
		}
		self.replication_topology_at_hash(
			bucket_id,
			format!("0x{}", hex::encode(finalized_hash)),
			Some(finalized_number),
		)
		.await
	}
}

#[allow(dead_code)]
impl ReplicationTopologySnapshot {
	fn calculated_hash(&self) -> [u8; 32] {
		let mut canonical = self.clone();
		canonical.snapshot_hash = [0; 32];
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		canonical.encode_to(&mut input);
		blake2_256(&input)
	}

	pub(crate) fn validate(
		&self,
		local_provider: [u8; 32],
		local_service_key: [u8; 32],
	) -> Result<(), ChainError> {
		let mut members = Vec::with_capacity(self.replicas.len().saturating_add(1));
		members.push(self.primary);
		members.extend_from_slice(&self.replicas);
		let mut unique = std::collections::BTreeSet::new();
		if members.iter().any(|member| !unique.insert(*member)) {
			return Err(ChainError::Rejected(
				"replication control bucket contains duplicate providers".into(),
			));
		}
		if !unique.contains(&local_provider) {
			return Err(ChainError::Rejected(
				"local provider is not a member of the replication bucket".into(),
			));
		}
		if self.providers.len() != members.len() {
			return Err(ChainError::Rejected("replication provider snapshot is incomplete".into()));
		}
		for (index, (expected, provider)) in members.iter().zip(&self.providers).enumerate() {
			if provider.provider != *expected
				|| usize::from(provider.order) != index
				|| provider.primary != (index == 0)
			{
				return Err(ChainError::Rejected(
					"replication provider order does not match bucket membership".into(),
				));
			}
			if provider.record_present
				== provider.exclusions.contains(&ReplicationProviderExclusion::MissingProvider)
			{
				return Err(ChainError::Rejected(
					"replication provider presence evidence is inconsistent".into(),
				));
			}
			if provider.endpoint.is_some() != provider.endpoint_hash.is_some() {
				return Err(ChainError::Rejected(
					"replication provider endpoint evidence is incomplete".into(),
				));
			}
			if provider.record_present != provider.endpoint.is_some() {
				return Err(ChainError::Rejected(
					"replication provider endpoint presence is inconsistent".into(),
				));
			}
			if let (Some(endpoint), Some(endpoint_hash)) =
				(&provider.endpoint, provider.endpoint_hash)
			{
				if blake2_256(endpoint) != endpoint_hash {
					return Err(ChainError::Rejected(
						"replication provider endpoint hash mismatch".into(),
					));
				}
				let invalid = validate_provider_endpoint(endpoint).is_err();
				if invalid
					!= provider.exclusions.contains(&ReplicationProviderExclusion::InvalidEndpoint)
				{
					return Err(ChainError::Rejected(
						"replication provider endpoint exclusion is inconsistent".into(),
					));
				}
			}
			let effective_key_valid =
				match (provider.active_service_key, provider.active_service_key_version) {
					(Some(key), Some(version)) if key != [0; 32] && version != 0 => true,
					(Some(_), Some(_)) => {
						return Err(ChainError::Rejected(
							"replication provider effective service key is invalid".into(),
						));
					},
					(None, None) => false,
					_ => {
						return Err(ChainError::Rejected(
							"replication provider service-key evidence is incomplete".into(),
						));
					},
				};
			let invalid_service_key =
				provider.exclusions.contains(&ReplicationProviderExclusion::InvalidServiceKey);
			if provider.record_present {
				if invalid_service_key == effective_key_valid {
					return Err(ChainError::Rejected(
						"replication provider service-key exclusion is inconsistent".into(),
					));
				}
			} else if effective_key_valid || invalid_service_key {
				return Err(ChainError::Rejected(
					"missing replication provider has service-key evidence".into(),
				));
			}
			if provider.usable != provider.exclusions.is_empty() {
				return Err(ChainError::Rejected(
					"replication provider usability evidence is inconsistent".into(),
				));
			}
			if provider.provider == local_provider {
				if provider
					.exclusions
					.iter()
					.copied()
					.any(ReplicationProviderExclusion::prevents_local_participation)
				{
					return Err(ChainError::Rejected(
						"local replication provider is excluded from participation".into(),
					));
				}
				if provider.active_service_key != Some(local_service_key) {
					return Err(ChainError::Rejected(
						"local replication service key does not match finalized provider state"
							.into(),
					));
				}
			}
		}
		if self
			.current_checkpoint
			.as_ref()
			.is_some_and(|checkpoint| checkpoint.bucket_id.as_bytes() != &self.bucket_id)
		{
			return Err(ChainError::Rejected(
				"replication checkpoint is bound to another bucket".into(),
			));
		}
		if self.snapshot_hash != self.calculated_hash() {
			return Err(ChainError::Rejected("replication topology hash mismatch".into()));
		}
		Ok(())
	}
}

#[allow(dead_code)]
fn replication_members(
	bucket: &ControlBucketInfo<AccountId32, H256, u32>,
	local_provider: &AccountId32,
) -> Result<Vec<AccountId32>, ChainError> {
	let mut members = Vec::with_capacity(bucket.replicas.len().saturating_add(1));
	members.push(bucket.primary.clone());
	members.extend(bucket.replicas.iter().cloned());
	let mut unique = std::collections::BTreeSet::new();
	if members.iter().any(|member| !unique.insert(account_bytes(member))) {
		return Err(ChainError::Rejected(
			"replication control bucket contains duplicate providers".into(),
		));
	}
	if !members.contains(local_provider) {
		return Err(ChainError::Rejected(
			"local provider is not a member of the replication bucket".into(),
		));
	}
	Ok(members)
}

#[allow(dead_code)]
fn replication_provider(
	provider: &AccountId32,
	index: usize,
	info: Option<ProviderInfo<H256, u32>>,
	governed_finalized_checkpoint: Option<u32>,
	eligible: bool,
	confirmed_checkpoint: Option<u32>,
) -> Result<ReplicationProviderSnapshot, ChainError> {
	let order = u8::try_from(index)
		.map_err(|_| ChainError::Rejected("replication provider order overflow".into()))?;
	let Some(info) = info else {
		return Ok(ReplicationProviderSnapshot {
			provider: account_bytes(provider),
			order,
			primary: index == 0,
			record_present: false,
			endpoint: None,
			endpoint_hash: None,
			active_service_key: None,
			active_service_key_version: None,
			status_active: false,
			organization_valid: false,
			authority_validated_at: None,
			overdue_challenges: 0,
			eligible,
			usable: false,
			exclusions: vec![ReplicationProviderExclusion::MissingProvider],
			confirmed_checkpoint,
		});
	};
	let status_active = info.status == ProviderStatus::Active;
	let organization_valid = governed_finalized_checkpoint.is_some_and(|checkpoint| {
		info.organization.valid_from <= checkpoint && checkpoint < info.organization.valid_until
	});
	let authority_validated = governed_finalized_checkpoint
		.is_some_and(|checkpoint| info.authority_validated_at == Some(checkpoint));
	let endpoint_valid = validate_provider_endpoint(&info.endpoint).is_ok();
	let service_key = governed_finalized_checkpoint
		.ok_or_else(|| ChainError::Rejected("governed checkpoint unavailable".into()))
		.and_then(|checkpoint| active_service_key(&info, checkpoint));
	let mut exclusions = Vec::new();
	if !status_active {
		exclusions.push(ReplicationProviderExclusion::Inactive);
	}
	if governed_finalized_checkpoint.is_none() {
		exclusions.push(ReplicationProviderExclusion::GovernedCheckpointUnavailable);
	}
	if !organization_valid {
		exclusions.push(ReplicationProviderExclusion::OrgInvalid);
	}
	if !authority_validated {
		exclusions.push(ReplicationProviderExclusion::AuthorityUnvalidated);
	}
	if info.overdue_challenges != 0 {
		exclusions.push(ReplicationProviderExclusion::OverdueChallenge);
	}
	if !eligible {
		exclusions.push(ReplicationProviderExclusion::RuntimeIneligible);
	}
	if !endpoint_valid {
		exclusions.push(ReplicationProviderExclusion::InvalidEndpoint);
	}
	if service_key.is_err() {
		exclusions.push(ReplicationProviderExclusion::InvalidServiceKey);
	}
	let (active_service_key, active_service_key_version) = service_key
		.map(|(key, version)| (Some(key), Some(version)))
		.unwrap_or((None, None));
	Ok(ReplicationProviderSnapshot {
		provider: account_bytes(provider),
		order,
		primary: index == 0,
		record_present: true,
		endpoint_hash: Some(blake2_256(&info.endpoint)),
		endpoint: Some(info.endpoint),
		active_service_key,
		active_service_key_version,
		status_active,
		organization_valid,
		authority_validated_at: info.authority_validated_at,
		overdue_challenges: info.overdue_challenges,
		eligible,
		usable: exclusions.is_empty(),
		exclusions,
		confirmed_checkpoint,
	})
}

#[allow(dead_code)]
fn apply_confirmation_evidence(
	providers: &mut [ReplicationProviderSnapshot],
	checkpoint: &Option<CheckpointInfo<AccountId32, H256, u32>>,
	governed_finalized_checkpoint: Option<u32>,
	replicas: &[AccountId32],
) -> Result<(), ChainError> {
	let confirmations = if let Some(checkpoint) = checkpoint {
		let mut confirmations = std::collections::BTreeSet::new();
		for provider in &checkpoint.replica_confirmations {
			let provider = account_bytes(provider);
			if !replicas.iter().any(|replica| account_bytes(replica) == provider)
				|| !confirmations.insert(provider)
			{
				return Err(ChainError::Rejected(
					"replication checkpoint confirmations do not match bucket replicas".into(),
				));
			}
		}
		Some((checkpoint.checkpoint_block, confirmations))
	} else {
		None
	};
	for provider in providers {
		let invalid = provider.confirmed_checkpoint.is_some_and(|confirmed| {
			governed_finalized_checkpoint.is_none_or(|governed| confirmed > governed)
		}) || match &confirmations {
			Some((checkpoint, confirmations)) => {
				provider.confirmed_checkpoint.is_some_and(|confirmed| confirmed > *checkpoint)
					|| if provider.primary {
						provider.confirmed_checkpoint != Some(*checkpoint)
					} else {
						confirmations.contains(&provider.provider)
							!= (provider.confirmed_checkpoint == Some(*checkpoint))
					}
			},
			None => provider.confirmed_checkpoint.is_some(),
		};
		if invalid {
			provider.exclusions.push(ReplicationProviderExclusion::ConfirmationInvalid);
		}
		provider.usable = provider.exclusions.is_empty();
	}
	Ok(())
}

#[allow(dead_code)]
fn active_service_key(
	info: &ProviderInfo<H256, u32>,
	finalized_number: u32,
) -> Result<([u8; 32], u64), ChainError> {
	let key = &info.service_key;
	let selected = match (key.pending, key.pending_version, key.pending_effective_at) {
		(Some(pending), Some(version), Some(effective_at)) => {
			if version <= key.active_version {
				return Err(ChainError::Rejected(
					"replication provider service-key version did not advance".into(),
				));
			}
			if effective_at <= finalized_number {
				Ok((pending, version))
			} else {
				Ok((key.active, key.active_version))
			}
		},
		(None, None, None) => Ok((key.active, key.active_version)),
		_ => Err(ChainError::Rejected(
			"replication provider has an incomplete service-key rotation".into(),
		)),
	}?;
	if selected.1 == 0 || selected.0 == [0; 32] {
		return Err(ChainError::Rejected(
			"replication provider has an invalid active service key".into(),
		));
	}
	Ok(selected)
}

#[allow(dead_code)]
fn validate_provider_endpoint(endpoint: &[u8]) -> Result<(), ChainError> {
	let endpoint = std::str::from_utf8(endpoint)
		.map_err(|_| ChainError::Rejected("replication provider endpoint is not UTF-8".into()))?;
	if endpoint.len() > 256 || endpoint.bytes().any(|byte| byte.is_ascii_control() || byte == b' ')
	{
		return Err(ChainError::Rejected("replication provider endpoint is malformed".into()));
	}
	let authority = endpoint
		.strip_prefix("https://")
		.or_else(|| endpoint.strip_prefix("http://"))
		.ok_or_else(|| {
			ChainError::Rejected("replication provider endpoint must use HTTP(S)".into())
		})?
		.split(['/', '?', '#'])
		.next()
		.unwrap_or_default();
	if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
		return Err(ChainError::Rejected(
			"replication provider endpoint has no valid authority".into(),
		));
	}
	Ok(())
}

#[allow(dead_code)]
fn account_bytes(account: &AccountId32) -> [u8; 32] {
	let bytes: &[u8] = account.as_ref();
	bytes.try_into().expect("AccountId32 always contains exactly 32 bytes")
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
	use std::{collections::BTreeMap, convert::Infallible, sync::Arc};

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

	#[derive(Clone)]
	struct TopologyFixture {
		response_version: u16,
		governed_finalized_checkpoint: Option<u32>,
		bucket: ControlBucketInfo<AccountId32, H256, u32>,
		providers: BTreeMap<[u8; 32], ProviderInfo<H256, u32>>,
		eligibility: BTreeMap<[u8; 32], bool>,
		checkpoint: Option<CheckpointInfo<AccountId32, H256, u32>>,
		replica_checkpoints: BTreeMap<[u8; 32], Option<u32>>,
	}

	fn provider_info(seed: u8) -> ProviderInfo<H256, u32> {
		ProviderInfo {
			endpoint: format!("https://provider-{seed}.invalid/storage").into_bytes(),
			organization: OrganizationInfo {
				entity_id: vec![seed],
				attestation_id: H256::repeat_byte(seed),
				schema_id: H256::repeat_byte(seed.saturating_add(1)),
				sla_commitment: H256::repeat_byte(seed.saturating_add(2)),
				sla_version: 1,
				valid_from: 1,
				valid_until: 1_000,
				rotation_predecessor: None,
			},
			service_key: ServiceKeyInfo {
				active: [seed.saturating_add(10); 32],
				active_version: 1,
				previous: None,
				pending: None,
				pending_version: None,
				pending_effective_at: None,
			},
			capacity_bytes: 1_000_000,
			allocated_bytes: 10,
			pending_bytes: 0,
			status: ProviderStatus::Active,
			last_heartbeat: 109,
			overdue_challenges: 0,
			authority_validated_at: Some(100),
		}
	}

	fn topology_fixture() -> TopologyFixture {
		let bucket_id = H256::repeat_byte(5);
		let primary = AccountId32::new([7; 32]);
		let local_replica = AccountId32::new([8; 32]);
		let other_replica = AccountId32::new([9; 32]);
		let mut providers = BTreeMap::new();
		providers.insert([7; 32], provider_info(7));
		let mut local = provider_info(8);
		local.service_key.pending = Some([9; 32]);
		local.service_key.pending_version = Some(2);
		local.service_key.pending_effective_at = Some(105);
		providers.insert([8; 32], local);
		providers.insert([9; 32], provider_info(9));
		TopologyFixture {
			response_version: RESPONSE_VERSION,
			governed_finalized_checkpoint: Some(100),
			bucket: ControlBucketInfo {
				bucket_id,
				owner: AccountId32::new([1; 32]),
				version: 4,
				policy: H256::repeat_byte(6),
				primary: primary.clone(),
				replicas: vec![local_replica.clone(), other_replica.clone()],
				grants: vec![],
				created_at: 1,
			},
			providers,
			eligibility: [([7; 32], true), ([8; 32], true), ([9; 32], true)].into_iter().collect(),
			checkpoint: Some(CheckpointInfo {
				bucket_id,
				commitment: orbis_storage_runtime_api::CommitmentInfo {
					mmr_root: H256::repeat_byte(11),
					start_seq: 0,
					leaf_count: 3,
				},
				checkpoint_block: 100,
				primary_signers: 1,
				commitment_nonce: 100,
				replica_confirmations: vec![local_replica],
			}),
			replica_checkpoints: [([8; 32], Some(100)), ([9; 32], Some(99))].into_iter().collect(),
		}
	}

	fn topology_runtime_response(method: &str, params: &[u8], fixture: &TopologyFixture) -> String {
		let encoded = match method {
			"StorageProviderApi_governed_finalized_checkpoint" => {
				fixture.governed_finalized_checkpoint.encode()
			},
			"StorageProviderApi_control_bucket" => {
				Versioned { version: fixture.response_version, value: Some(fixture.bucket.clone()) }
					.encode()
			},
			"StorageProviderApi_checkpoint" => {
				Versioned { version: fixture.response_version, value: fixture.checkpoint.clone() }
					.encode()
			},
			"StorageProviderApi_provider" => {
				let provider = AccountId32::decode(&mut &params[..]).unwrap();
				Versioned {
					version: fixture.response_version,
					value: fixture.providers.get(&account_bytes(&provider)).cloned(),
				}
				.encode()
			},
			"StorageProviderApi_provider_is_eligible" => {
				let provider = AccountId32::decode(&mut &params[..]).unwrap();
				fixture
					.eligibility
					.get(&account_bytes(&provider))
					.copied()
					.unwrap_or(false)
					.encode()
			},
			"StorageProviderApi_replica_checkpoint" => {
				let (_, provider) = <(H256, AccountId32)>::decode(&mut &params[..]).unwrap();
				fixture
					.replica_checkpoints
					.get(&account_bytes(&provider))
					.copied()
					.flatten()
					.encode()
			},
			other => panic!("unexpected topology runtime API method: {other}"),
		};
		format!("0x{}", hex::encode(encoded))
	}

	async fn topology_rpc_response(
		request: Request<hyper::body::Incoming>,
		fixture: Arc<TopologyFixture>,
		reads: Arc<Mutex<Vec<(String, String, Vec<u8>)>>>,
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
				let raw =
					hex::decode(params[1].as_str().unwrap().strip_prefix("0x").unwrap()).unwrap();
				let at = params[2].as_str().unwrap().to_owned();
				reads.lock().await.push((runtime_method.clone(), at, raw.clone()));
				json!(topology_runtime_response(&runtime_method, &raw, &fixture))
			},
			other => panic!("unexpected topology JSON-RPC method: {other}"),
		};
		let body = serde_json::to_vec(&json!({
			"jsonrpc": "2.0",
			"id": request["id"],
			"result": result,
		}))
		.unwrap();
		Ok(Response::new(Full::new(Bytes::from(body))))
	}

	async fn topology_authority(
		fixture: TopologyFixture,
	) -> (
		FinalizedRuntimeAuthority,
		Arc<Mutex<Vec<(String, String, Vec<u8>)>>>,
		tokio::task::JoinHandle<()>,
	) {
		let local_service_key = active_service_key(
			fixture.providers.get(&[8; 32]).expect("local provider fixture exists"),
			fixture
				.governed_finalized_checkpoint
				.expect("governed fixture checkpoint exists"),
		)
		.unwrap()
		.0;
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let reads = Arc::new(Mutex::new(Vec::new()));
		let server_reads = reads.clone();
		let fixture = Arc::new(fixture);
		let server = tokio::spawn(async move {
			loop {
				let (stream, _) = listener.accept().await.unwrap();
				let reads = server_reads.clone();
				let fixture = fixture.clone();
				tokio::spawn(async move {
					http1::Builder::new()
						.serve_connection(
							TokioIo::new(stream),
							service_fn(move |request| {
								topology_rpc_response(request, fixture.clone(), reads.clone())
							}),
						)
						.await
						.unwrap();
				});
			}
		});
		let authority = FinalizedRuntimeAuthority::connect(
			&format!("http://{address}"),
			AccountId32::new([8; 32]),
			local_service_key,
		)
		.unwrap();
		(authority, reads, server)
	}

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

	#[tokio::test]
	async fn replication_topology_pins_one_hash_and_preserves_member_order_and_rotation() {
		let (authority, reads, server) = topology_authority(topology_fixture()).await;
		let first = authority.replication_topology([5; 32]).await.unwrap();
		let second = authority.replication_topology([5; 32]).await.unwrap();

		assert_eq!(first, second);
		assert_eq!(first.genesis_hash, [0xbb; 32]);
		assert_eq!(first.finalized_hash, [0xaa; 32]);
		assert_eq!(first.finalized_number, 110);
		assert_eq!(first.governed_finalized_checkpoint, Some(100));
		assert_eq!(first.bucket_version, 4);
		assert_eq!(first.primary, [7; 32]);
		assert_eq!(first.replicas, vec![[8; 32], [9; 32]]);
		assert_eq!(
			first.providers.iter().map(|provider| provider.provider).collect::<Vec<_>>(),
			vec![[7; 32], [8; 32], [9; 32]]
		);
		assert_eq!(first.providers[1].active_service_key, Some([18; 32]));
		assert_eq!(first.providers[1].active_service_key_version, Some(1));
		assert_eq!(first.providers[1].confirmed_checkpoint, Some(100));
		assert_eq!(first.providers[2].confirmed_checkpoint, Some(99));
		for provider in &first.providers {
			assert_eq!(provider.endpoint_hash, provider.endpoint.as_deref().map(blake2_256));
			assert!(provider.usable);
			assert!(provider.exclusions.is_empty());
		}
		assert_eq!(first.snapshot_hash, first.calculated_hash());

		let reads = reads.lock().await.clone();
		assert!(reads.iter().all(|(_, at, _)| at == FINALIZED_HASH));
		let methods = reads[..11].iter().map(|(method, _, _)| method.as_str()).collect::<Vec<_>>();
		assert_eq!(
			methods,
			[
				"StorageProviderApi_governed_finalized_checkpoint",
				"StorageProviderApi_control_bucket",
				"StorageProviderApi_checkpoint",
				"StorageProviderApi_provider",
				"StorageProviderApi_provider_is_eligible",
				"StorageProviderApi_provider",
				"StorageProviderApi_provider_is_eligible",
				"StorageProviderApi_replica_checkpoint",
				"StorageProviderApi_provider",
				"StorageProviderApi_provider_is_eligible",
				"StorageProviderApi_replica_checkpoint",
			]
		);
		let provider_order = reads[..11]
			.iter()
			.filter(|(method, _, _)| method == "StorageProviderApi_provider")
			.map(|(_, _, params)| account_bytes(&AccountId32::decode(&mut &params[..]).unwrap()))
			.collect::<Vec<_>>();
		assert_eq!(provider_order, vec![[7; 32], [8; 32], [9; 32]]);
		server.abort();
	}

	#[tokio::test]
	async fn replication_topology_preserves_missing_and_ineligible_remote_members() {
		let mut missing = topology_fixture();
		missing.providers.remove(&[9; 32]);
		let (authority, _reads, server) = topology_authority(missing).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(snapshot.providers.len(), 3);
		assert_eq!(snapshot.providers[2].provider, [9; 32]);
		assert!(!snapshot.providers[2].record_present);
		assert_eq!(snapshot.providers[2].endpoint, None);
		assert_eq!(snapshot.providers[2].active_service_key, None);
		assert_eq!(
			snapshot.providers[2].exclusions,
			vec![ReplicationProviderExclusion::MissingProvider]
		);
		assert!(!snapshot.providers[2].usable);
		assert!(snapshot.providers[1].usable);
		server.abort();

		let mut unavailable = topology_fixture();
		unavailable.providers.get_mut(&[7; 32]).unwrap().status = ProviderStatus::Suspended;
		unavailable.providers.get_mut(&[7; 32]).unwrap().endpoint = b"corrupt".to_vec();
		unavailable.eligibility.insert([7; 32], false);
		let (authority, _reads, server) = topology_authority(unavailable).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(snapshot.providers[0].provider, [7; 32]);
		assert_eq!(
			snapshot.providers[0].exclusions,
			vec![
				ReplicationProviderExclusion::Inactive,
				ReplicationProviderExclusion::RuntimeIneligible,
				ReplicationProviderExclusion::InvalidEndpoint,
			]
		);
		assert!(!snapshot.providers[0].usable);
		assert!(snapshot.providers[1].usable);
		assert!(snapshot.providers[2].usable);
		assert_eq!(snapshot.snapshot_hash, snapshot.calculated_hash());
		server.abort();

		let mut unavailable = topology_fixture();
		unavailable.eligibility.insert([9; 32], false);
		unavailable.providers.get_mut(&[9; 32]).unwrap().overdue_challenges = 1;
		unavailable.providers.get_mut(&[9; 32]).unwrap().organization.valid_until = 100;
		unavailable.providers.get_mut(&[9; 32]).unwrap().authority_validated_at = None;
		let (authority, _reads, server) = topology_authority(unavailable).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(
			snapshot.providers[2].exclusions,
			vec![
				ReplicationProviderExclusion::OrgInvalid,
				ReplicationProviderExclusion::AuthorityUnvalidated,
				ReplicationProviderExclusion::OverdueChallenge,
				ReplicationProviderExclusion::RuntimeIneligible,
			]
		);
		server.abort();
	}

	#[tokio::test]
	async fn replication_topology_rejects_excluded_local_participation_authority() {
		let mut inactive = topology_fixture();
		inactive.providers.get_mut(&[8; 32]).unwrap().status = ProviderStatus::Suspended;
		let mut invalid_endpoint = topology_fixture();
		invalid_endpoint.providers.get_mut(&[8; 32]).unwrap().endpoint = b"corrupt".to_vec();
		let mut runtime_ineligible = topology_fixture();
		runtime_ineligible.eligibility.insert([8; 32], false);

		for fixture in [inactive, invalid_endpoint, runtime_ineligible] {
			let (authority, _reads, server) = topology_authority(fixture).await;
			assert!(matches!(
				authority.replication_topology([5; 32]).await,
				Err(ChainError::Rejected(_))
			));
			server.abort();
		}
	}

	#[tokio::test]
	async fn replication_topology_retains_confirmation_invalid_local_as_nonusable() {
		let mut fixture = topology_fixture();
		fixture.replica_checkpoints.insert([8; 32], Some(101));
		let (authority, _reads, server) = topology_authority(fixture).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(
			snapshot.providers[1].exclusions,
			vec![ReplicationProviderExclusion::ConfirmationInvalid]
		);
		assert!(snapshot.providers[1].eligible);
		assert!(!snapshot.providers[1].usable);
		assert_eq!(snapshot.snapshot_hash, snapshot.calculated_hash());
		server.abort();
	}

	#[tokio::test]
	async fn replication_topology_rejects_duplicate_and_local_nonmember() {
		let mut cases = Vec::new();
		let mut duplicate = topology_fixture();
		duplicate.bucket.replicas.push(AccountId32::new([8; 32]));
		cases.push(duplicate);
		let mut nonmember = topology_fixture();
		nonmember.bucket.replicas = vec![AccountId32::new([9; 32])];
		cases.push(nonmember);

		for fixture in cases {
			let (authority, _reads, server) = topology_authority(fixture).await;
			assert!(matches!(
				authority.replication_topology([5; 32]).await,
				Err(ChainError::Rejected(_))
			));
			server.abort();
		}
	}

	#[tokio::test]
	async fn replication_topology_uses_governed_checkpoint_for_service_key_activation() {
		let (authority, _reads, server) = topology_authority(topology_fixture()).await;
		let before = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(before.finalized_number, 110);
		assert_eq!(before.governed_finalized_checkpoint, Some(100));
		assert_eq!(before.providers[1].active_service_key, Some([18; 32]));
		assert_eq!(before.providers[1].active_service_key_version, Some(1));
		server.abort();

		let mut activated = topology_fixture();
		activated.governed_finalized_checkpoint = Some(105);
		for provider in activated.providers.values_mut() {
			provider.authority_validated_at = Some(105);
		}
		let (authority, _reads, server) = topology_authority(activated).await;
		let after = authority.replication_topology([5; 32]).await.unwrap();
		assert_eq!(after.finalized_number, 110);
		assert_eq!(after.governed_finalized_checkpoint, Some(105));
		assert_eq!(after.providers[1].active_service_key, Some([9; 32]));
		assert_eq!(after.providers[1].active_service_key_version, Some(2));
		assert!(after.providers.iter().all(|provider| provider.usable));
		server.abort();
	}

	#[tokio::test]
	async fn replication_topology_rejects_versions_bucket_ids_and_malformed_endpoints() {
		let mut cases = Vec::new();
		let mut wrong_version = topology_fixture();
		wrong_version.response_version = RESPONSE_VERSION + 1;
		cases.push(wrong_version);
		let mut wrong_bucket = topology_fixture();
		wrong_bucket.bucket.bucket_id = H256::repeat_byte(99);
		cases.push(wrong_bucket);
		let mut wrong_checkpoint = topology_fixture();
		wrong_checkpoint.checkpoint.as_mut().unwrap().bucket_id = H256::repeat_byte(99);
		cases.push(wrong_checkpoint);
		for fixture in cases {
			let (authority, _reads, server) = topology_authority(fixture).await;
			assert!(matches!(
				authority.replication_topology([5; 32]).await,
				Err(ChainError::Rejected(_))
			));
			server.abort();
		}
	}

	#[tokio::test]
	async fn replication_snapshot_validation_rejects_endpoint_hash_or_service_key_tampering() {
		let (authority, _reads, server) = topology_authority(topology_fixture()).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();
		let mut endpoint_tamper = snapshot.clone();
		endpoint_tamper.providers[1].endpoint_hash = Some([0; 32]);
		assert!(matches!(endpoint_tamper.validate([8; 32], [9; 32]), Err(ChainError::Rejected(_))));
		let mut key_tamper = snapshot;
		key_tamper.providers[1].active_service_key = Some([77; 32]);
		assert!(matches!(key_tamper.validate([8; 32], [9; 32]), Err(ChainError::Rejected(_))));
		server.abort();
	}

	#[tokio::test]
	async fn replication_snapshot_validation_enforces_service_key_evidence() {
		let (authority, _reads, server) = topology_authority(topology_fixture()).await;
		let snapshot = authority.replication_topology([5; 32]).await.unwrap();

		let mut missing_version = snapshot.clone();
		missing_version.providers[2].active_service_key_version = None;
		assert!(matches!(
			missing_version.validate([8; 32], [18; 32]),
			Err(ChainError::Rejected(_))
		));

		let mut zero_key = snapshot.clone();
		zero_key.providers[2].active_service_key = Some([0; 32]);
		assert!(matches!(zero_key.validate([8; 32], [18; 32]), Err(ChainError::Rejected(_))));

		let mut false_exclusion = snapshot.clone();
		false_exclusion.providers[2]
			.exclusions
			.push(ReplicationProviderExclusion::InvalidServiceKey);
		false_exclusion.providers[2].usable = false;
		assert!(matches!(
			false_exclusion.validate([8; 32], [18; 32]),
			Err(ChainError::Rejected(_))
		));

		let mut invalid_remote = snapshot;
		invalid_remote.providers[2].active_service_key = None;
		invalid_remote.providers[2].active_service_key_version = None;
		invalid_remote.providers[2]
			.exclusions
			.push(ReplicationProviderExclusion::InvalidServiceKey);
		invalid_remote.providers[2].usable = false;
		invalid_remote.snapshot_hash = invalid_remote.calculated_hash();
		invalid_remote.validate([8; 32], [18; 32]).unwrap();
		server.abort();
	}
}

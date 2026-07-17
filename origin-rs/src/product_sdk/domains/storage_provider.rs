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

use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, invalid, AccountId, AgreementId, BlockNumber, BucketId, ChallengeId, ContainerId,
	ContentCommitment, DomainResult, FinalizedQuery, Hash32, PageRequest, ProofCommitment,
	ReservationId, SubmitAndFinalize, Validate,
};

pub const MAX_ENDPOINT_BYTES: usize = 512;
pub const MAX_SERVICE_KEY_BYTES: usize = 128;

pub type StorageProviderRead = FinalizedQuery<StorageProviderQuery>;
pub type StorageProviderWrite = SubmitAndFinalize<StorageProviderCommand>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Endpoint(Vec<u8>);

impl Endpoint {
	pub fn new(value: Vec<u8>) -> DomainResult<Self> {
		ensure_bytes(&value, 1, MAX_ENDPOINT_BYTES, "provider endpoint")?;
		Ok(Self(value))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl Validate for Endpoint {
	fn validate(&self) -> DomainResult<()> {
		ensure_bytes(&self.0, 1, MAX_ENDPOINT_BYTES, "provider endpoint")
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ServiceKey(Vec<u8>);

impl ServiceKey {
	pub fn new(value: Vec<u8>) -> DomainResult<Self> {
		ensure_bytes(&value, 1, MAX_SERVICE_KEY_BYTES, "provider service key")?;
		Ok(Self(value))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl Validate for ServiceKey {
	fn validate(&self) -> DomainResult<()> {
		ensure_bytes(&self.0, 1, MAX_SERVICE_KEY_BYTES, "provider service key")
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
	Active,
	Suspended,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgreementStatus {
	Proposed,
	Active,
	Suspended,
	Cancelled,
	Expired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeStatus {
	Open,
	Proved,
	TimedOut,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderOrganizationView {
	pub entity_id: Vec<u8>,
	pub attestation: Hash32,
	pub schema: Hash32,
	pub sla_commitment: Hash32,
	pub sla_version: u16,
	pub valid_from: BlockNumber,
	pub valid_until: BlockNumber,
	pub rotation_predecessor: Option<Hash32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderServiceKeyView {
	pub active: ServiceKey,
	pub active_version: u64,
	pub previous: Option<ServiceKey>,
	pub pending: Option<ServiceKey>,
	pub pending_version: Option<u64>,
	pub pending_effective_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderView {
	pub provider: AccountId,
	pub endpoint: Endpoint,
	pub organization: ProviderOrganizationView,
	pub service_key: ProviderServiceKeyView,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub overdue_challenges: u32,
	pub authority_validated_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgreementView {
	pub agreement: AgreementId,
	pub owner: AccountId,
	pub bucket: BucketId,
	pub primary: AccountId,
	pub replicas: Vec<AccountId>,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub release_at: Option<BlockNumber>,
	pub state_version: u64,
	pub status: AgreementStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentView {
	pub mmr_root: ProofCommitment,
	pub start_seq: u64,
	pub leaf_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkLocationView {
	pub leaf_index: u64,
	pub chunk_index: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeView {
	pub challenge: ChallengeId,
	pub bucket: BucketId,
	pub provider: AccountId,
	pub expected_commitment: CommitmentView,
	pub location: ChunkLocationView,
	pub due_at: BlockNumber,
	pub status: ChallengeStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointView {
	pub bucket: BucketId,
	pub commitment: CommitmentView,
	pub checkpoint_block: BlockNumber,
	pub primary_signers: u8,
	pub commitment_nonce: BlockNumber,
	pub replica_confirmations: Vec<AccountId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum StorageProviderResponse {
	Provider(super::common::FinalizedValue<ProviderView>),
	Providers(super::common::FinalizedPage<AccountId>),
	Agreement(super::common::FinalizedValue<AgreementView>),
	Agreements(super::common::FinalizedPage<AgreementId>),
	AgreementNonce(super::common::FinalizedValue<u64>),
	Challenge(super::common::FinalizedValue<ChallengeView>),
	Challenges(super::common::FinalizedPage<ChallengeId>),
	CanAcceptCapacity(super::common::FinalizedValue<bool>),
	Checkpoint(super::common::FinalizedValue<CheckpointView>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum StorageProviderQuery {
	ProviderById {
		provider: AccountId,
	},
	Providers {
		page: PageRequest,
	},
	AgreementById {
		agreement: AgreementId,
	},
	ProviderAgreements {
		provider: AccountId,
		page: PageRequest,
	},
	AgreementNonce {
		owner: AccountId,
	},
	ChallengeById {
		challenge: ChallengeId,
	},
	ChallengesAt {
		block: BlockNumber,
		page: PageRequest,
	},
	CanAcceptCapacity {
		provider: AccountId,
		additional_bytes: u64,
	},
	BucketCheckpoint {
		bucket: BucketId,
	},
}

impl Validate for StorageProviderQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::ProviderById { provider } | Self::AgreementNonce { owner: provider } => {
				provider.validate()
			},
			Self::CanAcceptCapacity { provider, additional_bytes } => {
				let _ = additional_bytes;
				provider.validate()
			},
			Self::Providers { page } => page.validate(),
			Self::AgreementById { agreement } => agreement.validate(),
			Self::ProviderAgreements { provider, page } => {
				provider.validate()?;
				page.validate()
			},
			Self::ChallengeById { challenge } => challenge.validate(),
			Self::ChallengesAt { page, .. } => page.validate(),
			Self::BucketCheckpoint { bucket } => bucket.validate(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum StorageProviderCommand {
	RegisterProvider {
		provider: AccountId,
		endpoint: Endpoint,
		service_key: ServiceKey,
		capacity_bytes: u64,
	},
	UpdateProvider {
		provider: AccountId,
		endpoint: Endpoint,
		service_key: ServiceKey,
		capacity_bytes: u64,
	},
	SetProviderStatus {
		provider: AccountId,
		status: ProviderStatus,
	},
	RemoveProvider {
		provider: AccountId,
	},
	Heartbeat,
	ProposeAgreement {
		provider: AccountId,
		container: ContainerId,
		content_commitment: ContentCommitment,
		reservation_ref: Option<ReservationId>,
		bytes: u64,
		expires_at: BlockNumber,
	},
	AcceptAgreement {
		agreement: AgreementId,
	},
	CancelAgreement {
		agreement: AgreementId,
	},
	IssueChallenge {
		agreement: AgreementId,
		expected_commitment: ProofCommitment,
		due_at: BlockNumber,
	},
	TimeoutChallenge {
		challenge: ChallengeId,
	},
	RequestRenewal {
		agreement: AgreementId,
		expires_at: BlockNumber,
	},
	AcceptRenewal {
		agreement: AgreementId,
	},
	ExpireAgreement {
		agreement: AgreementId,
	},
	PruneAgreement {
		agreement: AgreementId,
	},
	AcknowledgeManifestDeletion {
		manifest: ContentCommitment,
		evidence_hash: ProofCommitment,
		service_key: ServiceKey,
		signature: Vec<u8>,
	},
}

impl StorageProviderCommand {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		match self {
			Self::ProposeAgreement { expires_at, .. } | Self::RequestRenewal { expires_at, .. }
				if *expires_at <= current_block =>
			{
				Err(invalid("agreement expiry must be in the future"))
			},
			Self::IssueChallenge { due_at, .. } if *due_at <= current_block => {
				Err(invalid("challenge deadline must be in the future"))
			},
			_ => Ok(()),
		}
	}
}

impl Validate for StorageProviderCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::RegisterProvider { provider, endpoint, service_key, capacity_bytes }
			| Self::UpdateProvider { provider, endpoint, service_key, capacity_bytes } => {
				provider.validate()?;
				endpoint.validate()?;
				service_key.validate()?;
				if *capacity_bytes == 0 {
					return Err(invalid("provider capacity must be non-zero"));
				}
				Ok(())
			},
			Self::SetProviderStatus { provider, .. } | Self::RemoveProvider { provider } => {
				provider.validate()
			},
			Self::Heartbeat => Ok(()),
			Self::ProposeAgreement {
				provider,
				container,
				content_commitment,
				reservation_ref,
				bytes,
				..
			} => {
				provider.validate()?;
				container.validate()?;
				content_commitment.validate()?;
				reservation_ref.as_ref().map_or(Ok(()), Validate::validate)?;
				if *bytes == 0 {
					return Err(invalid("agreement byte capacity must be non-zero"));
				}
				Ok(())
			},
			Self::AcceptAgreement { agreement }
			| Self::CancelAgreement { agreement }
			| Self::AcceptRenewal { agreement }
			| Self::ExpireAgreement { agreement }
			| Self::PruneAgreement { agreement } => agreement.validate(),
			Self::AcknowledgeManifestDeletion {
				manifest,
				evidence_hash,
				service_key,
				signature,
			} => {
				manifest.validate()?;
				evidence_hash.validate()?;
				service_key.validate()?;
				if service_key.as_bytes().len() != 32 || signature.len() != 64 {
					return Err(invalid(
						"manifest deletion service key or signature has the wrong length",
					));
				}
				Ok(())
			},
			Self::IssueChallenge { agreement, expected_commitment, .. } => {
				agreement.validate()?;
				expected_commitment.validate()
			},
			Self::TimeoutChallenge { challenge } => challenge.validate(),
			Self::RequestRenewal { agreement, .. } => agreement.validate(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checkpoint_query_is_bucket_scoped_and_removed_reads_do_not_decode() {
		let bucket = BucketId::new(format!("0x{}", "11".repeat(32))).unwrap();
		let query = StorageProviderQuery::BucketCheckpoint { bucket };
		query.validate().unwrap();
		assert!(serde_json::to_string(&query).unwrap().contains("bucket_checkpoint"));

		for stale in [
			r#"{"query":"owner_agreements","arguments":{}}"#,
			r#"{"query":"container_agreements","arguments":{}}"#,
			r#"{"query":"open_challenge_count","arguments":{}}"#,
			r#"{"query":"provider_root","arguments":{}}"#,
			r#"{"query":"deletion_acknowledgement","arguments":{}}"#,
		] {
			assert!(serde_json::from_str::<StorageProviderQuery>(stale).is_err());
		}
	}
}

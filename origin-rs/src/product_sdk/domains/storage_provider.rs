use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, invalid, AccountId, AgreementId, BlockNumber, ChallengeId, ContainerId,
	ContentCommitment, DomainResult, FinalizedQuery, PageRequest, ProofCommitment,
	ProviderReference, ReservationId, SubmitAndFinalize, Validate,
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
pub struct ProviderView {
	pub provider: AccountId,
	pub endpoint: Endpoint,
	pub service_key: ServiceKey,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub reputation: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgreementView {
	pub agreement: AgreementId,
	pub owner: AccountId,
	pub provider: AccountId,
	pub container: ContainerId,
	pub content_commitment: ContentCommitment,
	pub reservation_ref: Option<ReservationId>,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub pending_expiry: Option<BlockNumber>,
	pub status: AgreementStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeView {
	pub challenge: ChallengeId,
	pub provider: AccountId,
	pub agreement: AgreementId,
	pub expected_commitment: ProofCommitment,
	pub due_at: BlockNumber,
	pub proof_commitment: Option<ProofCommitment>,
	pub status: ChallengeStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointView {
	pub challenge: ChallengeId,
	pub proof_commitment: ProofCommitment,
	pub recorded_at: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionAcknowledgementView {
	pub content_commitment: ContentCommitment,
	pub tombstone_root: ProofCommitment,
	pub proof_commitment: ProofCommitment,
	pub acknowledged_at: BlockNumber,
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
	OpenChallengeCount(super::common::FinalizedValue<u32>),
	CanAcceptCapacity(super::common::FinalizedValue<bool>),
	Checkpoint(super::common::FinalizedValue<CheckpointView>),
	DeletionAcknowledgement(super::common::FinalizedValue<DeletionAcknowledgementView>),
	ResourceProviderRef(super::common::FinalizedValue<ProviderReference>),
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
	OwnerAgreements {
		owner: AccountId,
		page: PageRequest,
	},
	ContainerAgreements {
		container: ContainerId,
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
	OpenChallengeCount {
		agreement: AgreementId,
	},
	CanAcceptCapacity {
		provider: AccountId,
		additional_bytes: u64,
	},
	ProviderCheckpoint {
		provider: AccountId,
	},
	DeletionAcknowledgement {
		agreement: AgreementId,
	},
	/// Exact TransactionStorage `resource_provider_ref(reservation_id)` runtime API query.
	ResourceProviderRef {
		reservation_id: ReservationId,
	},
}

impl Validate for StorageProviderQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::ProviderById { provider }
			| Self::AgreementNonce { owner: provider }
			| Self::ProviderCheckpoint { provider } => provider.validate(),
			Self::CanAcceptCapacity { provider, additional_bytes } => {
				let _ = additional_bytes;
				provider.validate()
			},
			Self::Providers { page } => page.validate(),
			Self::AgreementById { agreement }
			| Self::OpenChallengeCount { agreement }
			| Self::DeletionAcknowledgement { agreement } => agreement.validate(),
			Self::ProviderAgreements { provider, page } => {
				provider.validate()?;
				page.validate()
			},
			Self::OwnerAgreements { owner, page } => {
				owner.validate()?;
				page.validate()
			},
			Self::ContainerAgreements { container, page } => {
				container.validate()?;
				page.validate()
			},
			Self::ChallengeById { challenge } => challenge.validate(),
			Self::ChallengesAt { page, .. } => page.validate(),
			Self::ResourceProviderRef { reservation_id } => reservation_id.validate(),
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
	SubmitCheckpoint {
		challenge: ChallengeId,
		proof_commitment: ProofCommitment,
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
	AcknowledgeDeletion {
		agreement: AgreementId,
		content_commitment: ContentCommitment,
		tombstone_root: ProofCommitment,
		proof_commitment: ProofCommitment,
	},
	/// Exact TransactionStorage `attach_provider(reservation_id, provider_ref)` call.
	AttachProvider {
		reservation_id: ReservationId,
		provider_ref: ProviderReference,
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
			Self::AcknowledgeDeletion {
				agreement,
				content_commitment,
				tombstone_root,
				proof_commitment,
			} => {
				agreement.validate()?;
				content_commitment.validate()?;
				tombstone_root.validate()?;
				proof_commitment.validate()
			},
			Self::IssueChallenge { agreement, expected_commitment, .. } => {
				agreement.validate()?;
				expected_commitment.validate()
			},
			Self::SubmitCheckpoint { challenge, proof_commitment } => {
				challenge.validate()?;
				proof_commitment.validate()
			},
			Self::TimeoutChallenge { challenge } => challenge.validate(),
			Self::RequestRenewal { agreement, .. } => agreement.validate(),
			Self::AttachProvider { reservation_id, provider_ref } => {
				reservation_id.validate()?;
				provider_ref.validate()
			},
		}
	}
}

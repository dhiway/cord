use std::collections::HashSet;

use codec::Encode;
use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, expired, invalid, AccountId, AttestationId, BlockNumber, DomainResult,
	FinalizedPage, FinalizedQuery, FinalizedValue, Hash32, PageRequest, PayloadCommitment,
	SchemaId, StatusCommitment, SubjectCommitment, SubmitAndFinalize, UniquenessCommitment,
	Validate,
};

pub const MAX_SCHEMA_DEFINITION_BYTES: usize = 16 * 1024;
pub const MAX_AUTHORIZED_ISSUERS: usize = 64;
pub const MAX_BATCH_SIZE: usize = 64;
pub const DELEGATED_INTENT_DOMAIN: &str = "cord:orbis:delegated-attestation:v1";
pub const DELEGATED_REVOKE_INTENT_DOMAIN: &str = "cord:orbis:delegated-revocation:v1";
pub const EXTERNAL_STATUS_DOMAIN: &str = "cord:orbis:external-status:v1";
pub const DELEGATED_INTENT_FIELD_ORDER: &[&str] = &[
	"genesis_hash",
	"spec_version",
	"action",
	"issuer",
	"delegate",
	"schema",
	"subject_commitment",
	"payload_commitment",
	"status_commitment",
	"parent",
	"expiry",
	"uniqueness_commitment",
	"revocable",
	"nonce",
	"deadline",
];
pub const DELEGATED_REVOKE_INTENT_FIELD_ORDER: &[&str] = &[
	"genesis_hash",
	"spec_version",
	"action",
	"revoker",
	"delegate",
	"attestation",
	"nonce",
	"deadline",
];

#[derive(Encode)]
enum CanonicalDelegatedAction {
	#[codec(index = 0)]
	Issue,
	#[codec(index = 1)]
	Revoke,
}

#[derive(Encode)]
struct CanonicalDelegatedIssueIntent {
	genesis_hash: [u8; 32],
	spec_version: u32,
	action: CanonicalDelegatedAction,
	issuer: sp_runtime::AccountId32,
	delegate: sp_runtime::AccountId32,
	schema: [u8; 32],
	subject_commitment: [u8; 32],
	payload_commitment: [u8; 32],
	status_commitment: [u8; 32],
	parent: Option<[u8; 32]>,
	expiry: Option<BlockNumber>,
	uniqueness_commitment: Option<[u8; 32]>,
	revocable: bool,
	nonce: u64,
	deadline: BlockNumber,
}

#[derive(Encode)]
struct CanonicalDelegatedRevokeIntent {
	genesis_hash: [u8; 32],
	spec_version: u32,
	action: CanonicalDelegatedAction,
	revoker: sp_runtime::AccountId32,
	delegate: sp_runtime::AccountId32,
	attestation: [u8; 32],
	nonce: u64,
	deadline: BlockNumber,
}

/// Produce the exact runtime SCALE signing payload for a typed delegated issuance intent.
pub fn delegated_issue_signing_payload(intent: &DelegatedIntent) -> DomainResult<Vec<u8>> {
	intent.validate()?;
	let canonical = CanonicalDelegatedIssueIntent {
		genesis_hash: hash_bytes(&intent.genesis_hash)?,
		spec_version: intent.spec_version,
		action: CanonicalDelegatedAction::Issue,
		issuer: canonical_account(&intent.issuer)?,
		delegate: canonical_account(&intent.delegate)?,
		schema: hash_bytes(intent.schema.as_hash())?,
		subject_commitment: hash_bytes(intent.subject_commitment.as_hash())?,
		payload_commitment: hash_bytes(intent.payload_commitment.as_hash())?,
		status_commitment: hash_bytes(intent.status_commitment.as_hash())?,
		parent: intent.parent.as_ref().map(|value| hash_bytes(value.as_hash())).transpose()?,
		expiry: intent.expiry,
		uniqueness_commitment: intent
			.uniqueness_commitment
			.as_ref()
			.map(|value| hash_bytes(value.as_hash()))
			.transpose()?,
		revocable: intent.revocable,
		nonce: intent.nonce,
		deadline: intent.deadline,
	};
	Ok((DELEGATED_INTENT_DOMAIN.as_bytes(), canonical).encode())
}

/// Produce the exact runtime SCALE signing payload for a typed delegated revocation intent.
pub fn delegated_revoke_signing_payload(intent: &DelegatedRevokeIntent) -> DomainResult<Vec<u8>> {
	intent.validate()?;
	let canonical = CanonicalDelegatedRevokeIntent {
		genesis_hash: hash_bytes(&intent.genesis_hash)?,
		spec_version: intent.spec_version,
		action: CanonicalDelegatedAction::Revoke,
		revoker: canonical_account(&intent.revoker)?,
		delegate: canonical_account(&intent.delegate)?,
		attestation: hash_bytes(intent.attestation.as_hash())?,
		nonce: intent.nonce,
		deadline: intent.deadline,
	};
	Ok((DELEGATED_REVOKE_INTENT_DOMAIN.as_bytes(), canonical).encode())
}

fn canonical_account(account: &AccountId) -> DomainResult<sp_runtime::AccountId32> {
	crate::types::account::ss58_to_account_id(account.as_str())
		.map_err(|error| invalid(format!("invalid delegated intent account: {error}")))
}

fn hash_bytes(hash: &Hash32) -> DomainResult<[u8; 32]> {
	hash.validate()?;
	let bytes = hex::decode(&hash.as_str()[2..]).map_err(|_| invalid("invalid hash hex"))?;
	bytes.try_into().map_err(|_| invalid("invalid hash length"))
}

pub type AttestationRead = FinalizedQuery<AttestationQuery>;
pub type AttestationWrite = SubmitAndFinalize<AttestationCommand>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaStatus {
	Active,
	Paused,
	Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexPolicy {
	None,
	Issuer,
	SubjectAndSchema,
	IssuerAndSubjectSchema,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaView {
	pub schema: SchemaId,
	pub creator: AccountId,
	pub definition: Vec<u8>,
	pub definition_commitment: Hash32,
	pub status: SchemaStatus,
	pub revocable: bool,
	pub unique: bool,
	pub index_policy: IndexPolicy,
	pub authorized_issuers: Vec<AccountId>,
	pub created_at: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationView {
	pub attestation: AttestationId,
	pub issuer: AccountId,
	pub schema: SchemaId,
	pub subject_commitment: SubjectCommitment,
	pub payload_commitment: PayloadCommitment,
	pub status_commitment: StatusCommitment,
	pub parent: Option<AttestationId>,
	pub expiry: Option<BlockNumber>,
	pub uniqueness_commitment: Option<UniquenessCommitment>,
	pub revocable: bool,
	pub issuance_nonce: u64,
	pub issued_at: BlockNumber,
	pub revoked_at: Option<BlockNumber>,
	pub revoked_by: Option<AccountId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveStatus {
	pub exists: bool,
	pub live: bool,
	pub evaluated_at: BlockNumber,
	pub expiry: Option<BlockNumber>,
	pub revoked_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalStatusView {
	pub key: Hash32,
	pub issuer: AccountId,
	pub status_commitment: StatusCommitment,
	pub revoked_at: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum AttestationResponse {
	Schema(FinalizedValue<SchemaView>),
	Attestation(FinalizedValue<AttestationView>),
	LiveStatus(FinalizedValue<LiveStatus>),
	Schemas(FinalizedPage<SchemaId>),
	Attestations(FinalizedPage<AttestationId>),
	NextDelegatedNonce(FinalizedValue<u64>),
	SchemaCount(FinalizedValue<u64>),
	AttestationCount(FinalizedValue<u64>),
	NextIssuanceNonce(FinalizedValue<u64>),
	ExternalStatus(FinalizedValue<ExternalStatusView>),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationEventKind {
	SchemaCreated,
	SchemaStatusChanged,
	AttestationIssued,
	DelegatedIntentConsumed,
	DelegatedRevocationConsumed,
	AttestationRevoked,
	ExternalStatusRevoked,
	EmergencyPauseChanged,
}

/// Typed decoding target for the native pallet's finalized events.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum AttestationEvent {
	SchemaCreated {
		schema: SchemaId,
		creator: AccountId,
		definition_commitment: Hash32,
		revocable: bool,
		unique: bool,
		index_policy: IndexPolicy,
	},
	SchemaStatusChanged {
		schema: SchemaId,
		status: SchemaStatus,
		forced: bool,
	},
	AttestationIssued {
		attestation: AttestationId,
		schema: SchemaId,
		issuer: AccountId,
		subject_commitment: SubjectCommitment,
	},
	DelegatedIntentConsumed {
		issuer: AccountId,
		delegate: AccountId,
		nonce: u64,
		attestation: AttestationId,
	},
	DelegatedRevocationConsumed {
		revoker: AccountId,
		delegate: AccountId,
		nonce: u64,
		attestation: AttestationId,
	},
	AttestationRevoked {
		attestation: AttestationId,
		by: Option<AccountId>,
		forced: bool,
	},
	ExternalStatusRevoked {
		key: Hash32,
		issuer: AccountId,
		status_commitment: StatusCommitment,
		revoked_at: BlockNumber,
	},
	EmergencyPauseChanged {
		paused: bool,
	},
}

/// Stable application outcome derived from one native attestation event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", content = "data", rename_all = "snake_case")]
pub enum AttestationOutcome {
	SchemaAvailable { schema: SchemaId },
	SchemaStatusChanged { schema: SchemaId, status: SchemaStatus },
	AttestationAvailable { attestation: AttestationId },
	DelegationConsumed { account: AccountId, nonce: u64, attestation: AttestationId },
	AttestationRevoked { attestation: AttestationId },
	ExternalStatusRevoked { key: Hash32 },
	EmergencyPauseChanged { paused: bool },
}

impl AttestationEvent {
	pub const fn kind(&self) -> AttestationEventKind {
		match self {
			Self::SchemaCreated { .. } => AttestationEventKind::SchemaCreated,
			Self::SchemaStatusChanged { .. } => AttestationEventKind::SchemaStatusChanged,
			Self::AttestationIssued { .. } => AttestationEventKind::AttestationIssued,
			Self::DelegatedIntentConsumed { .. } => AttestationEventKind::DelegatedIntentConsumed,
			Self::DelegatedRevocationConsumed { .. } => {
				AttestationEventKind::DelegatedRevocationConsumed
			},
			Self::AttestationRevoked { .. } => AttestationEventKind::AttestationRevoked,
			Self::ExternalStatusRevoked { .. } => AttestationEventKind::ExternalStatusRevoked,
			Self::EmergencyPauseChanged { .. } => AttestationEventKind::EmergencyPauseChanged,
		}
	}

	pub fn outcome(&self) -> AttestationOutcome {
		match self {
			Self::SchemaCreated { schema, .. } => {
				AttestationOutcome::SchemaAvailable { schema: schema.clone() }
			},
			Self::SchemaStatusChanged { schema, status, .. } => {
				AttestationOutcome::SchemaStatusChanged { schema: schema.clone(), status: *status }
			},
			Self::AttestationIssued { attestation, .. } => {
				AttestationOutcome::AttestationAvailable { attestation: attestation.clone() }
			},
			Self::DelegatedIntentConsumed { issuer, nonce, attestation, .. } => {
				AttestationOutcome::DelegationConsumed {
					account: issuer.clone(),
					nonce: *nonce,
					attestation: attestation.clone(),
				}
			},
			Self::DelegatedRevocationConsumed { revoker, nonce, attestation, .. } => {
				AttestationOutcome::DelegationConsumed {
					account: revoker.clone(),
					nonce: *nonce,
					attestation: attestation.clone(),
				}
			},
			Self::AttestationRevoked { attestation, .. } => {
				AttestationOutcome::AttestationRevoked { attestation: attestation.clone() }
			},
			Self::ExternalStatusRevoked { key, .. } => {
				AttestationOutcome::ExternalStatusRevoked { key: key.clone() }
			},
			Self::EmergencyPauseChanged { paused } => {
				AttestationOutcome::EmergencyPauseChanged { paused: *paused }
			},
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedAttestationEvent {
	pub finalized_block_hash: Hash32,
	pub event_index: u32,
	pub event: AttestationEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedAttestationOutcome {
	pub event: FinalizedAttestationEvent,
	pub outcome: AttestationOutcome,
}

/// Attestation-specific finalized subscription contract; transport adapters decode only these
/// pallet events and feed them through [`AttestationEvent::outcome`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationEventSubscription {
	pub finality: AttestationSubscriptionFinality,
	pub from_finalized_block: Hash32,
	pub kinds: Vec<AttestationEventKind>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationSubscriptionFinality {
	Finalized,
}

impl AttestationEventSubscription {
	pub fn new(
		from_finalized_block: Hash32,
		kinds: Vec<AttestationEventKind>,
	) -> DomainResult<Self> {
		from_finalized_block.validate()?;
		if kinds.is_empty()
			|| kinds.len() > 8
			|| kinds.iter().collect::<HashSet<_>>().len() != kinds.len()
		{
			return Err(invalid("attestation event subscription requires 1-8 unique kinds"));
		}
		Ok(Self {
			finality: AttestationSubscriptionFinality::Finalized,
			from_finalized_block,
			kinds,
		})
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationInput {
	pub schema: SchemaId,
	pub subject_commitment: SubjectCommitment,
	pub payload_commitment: PayloadCommitment,
	pub status_commitment: StatusCommitment,
	pub parent: Option<AttestationId>,
	pub expiry: Option<BlockNumber>,
	pub uniqueness_commitment: Option<UniquenessCommitment>,
	pub revocable: bool,
}

impl AttestationInput {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		if self.expiry.is_some_and(|expiry| expiry <= current_block) {
			return Err(expired("attestation expiry must be in the future"));
		}
		Ok(())
	}
}

impl Validate for AttestationInput {
	fn validate(&self) -> DomainResult<()> {
		self.schema.validate()?;
		self.subject_commitment.validate()?;
		self.payload_commitment.validate()?;
		self.status_commitment.validate()?;
		if let Some(parent) = &self.parent {
			parent.validate()?;
		}
		if let Some(commitment) = &self.uniqueness_commitment {
			commitment.validate()?;
		}
		Ok(())
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegatedAction {
	Issue,
	Revoke,
}

/// Exact signed fields for native delegated issuance.
///
/// This SDK does not encode the payload. The Subxt binding must encode the runtime's canonical
/// `(b"cord:orbis:delegated-attestation:v1", intent)` tuple using metadata-derived SCALE types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DelegatedIntent {
	pub genesis_hash: Hash32,
	pub spec_version: u32,
	pub action: DelegatedAction,
	pub issuer: AccountId,
	pub delegate: AccountId,
	pub schema: SchemaId,
	pub subject_commitment: SubjectCommitment,
	pub payload_commitment: PayloadCommitment,
	pub status_commitment: StatusCommitment,
	pub parent: Option<AttestationId>,
	pub expiry: Option<BlockNumber>,
	pub uniqueness_commitment: Option<UniquenessCommitment>,
	pub revocable: bool,
	pub nonce: u64,
	pub deadline: BlockNumber,
}

impl DelegatedIntent {
	pub fn input(&self) -> AttestationInput {
		AttestationInput {
			schema: self.schema.clone(),
			subject_commitment: self.subject_commitment.clone(),
			payload_commitment: self.payload_commitment.clone(),
			status_commitment: self.status_commitment.clone(),
			parent: self.parent.clone(),
			expiry: self.expiry,
			uniqueness_commitment: self.uniqueness_commitment.clone(),
			revocable: self.revocable,
		}
	}

	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		if self.deadline < current_block {
			return Err(expired("delegated intent deadline has passed"));
		}
		self.input().validate_at(current_block)
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DelegatedRevokeIntent {
	pub genesis_hash: Hash32,
	pub spec_version: u32,
	pub action: DelegatedAction,
	pub revoker: AccountId,
	pub delegate: AccountId,
	pub attestation: AttestationId,
	pub nonce: u64,
	pub deadline: BlockNumber,
}

impl Validate for DelegatedRevokeIntent {
	fn validate(&self) -> DomainResult<()> {
		self.genesis_hash.validate()?;
		if self.spec_version == 0 {
			return Err(invalid("delegated revocation spec_version must be non-zero"));
		}
		if self.action != DelegatedAction::Revoke {
			return Err(invalid("delegated revocation action must be revoke"));
		}
		self.revoker.validate()?;
		self.delegate.validate()?;
		self.attestation.validate()
	}
}

impl DelegatedRevokeIntent {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		if self.deadline < current_block {
			return Err(expired("delegated revocation deadline has passed"));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedDelegatedIssue {
	pub intent: DelegatedIntent,
	pub signature: Signature,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedDelegatedRevoke {
	pub intent: DelegatedRevokeIntent,
	pub signature: Signature,
}

impl Validate for DelegatedIntent {
	fn validate(&self) -> DomainResult<()> {
		self.genesis_hash.validate()?;
		if self.action != DelegatedAction::Issue {
			return Err(invalid("delegated issuance action must be issue"));
		}
		if self.spec_version == 0 {
			return Err(invalid("delegated intent spec_version must be non-zero"));
		}
		self.issuer.validate()?;
		self.delegate.validate()?;
		self.input().validate()
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureScheme {
	Sr25519,
	Ed25519,
	Ecdsa,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
	pub scheme: SignatureScheme,
	pub bytes: String,
}

impl Signature {
	pub fn new(scheme: SignatureScheme, bytes: impl Into<String>) -> DomainResult<Self> {
		let signature = Self { scheme, bytes: bytes.into() };
		signature.validate()?;
		Ok(signature)
	}

	pub fn raw_bytes(&self) -> DomainResult<Vec<u8>> {
		let value = self
			.bytes
			.strip_prefix("0x")
			.ok_or_else(|| invalid("signature must be 0x-prefixed hex"))?;
		hex::decode(value).map_err(|_| invalid("signature must be valid hex"))
	}
}

impl Validate for Signature {
	fn validate(&self) -> DomainResult<()> {
		let bytes = self.raw_bytes()?;
		let expected = match self.scheme {
			SignatureScheme::Sr25519 | SignatureScheme::Ed25519 => 64,
			SignatureScheme::Ecdsa => 65,
		};
		if bytes.len() != expected {
			return Err(invalid("invalid delegated signature length"));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum AttestationQuery {
	SchemaById {
		schema: SchemaId,
	},
	AttestationById {
		attestation: AttestationId,
	},
	LiveStatus {
		attestation: AttestationId,
	},
	CreatorSchemas {
		creator: AccountId,
		page: PageRequest,
	},
	IssuerAttestations {
		issuer: AccountId,
		page: PageRequest,
	},
	SubjectSchemaAttestations {
		subject_commitment: SubjectCommitment,
		schema: SchemaId,
		page: PageRequest,
	},
	NextDelegatedNonce {
		issuer: AccountId,
	},
	SchemaCount,
	AttestationCount,
	NextIssuanceNonce {
		issuer: AccountId,
	},
	ExternalStatus {
		issuer: AccountId,
		status_commitment: StatusCommitment,
	},
}

impl Validate for AttestationQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::SchemaById { schema } => schema.validate(),
			Self::AttestationById { attestation } | Self::LiveStatus { attestation } => {
				attestation.validate()
			},
			Self::CreatorSchemas { creator, page } => {
				creator.validate()?;
				page.validate()
			},
			Self::IssuerAttestations { issuer, page } => {
				issuer.validate()?;
				page.validate()
			},
			Self::SubjectSchemaAttestations { subject_commitment, schema, page } => {
				subject_commitment.validate()?;
				schema.validate()?;
				page.validate()
			},
			Self::NextDelegatedNonce { issuer } | Self::NextIssuanceNonce { issuer } => {
				issuer.validate()
			},
			Self::ExternalStatus { issuer, status_commitment } => {
				issuer.validate()?;
				status_commitment.validate()
			},
			Self::SchemaCount | Self::AttestationCount => Ok(()),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum AttestationCommand {
	CreateSchema {
		definition: Vec<u8>,
		authorized_issuers: Vec<AccountId>,
		revocable: bool,
		unique: bool,
		index_policy: IndexPolicy,
	},
	SetSchemaStatus {
		schema: SchemaId,
		status: SchemaStatus,
	},
	Issue {
		input: AttestationInput,
	},
	IssueDelegated {
		intent: DelegatedIntent,
		signature: Signature,
	},
	IssueBatch {
		inputs: Vec<AttestationInput>,
	},
	Revoke {
		attestation: AttestationId,
	},
	RevokeDelegated {
		intent: DelegatedRevokeIntent,
		signature: Signature,
	},
	IssueDelegatedBatch {
		items: Vec<SignedDelegatedIssue>,
	},
	RevokeBatch {
		attestations: Vec<AttestationId>,
	},
	RevokeDelegatedBatch {
		items: Vec<SignedDelegatedRevoke>,
	},
	RevokeExternalStatus {
		status_commitment: StatusCommitment,
	},
	RevokeExternalStatusBatch {
		status_commitments: Vec<StatusCommitment>,
	},
	SetEmergencyPause {
		paused: bool,
	},
	ForceSchemaStatus {
		schema: SchemaId,
		status: SchemaStatus,
	},
	ForceRevoke {
		attestation: AttestationId,
	},
}

impl AttestationCommand {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		match self {
			Self::Issue { input } => input.validate_at(current_block),
			Self::IssueDelegated { intent, .. } => intent.validate_at(current_block),
			Self::RevokeDelegated { intent, .. } => intent.validate_at(current_block),
			Self::IssueDelegatedBatch { items } => {
				for item in items {
					item.intent.validate_at(current_block)?;
				}
				Ok(())
			},
			Self::RevokeDelegatedBatch { items } => {
				for item in items {
					item.intent.validate_at(current_block)?;
				}
				Ok(())
			},
			Self::IssueBatch { inputs } => {
				for input in inputs {
					input.validate_at(current_block)?;
				}
				Ok(())
			},
			_ => Ok(()),
		}
	}
}

impl Validate for AttestationCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::CreateSchema { definition, authorized_issuers, .. } => {
				ensure_bytes(definition, 1, MAX_SCHEMA_DEFINITION_BYTES, "schema definition")?;
				if authorized_issuers.len() > MAX_AUTHORIZED_ISSUERS {
					return Err(invalid("authorized issuer limit exceeds 64"));
				}
				let mut seen = HashSet::new();
				for issuer in authorized_issuers {
					issuer.validate()?;
					if !seen.insert(issuer) {
						return Err(invalid("authorized issuers contain a duplicate"));
					}
				}
				Ok(())
			},
			Self::SetSchemaStatus { schema, .. } | Self::ForceSchemaStatus { schema, .. } => {
				schema.validate()
			},
			Self::Issue { input } => input.validate(),
			Self::IssueDelegated { intent, signature } => {
				intent.validate()?;
				signature.validate()
			},
			Self::RevokeDelegated { intent, signature } => {
				intent.validate()?;
				signature.validate()
			},
			Self::IssueBatch { inputs } => {
				if inputs.is_empty() || inputs.len() > MAX_BATCH_SIZE {
					return Err(invalid("attestation batch must contain between 1 and 64 items"));
				}
				for input in inputs {
					input.validate()?;
				}
				Ok(())
			},
			Self::IssueDelegatedBatch { items } => {
				ensure_batch_len(items.len())?;
				for item in items {
					item.intent.validate()?;
					item.signature.validate()?;
				}
				Ok(())
			},
			Self::RevokeBatch { attestations } => {
				ensure_batch_len(attestations.len())?;
				for attestation in attestations {
					attestation.validate()?;
				}
				Ok(())
			},
			Self::RevokeDelegatedBatch { items } => {
				ensure_batch_len(items.len())?;
				for item in items {
					item.intent.validate()?;
					item.signature.validate()?;
				}
				Ok(())
			},
			Self::RevokeExternalStatusBatch { status_commitments } => {
				ensure_batch_len(status_commitments.len())?;
				for status_commitment in status_commitments {
					status_commitment.validate()?;
				}
				Ok(())
			},
			Self::RevokeExternalStatus { status_commitment } => status_commitment.validate(),
			Self::Revoke { attestation } | Self::ForceRevoke { attestation } => {
				attestation.validate()
			},
			Self::SetEmergencyPause { .. } => Ok(()),
		}
	}
}

fn ensure_batch_len(len: usize) -> DomainResult<()> {
	if len == 0 || len > MAX_BATCH_SIZE {
		return Err(invalid("attestation batch must contain between 1 and 64 items"));
	}
	Ok(())
}

#[cfg(test)]
mod vector_tests {
	use std::collections::{BTreeMap, BTreeSet};

	use serde::Deserialize;

	use super::*;

	#[derive(Deserialize)]
	struct Vectors {
		signing: Vec<SigningVector>,
		events: Vec<EventVector>,
	}

	#[derive(Deserialize)]
	struct SigningVector {
		scheme: String,
		kind: String,
		intent: Option<serde_json::Value>,
		payload: String,
	}

	#[derive(Deserialize)]
	struct EventVector {
		runtime_event: AttestationEvent,
		semantic_outcome: AttestationOutcome,
	}

	#[test]
	fn shared_signing_and_event_vectors_match() {
		let vectors: Vectors = serde_json::from_str(include_str!(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../docs/sdk/vectors/attestation-v1.json"
		)))
		.unwrap();
		let schemes: BTreeSet<_> =
			vectors.signing.iter().map(|vector| vector.scheme.as_str()).collect();
		assert_eq!(schemes, BTreeSet::from(["ecdsa", "ed25519", "sr25519"]));
		let mut canonical = BTreeMap::<String, serde_json::Value>::new();
		for vector in vectors.signing {
			if let Some(intent) = vector.intent {
				canonical.insert(vector.kind.clone(), intent);
			}
			let intent = canonical.get(&vector.kind).unwrap().clone();
			let actual = match vector.kind.as_str() {
				"issue" => delegated_issue_signing_payload(
					&serde_json::from_value::<DelegatedIntent>(intent).unwrap(),
				),
				"revoke" => delegated_revoke_signing_payload(
					&serde_json::from_value::<DelegatedRevokeIntent>(intent).unwrap(),
				),
				other => panic!("unexpected signing vector kind {other}"),
			}
			.unwrap();
			assert_eq!(format!("0x{}", hex::encode(actual)), vector.payload);
		}
		for vector in vectors.events {
			assert_eq!(vector.runtime_event.outcome(), vector.semantic_outcome);
		}
	}
}

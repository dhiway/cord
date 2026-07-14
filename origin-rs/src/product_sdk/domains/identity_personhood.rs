use serde::{Deserialize, Serialize};

use super::{
	attestation::Signature,
	common::{
		ensure_bytes, invalid, AccountId, DomainResult, FinalizedQuery, FinalizedValue, Hash32,
		SubmitAndFinalize, Validate,
	},
};

pub const MAX_IDENTITY_RAW_BYTES: usize = 32;
pub const MAX_ADDITIONAL_IDENTITY_FIELDS: usize = 32;
pub const RING_VRF_KEY_BYTES: usize = 32;
pub const RING_VRF_SIGNATURE_BYTES: usize = 64;

pub type IdentityPersonhoodRead = FinalizedQuery<IdentityPersonhoodQuery>;
pub type IdentityPersonhoodWrite = SubmitAndFinalize<IdentityPersonhoodCommand>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum IdentityPersonhoodQuery {
	IdentityStatus { account: AccountId },
	PersonhoodStatus { account: AccountId },
	AttestationAllowance { account: AccountId },
}

impl Validate for IdentityPersonhoodQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::IdentityStatus { account } |
			Self::PersonhoodStatus { account } |
			Self::AttestationAllowance { account } => account.validate(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityStatusView {
	pub registered: bool,
	pub judgement_count: u32,
	pub requested: u32,
	pub reasonable: u32,
	pub known_good: u32,
	pub out_of_date: u32,
	pub low_quality: u32,
	pub erroneous: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersonhoodStatusView {
	pub full_personal_id: Option<u64>,
	pub full_recognized: bool,
	pub lite_recognized: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationAllowanceView {
	pub remaining: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum IdentityPersonhoodResponse {
	IdentityStatus(FinalizedValue<IdentityStatusView>),
	PersonhoodStatus(FinalizedValue<PersonhoodStatusView>),
	AttestationAllowance(FinalizedValue<AttestationAllowanceView>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IdentityData {
	None,
	Raw {
		value: String,
	},
	#[serde(rename = "blake2_256")]
	BlakeTwo256 {
		hash: Hash32,
	},
	#[serde(rename = "sha2_256")]
	Sha256 {
		hash: Hash32,
	},
	#[serde(rename = "keccak_256")]
	Keccak256 {
		hash: Hash32,
	},
	#[serde(rename = "sha3_256")]
	ShaThree256 {
		hash: Hash32,
	},
}

impl Validate for IdentityData {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::None => Ok(()),
			Self::Raw { value } =>
				ensure_bytes(value.as_bytes(), 1, MAX_IDENTITY_RAW_BYTES, "identity data"),
			Self::BlakeTwo256 { hash } |
			Self::Sha256 { hash } |
			Self::Keccak256 { hash } |
			Self::ShaThree256 { hash } => hash.validate(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityAdditionalField {
	pub key: IdentityData,
	pub value: IdentityData,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityInfo {
	pub additional: Vec<IdentityAdditionalField>,
	pub display: IdentityData,
	pub legal: IdentityData,
	pub web: IdentityData,
	pub email: IdentityData,
	pub image: IdentityData,
}

impl Validate for IdentityInfo {
	fn validate(&self) -> DomainResult<()> {
		if self.additional.len() > MAX_ADDITIONAL_IDENTITY_FIELDS {
			return Err(invalid("identity additional fields exceed the runtime bound"));
		}
		for field in &self.additional {
			field.key.validate()?;
			field.value.validate()?;
		}
		self.display.validate()?;
		self.legal.validate()?;
		self.web.validate()?;
		self.email.validate()?;
		self.image.validate()
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Judgement {
	Reasonable,
	KnownGood,
	OutOfDate,
	LowQuality,
	Erroneous,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RingVrfSignature(String);

impl RingVrfSignature {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = Self(value.into());
		value.validate()?;
		Ok(value)
	}

	pub fn raw_bytes(&self) -> DomainResult<Vec<u8>> {
		self.validate()?;
		hex::decode(&self.0[2..]).map_err(|_| invalid("invalid ring VRF signature hex"))
	}
}

impl Validate for RingVrfSignature {
	fn validate(&self) -> DomainResult<()> {
		let raw = self.0.as_bytes();
		if raw.len() != 2 + RING_VRF_SIGNATURE_BYTES * 2 ||
			!self.0.starts_with("0x") ||
			!raw[2..]
				.iter()
				.all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
		{
			return Err(invalid("ring VRF signature must be 64 lowercase hex bytes"));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IdentityPersonhoodCommand {
	SetIdentity {
		info: IdentityInfo,
	},
	ClearIdentity,
	RequestJudgement {
		registrar: AccountId,
	},
	CancelJudgement {
		registrar: AccountId,
	},
	ProvideJudgement {
		target: AccountId,
		judgement: Judgement,
		identity_hash: Hash32,
	},
	AttestLitePerson {
		candidate: AccountId,
		candidate_signature: Signature,
		ring_vrf_key: Hash32,
		proof_of_ownership: RingVrfSignature,
	},
}

impl Validate for IdentityPersonhoodCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::SetIdentity { info } => info.validate(),
			Self::ClearIdentity => Ok(()),
			Self::RequestJudgement { registrar } | Self::CancelJudgement { registrar } =>
				registrar.validate(),
			Self::ProvideJudgement { target, identity_hash, .. } => {
				target.validate()?;
				identity_hash.validate()
			},
			Self::AttestLitePerson {
				candidate,
				candidate_signature,
				ring_vrf_key,
				proof_of_ownership,
			} => {
				candidate.validate()?;
				candidate_signature.validate()?;
				ring_vrf_key.validate()?;
				proof_of_ownership.validate()
			},
		}
	}
}

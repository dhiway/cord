use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, invalid, AccountId, AttestationId, BlockNumber, ContentCommitment, DomainResult,
	FinalizedPage, FinalizedQuery, FinalizedValue, NameId, PageRequest, RegistrationCommitment,
	SubjectId, SubmitAndFinalize, Validate,
};

pub const LABEL_POLICY_VERSION: u16 = 1;
pub const MAX_LABEL_BYTES: usize = 63;
pub const MAX_SALT_BYTES: usize = 64;
pub const MAX_ADDRESS_BYTES: usize = 128;
pub const MAX_TEXT_KEY_BYTES: usize = 32;
pub const MAX_TEXT_VALUE_BYTES: usize = 256;

pub type DotnsRead = FinalizedQuery<DotnsQuery>;
pub type DotnsWrite = SubmitAndFinalize<DotnsCommand>;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		let bytes = value.as_bytes();
		let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
		if bytes.is_empty()
			|| bytes.len() > MAX_LABEL_BYTES
			|| !alphanumeric(bytes[0])
			|| !alphanumeric(bytes[bytes.len() - 1])
			|| !bytes.iter().all(|byte| alphanumeric(*byte) || *byte == b'-')
		{
			return Err(invalid(
				"DotNS label must use lowercase ASCII letters, digits, and internal hyphens",
			));
		}
		Ok(Self(value))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for Label {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

macro_rules! bounded_bytes {
	($name:ident, $min:expr, $max:expr, $field:literal) => {
		#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
		#[serde(transparent)]
		pub struct $name(Vec<u8>);

		impl $name {
			pub fn new(value: Vec<u8>) -> DomainResult<Self> {
				ensure_bytes(&value, $min, $max, $field)?;
				Ok(Self(value))
			}

			pub fn as_bytes(&self) -> &[u8] {
				&self.0
			}
		}

		impl Validate for $name {
			fn validate(&self) -> DomainResult<()> {
				ensure_bytes(&self.0, $min, $max, $field)
			}
		}
	};
}

bounded_bytes!(Salt, 0, MAX_SALT_BYTES, "DotNS salt");
bounded_bytes!(Address, 1, MAX_ADDRESS_BYTES, "DotNS address");
bounded_bytes!(TextKey, 1, MAX_TEXT_KEY_BYTES, "DotNS text key");
bounded_bytes!(TextValue, 1, MAX_TEXT_VALUE_BYTES, "DotNS text value");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NameView {
	pub name: NameId,
	pub parent: Option<NameId>,
	pub label: Label,
	pub owner: AccountId,
	pub expires_at: BlockNumber,
	pub depth: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NameStatus {
	pub exists: bool,
	pub active: bool,
	pub expires_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum DotnsResponse {
	Name(FinalizedValue<NameView>),
	NameId(FinalizedValue<NameId>),
	Names(FinalizedPage<NameId>),
	Address(FinalizedValue<Address>),
	Subject(FinalizedValue<SubjectId>),
	Attestation(FinalizedValue<AttestationId>),
	Content(FinalizedValue<ContentCommitment>),
	Text(FinalizedValue<TextValue>),
	Status(FinalizedValue<NameStatus>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum DotnsQuery {
	NameById { name: NameId },
	RootByLabel { label: Label },
	OwnerNames { owner: AccountId, page: PageRequest },
	ResolveAddress { name: NameId },
	ResolveSubject { name: NameId },
	ResolveAttestation { name: NameId },
	ResolveContent { name: NameId },
	ResolveText { name: NameId, key: TextKey },
	PrimaryName { owner: AccountId },
	NameStatus { name: NameId },
}

impl Validate for DotnsQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::NameById { name }
			| Self::ResolveAddress { name }
			| Self::ResolveSubject { name }
			| Self::ResolveAttestation { name }
			| Self::ResolveContent { name }
			| Self::NameStatus { name } => name.validate(),
			Self::RootByLabel { label } => label.validate(),
			Self::OwnerNames { owner, page } => {
				owner.validate()?;
				page.validate()
			},
			Self::ResolveText { name, key } => {
				name.validate()?;
				key.validate()
			},
			Self::PrimaryName { owner } => owner.validate(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum DotnsCommand {
	Commit {
		commitment: RegistrationCommitment,
	},
	CancelCommitment {
		commitment: RegistrationCommitment,
	},
	PruneExpiredCommitment {
		owner: AccountId,
		commitment: RegistrationCommitment,
	},
	Register {
		parent: Option<NameId>,
		label: Label,
		salt: Salt,
	},
	Renew {
		name: NameId,
		additional_blocks: BlockNumber,
	},
	Transfer {
		name: NameId,
		new_owner: AccountId,
	},
	AddController {
		name: NameId,
		controller: AccountId,
	},
	RemoveController {
		name: NameId,
		controller: AccountId,
	},
	SetAddress {
		name: NameId,
		address: Option<Address>,
	},
	SetSubject {
		name: NameId,
		subject: Option<SubjectId>,
	},
	SetAttestation {
		name: NameId,
		attestation: Option<AttestationId>,
	},
	SetContent {
		name: NameId,
		content: Option<ContentCommitment>,
	},
	SetText {
		name: NameId,
		key: TextKey,
		value: Option<TextValue>,
	},
	SetPrimaryName {
		name: Option<NameId>,
	},
	Release {
		name: NameId,
	},
	RemoveExpiredName {
		name: NameId,
	},
	ReserveName {
		parent: Option<NameId>,
		label: Label,
		beneficiary: Option<AccountId>,
		expires_at: Option<BlockNumber>,
	},
	ClearReservation {
		name: NameId,
	},
	SetLabelProtection {
		label: Label,
		protected: bool,
	},
	SetPaused {
		paused: bool,
	},
	ForceTransfer {
		name: NameId,
		new_owner: AccountId,
	},
	ForceRevoke {
		name: NameId,
	},
}

impl DotnsCommand {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		if let Self::ReserveName { expires_at: Some(expiry), .. } = self {
			if *expiry <= current_block {
				return Err(invalid("DotNS reservation expiry must be in the future"));
			}
		}
		Ok(())
	}
}

impl Validate for DotnsCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::Commit { commitment } | Self::CancelCommitment { commitment } => {
				commitment.validate()
			},
			Self::PruneExpiredCommitment { owner, commitment } => {
				owner.validate()?;
				commitment.validate()
			},
			Self::Register { parent, label, salt } => {
				if let Some(parent) = parent {
					parent.validate()?;
				}
				label.validate()?;
				salt.validate()
			},
			Self::Renew { name, additional_blocks } => {
				name.validate()?;
				if *additional_blocks == 0 {
					return Err(invalid("DotNS renewal must add at least one block"));
				}
				Ok(())
			},
			Self::Transfer { name, new_owner } | Self::ForceTransfer { name, new_owner } => {
				name.validate()?;
				new_owner.validate()
			},
			Self::AddController { name, controller }
			| Self::RemoveController { name, controller } => {
				name.validate()?;
				controller.validate()
			},
			Self::SetAddress { name, address } => {
				name.validate()?;
				address.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetSubject { name, subject } => {
				name.validate()?;
				subject.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetAttestation { name, attestation } => {
				name.validate()?;
				attestation.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetContent { name, content } => {
				name.validate()?;
				content.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetText { name, key, value } => {
				name.validate()?;
				key.validate()?;
				value.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetPrimaryName { name } => name.as_ref().map_or(Ok(()), Validate::validate),
			Self::Release { name }
			| Self::RemoveExpiredName { name }
			| Self::ClearReservation { name }
			| Self::ForceRevoke { name } => name.validate(),
			Self::ReserveName { parent, label, beneficiary, .. } => {
				if let Some(parent) = parent {
					parent.validate()?;
				}
				label.validate()?;
				beneficiary.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetLabelProtection { label, .. } => label.validate(),
			Self::SetPaused { .. } => Ok(()),
		}
	}
}

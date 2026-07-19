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

use std::collections::HashSet;

use codec::Encode;
use serde::{Deserialize, Serialize};
use sp_crypto_hashing::blake2_256;

use super::common::{
	ensure_bytes, invalid, AccountId, AttestationId, BlockNumber, ContentCommitment, DomainResult,
	FinalizedPage, FinalizedQuery, FinalizedValue, Hash32, NameId, PageRequest,
	RegistrationCommitment, SubjectId, SubmitAndFinalize, Validate,
};

pub const LABEL_POLICY_VERSION: u16 = 1;
pub const ATTESTATION_RESOLUTION_POLICY: &str =
	"live-only: missing, revoked, expired, or inactive-schema attestations resolve to null";
pub const MAX_LABEL_BYTES: usize = 63;
pub const MAX_SALT_BYTES: usize = 64;
pub const MAX_ADDRESS_BYTES: usize = 128;
pub const MAX_TEXT_KEY_BYTES: usize = 32;
pub const MAX_TEXT_VALUE_BYTES: usize = 256;
pub const NAME_ID_DOMAIN: &[u8] = b"cord:orbis:names:name:v1";
pub const COMMITMENT_DOMAIN: &[u8] = b"cord:orbis:names:commitment:v1";

pub type NamesRead = FinalizedQuery<NamesQuery>;
pub type NamesWrite = SubmitAndFinalize<NamesCommand>;

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
				"Orbis Names label must use lowercase ASCII letters, digits, and internal hyphens",
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

fn hash_bytes(value: &Hash32) -> DomainResult<[u8; 32]> {
	let bytes = hex::decode(&value.as_str()[2..]).map_err(|_| invalid("invalid 32-byte hash"))?;
	bytes.try_into().map_err(|_| invalid("invalid 32-byte hash"))
}

/// Derive the exact pallet `NameId` from the finalized network identity, parent, and label.
pub fn derive_name_id(
	genesis_hash: &Hash32,
	parent: Option<&NameId>,
	label: &Label,
) -> DomainResult<NameId> {
	genesis_hash.validate()?;
	label.validate()?;
	let genesis = hash_bytes(genesis_hash)?;
	let parent = parent.map(|value| hash_bytes(value.as_hash())).transpose()?;
	Ok(NameId(Hash32::from_bytes(blake2_256(
		&(NAME_ID_DOMAIN, genesis, parent, label.as_str().as_bytes()).encode(),
	))))
}

/// Derive the exact commit/reveal commitment accepted by the native Orbis Names pallet.
pub fn registration_commitment(
	genesis_hash: &Hash32,
	owner: &AccountId,
	parent: Option<&NameId>,
	label: &Label,
	salt: &Salt,
) -> DomainResult<RegistrationCommitment> {
	use crate::types::account::ss58_to_account_id;

	owner.validate()?;
	salt.validate()?;
	let name = hash_bytes(derive_name_id(genesis_hash, parent, label)?.as_hash())?;
	let genesis = hash_bytes(genesis_hash)?;
	let owner = ss58_to_account_id(owner.as_str())
		.map_err(|_| invalid("Orbis Names commitment owner must be a valid SS58 AccountId32"))?;
	Ok(RegistrationCommitment(Hash32::from_bytes(blake2_256(
		&(COMMITMENT_DOMAIN, genesis, owner, name, salt.as_bytes()).encode(),
	))))
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

bounded_bytes!(Salt, 1, MAX_SALT_BYTES, "Orbis Names salt");
bounded_bytes!(Address, 1, MAX_ADDRESS_BYTES, "Orbis Names address");
bounded_bytes!(TextKey, 1, MAX_TEXT_KEY_BYTES, "Orbis Names text key");
bounded_bytes!(TextValue, 1, MAX_TEXT_VALUE_BYTES, "Orbis Names text value");

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NamesEventKind {
	CommitmentStored,
	CommitmentRemoved,
	NameRegistered,
	NameRenewed,
	NameTransferred,
	NameReleased,
	ExpiredNameRemoved,
	ControllerAdded,
	ControllerRemoved,
	AddressSet,
	SubjectSet,
	AttestationSet,
	ContentSet,
	TextSet,
	PrimaryNameSet,
	NameReserved,
	ReservationCleared,
	LabelProtectionSet,
	PauseSet,
	EmergencyNameRevoked,
	RegistrarSet,
}

/// Stable, transport-neutral decoding target for every native Orbis Names event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum NamesEvent {
	CommitmentStored {
		owner: AccountId,
		commitment: RegistrationCommitment,
		at: BlockNumber,
	},
	CommitmentRemoved {
		owner: AccountId,
		commitment: RegistrationCommitment,
	},
	NameRegistered {
		name: NameId,
		parent: Option<NameId>,
		label: Label,
		owner: AccountId,
		expires_at: BlockNumber,
	},
	NameRenewed {
		name: NameId,
		expires_at: BlockNumber,
	},
	NameTransferred {
		name: NameId,
		from: AccountId,
		to: AccountId,
	},
	NameReleased {
		name: NameId,
		owner: AccountId,
	},
	ExpiredNameRemoved {
		name: NameId,
	},
	ControllerAdded {
		name: NameId,
		controller: AccountId,
	},
	ControllerRemoved {
		name: NameId,
		controller: AccountId,
	},
	AddressSet {
		name: NameId,
		present: bool,
	},
	SubjectSet {
		name: NameId,
		present: bool,
	},
	AttestationSet {
		name: NameId,
		present: bool,
	},
	ContentSet {
		name: NameId,
		present: bool,
	},
	TextSet {
		name: NameId,
		key: TextKey,
		present: bool,
	},
	PrimaryNameSet {
		owner: AccountId,
		name: Option<NameId>,
	},
	NameReserved {
		name: NameId,
		beneficiary: Option<AccountId>,
		expires_at: Option<BlockNumber>,
	},
	ReservationCleared {
		name: NameId,
	},
	LabelProtectionSet {
		label: Label,
		protected: bool,
	},
	PauseSet {
		paused: bool,
	},
	EmergencyNameRevoked {
		name: NameId,
	},
	RegistrarSet {
		registrar: AccountId,
		enabled: bool,
	},
}

/// Stable application outcome derived from one finalized Orbis Names event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", content = "data", rename_all = "snake_case")]
pub enum NamesOutcome {
	CommitmentStored { commitment: RegistrationCommitment },
	CommitmentRemoved { commitment: RegistrationCommitment },
	NameRegistered { name: NameId },
	NameRenewed { name: NameId, expires_at: BlockNumber },
	NameTransferred { name: NameId, owner: AccountId },
	NameReleased { name: NameId },
	ExpiredNameRemoved { name: NameId },
	ControllerAdded { name: NameId, controller: AccountId },
	ControllerRemoved { name: NameId, controller: AccountId },
	AddressSet { name: NameId, present: bool },
	SubjectSet { name: NameId, present: bool },
	AttestationSet { name: NameId, present: bool },
	ContentSet { name: NameId, present: bool },
	TextSet { name: NameId, key: TextKey, present: bool },
	PrimaryNameSet { owner: AccountId, name: Option<NameId> },
	NameReserved { name: NameId },
	ReservationCleared { name: NameId },
	LabelProtectionSet { label: Label, protected: bool },
	PauseSet { paused: bool },
	EmergencyNameRevoked { name: NameId },
	RegistrarSet { registrar: AccountId, enabled: bool },
}

impl NamesEvent {
	pub const fn kind(&self) -> NamesEventKind {
		match self {
			Self::CommitmentStored { .. } => NamesEventKind::CommitmentStored,
			Self::CommitmentRemoved { .. } => NamesEventKind::CommitmentRemoved,
			Self::NameRegistered { .. } => NamesEventKind::NameRegistered,
			Self::NameRenewed { .. } => NamesEventKind::NameRenewed,
			Self::NameTransferred { .. } => NamesEventKind::NameTransferred,
			Self::NameReleased { .. } => NamesEventKind::NameReleased,
			Self::ExpiredNameRemoved { .. } => NamesEventKind::ExpiredNameRemoved,
			Self::ControllerAdded { .. } => NamesEventKind::ControllerAdded,
			Self::ControllerRemoved { .. } => NamesEventKind::ControllerRemoved,
			Self::AddressSet { .. } => NamesEventKind::AddressSet,
			Self::SubjectSet { .. } => NamesEventKind::SubjectSet,
			Self::AttestationSet { .. } => NamesEventKind::AttestationSet,
			Self::ContentSet { .. } => NamesEventKind::ContentSet,
			Self::TextSet { .. } => NamesEventKind::TextSet,
			Self::PrimaryNameSet { .. } => NamesEventKind::PrimaryNameSet,
			Self::NameReserved { .. } => NamesEventKind::NameReserved,
			Self::ReservationCleared { .. } => NamesEventKind::ReservationCleared,
			Self::LabelProtectionSet { .. } => NamesEventKind::LabelProtectionSet,
			Self::PauseSet { .. } => NamesEventKind::PauseSet,
			Self::EmergencyNameRevoked { .. } => NamesEventKind::EmergencyNameRevoked,
			Self::RegistrarSet { .. } => NamesEventKind::RegistrarSet,
		}
	}

	pub fn outcome(&self) -> NamesOutcome {
		match self {
			Self::CommitmentStored { commitment, .. } => {
				NamesOutcome::CommitmentStored { commitment: commitment.clone() }
			},
			Self::CommitmentRemoved { commitment, .. } => {
				NamesOutcome::CommitmentRemoved { commitment: commitment.clone() }
			},
			Self::NameRegistered { name, .. } => {
				NamesOutcome::NameRegistered { name: name.clone() }
			},
			Self::NameRenewed { name, expires_at } => {
				NamesOutcome::NameRenewed { name: name.clone(), expires_at: *expires_at }
			},
			Self::NameTransferred { name, to, .. } => {
				NamesOutcome::NameTransferred { name: name.clone(), owner: to.clone() }
			},
			Self::NameReleased { name, .. } => NamesOutcome::NameReleased { name: name.clone() },
			Self::ExpiredNameRemoved { name } => {
				NamesOutcome::ExpiredNameRemoved { name: name.clone() }
			},
			Self::ControllerAdded { name, controller } => {
				NamesOutcome::ControllerAdded { name: name.clone(), controller: controller.clone() }
			},
			Self::ControllerRemoved { name, controller } => NamesOutcome::ControllerRemoved {
				name: name.clone(),
				controller: controller.clone(),
			},
			Self::AddressSet { name, present } => {
				NamesOutcome::AddressSet { name: name.clone(), present: *present }
			},
			Self::SubjectSet { name, present } => {
				NamesOutcome::SubjectSet { name: name.clone(), present: *present }
			},
			Self::AttestationSet { name, present } => {
				NamesOutcome::AttestationSet { name: name.clone(), present: *present }
			},
			Self::ContentSet { name, present } => {
				NamesOutcome::ContentSet { name: name.clone(), present: *present }
			},
			Self::TextSet { name, key, present } => {
				NamesOutcome::TextSet { name: name.clone(), key: key.clone(), present: *present }
			},
			Self::PrimaryNameSet { owner, name } => {
				NamesOutcome::PrimaryNameSet { owner: owner.clone(), name: name.clone() }
			},
			Self::NameReserved { name, .. } => NamesOutcome::NameReserved { name: name.clone() },
			Self::ReservationCleared { name } => {
				NamesOutcome::ReservationCleared { name: name.clone() }
			},
			Self::LabelProtectionSet { label, protected } => {
				NamesOutcome::LabelProtectionSet { label: label.clone(), protected: *protected }
			},
			Self::PauseSet { paused } => NamesOutcome::PauseSet { paused: *paused },
			Self::EmergencyNameRevoked { name } => {
				NamesOutcome::EmergencyNameRevoked { name: name.clone() }
			},
			Self::RegistrarSet { registrar, enabled } => {
				NamesOutcome::RegistrarSet { registrar: registrar.clone(), enabled: *enabled }
			},
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedNamesEvent {
	pub finalized_block_hash: Hash32,
	pub event_index: u32,
	pub event: NamesEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedNamesOutcome {
	pub event: FinalizedNamesEvent,
	pub outcome: NamesOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamesEventSubscription {
	pub finality: NamesSubscriptionFinality,
	pub from_finalized_block: Hash32,
	pub kinds: Vec<NamesEventKind>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NamesSubscriptionFinality {
	Finalized,
}

impl NamesEventSubscription {
	pub fn new(from_finalized_block: Hash32, kinds: Vec<NamesEventKind>) -> DomainResult<Self> {
		from_finalized_block.validate()?;
		if kinds.is_empty()
			|| kinds.len() > 21
			|| kinds.iter().collect::<HashSet<_>>().len() != kinds.len()
		{
			return Err(invalid("Orbis Names event subscription requires 1-21 unique kinds"));
		}
		Ok(Self { finality: NamesSubscriptionFinality::Finalized, from_finalized_block, kinds })
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum NamesResponse {
	LabelPolicyVersion(FinalizedValue<u16>),
	Name(FinalizedValue<NameView>),
	NameId(FinalizedValue<NameId>),
	Names(FinalizedPage<NameId>),
	Controllers(FinalizedValue<Vec<AccountId>>),
	Address(FinalizedValue<Address>),
	Subject(FinalizedValue<SubjectId>),
	Attestation(FinalizedValue<AttestationId>),
	Content(FinalizedValue<ContentCommitment>),
	Text(FinalizedValue<TextValue>),
	Status(FinalizedValue<NameStatus>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum NamesQuery {
	LabelPolicyVersion,
	NameById { name: NameId },
	RootByLabel { label: Label },
	OwnerNames { owner: AccountId, page: PageRequest },
	Controllers { name: NameId },
	ResolveAddress { name: NameId },
	ResolveSubject { name: NameId },
	ResolveAttestation { name: NameId },
	ResolveContent { name: NameId },
	ResolveText { name: NameId, key: TextKey },
	PrimaryName { owner: AccountId },
	NameStatus { name: NameId },
}

impl Validate for NamesQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::LabelPolicyVersion => Ok(()),
			Self::NameById { name }
			| Self::Controllers { name }
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
pub enum NamesCommand {
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
	SetRegistrar {
		registrar: AccountId,
		enabled: bool,
	},
}

impl NamesCommand {
	pub fn validate_at(&self, current_block: BlockNumber) -> DomainResult<()> {
		self.validate()?;
		if let Self::ReserveName { expires_at: Some(expiry), .. } = self {
			if *expiry <= current_block {
				return Err(invalid("Orbis Names reservation expiry must be in the future"));
			}
		}
		Ok(())
	}
}

impl Validate for NamesCommand {
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
					return Err(invalid("Orbis Names renewal must add at least one block"));
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
			Self::SetRegistrar { registrar, .. } => registrar.validate(),
			Self::SetPaused { .. } => Ok(()),
		}
	}
}

#[cfg(test)]
mod canonical_vector_tests {
	use super::*;

	#[derive(Deserialize)]
	struct DerivationVector {
		genesis_hash: String,
		parent: Option<String>,
		label: String,
		name_id: String,
	}
	#[derive(Deserialize)]
	struct CommitmentVector {
		genesis_hash: String,
		owner: String,
		parent: Option<String>,
		label: String,
		salt: String,
		name_id: String,
		commitment: String,
	}
	#[derive(Deserialize)]
	struct Vectors {
		label_policy_version: u16,
		attestation_resolution_policy: String,
		valid_labels: Vec<String>,
		invalid_labels: Vec<String>,
		derivations: Vec<DerivationVector>,
		commitments: Vec<CommitmentVector>,
		event_kinds: Vec<String>,
	}

	#[test]
	fn shared_vectors_match_label_name_commitment_and_event_contracts() {
		let vectors: Vectors =
			serde_json::from_str(include_str!("../../../../docs/sdk/vectors/names-v1.json"))
				.unwrap();
		assert_eq!(vectors.label_policy_version, LABEL_POLICY_VERSION);
		assert_eq!(vectors.attestation_resolution_policy, ATTESTATION_RESOLUTION_POLICY);
		for label in vectors.valid_labels {
			Label::new(label).unwrap();
		}
		for label in vectors.invalid_labels {
			assert!(Label::new(label).is_err());
		}
		for vector in vectors.derivations {
			let genesis = Hash32::new(vector.genesis_hash).unwrap();
			let parent = vector.parent.map(NameId::new).transpose().unwrap();
			let label = Label::new(vector.label).unwrap();
			assert_eq!(
				derive_name_id(&genesis, parent.as_ref(), &label).unwrap(),
				NameId::new(vector.name_id).unwrap()
			);
		}
		for vector in vectors.commitments {
			let genesis = Hash32::new(vector.genesis_hash).unwrap();
			let owner = AccountId::new(vector.owner).unwrap();
			let parent = vector.parent.map(NameId::new).transpose().unwrap();
			let label = Label::new(vector.label).unwrap();
			let salt = Salt::new(vector.salt.into_bytes()).unwrap();
			assert_eq!(
				derive_name_id(&genesis, parent.as_ref(), &label).unwrap(),
				NameId::new(vector.name_id).unwrap()
			);
			assert_eq!(
				registration_commitment(&genesis, &owner, parent.as_ref(), &label, &salt).unwrap(),
				RegistrationCommitment::new(vector.commitment).unwrap()
			);
		}
		let actual: Vec<String> = [
			NamesEventKind::CommitmentStored,
			NamesEventKind::CommitmentRemoved,
			NamesEventKind::NameRegistered,
			NamesEventKind::NameRenewed,
			NamesEventKind::NameTransferred,
			NamesEventKind::NameReleased,
			NamesEventKind::ExpiredNameRemoved,
			NamesEventKind::ControllerAdded,
			NamesEventKind::ControllerRemoved,
			NamesEventKind::AddressSet,
			NamesEventKind::SubjectSet,
			NamesEventKind::AttestationSet,
			NamesEventKind::ContentSet,
			NamesEventKind::TextSet,
			NamesEventKind::PrimaryNameSet,
			NamesEventKind::NameReserved,
			NamesEventKind::ReservationCleared,
			NamesEventKind::LabelProtectionSet,
			NamesEventKind::PauseSet,
			NamesEventKind::EmergencyNameRevoked,
			NamesEventKind::RegistrarSet,
		]
		.into_iter()
		.map(|kind| serde_json::to_value(kind).unwrap().as_str().unwrap().to_owned())
		.collect();
		assert_eq!(actual, vectors.event_kinds);
	}
}

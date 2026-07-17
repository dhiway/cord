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

//! Private P3 projection of the frozen `cord.origin.host/2` Identity contract.
//!
//! Each request, grant and result is a distinct type. This module is crate-private until the P5
//! authority cutover removes the legacy product taxonomy.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u16)]
pub(crate) enum IdentityV2Operation {
	#[serde(rename = "identity.account")]
	IdentityAccount = 1100,
	#[serde(rename = "identity.profile.read")]
	IdentityProfileRead = 1101,
	#[serde(rename = "identity.profile.disclose")]
	IdentityProfileDisclose = 1102,
	#[serde(rename = "identity.humanity.status")]
	IdentityHumanityStatus = 1103,
	#[serde(rename = "identity.humanity.prove")]
	IdentityHumanityProve = 1104,
	#[serde(rename = "identity.subject.derive")]
	IdentitySubjectDerive = 1105,
	#[serde(rename = "identity.entitlements.read")]
	IdentityEntitlementsRead = 1106,
	#[serde(rename = "transaction.sign")]
	TransactionSign = 1200,
}

impl IdentityV2Operation {
	pub(crate) const ALL: [Self; 8] = [
		Self::IdentityAccount,
		Self::IdentityProfileRead,
		Self::IdentityProfileDisclose,
		Self::IdentityHumanityStatus,
		Self::IdentityHumanityProve,
		Self::IdentitySubjectDerive,
		Self::IdentityEntitlementsRead,
		Self::TransactionSign,
	];

	pub(crate) const fn name(self) -> &'static str {
		match self {
			Self::IdentityAccount => "identity.account",
			Self::IdentityProfileRead => "identity.profile.read",
			Self::IdentityProfileDisclose => "identity.profile.disclose",
			Self::IdentityHumanityStatus => "identity.humanity.status",
			Self::IdentityHumanityProve => "identity.humanity.prove",
			Self::IdentitySubjectDerive => "identity.subject.derive",
			Self::IdentityEntitlementsRead => "identity.entitlements.read",
			Self::TransactionSign => "transaction.sign",
		}
	}

	pub(crate) const fn requires_fresh_consent(self) -> bool {
		matches!(
			self,
			Self::IdentityProfileDisclose | Self::IdentityHumanityProve | Self::TransactionSign
		)
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum IdentityV2Error {
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid,
	#[error("GRANT_SCOPE_DENIED")]
	GrantScopeDenied,
	#[error("GRANT_EXPIRED")]
	GrantExpired,
	#[error("GRANT_REVOKED")]
	GrantRevoked,
	#[error("IDENTITY_AUDIENCE_INVALID")]
	AudienceInvalid,
	#[error("IDENTITY_CHALLENGE_REPLAY")]
	ChallengeReplay,
	#[error("IDENTITY_OLD_INCARNATION")]
	OldIncarnation,
	#[error("SIGNING_CONSENT_REQUIRED")]
	FreshConsentRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinalizedIdentityV2 {
	pub(crate) block_number: u64,
	pub(crate) block_hash: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityReceiptV2 {
	pub(crate) commitment: [u8; 32],
	pub(crate) valid_until: u64,
	pub(crate) finalized: Option<FinalizedIdentityV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityAccountRequestV2 {
	pub(crate) session: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityAccountResultV2 {
	pub(crate) account: [u8; 32],
	pub(crate) session_expires_at: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityProfileReadRequestV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) fields: Vec<String>,
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityProfileReadResultV2 {
	pub(crate) receipt: IdentityReceiptV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityProfileDiscloseRequestV2 {
	pub(crate) audience: String,
	pub(crate) fields: Vec<String>,
	pub(crate) purpose: String,
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityProfileDiscloseResultV2 {
	pub(crate) receipt: IdentityReceiptV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityHumanityStatusRequestV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityHumanityStatusResultV2 {
	pub(crate) status: u16,
	pub(crate) fresh_until: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityHumanityProveRequestV2 {
	pub(crate) audience: String,
	pub(crate) challenge: Vec<u8>,
	pub(crate) expires_at: u64,
	pub(crate) claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityHumanityProveResultV2 {
	pub(crate) proof: Vec<u8>,
	pub(crate) derived_public_key: [u8; 32],
	pub(crate) proof_hash: [u8; 32],
	pub(crate) continuity: bool,
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentitySubjectDeriveRequestV2 {
	pub(crate) product_id: String,
	pub(crate) context: String,
	pub(crate) verifier_audience: String,
	pub(crate) epoch: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentitySubjectDeriveResultV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) derived_public_key: [u8; 32],
	pub(crate) epoch: u32,
	pub(crate) recovery_incarnation_hash: [u8; 32],
	pub(crate) continuity: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityEntitlementsReadRequestV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) scope: String,
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityEntitlementsReadResultV2 {
	pub(crate) allowed: bool,
	pub(crate) scope: String,
	pub(crate) policy_version: u32,
	pub(crate) expires_at: u64,
	pub(crate) fresh_until: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransactionSignRequestV2 {
	pub(crate) payload_hash: [u8; 32],
	pub(crate) policy_hash: [u8; 32],
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransactionSignResultV2 {
	pub(crate) transaction_hash: [u8; 32],
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", content = "input")]
pub(crate) enum IdentityRequestV2 {
	#[serde(rename = "identity.account")]
	Account(IdentityAccountRequestV2),
	#[serde(rename = "identity.profile.read")]
	ProfileRead(IdentityProfileReadRequestV2),
	#[serde(rename = "identity.profile.disclose")]
	ProfileDisclose(IdentityProfileDiscloseRequestV2),
	#[serde(rename = "identity.humanity.status")]
	HumanityStatus(IdentityHumanityStatusRequestV2),
	#[serde(rename = "identity.humanity.prove")]
	HumanityProve(IdentityHumanityProveRequestV2),
	#[serde(rename = "identity.subject.derive")]
	SubjectDerive(IdentitySubjectDeriveRequestV2),
	#[serde(rename = "identity.entitlements.read")]
	EntitlementsRead(IdentityEntitlementsReadRequestV2),
	#[serde(rename = "transaction.sign")]
	TransactionSign(TransactionSignRequestV2),
}

impl IdentityRequestV2 {
	pub(crate) const fn operation(&self) -> IdentityV2Operation {
		match self {
			Self::Account(_) => IdentityV2Operation::IdentityAccount,
			Self::ProfileRead(_) => IdentityV2Operation::IdentityProfileRead,
			Self::ProfileDisclose(_) => IdentityV2Operation::IdentityProfileDisclose,
			Self::HumanityStatus(_) => IdentityV2Operation::IdentityHumanityStatus,
			Self::HumanityProve(_) => IdentityV2Operation::IdentityHumanityProve,
			Self::SubjectDerive(_) => IdentityV2Operation::IdentitySubjectDerive,
			Self::EntitlementsRead(_) => IdentityV2Operation::IdentityEntitlementsRead,
			Self::TransactionSign(_) => IdentityV2Operation::TransactionSign,
		}
	}

	fn audience(&self) -> Option<&str> {
		match self {
			Self::ProfileDisclose(request) => Some(&request.audience),
			Self::HumanityProve(request) => Some(&request.audience),
			Self::SubjectDerive(request) => Some(&request.verifier_audience),
			_ => None,
		}
	}

	fn validate(&self) -> Result<(), IdentityV2Error> {
		match self {
			Self::Account(request) => text(&request.session, 128),
			Self::ProfileRead(request) => fields(&request.fields),
			Self::ProfileDisclose(request) => {
				text(&request.audience, 256)?;
				fields(&request.fields)?;
				text(&request.purpose, 256)
			},
			Self::HumanityStatus(_) => Ok(()),
			Self::HumanityProve(request) => {
				text(&request.audience, 256)?;
				if !(16..=64).contains(&request.challenge.len()) || request.claims.len() > 64 {
					return Err(IdentityV2Error::WireSchemaInvalid);
				}
				for claim in &request.claims {
					text(claim, 128)?;
				}
				Ok(())
			},
			Self::SubjectDerive(request) => {
				text(&request.product_id, 128)?;
				text(&request.context, 256)?;
				text(&request.verifier_audience, 256)
			},
			Self::EntitlementsRead(request) => text(&request.scope, 256),
			Self::TransactionSign(_) => Ok(()),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", content = "result")]
pub(crate) enum IdentityResultV2 {
	#[serde(rename = "identity.account")]
	Account(IdentityAccountResultV2),
	#[serde(rename = "identity.profile.read")]
	ProfileRead(IdentityProfileReadResultV2),
	#[serde(rename = "identity.profile.disclose")]
	ProfileDisclose(IdentityProfileDiscloseResultV2),
	#[serde(rename = "identity.humanity.status")]
	HumanityStatus(IdentityHumanityStatusResultV2),
	#[serde(rename = "identity.humanity.prove")]
	HumanityProve(IdentityHumanityProveResultV2),
	#[serde(rename = "identity.subject.derive")]
	SubjectDerive(IdentitySubjectDeriveResultV2),
	#[serde(rename = "identity.entitlements.read")]
	EntitlementsRead(IdentityEntitlementsReadResultV2),
	#[serde(rename = "transaction.sign")]
	TransactionSign(TransactionSignResultV2),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityGrantV2 {
	pub(crate) version: u8,
	pub(crate) id: [u8; 32],
	pub(crate) product_id: String,
	pub(crate) scope: IdentityV2Operation,
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) expires_at: u64,
	pub(crate) audience: Option<String>,
	pub(crate) revoked: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityInvocationV2 {
	pub(crate) protocol: String,
	pub(crate) code: u16,
	pub(crate) product_id: String,
	pub(crate) grant_id: [u8; 32],
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) request: IdentityRequestV2,
	pub(crate) operation_id: Option<[u8; 16]>,
}

pub(crate) fn prepare_identity_v2_invocation(
	product_id: &str,
	grant: &IdentityGrantV2,
	request: IdentityRequestV2,
	finalized_block: u64,
	current_recovery_incarnation: [u8; 32],
	operation_id: Option<[u8; 16]>,
) -> Result<IdentityInvocationV2, IdentityV2Error> {
	text(product_id, 128)?;
	request.validate()?;
	let operation = request.operation();
	if grant.version != 2 || grant.product_id != product_id || grant.scope != operation {
		return Err(IdentityV2Error::GrantScopeDenied);
	}
	if grant.revoked {
		return Err(IdentityV2Error::GrantRevoked);
	}
	if grant.expires_at <= finalized_block {
		return Err(IdentityV2Error::GrantExpired);
	}
	if grant.recovery_incarnation != current_recovery_incarnation {
		return Err(IdentityV2Error::OldIncarnation);
	}
	if let Some(audience) = request.audience() {
		if grant.audience.as_deref() != Some(audience) {
			return Err(IdentityV2Error::AudienceInvalid);
		}
	}
	if operation.requires_fresh_consent() != operation_id.is_some() {
		return Err(if operation.requires_fresh_consent() {
			IdentityV2Error::FreshConsentRequired
		} else {
			IdentityV2Error::WireSchemaInvalid
		});
	}
	Ok(IdentityInvocationV2 {
		protocol: "cord.origin.host/2".into(),
		code: operation as u16,
		product_id: product_id.into(),
		grant_id: grant.id,
		recovery_incarnation: grant.recovery_incarnation,
		request,
		operation_id,
	})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdentityRecoveryEvidenceV2 {
	pub(crate) same_store: bool,
	pub(crate) authenticated: bool,
	pub(crate) complete_replay_journal: bool,
	pub(crate) monotonic: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityRecoveryDispositionV2 {
	RestoreCompleteStore,
	InstallFreshRoot { epoch: u32 },
}

pub(crate) const fn identity_recovery_disposition_v2(
	evidence: IdentityRecoveryEvidenceV2,
) -> IdentityRecoveryDispositionV2 {
	if evidence.same_store &&
		evidence.authenticated &&
		evidence.complete_replay_journal &&
		evidence.monotonic
	{
		IdentityRecoveryDispositionV2::RestoreCompleteStore
	} else {
		IdentityRecoveryDispositionV2::InstallFreshRoot { epoch: 0 }
	}
}

#[derive(Default)]
pub(crate) struct FreshConsentJournalV2 {
	consumed: BTreeSet<[u8; 16]>,
}

impl FreshConsentJournalV2 {
	pub(crate) fn consume(&mut self, operation_id: [u8; 16]) -> Result<(), IdentityV2Error> {
		if !self.consumed.insert(operation_id) {
			return Err(IdentityV2Error::ChallengeReplay);
		}
		Ok(())
	}
}

fn fields(values: &[String]) -> Result<(), IdentityV2Error> {
	if values.is_empty() || values.len() > 64 {
		return Err(IdentityV2Error::WireSchemaInvalid);
	}
	for value in values {
		text(value, 128)?;
	}
	Ok(())
}

fn text(value: &str, maximum: usize) -> Result<(), IdentityV2Error> {
	if value.is_empty() || value.len() > maximum || value.nfc().collect::<String>() != value {
		return Err(IdentityV2Error::WireSchemaInvalid);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use serde_json::Value;

	use super::*;

	const OPERATIONS: &str =
		include_str!("../../../../docs/specs/origin-host-registry-v2.operations.json");
	const VECTORS: &str = include_str!("../../../../docs/specs/identity-v2.vectors.json");

	fn request(operation: IdentityV2Operation) -> IdentityRequestV2 {
		match operation {
			IdentityV2Operation::IdentityAccount =>
				IdentityRequestV2::Account(IdentityAccountRequestV2 { session: "selected".into() }),
			IdentityV2Operation::IdentityProfileRead =>
				IdentityRequestV2::ProfileRead(IdentityProfileReadRequestV2 {
					subject: [1; 32],
					fields: vec!["display".into()],
					at: None,
				}),
			IdentityV2Operation::IdentityProfileDisclose =>
				IdentityRequestV2::ProfileDisclose(IdentityProfileDiscloseRequestV2 {
					audience: "festival.example".into(),
					fields: vec!["email".into()],
					purpose: "ticket".into(),
					expires_at: 120,
				}),
			IdentityV2Operation::IdentityHumanityStatus =>
				IdentityRequestV2::HumanityStatus(IdentityHumanityStatusRequestV2 {
					subject: [2; 32],
					at: None,
				}),
			IdentityV2Operation::IdentityHumanityProve =>
				IdentityRequestV2::HumanityProve(IdentityHumanityProveRequestV2 {
					audience: "festival.example".into(),
					challenge: vec![3; 16],
					expires_at: 120,
					claims: vec!["adult".into()],
				}),
			IdentityV2Operation::IdentitySubjectDerive =>
				IdentityRequestV2::SubjectDerive(IdentitySubjectDeriveRequestV2 {
					product_id: "festival".into(),
					context: "attendee".into(),
					verifier_audience: "festival.example".into(),
					epoch: None,
				}),
			IdentityV2Operation::IdentityEntitlementsRead =>
				IdentityRequestV2::EntitlementsRead(IdentityEntitlementsReadRequestV2 {
					subject: [4; 32],
					scope: "festival.entry".into(),
					at: None,
				}),
			IdentityV2Operation::TransactionSign =>
				IdentityRequestV2::TransactionSign(TransactionSignRequestV2 {
					payload_hash: [5; 32],
					policy_hash: [6; 32],
					expires_at: 120,
				}),
		}
	}

	fn grant(operation: IdentityV2Operation) -> IdentityGrantV2 {
		IdentityGrantV2 {
			version: 2,
			id: [operation as u8; 32],
			product_id: "festival".into(),
			scope: operation,
			recovery_incarnation: [9; 32],
			expires_at: 200,
			audience: matches!(
				operation,
				IdentityV2Operation::IdentityProfileDisclose |
					IdentityV2Operation::IdentityHumanityProve |
					IdentityV2Operation::IdentitySubjectDerive
			)
			.then(|| "festival.example".into()),
			revoked: false,
		}
	}

	#[test]
	fn operation_codes_equal_the_frozen_registry() {
		let registry: Value = serde_json::from_str(OPERATIONS).unwrap();
		let rows = registry["operations"].as_array().unwrap();
		for operation in IdentityV2Operation::ALL {
			let row = rows.iter().find(|row| row["name"] == operation.name()).unwrap();
			assert_eq!(row["code"].as_u64(), Some(operation as u64));
			assert_eq!(row["grant_scope"], operation.name());
		}
	}

	#[test]
	fn grants_are_isolated_and_transaction_signing_is_separate() {
		for (index, operation) in IdentityV2Operation::ALL.iter().copied().enumerate() {
			let wrong = IdentityV2Operation::ALL[(index + 1) % IdentityV2Operation::ALL.len()];
			let operation_id = operation.requires_fresh_consent().then_some([7; 16]);
			assert_eq!(
				prepare_identity_v2_invocation(
					"festival",
					&grant(wrong),
					request(operation),
					100,
					[9; 32],
					operation_id,
				),
				Err(IdentityV2Error::GrantScopeDenied),
			);
			assert!(prepare_identity_v2_invocation(
				"festival",
				&grant(operation),
				request(operation),
				100,
				[9; 32],
				operation_id,
			)
			.is_ok());
		}
	}

	#[test]
	fn audience_and_recovery_incarnation_fail_closed() {
		let operation = IdentityV2Operation::IdentityHumanityProve;
		let mut wrong_audience = grant(operation);
		wrong_audience.audience = Some("other.example".into());
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&wrong_audience,
				request(operation),
				100,
				[9; 32],
				Some([7; 16]),
			),
			Err(IdentityV2Error::AudienceInvalid),
		);
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentitySubjectDerive),
				request(IdentityV2Operation::IdentitySubjectDerive),
				100,
				[8; 32],
				None,
			),
			Err(IdentityV2Error::OldIncarnation),
		);
	}

	#[test]
	fn replay_and_recovery_rules_match_the_frozen_vectors() {
		let vectors: Value = serde_json::from_str(VECTORS).unwrap();
		let proof = vectors["executable_vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|row| row["id"] == "subject-proof-v2")
			.unwrap();
		let replay = proof["negative_vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|row| row["id"] == "subject-proof-replay")
			.unwrap();
		assert_eq!(replay["expected_error"], IdentityV2Error::ChallengeReplay.to_string());
		let mut journal = FreshConsentJournalV2::default();
		assert_eq!(journal.consume([7; 16]), Ok(()));
		assert_eq!(journal.consume([7; 16]), Err(IdentityV2Error::ChallengeReplay));

		let recovery = vectors["recovery_vectors"].as_array().unwrap();
		let expected = |id: &str| {
			recovery.iter().find(|row| row["id"] == id).unwrap()["continuity"]
				.as_bool()
				.unwrap()
		};
		assert_eq!(
			identity_recovery_disposition_v2(IdentityRecoveryEvidenceV2 {
				same_store: true,
				authenticated: true,
				complete_replay_journal: true,
				monotonic: true,
			}),
			IdentityRecoveryDispositionV2::RestoreCompleteStore,
		);
		assert!(expected("same-store-restart"));
		for id in ["seed-only-backup", "stale-backup"] {
			assert!(!expected(id));
			assert_eq!(
				identity_recovery_disposition_v2(IdentityRecoveryEvidenceV2 {
					same_store: id == "stale-backup",
					authenticated: true,
					complete_replay_journal: false,
					monotonic: false,
				}),
				IdentityRecoveryDispositionV2::InstallFreshRoot { epoch: 0 },
			);
		}
	}

	#[test]
	fn result_shapes_reject_joined_authority_fields() {
		let joined = serde_json::json!({
			"operation": "identity.entitlements.read",
			"result": {
				"allowed": true,
				"scope": "festival.entry",
				"policy_version": 2,
				"expires_at": 120,
				"fresh_until": 110,
				"finalized": {"block_number": 100, "block_hash": vec![8; 32]},
				"profile": {"email": "hidden@example"}
			}
		});
		assert!(serde_json::from_value::<IdentityResultV2>(joined).is_err());
	}
}

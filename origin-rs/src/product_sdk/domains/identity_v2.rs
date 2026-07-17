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

use std::collections::{BTreeMap, BTreeSet};

use ciborium::value::Value as CborValue;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::product_sdk::host_v2::{
	codec::{CodecError as HostV2CodecError, Dto as HostV2Dto},
	generated::{
		IdentityAccountFrame, IdentityEntitlementsReadFrame, IdentityHumanityProveFrame,
		IdentityHumanityStatusFrame, IdentityProfileDiscloseFrame, IdentityProfileReadFrame,
		IdentitySubjectDeriveFrame, TransactionSignFrame,
	},
};

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

	pub(crate) const fn cddl(self) -> (&'static str, &'static str, &'static str) {
		match self {
			Self::IdentityAccount => {
				("IdentityAccountRequest", "IdentityAccountResult", "IdentityAccountError")
			},
			Self::IdentityProfileRead => (
				"IdentityProfileReadRequest",
				"IdentityProfileReadResult",
				"IdentityProfileReadError",
			),
			Self::IdentityProfileDisclose => (
				"IdentityProfileDiscloseRequest",
				"IdentityProfileDiscloseResult",
				"IdentityProfileDiscloseError",
			),
			Self::IdentityHumanityStatus => (
				"IdentityHumanityStatusRequest",
				"IdentityHumanityStatusResult",
				"IdentityHumanityStatusError",
			),
			Self::IdentityHumanityProve => (
				"IdentityHumanityProveRequest",
				"IdentityHumanityProveResult",
				"IdentityHumanityProveError",
			),
			Self::IdentitySubjectDerive => (
				"IdentitySubjectDeriveRequest",
				"IdentitySubjectDeriveResult",
				"IdentitySubjectDeriveError",
			),
			Self::IdentityEntitlementsRead => (
				"IdentityEntitlementsReadRequest",
				"IdentityEntitlementsReadResult",
				"IdentityEntitlementsReadError",
			),
			Self::TransactionSign => {
				("TransactionSignRequest", "TransactionSignResult", "TransactionSignError")
			},
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdentityV2ErrorTuple {
	pub(crate) code: u16,
	pub(crate) name: &'static str,
	pub(crate) retryable: bool,
}

pub(crate) const IDENTITY_V2_ERRORS: [IdentityV2ErrorTuple; 29] = [
	IdentityV2ErrorTuple { code: 100, name: "WIRE_SCHEMA_INVALID", retryable: false },
	IdentityV2ErrorTuple { code: 101, name: "WIRE_NON_CANONICAL", retryable: false },
	IdentityV2ErrorTuple { code: 102, name: "WIRE_VERSION_MISMATCH", retryable: false },
	IdentityV2ErrorTuple { code: 103, name: "WIRE_GENESIS_MISMATCH", retryable: false },
	IdentityV2ErrorTuple { code: 104, name: "WIRE_DESCRIPTOR_MISMATCH", retryable: false },
	IdentityV2ErrorTuple { code: 105, name: "WIRE_SEQUENCE_INVALID", retryable: false },
	IdentityV2ErrorTuple { code: 106, name: "REQUEST_DEADLINE_EXPIRED", retryable: false },
	IdentityV2ErrorTuple { code: 107, name: "REQUEST_CANCELLED", retryable: false },
	IdentityV2ErrorTuple { code: 108, name: "REQUEST_NOT_FOUND", retryable: false },
	IdentityV2ErrorTuple { code: 109, name: "GRANT_REQUIRED", retryable: false },
	IdentityV2ErrorTuple { code: 110, name: "GRANT_SCOPE_DENIED", retryable: false },
	IdentityV2ErrorTuple { code: 111, name: "GRANT_EXPIRED", retryable: false },
	IdentityV2ErrorTuple { code: 112, name: "GRANT_REVOKED", retryable: false },
	IdentityV2ErrorTuple { code: 113, name: "HOST_OUTBOX_UNAVAILABLE", retryable: false },
	IdentityV2ErrorTuple { code: 114, name: "HOST_OUTBOX_FULL", retryable: true },
	IdentityV2ErrorTuple { code: 115, name: "HOST_OUTBOX_CORRUPT", retryable: false },
	IdentityV2ErrorTuple { code: 116, name: "HOST_OUTBOX_EXPIRED", retryable: false },
	IdentityV2ErrorTuple { code: 400, name: "IDENTITY_AUDIENCE_INVALID", retryable: false },
	IdentityV2ErrorTuple { code: 401, name: "IDENTITY_CHALLENGE_REPLAY", retryable: false },
	IdentityV2ErrorTuple { code: 402, name: "IDENTITY_PROOF_EXPIRED", retryable: false },
	IdentityV2ErrorTuple { code: 403, name: "IDENTITY_EPOCH_INVALID", retryable: false },
	IdentityV2ErrorTuple { code: 404, name: "IDENTITY_DISCLOSURE_DENIED", retryable: false },
	IdentityV2ErrorTuple { code: 405, name: "IDENTITY_HUMANITY_UNAVAILABLE", retryable: true },
	IdentityV2ErrorTuple { code: 406, name: "IDENTITY_ENTITLEMENT_UNAVAILABLE", retryable: true },
	IdentityV2ErrorTuple { code: 407, name: "SIGNING_CONSENT_REQUIRED", retryable: false },
	IdentityV2ErrorTuple { code: 408, name: "IDENTITY_RECOVERY_ENTROPY_FAILED", retryable: false },
	IdentityV2ErrorTuple { code: 409, name: "IDENTITY_RECOVERY_INSTALL_FAILED", retryable: false },
	IdentityV2ErrorTuple { code: 410, name: "IDENTITY_OLD_INCARNATION", retryable: false },
	IdentityV2ErrorTuple { code: 411, name: "IDENTITY_RETIRED_SET_FULL", retryable: false },
];

pub(crate) const fn identity_v2_errors_for(
	_operation: IdentityV2Operation,
) -> &'static [IdentityV2ErrorTuple; 29] {
	&IDENTITY_V2_ERRORS
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum IdentityV2Error {
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid,
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical,
	#[error("REQUEST_DEADLINE_EXPIRED")]
	DeadlineExpired,
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
	#[error("IDENTITY_PROOF_EXPIRED")]
	ProofExpired,
	#[error("IDENTITY_OLD_INCARNATION")]
	OldIncarnation,
	#[error("SIGNING_CONSENT_REQUIRED")]
	FreshConsentRequired,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityV2ErrorDetails {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) message: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) lower: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) upper: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityV2ErrorEnvelope {
	pub(crate) code: u16,
	pub(crate) name: String,
	pub(crate) retryable: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) details: Option<IdentityV2ErrorDetails>,
}

impl IdentityV2ErrorEnvelope {
	pub(crate) fn validate_for(
		&self,
		operation: IdentityV2Operation,
	) -> Result<(), IdentityV2Error> {
		let frozen = identity_v2_errors_for(operation).iter().find(|error| error.code == self.code);
		if !frozen.is_some_and(|error| error.name == self.name && error.retryable == self.retryable)
		{
			return Err(IdentityV2Error::WireSchemaInvalid);
		}
		if self
			.details
			.as_ref()
			.and_then(|details| details.message.as_ref())
			.is_some_and(|message| text(message, 256).is_err())
		{
			return Err(IdentityV2Error::WireSchemaInvalid);
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FinalizedIdentityV2 {
	pub(crate) block_number: u64,
	pub(crate) block_hash: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityReceiptV2 {
	pub(crate) commitment: [u8; 32],
	pub(crate) valid_until: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) finalized: Option<FinalizedIdentityV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityAccountRequestV2 {
	pub(crate) session: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityAccountResultV2 {
	pub(crate) account: [u8; 32],
	pub(crate) session_expires_at: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityProfileReadRequestV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) fields: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityProfileReadResultV2 {
	pub(crate) receipt: IdentityReceiptV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityProfileDiscloseRequestV2 {
	pub(crate) audience: String,
	pub(crate) fields: Vec<String>,
	pub(crate) purpose: String,
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityProfileDiscloseResultV2 {
	pub(crate) receipt: IdentityReceiptV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityHumanityStatusRequestV2 {
	pub(crate) subject: [u8; 32],
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityHumanityStatusResultV2 {
	pub(crate) status: u16,
	pub(crate) fresh_until: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityHumanityProveRequestV2 {
	pub(crate) audience: String,
	pub(crate) challenge: Vec<u8>,
	pub(crate) expires_at: u64,
	pub(crate) claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityHumanityProveResultV2 {
	pub(crate) proof: Vec<u8>,
	pub(crate) derived_public_key: [u8; 32],
	pub(crate) proof_hash: [u8; 32],
	pub(crate) continuity: bool,
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentitySubjectDeriveRequestV2 {
	pub(crate) product_id: String,
	pub(crate) context: String,
	pub(crate) verifier_audience: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) epoch: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentitySubjectDeriveResultV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) derived_public_key: [u8; 32],
	pub(crate) epoch: u32,
	pub(crate) recovery_incarnation_hash: [u8; 32],
	pub(crate) continuity: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityEntitlementsReadRequestV2 {
	pub(crate) subject: [u8; 32],
	pub(crate) scope: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) at: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityEntitlementsReadResultV2 {
	pub(crate) allowed: bool,
	pub(crate) scope: String,
	pub(crate) policy_version: u32,
	pub(crate) expires_at: u64,
	pub(crate) fresh_until: u64,
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TransactionSignRequestV2 {
	pub(crate) payload_hash: [u8; 32],
	pub(crate) policy_hash: [u8; 32],
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TransactionSignResultV2 {
	pub(crate) transaction_hash: [u8; 32],
	pub(crate) finalized: FinalizedIdentityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
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

	fn wire_value(&self) -> CborValue {
		match self {
			Self::Account(request) => cbor_map(vec![(0, CborValue::Text(request.session.clone()))]),
			Self::ProfileRead(request) => {
				let mut fields = vec![
					(0, CborValue::Bytes(request.subject.to_vec())),
					(
						1,
						CborValue::Array(
							request.fields.iter().cloned().map(CborValue::Text).collect(),
						),
					),
				];
				if let Some(at) = request.at {
					fields.push((2, CborValue::Bytes(at.to_vec())));
				}
				cbor_map(fields)
			},
			Self::ProfileDisclose(request) => cbor_map(vec![
				(0, CborValue::Text(request.audience.clone())),
				(
					1,
					CborValue::Array(request.fields.iter().cloned().map(CborValue::Text).collect()),
				),
				(2, CborValue::Text(request.purpose.clone())),
				(3, cbor_uint(request.expires_at)),
			]),
			Self::HumanityStatus(request) => {
				let mut fields = vec![(0, CborValue::Bytes(request.subject.to_vec()))];
				if let Some(at) = request.at {
					fields.push((1, CborValue::Bytes(at.to_vec())));
				}
				cbor_map(fields)
			},
			Self::HumanityProve(request) => cbor_map(vec![
				(0, CborValue::Text(request.audience.clone())),
				(1, CborValue::Bytes(request.challenge.clone())),
				(2, cbor_uint(request.expires_at)),
				(
					3,
					CborValue::Array(request.claims.iter().cloned().map(CborValue::Text).collect()),
				),
			]),
			Self::SubjectDerive(request) => {
				let mut fields = vec![
					(0, CborValue::Text(request.product_id.clone())),
					(1, CborValue::Text(request.context.clone())),
					(2, CborValue::Text(request.verifier_audience.clone())),
				];
				if let Some(epoch) = request.epoch {
					fields.push((3, cbor_uint(epoch.into())));
				}
				cbor_map(fields)
			},
			Self::EntitlementsRead(request) => {
				let mut fields = vec![
					(0, CborValue::Bytes(request.subject.to_vec())),
					(1, CborValue::Text(request.scope.clone())),
				];
				if let Some(at) = request.at {
					fields.push((2, CborValue::Bytes(at.to_vec())));
				}
				cbor_map(fields)
			},
			Self::TransactionSign(request) => cbor_map(vec![
				(0, CborValue::Bytes(request.payload_hash.to_vec())),
				(1, CborValue::Bytes(request.policy_hash.to_vec())),
				(2, cbor_uint(request.expires_at)),
			]),
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

impl IdentityResultV2 {
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

	fn validate_shape_for(&self, expected: IdentityV2Operation) -> Result<(), IdentityV2Error> {
		if self.operation() != expected {
			return Err(IdentityV2Error::WireSchemaInvalid);
		}
		match self {
			Self::HumanityProve(result) if !(64..=4096).contains(&result.proof.len()) => {
				Err(IdentityV2Error::WireSchemaInvalid)
			},
			Self::EntitlementsRead(result) => text(&result.scope, 256),
			_ => Ok(()),
		}
	}

	pub(crate) fn validate_for_request(
		&self,
		request: &IdentityRequestV2,
		finalized_block: u64,
		finalized_hash: [u8; 32],
	) -> Result<(), IdentityV2Error> {
		self.validate_shape_for(request.operation())?;
		match (self, request) {
			(Self::Account(result), IdentityRequestV2::Account(_)) => {
				let context = validate_result_finality(
					&result.finalized,
					finalized_block,
					finalized_hash,
					None,
				)?;
				if result.session_expires_at <= context {
					Err(IdentityV2Error::WireSchemaInvalid)
				} else {
					Ok(())
				}
			},
			(Self::ProfileRead(result), IdentityRequestV2::ProfileRead(request)) => {
				validate_profile_receipt(
					&result.receipt,
					finalized_block,
					finalized_hash,
					request.at,
				)
			},
			(Self::ProfileDisclose(result), IdentityRequestV2::ProfileDisclose(request)) => {
				validate_profile_receipt(&result.receipt, finalized_block, finalized_hash, None)?;
				if result.receipt.valid_until > request.expires_at {
					Err(IdentityV2Error::WireSchemaInvalid)
				} else {
					Ok(())
				}
			},
			(Self::HumanityStatus(result), IdentityRequestV2::HumanityStatus(request)) => {
				let context = validate_result_finality(
					&result.finalized,
					finalized_block,
					finalized_hash,
					request.at,
				)?;
				if result.fresh_until <= context {
					Err(IdentityV2Error::WireSchemaInvalid)
				} else {
					Ok(())
				}
			},
			(Self::HumanityProve(result), IdentityRequestV2::HumanityProve(request)) => {
				if result.expires_at <= finalized_block || result.expires_at > request.expires_at {
					Err(IdentityV2Error::ProofExpired)
				} else {
					Ok(())
				}
			},
			(Self::EntitlementsRead(result), IdentityRequestV2::EntitlementsRead(request)) => {
				let context = validate_result_finality(
					&result.finalized,
					finalized_block,
					finalized_hash,
					request.at,
				)?;
				if result.scope != request.scope
					|| result.expires_at <= context
					|| result.fresh_until <= context
					|| result.fresh_until > result.expires_at
				{
					Err(IdentityV2Error::WireSchemaInvalid)
				} else {
					Ok(())
				}
			},
			(Self::TransactionSign(result), IdentityRequestV2::TransactionSign(_)) => {
				validate_result_finality(&result.finalized, finalized_block, finalized_hash, None)
					.map(|_| ())
			},
			_ => Ok(()),
		}
	}
}

fn validate_result_finality(
	finalized: &FinalizedIdentityV2,
	invocation_finalized_block: u64,
	invocation_finalized_hash: [u8; 32],
	requested_hash: Option<[u8; 32]>,
) -> Result<u64, IdentityV2Error> {
	if let Some(hash) = requested_hash {
		if finalized.block_hash != hash || finalized.block_number > invocation_finalized_block {
			return Err(IdentityV2Error::WireSchemaInvalid);
		}
	} else if finalized.block_number != invocation_finalized_block
		|| finalized.block_hash != invocation_finalized_hash
	{
		return Err(IdentityV2Error::WireSchemaInvalid);
	}
	Ok(finalized.block_number)
}

fn validate_profile_receipt(
	receipt: &IdentityReceiptV2,
	finalized_block: u64,
	finalized_hash: [u8; 32],
	requested_hash: Option<[u8; 32]>,
) -> Result<(), IdentityV2Error> {
	let Some(finalized) = &receipt.finalized else {
		return Err(IdentityV2Error::WireSchemaInvalid);
	};
	let context =
		validate_result_finality(finalized, finalized_block, finalized_hash, requested_hash)?;
	if receipt.valid_until <= context {
		return Err(IdentityV2Error::WireSchemaInvalid);
	}
	Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityGrantCoreV2 {
	pub(crate) version: u8,
	pub(crate) id: [u8; 32],
	pub(crate) product_id: String,
	pub(crate) scope: IdentityV2Operation,
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) expires_at: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) audience: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) revoked: Option<bool>,
}

pub(crate) trait IdentityGrantV2 {
	fn operation(&self) -> IdentityV2Operation;
	fn core(&self) -> &IdentityGrantCoreV2;
}

impl<T: IdentityGrantV2 + ?Sized> IdentityGrantV2 for Box<T> {
	fn operation(&self) -> IdentityV2Operation {
		(**self).operation()
	}
	fn core(&self) -> &IdentityGrantCoreV2 {
		(**self).core()
	}
}

macro_rules! identity_grant {
	($name:ident, $operation:expr) => {
		#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
		#[serde(transparent)]
		pub(crate) struct $name(pub(crate) IdentityGrantCoreV2);

		impl IdentityGrantV2 for $name {
			fn operation(&self) -> IdentityV2Operation {
				$operation
			}
			fn core(&self) -> &IdentityGrantCoreV2 {
				&self.0
			}
		}
	};
}

identity_grant!(IdentityAccountGrantV2, IdentityV2Operation::IdentityAccount);
identity_grant!(IdentityProfileReadGrantV2, IdentityV2Operation::IdentityProfileRead);
identity_grant!(IdentityProfileDiscloseGrantV2, IdentityV2Operation::IdentityProfileDisclose);
identity_grant!(IdentityHumanityStatusGrantV2, IdentityV2Operation::IdentityHumanityStatus);
identity_grant!(IdentityHumanityProveGrantV2, IdentityV2Operation::IdentityHumanityProve);
identity_grant!(IdentitySubjectDeriveGrantV2, IdentityV2Operation::IdentitySubjectDerive);
identity_grant!(IdentityEntitlementsReadGrantV2, IdentityV2Operation::IdentityEntitlementsRead);
identity_grant!(TransactionSignGrantV2, IdentityV2Operation::TransactionSign);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityInvocationV2 {
	pub(crate) protocol: String,
	pub(crate) code: u16,
	pub(crate) request_id: [u8; 16],
	pub(crate) product_id: String,
	pub(crate) grant_id: [u8; 32],
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) deadline_block: u64,
	#[serde(skip, default)]
	finalized_block: u64,
	#[serde(skip, default)]
	finalized_hash: [u8; 32],
	pub(crate) operation: IdentityV2Operation,
	pub(crate) input: IdentityRequestV2,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub(crate) operation_id: Option<[u8; 16]>,
	#[serde(skip, default)]
	replay_commit: FreshConsentReplayV2,
}

impl IdentityInvocationV2 {
	pub(crate) fn validate_result(&self, result: &IdentityResultV2) -> Result<(), IdentityV2Error> {
		result.validate_for_request(&self.input, self.finalized_block, self.finalized_hash)
	}

	pub(crate) fn commit_durable_acceptance(
		&self,
		replay_journal: &mut FreshConsentJournalV2,
	) -> Result<(), IdentityV2Error> {
		replay_journal.commit(&self.replay_commit)
	}

	pub(crate) fn rollback_pre_accept(&self, replay_journal: &mut FreshConsentJournalV2) {
		replay_journal.rollback(&self.replay_commit);
	}

	fn frame_value(&self) -> CborValue {
		let mut fields = vec![
			(0, cbor_uint(2)),
			(1, CborValue::Bytes(self.request_id.to_vec())),
			(2, CborValue::Text(self.product_id.clone())),
			(3, cbor_uint(self.code.into())),
			(4, CborValue::Bytes(self.grant_id.to_vec())),
		];
		if let Some(operation_id) = self.operation_id {
			fields.push((5, CborValue::Bytes(operation_id.to_vec())));
		}
		fields.extend([(7, cbor_uint(self.deadline_block)), (8, self.input.wire_value())]);
		cbor_map(fields)
	}

	pub(crate) fn canonical_frame(&self) -> Result<Vec<u8>, IdentityV2Error> {
		let value = self.frame_value();
		macro_rules! encode {
			($production:ty) => {
				HostV2Dto::<$production>::from_value(value)
					.map(|dto| dto.canonical().to_vec())
					.map_err(map_host_codec_error)
			};
		}
		match self.operation {
			IdentityV2Operation::IdentityAccount => encode!(IdentityAccountFrame),
			IdentityV2Operation::IdentityProfileRead => encode!(IdentityProfileReadFrame),
			IdentityV2Operation::IdentityProfileDisclose => encode!(IdentityProfileDiscloseFrame),
			IdentityV2Operation::IdentityHumanityStatus => encode!(IdentityHumanityStatusFrame),
			IdentityV2Operation::IdentityHumanityProve => encode!(IdentityHumanityProveFrame),
			IdentityV2Operation::IdentitySubjectDerive => encode!(IdentitySubjectDeriveFrame),
			IdentityV2Operation::IdentityEntitlementsRead => encode!(IdentityEntitlementsReadFrame),
			IdentityV2Operation::TransactionSign => encode!(TransactionSignFrame),
		}
	}

	pub(crate) fn decode_canonical_frame(
		operation: IdentityV2Operation,
		bytes: &[u8],
	) -> Result<Vec<u8>, IdentityV2Error> {
		macro_rules! decode {
			($production:ty) => {
				HostV2Dto::<$production>::decode(bytes)
					.map(|dto| dto.canonical().to_vec())
					.map_err(map_host_codec_error)
			};
		}
		match operation {
			IdentityV2Operation::IdentityAccount => decode!(IdentityAccountFrame),
			IdentityV2Operation::IdentityProfileRead => decode!(IdentityProfileReadFrame),
			IdentityV2Operation::IdentityProfileDisclose => decode!(IdentityProfileDiscloseFrame),
			IdentityV2Operation::IdentityHumanityStatus => decode!(IdentityHumanityStatusFrame),
			IdentityV2Operation::IdentityHumanityProve => decode!(IdentityHumanityProveFrame),
			IdentityV2Operation::IdentitySubjectDerive => decode!(IdentitySubjectDeriveFrame),
			IdentityV2Operation::IdentityEntitlementsRead => decode!(IdentityEntitlementsReadFrame),
			IdentityV2Operation::TransactionSign => decode!(TransactionSignFrame),
		}
	}
}

fn map_host_codec_error(error: HostV2CodecError) -> IdentityV2Error {
	match error {
		HostV2CodecError::Schema(_) => IdentityV2Error::WireSchemaInvalid,
		HostV2CodecError::NonCanonical(_) => IdentityV2Error::WireNonCanonical,
	}
}

pub(crate) fn prepare_identity_v2_invocation(
	product_id: &str,
	grant: &dyn IdentityGrantV2,
	request: IdentityRequestV2,
	request_id: [u8; 16],
	deadline_block: u64,
	finalized_block: u64,
	finalized_hash: [u8; 32],
	current_recovery_incarnation: [u8; 32],
	operation_id: Option<[u8; 16]>,
	replay_journal: &mut FreshConsentJournalV2,
) -> Result<IdentityInvocationV2, IdentityV2Error> {
	text(product_id, 128)?;
	request.validate()?;
	let operation = request.operation();
	let grant_core = grant.core();
	if grant.operation() != operation
		|| grant_core.version != 2
		|| grant_core.product_id != product_id
		|| grant_core.scope != operation
	{
		return Err(IdentityV2Error::GrantScopeDenied);
	}
	if grant_core.revoked == Some(true) {
		return Err(IdentityV2Error::GrantRevoked);
	}
	if grant_core.expires_at <= finalized_block {
		return Err(IdentityV2Error::GrantExpired);
	}
	if grant_core.recovery_incarnation != current_recovery_incarnation {
		return Err(IdentityV2Error::OldIncarnation);
	}
	if deadline_block <= finalized_block {
		return Err(IdentityV2Error::DeadlineExpired);
	}
	if let IdentityRequestV2::SubjectDerive(subject) = &request {
		if subject.product_id != product_id {
			return Err(IdentityV2Error::GrantScopeDenied);
		}
	}
	if let Some(audience) = request.audience() {
		if grant_core.audience.as_deref() != Some(audience) {
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
	if let IdentityRequestV2::HumanityProve(proof) = &request {
		if proof.expires_at <= finalized_block {
			return Err(IdentityV2Error::ProofExpired);
		}
	}
	if let IdentityRequestV2::TransactionSign(signing) = &request {
		if signing.expires_at <= finalized_block {
			return Err(IdentityV2Error::DeadlineExpired);
		}
	}
	let replay_commit = replay_journal.reserve(&request, operation_id)?;
	Ok(IdentityInvocationV2 {
		protocol: "cord.origin.host/2".into(),
		code: operation as u16,
		request_id,
		product_id: product_id.into(),
		grant_id: grant_core.id,
		recovery_incarnation: grant_core.recovery_incarnation,
		deadline_block,
		finalized_block,
		finalized_hash,
		operation,
		input: request,
		operation_id,
		replay_commit,
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
	if evidence.same_store
		&& evidence.authenticated
		&& evidence.complete_replay_journal
		&& evidence.monotonic
	{
		IdentityRecoveryDispositionV2::RestoreCompleteStore
	} else {
		IdentityRecoveryDispositionV2::InstallFreshRoot { epoch: 0 }
	}
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FreshConsentReplayV2 {
	reservation_id: Option<u64>,
	operation_id: Option<[u8; 16]>,
	proof_challenge: Option<Vec<u8>>,
}

#[derive(Default)]
pub(crate) struct FreshConsentJournalV2 {
	operation_ids: BTreeSet<[u8; 16]>,
	proof_challenges: BTreeSet<Vec<u8>>,
	reserved_operation_ids: BTreeMap<[u8; 16], u64>,
	reserved_proof_challenges: BTreeMap<Vec<u8>, u64>,
	next_reservation_id: u64,
}

impl FreshConsentJournalV2 {
	fn reserve(
		&mut self,
		request: &IdentityRequestV2,
		operation_id: Option<[u8; 16]>,
	) -> Result<FreshConsentReplayV2, IdentityV2Error> {
		if let Some(operation_id) = operation_id {
			if self.operation_ids.contains(&operation_id)
				|| self.reserved_operation_ids.contains_key(&operation_id)
			{
				return Err(IdentityV2Error::ChallengeReplay);
			}
		}
		let proof_challenge = if let IdentityRequestV2::HumanityProve(proof) = request {
			if self.proof_challenges.contains(&proof.challenge)
				|| self.reserved_proof_challenges.contains_key(&proof.challenge)
			{
				return Err(IdentityV2Error::ChallengeReplay);
			}
			Some(proof.challenge.clone())
		} else {
			None
		};
		if operation_id.is_none() && proof_challenge.is_none() {
			return Ok(FreshConsentReplayV2::default());
		}
		self.next_reservation_id = self
			.next_reservation_id
			.checked_add(1)
			.ok_or(IdentityV2Error::ChallengeReplay)?;
		let reservation_id = self.next_reservation_id;
		if let Some(operation_id) = operation_id {
			self.reserved_operation_ids.insert(operation_id, reservation_id);
		}
		if let Some(challenge) = &proof_challenge {
			self.reserved_proof_challenges.insert(challenge.clone(), reservation_id);
		}
		Ok(FreshConsentReplayV2 {
			reservation_id: Some(reservation_id),
			operation_id,
			proof_challenge,
		})
	}

	fn commit(&mut self, accepted: &FreshConsentReplayV2) -> Result<(), IdentityV2Error> {
		let reservation_id = accepted.reservation_id;
		if accepted.operation_id.is_some_and(|operation_id| {
			self.operation_ids.contains(&operation_id)
				|| self.reserved_operation_ids.get(&operation_id).copied() != reservation_id
		}) || accepted.proof_challenge.as_ref().is_some_and(|challenge| {
			self.proof_challenges.contains(challenge)
				|| self.reserved_proof_challenges.get(challenge).copied() != reservation_id
		}) {
			return Err(IdentityV2Error::ChallengeReplay);
		}
		if let Some(operation_id) = accepted.operation_id {
			self.reserved_operation_ids.remove(&operation_id);
			self.operation_ids.insert(operation_id);
		}
		if let Some(challenge) = &accepted.proof_challenge {
			self.reserved_proof_challenges.remove(challenge);
			self.proof_challenges.insert(challenge.clone());
		}
		Ok(())
	}

	fn rollback(&mut self, accepted: &FreshConsentReplayV2) {
		if let (Some(operation_id), Some(reservation_id)) =
			(accepted.operation_id, accepted.reservation_id)
		{
			if self.reserved_operation_ids.get(&operation_id) == Some(&reservation_id) {
				self.reserved_operation_ids.remove(&operation_id);
			}
		}
		if let (Some(challenge), Some(reservation_id)) =
			(&accepted.proof_challenge, accepted.reservation_id)
		{
			if self.reserved_proof_challenges.get(challenge) == Some(&reservation_id) {
				self.reserved_proof_challenges.remove(challenge);
			}
		}
	}
}

fn cbor_uint(value: u64) -> CborValue {
	CborValue::Integer(value.into())
}

fn cbor_map(fields: Vec<(u64, CborValue)>) -> CborValue {
	CborValue::Map(
		fields
			.into_iter()
			.map(|(key, value)| (CborValue::Integer(key.into()), value))
			.collect(),
	)
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
	use sha2::{Digest, Sha256};

	use super::*;

	const OPERATIONS: &str =
		include_str!("../../../../docs/specs/origin-host-registry-v2.operations.json");
	const VECTORS: &str = include_str!("../../../../docs/specs/identity-v2.vectors.json");
	const HOST_VECTORS: &str =
		include_str!("../../../../docs/specs/origin-host-registry-v2.vectors.json");

	fn request(operation: IdentityV2Operation) -> IdentityRequestV2 {
		match operation {
			IdentityV2Operation::IdentityAccount => {
				IdentityRequestV2::Account(IdentityAccountRequestV2 { session: "selected".into() })
			},
			IdentityV2Operation::IdentityProfileRead => {
				IdentityRequestV2::ProfileRead(IdentityProfileReadRequestV2 {
					subject: [1; 32],
					fields: vec!["display".into()],
					at: None,
				})
			},
			IdentityV2Operation::IdentityProfileDisclose => {
				IdentityRequestV2::ProfileDisclose(IdentityProfileDiscloseRequestV2 {
					audience: "festival.example".into(),
					fields: vec!["email".into()],
					purpose: "ticket".into(),
					expires_at: 120,
				})
			},
			IdentityV2Operation::IdentityHumanityStatus => {
				IdentityRequestV2::HumanityStatus(IdentityHumanityStatusRequestV2 {
					subject: [2; 32],
					at: None,
				})
			},
			IdentityV2Operation::IdentityHumanityProve => {
				IdentityRequestV2::HumanityProve(IdentityHumanityProveRequestV2 {
					audience: "festival.example".into(),
					challenge: vec![3; 16],
					expires_at: 120,
					claims: vec!["adult".into()],
				})
			},
			IdentityV2Operation::IdentitySubjectDerive => {
				IdentityRequestV2::SubjectDerive(IdentitySubjectDeriveRequestV2 {
					product_id: "festival".into(),
					context: "attendee".into(),
					verifier_audience: "festival.example".into(),
					epoch: None,
				})
			},
			IdentityV2Operation::IdentityEntitlementsRead => {
				IdentityRequestV2::EntitlementsRead(IdentityEntitlementsReadRequestV2 {
					subject: [4; 32],
					scope: "festival.entry".into(),
					at: None,
				})
			},
			IdentityV2Operation::TransactionSign => {
				IdentityRequestV2::TransactionSign(TransactionSignRequestV2 {
					payload_hash: [5; 32],
					policy_hash: [6; 32],
					expires_at: 120,
				})
			},
		}
	}

	fn frozen_request(operation: IdentityV2Operation) -> IdentityRequestV2 {
		match operation {
			IdentityV2Operation::IdentityAccount => {
				IdentityRequestV2::Account(IdentityAccountRequestV2 { session: "a".into() })
			},
			IdentityV2Operation::IdentityProfileRead => {
				IdentityRequestV2::ProfileRead(IdentityProfileReadRequestV2 {
					subject: [0x22; 32],
					fields: vec!["a".into()],
					at: None,
				})
			},
			IdentityV2Operation::IdentityProfileDisclose => {
				IdentityRequestV2::ProfileDisclose(IdentityProfileDiscloseRequestV2 {
					audience: "a".into(),
					fields: vec!["a".into()],
					purpose: "a".into(),
					expires_at: 1,
				})
			},
			IdentityV2Operation::IdentityHumanityStatus => {
				IdentityRequestV2::HumanityStatus(IdentityHumanityStatusRequestV2 {
					subject: [0x22; 32],
					at: None,
				})
			},
			IdentityV2Operation::IdentityHumanityProve => {
				IdentityRequestV2::HumanityProve(IdentityHumanityProveRequestV2 {
					audience: "a".into(),
					challenge: vec![0x33; 16],
					expires_at: 1,
					claims: vec![],
				})
			},
			IdentityV2Operation::IdentitySubjectDerive => {
				IdentityRequestV2::SubjectDerive(IdentitySubjectDeriveRequestV2 {
					product_id: "a".into(),
					context: "a".into(),
					verifier_audience: "a".into(),
					epoch: None,
				})
			},
			IdentityV2Operation::IdentityEntitlementsRead => {
				IdentityRequestV2::EntitlementsRead(IdentityEntitlementsReadRequestV2 {
					subject: [0x22; 32],
					scope: "a".into(),
					at: None,
				})
			},
			IdentityV2Operation::TransactionSign => {
				IdentityRequestV2::TransactionSign(TransactionSignRequestV2 {
					payload_hash: [0x22; 32],
					policy_hash: [0x22; 32],
					expires_at: 1,
				})
			},
		}
	}

	fn grant_core(operation: IdentityV2Operation) -> IdentityGrantCoreV2 {
		IdentityGrantCoreV2 {
			version: 2,
			id: [operation as u8; 32],
			product_id: "festival".into(),
			scope: operation,
			recovery_incarnation: [9; 32],
			expires_at: 200,
			audience: matches!(
				operation,
				IdentityV2Operation::IdentityProfileDisclose
					| IdentityV2Operation::IdentityHumanityProve
					| IdentityV2Operation::IdentitySubjectDerive
			)
			.then(|| "festival.example".into()),
			revoked: Some(false),
		}
	}

	fn grant(operation: IdentityV2Operation) -> Box<dyn IdentityGrantV2> {
		let core = grant_core(operation);
		match operation {
			IdentityV2Operation::IdentityAccount => Box::new(IdentityAccountGrantV2(core)),
			IdentityV2Operation::IdentityProfileRead => Box::new(IdentityProfileReadGrantV2(core)),
			IdentityV2Operation::IdentityProfileDisclose => {
				Box::new(IdentityProfileDiscloseGrantV2(core))
			},
			IdentityV2Operation::IdentityHumanityStatus => {
				Box::new(IdentityHumanityStatusGrantV2(core))
			},
			IdentityV2Operation::IdentityHumanityProve => {
				Box::new(IdentityHumanityProveGrantV2(core))
			},
			IdentityV2Operation::IdentitySubjectDerive => {
				Box::new(IdentitySubjectDeriveGrantV2(core))
			},
			IdentityV2Operation::IdentityEntitlementsRead => {
				Box::new(IdentityEntitlementsReadGrantV2(core))
			},
			IdentityV2Operation::TransactionSign => Box::new(TransactionSignGrantV2(core)),
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
			assert_eq!(row["operation_id_required"], operation.requires_fresh_consent());
			assert_eq!(
				row["consent_mode"],
				if operation.requires_fresh_consent() { "fresh-user-consent" } else { "grant" },
			);
			let (request, result, error) = operation.cddl();
			assert_eq!(row["cddl"]["Request"], request);
			assert_eq!(row["cddl"]["Result"], result);
			assert_eq!(row["cddl"]["Error"], error);
			let allowed = row["allowed_errors"].as_array().unwrap();
			assert_eq!(allowed.len(), 29);
			for (actual, frozen) in allowed.iter().zip(identity_v2_errors_for(operation)) {
				assert_eq!(actual["code"].as_u64(), Some(frozen.code.into()));
				assert_eq!(actual["name"], frozen.name);
				assert_eq!(actual["retryable"], frozen.retryable);
			}
		}
	}

	#[test]
	fn semantic_identity_dtos_round_trip_every_generated_host_frame() {
		let fixture: Value = serde_json::from_str(HOST_VECTORS).unwrap();
		let vectors = fixture["vectors"].as_array().unwrap();
		for operation in IdentityV2Operation::ALL {
			let id = format!("{}-positive", operation as u16);
			let vector = vectors.iter().find(|vector| vector["id"] == id).unwrap();
			assert_eq!(vector["operation"], operation.name());
			assert_eq!(vector["cddl"]["Request"], operation.cddl().0);
			let wire = hex::decode(vector["wire_hex"].as_str().unwrap()).unwrap();
			assert_eq!(hex::encode(Sha256::digest(&wire)), vector["wire_sha256"]);
			let invocation = IdentityInvocationV2 {
				protocol: "cord.origin.host/2".into(),
				code: operation as u16,
				request_id: [0x11; 16],
				product_id: "festival".into(),
				grant_id: [0x22; 32],
				recovery_incarnation: [0x44; 32],
				deadline_block: 100,
				finalized_block: 100,
				finalized_hash: [8; 32],
				operation,
				input: frozen_request(operation),
				operation_id: operation.requires_fresh_consent().then_some([0x33; 16]),
				replay_commit: FreshConsentReplayV2::default(),
			};
			assert_eq!(invocation.request_id, [0x11; 16]);
			assert_eq!(invocation.deadline_block, 100);
			assert_eq!(invocation.canonical_frame().unwrap(), wire, "{}", operation.name());
			assert_eq!(
				IdentityInvocationV2::decode_canonical_frame(operation, &wire).unwrap(),
				wire
			);
			let mut noncanonical = vec![0xb8, wire[0] & 0x1f];
			noncanonical.extend_from_slice(&wire[1..]);
			assert_eq!(
				IdentityInvocationV2::decode_canonical_frame(operation, &noncanonical),
				Err(IdentityV2Error::WireNonCanonical),
			);
		}
	}

	#[test]
	fn unified_identity_keeps_seven_grants_and_separate_signing() {
		let identity_operations = IdentityV2Operation::ALL
			.into_iter()
			.filter(|operation| operation.name().starts_with("identity."))
			.count();
		assert_eq!(identity_operations, 7);
		assert_eq!(IdentityV2Operation::ALL.len(), 8);
		assert_eq!(IdentityV2Operation::TransactionSign.name(), "transaction.sign");
	}

	#[test]
	fn grants_are_isolated_and_transaction_signing_is_separate() {
		for (index, operation) in IdentityV2Operation::ALL.iter().copied().enumerate() {
			let mut journal = FreshConsentJournalV2::default();
			let wrong = IdentityV2Operation::ALL[(index + 1) % IdentityV2Operation::ALL.len()];
			let operation_id = operation.requires_fresh_consent().then_some([7; 16]);
			assert_eq!(
				prepare_identity_v2_invocation(
					"festival",
					&grant(wrong),
					request(operation),
					[0x11; 16],
					150,
					100,
					[8; 32],
					[9; 32],
					operation_id,
					&mut journal,
				),
				Err(IdentityV2Error::GrantScopeDenied),
			);
			assert!(prepare_identity_v2_invocation(
				"festival",
				&grant(operation),
				request(operation),
				[0x11; 16],
				150,
				100,
				[8; 32],
				[9; 32],
				operation_id,
				&mut journal,
			)
			.is_ok());
		}
	}

	#[test]
	fn invocation_json_matches_the_typescript_camel_case_envelope() {
		let mut journal = FreshConsentJournalV2::default();
		let invocation = prepare_identity_v2_invocation(
			"festival",
			&grant(IdentityV2Operation::IdentityAccount),
			request(IdentityV2Operation::IdentityAccount),
			[0x11; 16],
			150,
			100,
			[8; 32],
			[9; 32],
			None,
			&mut journal,
		)
		.unwrap();
		let value = serde_json::to_value(&invocation).unwrap();
		let keys = value.as_object().unwrap().keys().cloned().collect::<BTreeSet<_>>();
		assert_eq!(
			keys,
			[
				"protocol",
				"code",
				"requestId",
				"productId",
				"grantId",
				"recoveryIncarnation",
				"deadlineBlock",
				"operation",
				"input"
			]
			.into_iter()
			.map(str::to_string)
			.collect(),
		);
		assert_eq!(value["operation"], "identity.account");
		assert_eq!(value["input"]["session"], "selected");
		let mut joined = value;
		joined
			.as_object_mut()
			.unwrap()
			.insert("alternateAuthority".into(), Value::Bool(true));
		assert!(serde_json::from_value::<IdentityInvocationV2>(joined).is_err());
	}

	#[test]
	fn results_are_operation_exact_and_bounded() {
		let oversized = IdentityResultV2::HumanityProve(IdentityHumanityProveResultV2 {
			proof: vec![0; 4097],
			derived_public_key: [1; 32],
			proof_hash: [2; 32],
			continuity: true,
			expires_at: 200,
		});
		assert_eq!(
			oversized.validate_for_request(
				&request(IdentityV2Operation::IdentityHumanityProve),
				100,
				[8; 32]
			),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let exact = IdentityResultV2::Account(IdentityAccountResultV2 {
			account: [1; 32],
			session_expires_at: 200,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [2; 32] },
		});
		assert_eq!(
			exact.validate_for_request(
				&request(IdentityV2Operation::IdentityProfileRead),
				100,
				[8; 32]
			),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
	}

	#[test]
	fn audience_and_recovery_incarnation_fail_closed() {
		let operation = IdentityV2Operation::IdentityHumanityProve;
		let mut journal = FreshConsentJournalV2::default();
		let mut wrong_audience = grant_core(operation);
		wrong_audience.audience = Some("other.example".into());
		let wrong_audience = IdentityHumanityProveGrantV2(wrong_audience);
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&wrong_audience,
				request(operation),
				[0x11; 16],
				150,
				100,
				[8; 32],
				[9; 32],
				Some([7; 16]),
				&mut journal,
			),
			Err(IdentityV2Error::AudienceInvalid),
		);
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentitySubjectDerive),
				request(IdentityV2Operation::IdentitySubjectDerive),
				[0x11; 16],
				150,
				100,
				[8; 32],
				[8; 32],
				None,
				&mut journal,
			),
			Err(IdentityV2Error::OldIncarnation),
		);
		let wrong_product = IdentityRequestV2::SubjectDerive(IdentitySubjectDeriveRequestV2 {
			product_id: "other".into(),
			context: "attendee".into(),
			verifier_audience: "festival.example".into(),
			epoch: None,
		});
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentitySubjectDerive),
				wrong_product,
				[0x11; 16],
				150,
				100,
				[8; 32],
				[9; 32],
				None,
				&mut journal,
			),
			Err(IdentityV2Error::GrantScopeDenied),
		);
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentityAccount),
				request(IdentityV2Operation::IdentityAccount),
				[0x11; 16],
				100,
				100,
				[8; 32],
				[9; 32],
				None,
				&mut journal,
			),
			Err(IdentityV2Error::DeadlineExpired),
		);
	}

	#[test]
	fn results_bind_to_request_and_finalized_freshness_context() {
		let account_request =
			IdentityRequestV2::Account(IdentityAccountRequestV2 { session: "selected".into() });
		let stale_account = IdentityResultV2::Account(IdentityAccountResultV2 {
			account: [1; 32],
			session_expires_at: 100,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [8; 32] },
		});
		assert_eq!(
			stale_account.validate_for_request(&account_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let cross_snapshot_account = IdentityResultV2::Account(IdentityAccountResultV2 {
			account: [1; 32],
			session_expires_at: 120,
			finalized: FinalizedIdentityV2 { block_number: 99, block_hash: [7; 32] },
		});
		assert_eq!(
			cross_snapshot_account.validate_for_request(&account_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let wrong_current_hash = IdentityResultV2::Account(IdentityAccountResultV2 {
			account: [1; 32],
			session_expires_at: 120,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [7; 32] },
		});
		assert_eq!(
			wrong_current_hash.validate_for_request(&account_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let profile_request = IdentityRequestV2::ProfileRead(IdentityProfileReadRequestV2 {
			subject: [1; 32],
			fields: vec!["display".into()],
			at: Some([8; 32]),
		});
		let missing_finality = IdentityResultV2::ProfileRead(IdentityProfileReadResultV2 {
			receipt: IdentityReceiptV2 { commitment: [2; 32], valid_until: 120, finalized: None },
		});
		assert_eq!(
			missing_finality.validate_for_request(&profile_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let wrong_hash = IdentityResultV2::ProfileRead(IdentityProfileReadResultV2 {
			receipt: IdentityReceiptV2 {
				commitment: [2; 32],
				valid_until: 120,
				finalized: Some(FinalizedIdentityV2 { block_number: 100, block_hash: [7; 32] }),
			},
		});
		assert_eq!(
			wrong_hash.validate_for_request(&profile_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let historical_profile = IdentityResultV2::ProfileRead(IdentityProfileReadResultV2 {
			receipt: IdentityReceiptV2 {
				commitment: [2; 32],
				valid_until: 110,
				finalized: Some(FinalizedIdentityV2 { block_number: 99, block_hash: [8; 32] }),
			},
		});
		assert_eq!(historical_profile.validate_for_request(&profile_request, 100, [8; 32]), Ok(()));
		let future_profile = IdentityResultV2::ProfileRead(IdentityProfileReadResultV2 {
			receipt: IdentityReceiptV2 {
				commitment: [2; 32],
				valid_until: 120,
				finalized: Some(FinalizedIdentityV2 { block_number: 101, block_hash: [8; 32] }),
			},
		});
		assert_eq!(
			future_profile.validate_for_request(&profile_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let disclose_request =
			IdentityRequestV2::ProfileDisclose(IdentityProfileDiscloseRequestV2 {
				audience: "festival.example".into(),
				fields: vec!["display".into()],
				purpose: "entry".into(),
				expires_at: 120,
			});
		let overlong_disclosure =
			IdentityResultV2::ProfileDisclose(IdentityProfileDiscloseResultV2 {
				receipt: IdentityReceiptV2 {
					commitment: [2; 32],
					valid_until: 121,
					finalized: Some(FinalizedIdentityV2 { block_number: 100, block_hash: [8; 32] }),
				},
			});
		assert_eq!(
			overlong_disclosure.validate_for_request(&disclose_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let humanity_request = IdentityRequestV2::HumanityStatus(IdentityHumanityStatusRequestV2 {
			subject: [1; 32],
			at: None,
		});
		let stale_humanity = IdentityResultV2::HumanityStatus(IdentityHumanityStatusResultV2 {
			status: 1,
			fresh_until: 100,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [8; 32] },
		});
		assert_eq!(
			stale_humanity.validate_for_request(&humanity_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);

		let entitlement_request =
			IdentityRequestV2::EntitlementsRead(IdentityEntitlementsReadRequestV2 {
				subject: [1; 32],
				scope: "festival.entry".into(),
				at: None,
			});
		let wrong_scope = IdentityResultV2::EntitlementsRead(IdentityEntitlementsReadResultV2 {
			allowed: true,
			scope: "other".into(),
			policy_version: 1,
			expires_at: 120,
			fresh_until: 110,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [8; 32] },
		});
		assert_eq!(
			wrong_scope.validate_for_request(&entitlement_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let stale = IdentityResultV2::EntitlementsRead(IdentityEntitlementsReadResultV2 {
			allowed: true,
			scope: "festival.entry".into(),
			policy_version: 1,
			expires_at: 120,
			fresh_until: 100,
			finalized: FinalizedIdentityV2 { block_number: 100, block_hash: [8; 32] },
		});
		assert_eq!(
			stale.validate_for_request(&entitlement_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
		let cross_snapshot_entitlement =
			IdentityResultV2::EntitlementsRead(IdentityEntitlementsReadResultV2 {
				allowed: true,
				scope: "festival.entry".into(),
				policy_version: 1,
				expires_at: 120,
				fresh_until: 110,
				finalized: FinalizedIdentityV2 { block_number: 99, block_hash: [7; 32] },
			});
		assert_eq!(
			cross_snapshot_entitlement.validate_for_request(&entitlement_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);

		let proof_request = IdentityRequestV2::HumanityProve(IdentityHumanityProveRequestV2 {
			audience: "festival.example".into(),
			challenge: vec![3; 16],
			expires_at: 120,
			claims: vec![],
		});
		let overlong = IdentityResultV2::HumanityProve(IdentityHumanityProveResultV2 {
			proof: vec![4; 64],
			derived_public_key: [5; 32],
			proof_hash: [6; 32],
			continuity: true,
			expires_at: 121,
		});
		assert_eq!(
			overlong.validate_for_request(&proof_request, 100, [8; 32]),
			Err(IdentityV2Error::ProofExpired),
		);
		let transaction_request = IdentityRequestV2::TransactionSign(TransactionSignRequestV2 {
			payload_hash: [1; 32],
			policy_hash: [2; 32],
			expires_at: 120,
		});
		let cross_snapshot_transaction =
			IdentityResultV2::TransactionSign(TransactionSignResultV2 {
				transaction_hash: [3; 32],
				finalized: FinalizedIdentityV2 { block_number: 99, block_hash: [7; 32] },
			});
		assert_eq!(
			cross_snapshot_transaction.validate_for_request(&transaction_request, 100, [8; 32]),
			Err(IdentityV2Error::WireSchemaInvalid),
		);
	}

	#[test]
	fn exact_numeric_error_envelopes_fail_closed() {
		for operation in IdentityV2Operation::ALL {
			for frozen in identity_v2_errors_for(operation) {
				let envelope = IdentityV2ErrorEnvelope {
					code: frozen.code,
					name: frozen.name.into(),
					retryable: frozen.retryable,
					details: None,
				};
				assert_eq!(envelope.validate_for(operation), Ok(()));
				let mut drift = envelope.clone();
				drift.retryable = !drift.retryable;
				assert_eq!(drift.validate_for(operation), Err(IdentityV2Error::WireSchemaInvalid));
			}
		}
		let joined = serde_json::json!({
			"code": 400,
			"name": "IDENTITY_AUDIENCE_INVALID",
			"retryable": false,
			"profile": {"email": "hidden@example"}
		});
		assert!(serde_json::from_value::<IdentityV2ErrorEnvelope>(joined).is_err());
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
		let proof_request = request(IdentityV2Operation::IdentityHumanityProve);
		let accepted = journal.reserve(&proof_request, Some([7; 16])).unwrap();
		assert_eq!(journal.commit(&accepted), Ok(()));
		assert_eq!(
			journal.reserve(&proof_request, Some([8; 16])),
			Err(IdentityV2Error::ChallengeReplay),
		);
		let fresh_challenge = IdentityRequestV2::HumanityProve(IdentityHumanityProveRequestV2 {
			audience: "festival.example".into(),
			challenge: vec![4; 16],
			expires_at: 120,
			claims: vec![],
		});
		let operation_id_was_not_burned = journal.reserve(&fresh_challenge, Some([8; 16])).unwrap();
		assert_eq!(journal.commit(&operation_id_was_not_burned), Ok(()));
		let mut integrated = FreshConsentJournalV2::default();
		let mut bridge_effects = 0;
		let pending = prepare_identity_v2_invocation(
			"festival",
			&grant(IdentityV2Operation::IdentityHumanityProve),
			request(IdentityV2Operation::IdentityHumanityProve),
			[0x11; 16],
			150,
			100,
			[8; 32],
			[9; 32],
			Some([7; 16]),
			&mut integrated,
		)
		.unwrap();
		bridge_effects += 1;
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentityHumanityProve),
				request(IdentityV2Operation::IdentityHumanityProve),
				[0x11; 16],
				150,
				100,
				[8; 32],
				[9; 32],
				Some([7; 16]),
				&mut integrated,
			),
			Err(IdentityV2Error::ChallengeReplay),
		);
		assert_eq!(bridge_effects, 1, "only the reservation holder may reach the bridge");
		pending.rollback_pre_accept(&mut integrated);
		let retry = prepare_identity_v2_invocation(
			"festival",
			&grant(IdentityV2Operation::IdentityHumanityProve),
			request(IdentityV2Operation::IdentityHumanityProve),
			[0x11; 16],
			150,
			100,
			[8; 32],
			[9; 32],
			Some([7; 16]),
			&mut integrated,
		)
		.unwrap();
		bridge_effects += 1;
		assert_eq!(retry.commit_durable_acceptance(&mut integrated), Ok(()));
		assert_eq!(bridge_effects, 2, "a rolled-back pre-accept lease remains retryable");
		assert_eq!(
			prepare_identity_v2_invocation(
				"festival",
				&grant(IdentityV2Operation::IdentityHumanityProve),
				request(IdentityV2Operation::IdentityHumanityProve),
				[0x11; 16],
				150,
				100,
				[8; 32],
				[9; 32],
				Some([8; 16]),
				&mut integrated,
			),
			Err(IdentityV2Error::ChallengeReplay),
		);

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

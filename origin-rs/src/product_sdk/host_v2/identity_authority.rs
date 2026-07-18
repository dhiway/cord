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

//! Private durable authority for the closed host-v2 Identity surface.

use std::{
	collections::{BTreeMap, BTreeSet},
	fs::{self, File, OpenOptions},
	io::{Read, Write},
	path::{Path, PathBuf},
	sync::{Arc, Mutex, RwLock},
};

use ciborium::value::Value;
use codec::{Decode, Encode};
use ed25519_dalek::{Signer as _, SigningKey};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::product_sdk::host_outbox::{decrypt, encrypt, sync_dir, HostOutboxError};

use super::{
	codec::Dto,
	execution::{
		FinalizedIdentityRuntimeV2, HostCallV2, HostExecutionErrorV2, HostExecutionV2,
		HostIdentityAuthorityV2, HostSigningAuthorityV2,
	},
	generated::{
		AcceptedEventV2, ErrorCode, ErrorEventV2, IdentityAccountAccepted, IdentityAccountError,
		IdentityAccountFrame, IdentityAccountResult, IdentityEntitlementsReadAccepted,
		IdentityEntitlementsReadError, IdentityEntitlementsReadFrame,
		IdentityEntitlementsReadResult, IdentityHumanityProveAccepted, IdentityHumanityProveError,
		IdentityHumanityProveFrame, IdentityHumanityProveProgress, IdentityHumanityProveResult,
		IdentityHumanityStatusAccepted, IdentityHumanityStatusError, IdentityHumanityStatusFrame,
		IdentityHumanityStatusResult, IdentityProfileDiscloseAccepted,
		IdentityProfileDiscloseError, IdentityProfileDiscloseFrame,
		IdentityProfileDiscloseProgress, IdentityProfileDiscloseResult,
		IdentityProfileReadAccepted, IdentityProfileReadError, IdentityProfileReadFrame,
		IdentityProfileReadResult, IdentitySubjectDeriveAccepted, IdentitySubjectDeriveError,
		IdentitySubjectDeriveFrame, IdentitySubjectDeriveResult, Production, ProgressEventV2,
		RecoveryInstallV2, RecoveryReceiptV2, ResultEventV2, SubjectContextV2,
		SubjectProofEnvelopeV2, SubjectProofV2, TransactionSignAccepted, TransactionSignError,
		TransactionSignFrame, TransactionSignProgress, TransactionSignResult, ERRORS, OPERATIONS,
	},
};

const STATE_VERSION: u8 = 2;
const ROOT: &str = "identity-authority-v2";
const STATE: &str = "state.identity";
const QUARANTINE: &str = "quarantine";
const QUARANTINE_MARKER: &str = "recovery-required.identity";
const STATE_KEY_DOMAIN: &[u8] = b"cord/identity/authority-state-key/v2";
const STATE_NONCE_DOMAIN: &[u8] = b"cord/identity/authority-state-nonce/v2";
const STATE_AAD_DOMAIN: &[u8] = b"cord/identity/authority-state/v2";
const MAX_GRANTS: usize = 256;
const MAX_OPERATIONS: usize = 4_096;
const MAX_CHALLENGES: usize = 4_096;
const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;
const SUBJECT_KDF_DOMAIN: &[u8] = b"cord.identity.subject.kdf.v2";
const SUBJECT_ID_DOMAIN: &[u8] = b"cord.identity.subject.id.v2";
const SUBJECT_PROOF_DOMAIN: &[u8] = b"cord.identity.subject.proof.v2";
const CHALLENGE_DOMAIN: &[u8] = b"cord.identity.challenge.v2";
const EFFECT_DOMAIN: &[u8] = b"cord.identity.effect.v2";
const RECOVERY_RECEIPT_DOMAIN: &[u8] = b"cord.identity.recovery.receipt.v2";
const PROOF_MAX_BLOCKS: u64 = 128;
const MAX_RETIRED_ROOTS: usize = 16_384;
const MAX_RECOVERY_RECEIPTS: usize = MAX_RETIRED_ROOTS;
const RECOVERY_ENTROPY_ATTEMPTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum IdentityAuthorityErrorV2 {
	#[error("HOST_IDENTITY_KEYSTORE_UNAVAILABLE")]
	KeystoreUnavailable,
	#[error("HOST_IDENTITY_GRANTS_UNAVAILABLE")]
	GrantsUnavailable,
	#[error("HOST_IDENTITY_STATE_CORRUPT")]
	Corrupt,
	#[error("HOST_IDENTITY_STATE_FULL")]
	Full,
	#[error("HOST_IDENTITY_STATE_UNAVAILABLE")]
	Unavailable,
	#[error("GRANT_REQUIRED")]
	GrantRequired,
	#[error("GRANT_SCOPE_DENIED")]
	GrantScopeDenied,
	#[error("GRANT_EXPIRED")]
	GrantExpired,
	#[error("GRANT_REVOKED")]
	GrantRevoked,
	#[error("REQUEST_DEADLINE_EXPIRED")]
	DeadlineExpired,
	#[error("IDENTITY_AUDIENCE_INVALID")]
	AudienceInvalid,
	#[error("IDENTITY_CHALLENGE_REPLAY")]
	ChallengeReplay,
	#[error("IDENTITY_PROOF_EXPIRED")]
	ProofExpired,
	#[error("IDENTITY_OLD_INCARNATION")]
	OldIncarnation,
	#[error("IDENTITY_EPOCH_INVALID")]
	EpochInvalid,
	#[error("IDENTITY_AUTHORITY_UNAVAILABLE")]
	AuthorityUnavailable,
	#[error("IDENTITY_EFFECT_CONFLICT")]
	EffectConflict,
	#[error("IDENTITY_RECOVERY_ENTROPY_FAILED")]
	RecoveryEntropyFailed,
	#[error("IDENTITY_RECOVERY_INSTALL_FAILED")]
	RecoveryInstallFailed,
	#[error("IDENTITY_RETIRED_SET_FULL")]
	RetiredSetFull,
}

impl IdentityAuthorityErrorV2 {
	fn code(self, _operation: u16) -> ErrorCode {
		match self {
			Self::KeystoreUnavailable | Self::Unavailable => ErrorCode::HostOutboxUnavailable,
			Self::GrantsUnavailable | Self::GrantRequired => ErrorCode::GrantRequired,
			Self::Corrupt => ErrorCode::HostOutboxCorrupt,
			Self::Full => ErrorCode::HostOutboxFull,
			Self::GrantScopeDenied => ErrorCode::GrantScopeDenied,
			Self::GrantExpired => ErrorCode::GrantExpired,
			Self::GrantRevoked => ErrorCode::GrantRevoked,
			Self::DeadlineExpired => ErrorCode::RequestDeadlineExpired,
			Self::AudienceInvalid => ErrorCode::IdentityAudienceInvalid,
			Self::ChallengeReplay => ErrorCode::IdentityChallengeReplay,
			Self::ProofExpired => ErrorCode::IdentityProofExpired,
			Self::OldIncarnation => ErrorCode::IdentityOldIncarnation,
			Self::EpochInvalid => ErrorCode::IdentityEpochInvalid,
			Self::AuthorityUnavailable => ErrorCode::IdentityAuthorityUnavailable,
			Self::EffectConflict => ErrorCode::IdentityEffectConflict,
			Self::RecoveryEntropyFailed => ErrorCode::IdentityRecoveryEntropyFailed,
			Self::RecoveryInstallFailed => ErrorCode::IdentityRecoveryInstallFailed,
			Self::RetiredSetFull => ErrorCode::IdentityRetiredSetFull,
		}
	}
}

impl From<HostOutboxError> for IdentityAuthorityErrorV2 {
	fn from(error: HostOutboxError) -> Self {
		match error {
			HostOutboxError::Full => Self::Full,
			HostOutboxError::Corrupt => Self::Corrupt,
			_ => Self::Unavailable,
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdentityAuthorityContextV2 {
	pub(crate) profile_id: [u8; 32],
	pub(crate) genesis_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdentityKeystoreMaterialV2 {
	pub(crate) state_key: [u8; 32],
	pub(crate) recovery_receipt_signing_seed: [u8; 32],
	pub(crate) subject_master_seed: [u8; 32],
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) epoch: u32,
	pub(crate) continuity: bool,
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct FinalizedIdentityEffectV2 {
	pub(crate) block_number: u64,
	pub(crate) block_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedProfileAuthorityV2 {
	pub(crate) commitment: [u8; 32],
	pub(crate) valid_until: u64,
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedHumanityAuthorityV2 {
	pub(crate) status: u16,
	pub(crate) fresh_until: u64,
	pub(crate) commitment: [u8; 32],
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedEntitlementAuthorityV2 {
	pub(crate) allowed: bool,
	pub(crate) scope: String,
	pub(crate) policy_version: u32,
	pub(crate) expires_at: u64,
	pub(crate) fresh_until: u64,
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAccountSessionV2 {
	pub(crate) account: [u8; 32],
	pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostProfileDisclosureV2 {
	pub(crate) commitment: [u8; 32],
	pub(crate) valid_until: u64,
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProfileDisclosureRequestV2 {
	pub(crate) effect_id: [u8; 32],
	pub(crate) operation_id: [u8; 16],
	pub(crate) grant_id: [u8; 32],
	pub(crate) product_id: String,
	pub(crate) audience: String,
	pub(crate) fields: Vec<String>,
	pub(crate) purpose: String,
	pub(crate) expires_at: u64,
	pub(crate) request_hash: [u8; 32],
	pub(crate) consent_receipt: [u8; 32],
	pub(crate) intent_finality: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdentityConsentRequestV2 {
	pub(crate) effect_id: [u8; 32],
	pub(crate) operation_id: [u8; 16],
	pub(crate) operation: u16,
	pub(crate) grant_id: [u8; 32],
	pub(crate) request_hash: [u8; 32],
	pub(crate) effect_hash: [u8; 32],
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransactionSigningRequestV2 {
	pub(crate) effect_id: [u8; 32],
	pub(crate) operation_id: [u8; 16],
	pub(crate) grant_id: [u8; 32],
	pub(crate) genesis_hash: [u8; 32],
	pub(crate) payload_hash: [u8; 32],
	pub(crate) policy_hash: [u8; 32],
	pub(crate) expires_at: u64,
	pub(crate) consent_receipt: [u8; 32],
	pub(crate) intent_finality: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedTransactionEffectV2 {
	pub(crate) transaction_hash: [u8; 32],
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

pub(crate) trait FinalizedIdentitySourceV2: Send {
	fn head(&mut self) -> Result<FinalizedIdentityEffectV2, IdentityAuthorityErrorV2>;
	fn profile(
		&mut self,
		subject: [u8; 32],
		fields: &[String],
		at: Option<[u8; 32]>,
	) -> Result<FinalizedProfileAuthorityV2, IdentityAuthorityErrorV2>;
	fn humanity_status(
		&mut self,
		subject: [u8; 32],
		at: Option<[u8; 32]>,
	) -> Result<FinalizedHumanityAuthorityV2, IdentityAuthorityErrorV2>;
	fn humanity_proof(
		&mut self,
		product_id: &str,
		audience: &str,
		claims: &[String],
	) -> Result<FinalizedHumanityAuthorityV2, IdentityAuthorityErrorV2>;
	fn entitlement(
		&mut self,
		subject: [u8; 32],
		scope: &str,
		at: Option<[u8; 32]>,
	) -> Result<FinalizedEntitlementAuthorityV2, IdentityAuthorityErrorV2>;
	fn is_finalized(
		&mut self,
		finality: FinalizedIdentityEffectV2,
	) -> Result<bool, IdentityAuthorityErrorV2>;
}

/// Production host adapter contract for disclosure side effects. Implementations must bind the
/// complete request to the nonzero `effect_id`, recover the original result after restart, and
/// never apply the disclosure more than once for that identifier.
pub(crate) trait HostIdentityDeliveryV2: Send {
	fn account(&mut self, session: &str) -> Result<HostAccountSessionV2, IdentityAuthorityErrorV2>;
	fn recover_or_disclose(
		&mut self,
		effect_id: [u8; 32],
		request: &ProfileDisclosureRequestV2,
	) -> Result<HostProfileDisclosureV2, IdentityAuthorityErrorV2>;
}

/// Production consent adapter contract. A repeated `effect_id` with the same request must return
/// the original receipt; reuse with a different request must fail with `EffectConflict`.
pub(crate) trait FreshIdentityConsentV2: Send {
	fn recover_or_consume(
		&mut self,
		effect_id: [u8; 32],
		request: &IdentityConsentRequestV2,
	) -> Result<[u8; 32], IdentityAuthorityErrorV2>;
}

/// Production signing adapter contract. Implementations recover or execute one finalized signing
/// effect for the stable `effect_id`; they must not sign a second time after a lost response.
pub(crate) trait FinalizedTransactionSignerV2: Send {
	fn recover_or_sign_and_finalize(
		&mut self,
		effect_id: [u8; 32],
		request: &TransactionSigningRequestV2,
	) -> Result<FinalizedTransactionEffectV2, IdentityAuthorityErrorV2>;
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct IdentityGrantRecordV2 {
	pub(crate) id: [u8; 32],
	pub(crate) product_id: Vec<u8>,
	pub(crate) operation: u16,
	pub(crate) recovery_incarnation: [u8; 32],
	pub(crate) expires_at: u64,
	pub(crate) audience: Option<Vec<u8>>,
	pub(crate) revoked: bool,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct DurableIdentityOperationV2 {
	pub(crate) operation_id: [u8; 16],
	pub(crate) operation: u16,
	pub(crate) request_hash: [u8; 32],
	pub(crate) grant_id: [u8; 32],
	pub(crate) external_effect_id: [u8; 32],
	pub(crate) effect_hash: [u8; 32],
	pub(crate) finalized_number: u64,
	pub(crate) finalized_hash: [u8; 32],
	pub(crate) consent_receipt: [u8; 32],
	pub(crate) consent_recorded: bool,
	pub(crate) events: Vec<Vec<u8>>,
	pub(crate) completed: bool,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct DurableIdentityRecoveryV2 {
	operation_id: [u8; 16],
	install: Vec<u8>,
	receipt: Vec<u8>,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct DurableIdentityStateV2 {
	version: u8,
	revision: u64,
	profile_id: [u8; 32],
	genesis_hash: [u8; 32],
	subject_master_seed: [u8; 32],
	recovery_incarnation: [u8; 32],
	recovery_receipt_public_key: [u8; 32],
	epoch: u32,
	continuity: bool,
	grants: BTreeMap<[u8; 32], IdentityGrantRecordV2>,
	operations: BTreeMap<[u8; 16], DurableIdentityOperationV2>,
	challenges: BTreeSet<[u8; 32]>,
	recoveries: BTreeMap<[u8; 16], DurableIdentityRecoveryV2>,
	retired_roots: BTreeSet<[u8; 32]>,
	retired_grants: BTreeSet<[u8; 32]>,
	pending_recovery: Option<[u8; 16]>,
}

impl DurableIdentityStateV2 {
	fn validate(
		&self,
		context: IdentityAuthorityContextV2,
	) -> Result<(), IdentityAuthorityErrorV2> {
		if self.version != STATE_VERSION ||
			self.revision == 0 ||
			self.profile_id != context.profile_id ||
			self.genesis_hash != context.genesis_hash ||
			self.subject_master_seed == [0; 32] ||
			self.recovery_incarnation == [0; 32] ||
			self.recovery_receipt_public_key == [0; 32] ||
			self.grants.len() > MAX_GRANTS ||
			self.operations.len() > MAX_OPERATIONS ||
			self.challenges.len() > MAX_CHALLENGES ||
			self.recoveries.len() > MAX_RECOVERY_RECEIPTS ||
			self.retired_roots.len() > MAX_RETIRED_ROOTS ||
			self.retired_roots.contains(&recovery_tombstone(
				self.subject_master_seed,
				self.recovery_incarnation,
			)) || (self.pending_recovery.is_some() &&
			(!self.grants.is_empty() ||
				!self.operations.is_empty() ||
				!self.challenges.is_empty()))
		{
			return Err(IdentityAuthorityErrorV2::Corrupt);
		}
		for (id, grant) in &self.grants {
			if *id != grant.id ||
				self.retired_grants.contains(id) ||
				grant.product_id.is_empty() ||
				grant.product_id.len() > 128 ||
				grant.recovery_incarnation != self.recovery_incarnation
			{
				return Err(IdentityAuthorityErrorV2::Corrupt);
			}
		}
		for (id, operation) in &self.operations {
			if *id != operation.operation_id ||
				operation.external_effect_id == [0; 32] ||
				operation.events.len() > 3 ||
				(operation.consent_recorded == (operation.consent_receipt == [0; 32])) ||
				(operation.completed &&
					(!operation.consent_recorded || operation.events.is_empty()))
			{
				return Err(IdentityAuthorityErrorV2::Corrupt);
			}
		}
		for (operation_id, recovery) in &self.recoveries {
			if operation_id != &recovery.operation_id ||
				recovery.install.len() > 1_024 ||
				recovery.receipt.len() > 1_024 ||
				Dto::<RecoveryInstallV2>::decode(&recovery.install).is_err() ||
				Dto::<RecoveryReceiptV2>::decode(&recovery.receipt).is_err()
			{
				return Err(IdentityAuthorityErrorV2::Corrupt);
			}
		}
		Ok(())
	}
}

trait IdentityRecoveryEntropyV2 {
	fn fill(&mut self, output: &mut [u8; 32]) -> Result<(), IdentityAuthorityErrorV2>;
}

struct OsIdentityRecoveryEntropyV2;

impl IdentityRecoveryEntropyV2 for OsIdentityRecoveryEntropyV2 {
	fn fill(&mut self, output: &mut [u8; 32]) -> Result<(), IdentityAuthorityErrorV2> {
		OsRng
			.try_fill_bytes(output)
			.map_err(|_| IdentityAuthorityErrorV2::RecoveryEntropyFailed)
	}
}

pub(crate) struct IdentityAuthorityStoreV2 {
	root: PathBuf,
	context: IdentityAuthorityContextV2,
	key: [u8; 32],
	aad: Vec<u8>,
	recovery_receipt_signing_seed: [u8; 32],
	state: RwLock<DurableIdentityStateV2>,
	#[cfg(test)]
	persist_fault: Mutex<Option<TestPersistFaultV2>>,
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum TestPersistFaultPointV2 {
	BeforeWrite,
	AfterFileSync,
	AfterRename,
}

#[cfg(test)]
struct TestPersistFaultV2 {
	successful_persists_before_failure: usize,
	point: TestPersistFaultPointV2,
}

impl IdentityAuthorityStoreV2 {
	pub(crate) fn open(
		root: impl AsRef<Path>,
		context: IdentityAuthorityContextV2,
		keystore: Option<IdentityKeystoreMaterialV2>,
		grants: Option<Vec<IdentityGrantRecordV2>>,
	) -> Result<Self, IdentityAuthorityErrorV2> {
		let keystore = keystore.ok_or(IdentityAuthorityErrorV2::KeystoreUnavailable)?;
		let grants = grants.ok_or(IdentityAuthorityErrorV2::GrantsUnavailable)?;
		let root = root.as_ref().join(ROOT);
		ensure_directory(&root)?;
		ensure_directory(&root.join(QUARANTINE))?;
		if root.join(QUARANTINE_MARKER).exists() {
			return Err(IdentityAuthorityErrorV2::Corrupt);
		}
		cleanup_stale_temporaries(&root)?;
		let key = state_key(keystore.state_key, context)?;
		validate_recovery_receipt_signing_seed(keystore.recovery_receipt_signing_seed)?;
		let aad = state_aad(context);
		let path = root.join(STATE);
		let existing = path.exists();
		if existing {
			validate_state_key(keystore.state_key)?;
		} else {
			validate_keystore(keystore)?;
		}
		let state = if existing {
			match load_state(&path, &key, &aad, context) {
				Ok(state) => state,
				Err(error) => {
					quarantine(&root, &path)?;
					return Err(error);
				},
			}
		} else {
			DurableIdentityStateV2 {
				version: STATE_VERSION,
				revision: 1,
				profile_id: context.profile_id,
				genesis_hash: context.genesis_hash,
				subject_master_seed: keystore.subject_master_seed,
				recovery_incarnation: keystore.recovery_incarnation,
				epoch: keystore.epoch,
				continuity: keystore.continuity,
				recovery_receipt_public_key: recovery_receipt_public_key(
					keystore.recovery_receipt_signing_seed,
				),
				grants: grants_map(grants.clone(), keystore.recovery_incarnation)?,
				operations: BTreeMap::new(),
				challenges: BTreeSet::new(),
				recoveries: BTreeMap::new(),
				retired_roots: BTreeSet::new(),
				retired_grants: BTreeSet::new(),
				pending_recovery: None,
			}
		};
		state.validate(context)?;
		if state.recovery_receipt_public_key !=
			recovery_receipt_public_key(keystore.recovery_receipt_signing_seed)
		{
			return Err(IdentityAuthorityErrorV2::KeystoreUnavailable);
		}
		let grants = grants_map(grants, state.recovery_incarnation)?;
		let store = Self {
			root,
			context,
			key,
			aad,
			recovery_receipt_signing_seed: keystore.recovery_receipt_signing_seed,
			state: RwLock::new(state),
			#[cfg(test)]
			persist_fault: Mutex::new(None),
		};
		let replace_grants = store.read()?.grants != grants;
		if replace_grants {
			store.mutate(|state| {
				state.grants = grants;
				Ok(())
			})?;
		} else if !path.exists() {
			let state = store.read()?;
			store.persist(&state)?;
		}
		Ok(store)
	}

	/// Clears a persistent quarantine only as part of an explicit recovery/install flow.
	pub(crate) fn authorize_recovery_install(
		root: impl AsRef<Path>,
	) -> Result<(), IdentityAuthorityErrorV2> {
		let root = root.as_ref().join(ROOT);
		let marker = root.join(QUARANTINE_MARKER);
		if marker.exists() {
			fs::remove_file(marker).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
			sync_dir(&root)?;
		}
		Ok(())
	}

	pub(crate) fn context(&self) -> IdentityAuthorityContextV2 {
		self.context
	}

	#[cfg(test)]
	fn fail_persist_after(
		&self,
		successful_persists_before_failure: usize,
		point: TestPersistFaultPointV2,
	) {
		*self.persist_fault.lock().unwrap() =
			Some(TestPersistFaultV2 { successful_persists_before_failure, point });
	}

	pub(crate) fn root_material(
		&self,
	) -> Result<([u8; 32], [u8; 32], u32, bool), IdentityAuthorityErrorV2> {
		let state = self.read()?;
		Ok((state.subject_master_seed, state.recovery_incarnation, state.epoch, state.continuity))
	}

	/// Installs a fresh root for any import, cross-device restore, stale backup, or other recovery
	/// that cannot prove an authenticated monotonic same-store restart. The encrypted state,
	/// install record, signed receipt, old-root tombstone, cleared grants, cleared operations, and
	/// cleared replay journal are committed by one atomic state-file replacement.
	pub(crate) fn recover_unproven(
		&self,
		operation_id: [u8; 16],
		installed_at: u64,
	) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
		self.recover_unproven_with(operation_id, installed_at, &mut OsIdentityRecoveryEntropyV2)
	}

	fn recover_unproven_with(
		&self,
		operation_id: [u8; 16],
		installed_at: u64,
		entropy: &mut impl IdentityRecoveryEntropyV2,
	) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
		if operation_id == [0; 16] {
			return Err(IdentityAuthorityErrorV2::RecoveryInstallFailed);
		}
		if let Some(receipt) =
			self.read()?.recoveries.get(&operation_id).map(|record| record.receipt.clone())
		{
			return Ok(receipt);
		}

		// Persist the recovery barrier before entropy or installation. Once this succeeds, no old
		// grant, prepared operation, or replay entry can authorize a derivation after a later
		// entropy/install failure or restart.
		self.mutate(|state| match state.pending_recovery {
			Some(pending) if pending != operation_id =>
				Err(IdentityAuthorityErrorV2::RecoveryInstallFailed),
			Some(_) => Ok(()),
			None => {
				state.pending_recovery = Some(operation_id);
				state.retired_grants.extend(state.grants.keys().copied());
				state.grants.clear();
				state.operations.clear();
				state.challenges.clear();
				Ok(())
			},
		})
		.map_err(recovery_install_error)?;

		let (old_seed, old_incarnation, old_tombstone, previous_root, retired_roots) = {
			let state = self.read()?;
			if state.recoveries.len() >= MAX_RECOVERY_RECEIPTS {
				return Err(IdentityAuthorityErrorV2::RecoveryInstallFailed);
			}
			if state.retired_roots.len() >= MAX_RETIRED_ROOTS {
				return Err(IdentityAuthorityErrorV2::RetiredSetFull);
			}
			let old_tombstone =
				recovery_tombstone(state.subject_master_seed, state.recovery_incarnation);
			(
				state.subject_master_seed,
				state.recovery_incarnation,
				old_tombstone,
				Sha256::digest(state.subject_master_seed).into(),
				state.retired_roots.clone(),
			)
		};
		let (new_seed, new_incarnation) =
			fresh_recovery_root(entropy, old_tombstone, &retired_roots)?;
		let (install, receipt) = recovery_records(
			operation_id,
			new_seed,
			new_incarnation,
			self.recovery_receipt_signing_seed,
			Some(previous_root),
			installed_at,
		)?;

		self.mutate(|state| {
			if let Some(existing) = state.recoveries.get(&operation_id) {
				return Ok(existing.receipt.clone());
			}
			if state.pending_recovery != Some(operation_id) ||
				state.subject_master_seed != old_seed ||
				state.recovery_incarnation != old_incarnation
			{
				return Err(IdentityAuthorityErrorV2::RecoveryInstallFailed);
			}
			state.subject_master_seed = new_seed;
			state.recovery_incarnation = new_incarnation;
			state.epoch = 0;
			state.continuity = false;
			state.retired_roots.insert(old_tombstone);
			state.recoveries.insert(
				operation_id,
				DurableIdentityRecoveryV2 { operation_id, install, receipt: receipt.clone() },
			);
			state.pending_recovery = None;
			Ok(receipt)
		})
		.map_err(recovery_install_error)
	}

	#[cfg(test)]
	fn retired_root_count(&self) -> Result<usize, IdentityAuthorityErrorV2> {
		Ok(self.read()?.retired_roots.len())
	}

	pub(crate) fn grant(
		&self,
		grant_id: [u8; 32],
	) -> Result<Option<IdentityGrantRecordV2>, IdentityAuthorityErrorV2> {
		let state = self.read()?;
		if let Some(grant) = state.grants.get(&grant_id) {
			return Ok(Some(grant.clone()));
		}
		if state.retired_grants.contains(&grant_id) {
			return Err(IdentityAuthorityErrorV2::OldIncarnation);
		}
		Ok(None)
	}

	pub(crate) fn operation(
		&self,
		operation_id: [u8; 16],
	) -> Result<Option<DurableIdentityOperationV2>, IdentityAuthorityErrorV2> {
		Ok(self.read()?.operations.get(&operation_id).cloned())
	}

	pub(crate) fn challenge_consumed(
		&self,
		challenge: [u8; 32],
	) -> Result<bool, IdentityAuthorityErrorV2> {
		Ok(self.read()?.challenges.contains(&challenge))
	}

	pub(crate) fn prepare_operation(
		&self,
		record: DurableIdentityOperationV2,
		challenge: Option<[u8; 32]>,
	) -> Result<(), IdentityAuthorityErrorV2> {
		self.mutate(|state| {
			if let Some(existing) = state.operations.get(&record.operation_id) {
				return if same_operation(existing, &record) {
					Ok(())
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			if state.operations.len() >= MAX_OPERATIONS {
				return Err(IdentityAuthorityErrorV2::Full);
			}
			if let Some(challenge) = challenge {
				if state.challenges.contains(&challenge) {
					return Err(IdentityAuthorityErrorV2::ChallengeReplay);
				}
				if state.challenges.len() >= MAX_CHALLENGES {
					return Err(IdentityAuthorityErrorV2::Full);
				}
				state.challenges.insert(challenge);
			}
			state.operations.insert(record.operation_id, record);
			Ok(())
		})
	}

	pub(crate) fn complete_operation(
		&self,
		operation_id: [u8; 16],
		effect_hash: [u8; 32],
		finalized_number: u64,
		finalized_hash: [u8; 32],
		events: Vec<Vec<u8>>,
	) -> Result<(), IdentityAuthorityErrorV2> {
		self.mutate(|state| {
			let record = state
				.operations
				.get_mut(&operation_id)
				.ok_or(IdentityAuthorityErrorV2::EffectConflict)?;
			if record.completed {
				return if record.effect_hash == effect_hash && record.events == events {
					Ok(())
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			record.effect_hash = effect_hash;
			record.finalized_number = finalized_number;
			record.finalized_hash = finalized_hash;
			record.events = events;
			record.completed = true;
			Ok(())
		})
	}

	pub(crate) fn record_consent(
		&self,
		operation_id: [u8; 16],
		consent_receipt: [u8; 32],
	) -> Result<(), IdentityAuthorityErrorV2> {
		if consent_receipt == [0; 32] {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		self.mutate(|state| {
			let record = state
				.operations
				.get_mut(&operation_id)
				.ok_or(IdentityAuthorityErrorV2::EffectConflict)?;
			if record.consent_recorded {
				return if record.consent_receipt == consent_receipt {
					Ok(())
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			record.consent_receipt = consent_receipt;
			record.consent_recorded = true;
			Ok(())
		})
	}

	fn read(
		&self,
	) -> Result<std::sync::RwLockReadGuard<'_, DurableIdentityStateV2>, IdentityAuthorityErrorV2> {
		self.state.read().map_err(|_| IdentityAuthorityErrorV2::Corrupt)
	}

	fn mutate<T>(
		&self,
		change: impl FnOnce(&mut DurableIdentityStateV2) -> Result<T, IdentityAuthorityErrorV2>,
	) -> Result<T, IdentityAuthorityErrorV2> {
		let mut current = self.state.write().map_err(|_| IdentityAuthorityErrorV2::Corrupt)?;
		let mut next = current.clone();
		let output = change(&mut next)?;
		if next == *current {
			return Ok(output);
		}
		next.revision = next.revision.checked_add(1).ok_or(IdentityAuthorityErrorV2::Full)?;
		next.validate(self.context)?;
		if let Err(error) = self.persist(&next) {
			// Rename may have committed the complete authenticated state before a directory-sync
			// failure was reported. Reconcile memory with that exact state so a same-process retry
			// observes the durable idempotency journal instead of installing different material.
			if load_state(&self.root.join(STATE), &self.key, &self.aad, self.context)
				.is_ok_and(|committed| committed == next)
			{
				*current = next;
			}
			return Err(error);
		}
		*current = next;
		Ok(output)
	}

	fn persist(&self, state: &DurableIdentityStateV2) -> Result<(), IdentityAuthorityErrorV2> {
		#[cfg(test)]
		let fault = {
			let mut fault = self.persist_fault.lock().unwrap();
			match fault.as_mut() {
				Some(armed) if armed.successful_persists_before_failure == 0 =>
					fault.take().map(|armed| armed.point),
				Some(armed) => {
					armed.successful_persists_before_failure -= 1;
					None
				},
				None => None,
			}
		};
		#[cfg(test)]
		if matches!(fault, Some(TestPersistFaultPointV2::BeforeWrite)) {
			return Err(IdentityAuthorityErrorV2::Unavailable);
		}
		let plaintext = state.encode();
		if plaintext.len() as u64 > MAX_STATE_BYTES {
			return Err(IdentityAuthorityErrorV2::Full);
		}
		let nonce = state_nonce(state.revision, &plaintext);
		let envelope = encrypt(&plaintext, &self.key, nonce, &self.aad)?;
		let target = self.root.join(STATE);
		let temporary = self.root.join(format!(".{STATE}.{}.tmp", state.revision));
		let result: Result<(), IdentityAuthorityErrorV2> = (|| {
			let mut file = OpenOptions::new()
				.create_new(true)
				.write(true)
				.open(&temporary)
				.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
			set_private_file(&file)?;
			file.write_all(&envelope).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
			file.sync_all().map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
			#[cfg(test)]
			if matches!(fault, Some(TestPersistFaultPointV2::AfterFileSync)) {
				return Err(IdentityAuthorityErrorV2::Unavailable);
			}
			fs::rename(&temporary, &target).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
			#[cfg(test)]
			if matches!(fault, Some(TestPersistFaultPointV2::AfterRename)) {
				return Err(IdentityAuthorityErrorV2::Unavailable);
			}
			sync_dir(&self.root)?;
			Ok(())
		})();
		if result.is_err() && temporary.exists() {
			let _ = fs::remove_file(&temporary);
			let _ = sync_dir(&self.root);
		}
		result
	}
}

struct IdentityAuthorityCoreV2<R, H, C, S> {
	store: IdentityAuthorityStoreV2,
	runtime: R,
	host: H,
	consent: C,
	signer: S,
}

pub(crate) struct DurableIdentityAuthorityV2<R, H, C, S> {
	core: Arc<Mutex<IdentityAuthorityCoreV2<R, H, C, S>>>,
}

impl<R, H, C, S> Clone for DurableIdentityAuthorityV2<R, H, C, S> {
	fn clone(&self) -> Self {
		Self { core: Arc::clone(&self.core) }
	}
}

impl<R, H, C, S> DurableIdentityAuthorityV2<R, H, C, S>
where
	R: FinalizedIdentitySourceV2,
	H: HostIdentityDeliveryV2,
	C: FreshIdentityConsentV2,
	S: FinalizedTransactionSignerV2,
{
	pub(crate) fn open(
		root: impl AsRef<Path>,
		context: IdentityAuthorityContextV2,
		keystore: Option<IdentityKeystoreMaterialV2>,
		grants: Option<Vec<IdentityGrantRecordV2>>,
		runtime: R,
		host: H,
		consent: C,
		signer: S,
	) -> Result<Self, IdentityAuthorityErrorV2> {
		let store = IdentityAuthorityStoreV2::open(root, context, keystore, grants)?;
		Ok(Self {
			core: Arc::new(Mutex::new(IdentityAuthorityCoreV2 {
				store,
				runtime,
				host,
				consent,
				signer,
			})),
		})
	}

	fn execute_account(
		&mut self,
		call: HostCallV2<'_, IdentityAccountFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let authorized = authorize(&mut core, &call, 1100, None)?;
		let session = text_field(payload(call.frame.value())?, 0)?;
		let account = core.host.account(session)?;
		if account.expires_at <= authorized.head.block_number {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		read_execution::<IdentityAccountAccepted, IdentityAccountResult>(
			call.meta.request_id,
			map(vec![
				(0, Value::Bytes(account.account.to_vec())),
				(1, uint(account.expires_at)),
				(2, fin(authorized.head)),
			]),
		)
	}

	fn execute_profile_read(
		&mut self,
		call: HostCallV2<'_, IdentityProfileReadFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let authorized = authorize(&mut core, &call, 1101, None)?;
		let input = payload(call.frame.value())?;
		let subject = fixed_field(input, 0, 32)?.try_into().expect("length checked");
		let fields = text_array(input, 1)?;
		let at = optional_fixed_field(input, 2, 32)?.map(|value| value.try_into().unwrap());
		let profile = core.runtime.profile(subject, &fields, at)?;
		validate_query_finality(&mut core.runtime, authorized.head, at, profile.finalized)?;
		if profile.valid_until <= profile.finalized.block_number {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		read_execution::<IdentityProfileReadAccepted, IdentityProfileReadResult>(
			call.meta.request_id,
			map(vec![(
				0,
				identity_receipt(profile.commitment, profile.valid_until, profile.finalized),
			)]),
		)
	}

	fn execute_profile_disclose(
		&mut self,
		call: HostCallV2<'_, IdentityProfileDiscloseFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let input = payload(call.frame.value())?;
		let audience = text_field(input, 0)?.to_owned();
		let authorized = authorize(&mut core, &call, 1102, Some(&audience))?;
		if let Some(execution) = replay(&core.store, &authorized)? {
			return Ok(execution);
		}
		let fields = text_array(input, 1)?;
		let purpose = text_field(input, 2)?;
		let expires_at = uint_field(input, 3)?;
		if expires_at <= authorized.head.block_number {
			return Err(IdentityAuthorityErrorV2::DeadlineExpired);
		}
		let operation_id =
			authorized.operation_id.ok_or(IdentityAuthorityErrorV2::GrantScopeDenied)?;
		let external_effect_id = effect_hash(&[
			b"profile-disclosure",
			&operation_id,
			&authorized.request_hash,
			&authorized.grant.id,
			&core.store.context().genesis_hash,
		]);
		let mut prepared = match core.store.operation(operation_id)? {
			Some(record) => {
				validate_replay_record(&record, &authorized)?;
				record
			},
			None => {
				let record = DurableIdentityOperationV2 {
					operation_id,
					operation: authorized.operation,
					request_hash: authorized.request_hash,
					grant_id: authorized.grant.id,
					external_effect_id,
					effect_hash: external_effect_id,
					finalized_number: authorized.head.block_number,
					finalized_hash: authorized.head.block_hash,
					consent_receipt: [0; 32],
					consent_recorded: false,
					events: Vec::new(),
					completed: false,
				};
				core.store.prepare_operation(record.clone(), None)?;
				record
			},
		};
		validate_replay_record(&prepared, &authorized)?;
		if prepared.external_effect_id != external_effect_id ||
			prepared.effect_hash != external_effect_id
		{
			return Err(IdentityAuthorityErrorV2::EffectConflict);
		}
		if !prepared.consent_recorded {
			let consent = IdentityConsentRequestV2 {
				effect_id: external_effect_id,
				operation_id,
				operation: authorized.operation,
				grant_id: authorized.grant.id,
				request_hash: authorized.request_hash,
				effect_hash: external_effect_id,
				finalized: FinalizedIdentityEffectV2 {
					block_number: prepared.finalized_number,
					block_hash: prepared.finalized_hash,
				},
			};
			let receipt = core.consent.recover_or_consume(external_effect_id, &consent)?;
			core.store.record_consent(operation_id, receipt)?;
			prepared = core
				.store
				.operation(operation_id)?
				.ok_or(IdentityAuthorityErrorV2::EffectConflict)?;
		}
		let disclosure_request = ProfileDisclosureRequestV2 {
			effect_id: external_effect_id,
			operation_id,
			grant_id: authorized.grant.id,
			product_id: authorized.product_id.clone(),
			audience,
			fields,
			purpose: purpose.to_owned(),
			expires_at,
			request_hash: authorized.request_hash,
			consent_receipt: prepared.consent_receipt,
			intent_finality: FinalizedIdentityEffectV2 {
				block_number: prepared.finalized_number,
				block_hash: prepared.finalized_hash,
			},
		};
		let disclosure = core.host.recover_or_disclose(external_effect_id, &disclosure_request)?;
		validate_query_finality(
			&mut core.runtime,
			authorized.head,
			Some(prepared.finalized_hash),
			disclosure.finalized,
		)?;
		if disclosure.valid_until <= disclosure.finalized.block_number ||
			disclosure.valid_until > expires_at
		{
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		let execution = consent_execution::<
			IdentityProfileDiscloseAccepted,
			IdentityProfileDiscloseProgress,
			IdentityProfileDiscloseResult,
		>(
			call.meta.request_id,
			map(vec![(
				0,
				identity_receipt(
					disclosure.commitment,
					disclosure.valid_until,
					disclosure.finalized,
				),
			)]),
		)?;
		let final_effect = execution_hash(&execution);
		core.store.complete_operation(
			operation_id,
			final_effect,
			disclosure.finalized.block_number,
			disclosure.finalized.block_hash,
			execution.events.clone(),
		)?;
		Ok(execution)
	}

	fn execute_humanity_status(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityStatusFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let authorized = authorize(&mut core, &call, 1103, None)?;
		let input = payload(call.frame.value())?;
		let subject = fixed_field(input, 0, 32)?.try_into().expect("length checked");
		let at = optional_fixed_field(input, 1, 32)?.map(|value| value.try_into().unwrap());
		let humanity = core.runtime.humanity_status(subject, at)?;
		validate_query_finality(&mut core.runtime, authorized.head, at, humanity.finalized)?;
		if humanity.fresh_until <= humanity.finalized.block_number {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		read_execution::<IdentityHumanityStatusAccepted, IdentityHumanityStatusResult>(
			call.meta.request_id,
			map(vec![
				(0, uint(humanity.status.into())),
				(1, uint(humanity.fresh_until)),
				(2, fin(humanity.finalized)),
			]),
		)
	}

	fn execute_humanity_prove(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityProveFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let input = payload(call.frame.value())?;
		let audience = text_field(input, 0)?.to_owned();
		let authorized = authorize(&mut core, &call, 1104, Some(&audience))?;
		if let Some(execution) = replay(&core.store, &authorized)? {
			return Ok(execution);
		}
		let challenge = bytes_field(input, 1)?.to_vec();
		let expires_at = uint_field(input, 2)?;
		let claims = text_array(input, 3)?;
		if expires_at <= authorized.head.block_number ||
			expires_at > authorized.head.block_number.saturating_add(PROOF_MAX_BLOCKS)
		{
			return Err(IdentityAuthorityErrorV2::ProofExpired);
		}
		let claims_commitment = claims_commitment(&claims);
		let humanity = core.runtime.humanity_proof(&authorized.product_id, &audience, &claims)?;
		validate_query_finality(&mut core.runtime, authorized.head, None, humanity.finalized)?;
		if humanity.status == 0 || humanity.fresh_until < expires_at {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		let proof_context = format!(
			"humanity:{}:{}",
			hex::encode(claims_commitment),
			hex::encode(humanity.commitment)
		);
		let derived =
			derive_subject(&core.store, &authorized.product_id, &proof_context, &audience, None)?;
		let proof = subject_proof(
			core.store.context().genesis_hash,
			&authorized.product_id,
			&proof_context,
			&audience,
			&derived,
			&challenge,
			authorized.head.block_number,
			expires_at,
			call.meta.request_id,
		)?;
		let proof_hash: [u8; 32] = Sha256::digest(&proof).into();
		let execution = consent_execution::<
			IdentityHumanityProveAccepted,
			IdentityHumanityProveProgress,
			IdentityHumanityProveResult,
		>(
			call.meta.request_id,
			map(vec![
				(0, Value::Bytes(proof)),
				(1, Value::Bytes(derived.public_key.to_vec())),
				(2, Value::Bytes(proof_hash.to_vec())),
				(3, Value::Bool(derived.continuity)),
				(4, uint(expires_at)),
			]),
		)?;
		let challenge_key = challenge_key(
			core.store.context().genesis_hash,
			&audience,
			derived.incarnation,
			derived.epoch,
			&challenge,
		);
		complete_pure_consent(&mut core, &authorized, execution, Some(challenge_key))
	}

	fn execute_subject_derive(
		&mut self,
		call: HostCallV2<'_, IdentitySubjectDeriveFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let input = payload(call.frame.value())?;
		let product = text_field(input, 0)?.to_owned();
		let context = text_field(input, 1)?;
		let audience = text_field(input, 2)?.to_owned();
		let epoch = optional_uint_field(input, 3)?
			.map(|value| value.try_into())
			.transpose()
			.map_err(|_| IdentityAuthorityErrorV2::EpochInvalid)?;
		let authorized = authorize(&mut core, &call, 1105, Some(&audience))?;
		if product != authorized.product_id {
			return Err(IdentityAuthorityErrorV2::GrantScopeDenied);
		}
		let derived = derive_subject(&core.store, &product, context, &audience, epoch)?;
		read_execution::<IdentitySubjectDeriveAccepted, IdentitySubjectDeriveResult>(
			call.meta.request_id,
			map(vec![
				(0, Value::Bytes(derived.subject.to_vec())),
				(1, Value::Bytes(derived.public_key.to_vec())),
				(2, uint(derived.epoch.into())),
				(3, Value::Bytes(Sha256::digest(derived.incarnation).to_vec())),
				(4, Value::Bool(derived.continuity)),
			]),
		)
	}

	fn execute_entitlements_read(
		&mut self,
		call: HostCallV2<'_, IdentityEntitlementsReadFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let authorized = authorize(&mut core, &call, 1106, None)?;
		let input = payload(call.frame.value())?;
		let subject = fixed_field(input, 0, 32)?.try_into().expect("length checked");
		let scope = text_field(input, 1)?;
		let at = optional_fixed_field(input, 2, 32)?.map(|value| value.try_into().unwrap());
		let entitlement = core.runtime.entitlement(subject, scope, at)?;
		validate_query_finality(&mut core.runtime, authorized.head, at, entitlement.finalized)?;
		if entitlement.scope != scope ||
			entitlement.fresh_until <= entitlement.finalized.block_number ||
			entitlement.expires_at < entitlement.fresh_until
		{
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		read_execution::<IdentityEntitlementsReadAccepted, IdentityEntitlementsReadResult>(
			call.meta.request_id,
			map(vec![
				(0, Value::Bool(entitlement.allowed)),
				(1, Value::Text(entitlement.scope)),
				(2, uint(entitlement.policy_version.into())),
				(3, uint(entitlement.expires_at)),
				(4, uint(entitlement.fresh_until)),
				(5, fin(entitlement.finalized)),
			]),
		)
	}

	fn execute_transaction_sign(
		&mut self,
		call: HostCallV2<'_, TransactionSignFrame>,
	) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
		let mut core = self.lock()?;
		let authorized = authorize(&mut core, &call, 1200, None)?;
		if let Some(execution) = replay(&core.store, &authorized)? {
			return Ok(execution);
		}
		let input = payload(call.frame.value())?;
		let payload_hash: [u8; 32] = fixed_field(input, 0, 32)?.try_into().expect("length checked");
		let policy_hash: [u8; 32] = fixed_field(input, 1, 32)?.try_into().expect("length checked");
		let expires_at = uint_field(input, 2)?;
		if expires_at <= authorized.head.block_number {
			return Err(IdentityAuthorityErrorV2::DeadlineExpired);
		}
		let operation_id =
			authorized.operation_id.ok_or(IdentityAuthorityErrorV2::GrantScopeDenied)?;
		let external_effect_id = effect_hash(&[
			b"transaction-sign",
			&operation_id,
			&authorized.request_hash,
			&authorized.grant.id,
			&core.store.context().genesis_hash,
		]);
		let prepared = match core.store.operation(operation_id)? {
			Some(record) => {
				validate_replay_record(&record, &authorized)?;
				record
			},
			None => {
				let record = DurableIdentityOperationV2 {
					operation_id,
					operation: authorized.operation,
					request_hash: authorized.request_hash,
					grant_id: authorized.grant.id,
					external_effect_id,
					effect_hash: external_effect_id,
					finalized_number: authorized.head.block_number,
					finalized_hash: authorized.head.block_hash,
					consent_receipt: [0; 32],
					consent_recorded: false,
					events: Vec::new(),
					completed: false,
				};
				core.store.prepare_operation(record.clone(), None)?;
				record
			},
		};
		if prepared.external_effect_id != external_effect_id ||
			prepared.effect_hash != external_effect_id
		{
			return Err(IdentityAuthorityErrorV2::EffectConflict);
		}
		let prepared = if prepared.consent_recorded {
			prepared
		} else {
			let consent = IdentityConsentRequestV2 {
				effect_id: external_effect_id,
				operation_id,
				operation: authorized.operation,
				grant_id: authorized.grant.id,
				request_hash: authorized.request_hash,
				effect_hash: external_effect_id,
				finalized: FinalizedIdentityEffectV2 {
					block_number: prepared.finalized_number,
					block_hash: prepared.finalized_hash,
				},
			};
			let consent_receipt = core.consent.recover_or_consume(external_effect_id, &consent)?;
			core.store.record_consent(operation_id, consent_receipt)?;
			core.store
				.operation(operation_id)?
				.ok_or(IdentityAuthorityErrorV2::EffectConflict)?
		};
		let genesis_hash = core.store.context().genesis_hash;
		let signing_request = TransactionSigningRequestV2 {
			effect_id: external_effect_id,
			operation_id,
			grant_id: authorized.grant.id,
			genesis_hash,
			payload_hash,
			policy_hash,
			expires_at,
			consent_receipt: prepared.consent_receipt,
			intent_finality: FinalizedIdentityEffectV2 {
				block_number: prepared.finalized_number,
				block_hash: prepared.finalized_hash,
			},
		};
		let effect =
			core.signer.recover_or_sign_and_finalize(external_effect_id, &signing_request)?;
		if !core.runtime.is_finalized(effect.finalized)? {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		let execution = consent_execution::<
			TransactionSignAccepted,
			TransactionSignProgress,
			TransactionSignResult,
		>(
			call.meta.request_id,
			map(vec![
				(0, Value::Bytes(effect.transaction_hash.to_vec())),
				(1, fin(effect.finalized)),
			]),
		)?;
		let final_effect = execution_hash(&execution);
		core.store.complete_operation(
			operation_id,
			final_effect,
			effect.finalized.block_number,
			effect.finalized.block_hash,
			execution.events.clone(),
		)?;
		Ok(execution)
	}

	fn lock(
		&self,
	) -> Result<
		std::sync::MutexGuard<'_, IdentityAuthorityCoreV2<R, H, C, S>>,
		IdentityAuthorityErrorV2,
	> {
		self.core.lock().map_err(|_| IdentityAuthorityErrorV2::Corrupt)
	}
}

impl<R, H, C, S> FinalizedIdentityRuntimeV2 for DurableIdentityAuthorityV2<R, H, C, S>
where
	R: FinalizedIdentitySourceV2,
	H: HostIdentityDeliveryV2,
	C: FreshIdentityConsentV2,
	S: FinalizedTransactionSignerV2,
{
	fn identity_account(
		&mut self,
		call: HostCallV2<'_, IdentityAccountFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityAccountError>(request_id, 1100, self.execute_account(call))
	}

	fn identity_humanity_status(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityStatusFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityHumanityStatusError>(
			request_id,
			1103,
			self.execute_humanity_status(call),
		)
	}

	fn identity_entitlements_read(
		&mut self,
		call: HostCallV2<'_, IdentityEntitlementsReadFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityEntitlementsReadError>(
			request_id,
			1106,
			self.execute_entitlements_read(call),
		)
	}
}

impl<R, H, C, S> HostIdentityAuthorityV2 for DurableIdentityAuthorityV2<R, H, C, S>
where
	R: FinalizedIdentitySourceV2,
	H: HostIdentityDeliveryV2,
	C: FreshIdentityConsentV2,
	S: FinalizedTransactionSignerV2,
{
	fn identity_profile_read(
		&mut self,
		call: HostCallV2<'_, IdentityProfileReadFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityProfileReadError>(
			request_id,
			1101,
			self.execute_profile_read(call),
		)
	}

	fn identity_profile_disclose(
		&mut self,
		call: HostCallV2<'_, IdentityProfileDiscloseFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityProfileDiscloseError>(
			request_id,
			1102,
			self.execute_profile_disclose(call),
		)
	}

	fn identity_humanity_prove(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityProveFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentityHumanityProveError>(
			request_id,
			1104,
			self.execute_humanity_prove(call),
		)
	}

	fn identity_subject_derive(
		&mut self,
		call: HostCallV2<'_, IdentitySubjectDeriveFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<IdentitySubjectDeriveError>(
			request_id,
			1105,
			self.execute_subject_derive(call),
		)
	}
}

impl<R, H, C, S> HostSigningAuthorityV2 for DurableIdentityAuthorityV2<R, H, C, S>
where
	R: FinalizedIdentitySourceV2,
	H: HostIdentityDeliveryV2,
	C: FreshIdentityConsentV2,
	S: FinalizedTransactionSignerV2,
{
	fn transaction_sign(
		&mut self,
		call: HostCallV2<'_, TransactionSignFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let request_id = call.meta.request_id;
		authority_result::<TransactionSignError>(
			request_id,
			1200,
			self.execute_transaction_sign(call),
		)
	}
}

#[derive(Clone)]
struct AuthorizedIdentityCallV2 {
	operation: u16,
	product_id: String,
	grant: IdentityGrantRecordV2,
	request_hash: [u8; 32],
	operation_id: Option<[u8; 16]>,
	head: FinalizedIdentityEffectV2,
}

#[derive(Clone)]
struct DerivedSubjectV2 {
	subject: [u8; 32],
	public_key: [u8; 32],
	seed: [u8; 32],
	epoch: u32,
	incarnation: [u8; 32],
	continuity: bool,
}

fn authorize<R, H, C, S, P>(
	core: &mut IdentityAuthorityCoreV2<R, H, C, S>,
	call: &HostCallV2<'_, P>,
	operation: u16,
	audience: Option<&str>,
) -> Result<AuthorizedIdentityCallV2, IdentityAuthorityErrorV2>
where
	R: FinalizedIdentitySourceV2,
	P: Production,
{
	let authority: [u8; 32] =
		call.authority.try_into().map_err(|_| IdentityAuthorityErrorV2::GrantRequired)?;
	let frame_grant: [u8; 32] =
		fixed_field(call.frame.value(), 4, 32)?.try_into().expect("length checked");
	if authority != frame_grant {
		return Err(IdentityAuthorityErrorV2::GrantScopeDenied);
	}
	let grant = core.store.grant(authority)?.ok_or(IdentityAuthorityErrorV2::GrantRequired)?;
	let product_id = text_field(call.frame.value(), 2)?.to_owned();
	let (_, incarnation, _, _) = core.store.root_material()?;
	if grant.operation != operation || grant.product_id != product_id.as_bytes() {
		return Err(IdentityAuthorityErrorV2::GrantScopeDenied);
	}
	if grant.revoked {
		return Err(IdentityAuthorityErrorV2::GrantRevoked);
	}
	if grant.recovery_incarnation != incarnation {
		return Err(IdentityAuthorityErrorV2::OldIncarnation);
	}
	match (audience, grant.audience.as_deref()) {
		(Some(expected), Some(actual)) if actual == expected.as_bytes() => {},
		(None, None) => {},
		_ => return Err(IdentityAuthorityErrorV2::AudienceInvalid),
	}
	let head = core.runtime.head()?;
	if grant.expires_at <= head.block_number {
		return Err(IdentityAuthorityErrorV2::GrantExpired);
	}
	if call.meta.deadline_block <= head.block_number {
		return Err(IdentityAuthorityErrorV2::DeadlineExpired);
	}
	let operation_id_required = matches!(operation, 1102 | 1104 | 1200);
	if operation_id_required != call.meta.operation_id.is_some() {
		return Err(IdentityAuthorityErrorV2::GrantScopeDenied);
	}
	Ok(AuthorizedIdentityCallV2 {
		operation,
		product_id,
		grant,
		request_hash: Sha256::digest(call.frame.canonical()).into(),
		operation_id: call.meta.operation_id,
		head,
	})
}

fn replay(
	store: &IdentityAuthorityStoreV2,
	authorized: &AuthorizedIdentityCallV2,
) -> Result<Option<HostExecutionV2>, IdentityAuthorityErrorV2> {
	let Some(operation_id) = authorized.operation_id else { return Ok(None) };
	let Some(record) = store.operation(operation_id)? else { return Ok(None) };
	validate_replay_record(&record, authorized)?;
	if !record.consent_recorded || record.events.is_empty() {
		return Ok(None);
	}
	if !record.completed {
		store.complete_operation(
			operation_id,
			record.effect_hash,
			record.finalized_number,
			record.finalized_hash,
			record.events.clone(),
		)?;
	}
	Ok(Some(execution_from_events(record.events)?))
}

fn validate_replay_record(
	record: &DurableIdentityOperationV2,
	authorized: &AuthorizedIdentityCallV2,
) -> Result<(), IdentityAuthorityErrorV2> {
	if record.operation != authorized.operation ||
		record.request_hash != authorized.request_hash ||
		record.grant_id != authorized.grant.id
	{
		return Err(IdentityAuthorityErrorV2::EffectConflict);
	}
	Ok(())
}

fn complete_pure_consent<R, H, C, S>(
	core: &mut IdentityAuthorityCoreV2<R, H, C, S>,
	authorized: &AuthorizedIdentityCallV2,
	execution: HostExecutionV2,
	challenge: Option<[u8; 32]>,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2>
where
	C: FreshIdentityConsentV2,
{
	let operation_id = authorized.operation_id.ok_or(IdentityAuthorityErrorV2::GrantScopeDenied)?;
	let result_effect_hash = execution_hash(&execution);
	let external_effect_id = effect_hash(&[
		b"identity-consent",
		&operation_id,
		&authorized.request_hash,
		&authorized.grant.id,
		&core.store.context().genesis_hash,
	]);
	let (prepared, execution) = match core.store.operation(operation_id)? {
		Some(prepared) => {
			validate_replay_record(&prepared, authorized)?;
			if prepared.external_effect_id != external_effect_id || prepared.events.is_empty() {
				return Err(IdentityAuthorityErrorV2::EffectConflict);
			}
			let exact_execution = execution_from_events(prepared.events.clone())?;
			(prepared, exact_execution)
		},
		None => {
			let prepared = DurableIdentityOperationV2 {
				operation_id,
				operation: authorized.operation,
				request_hash: authorized.request_hash,
				grant_id: authorized.grant.id,
				external_effect_id,
				effect_hash: result_effect_hash,
				finalized_number: authorized.head.block_number,
				finalized_hash: authorized.head.block_hash,
				consent_receipt: [0; 32],
				consent_recorded: false,
				events: execution.events.clone(),
				completed: false,
			};
			core.store.prepare_operation(prepared.clone(), challenge)?;
			(prepared, execution)
		},
	};
	validate_replay_record(&prepared, authorized)?;
	if !prepared.consent_recorded {
		let consent = IdentityConsentRequestV2 {
			effect_id: external_effect_id,
			operation_id,
			operation: authorized.operation,
			grant_id: authorized.grant.id,
			request_hash: authorized.request_hash,
			effect_hash: prepared.effect_hash,
			finalized: FinalizedIdentityEffectV2 {
				block_number: prepared.finalized_number,
				block_hash: prepared.finalized_hash,
			},
		};
		let consent_receipt = core.consent.recover_or_consume(external_effect_id, &consent)?;
		core.store.record_consent(operation_id, consent_receipt)?;
	}
	core.store.complete_operation(
		operation_id,
		prepared.effect_hash,
		prepared.finalized_number,
		prepared.finalized_hash,
		execution.events.clone(),
	)?;
	Ok(execution)
}

fn validate_query_finality<R: FinalizedIdentitySourceV2>(
	runtime: &mut R,
	head: FinalizedIdentityEffectV2,
	requested_hash: Option<[u8; 32]>,
	actual: FinalizedIdentityEffectV2,
) -> Result<(), IdentityAuthorityErrorV2> {
	if let Some(hash) = requested_hash {
		if actual.block_hash != hash || actual.block_number > head.block_number {
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
	} else if actual != head {
		return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
	}
	if !runtime.is_finalized(actual)? {
		return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
	}
	Ok(())
}

fn derive_subject(
	store: &IdentityAuthorityStoreV2,
	product_id: &str,
	context: &str,
	audience: &str,
	requested_epoch: Option<u32>,
) -> Result<DerivedSubjectV2, IdentityAuthorityErrorV2> {
	let (master_seed, incarnation, epoch, continuity) = store.root_material()?;
	if requested_epoch.is_some_and(|requested| requested != epoch) {
		return Err(IdentityAuthorityErrorV2::EpochInvalid);
	}
	let context_value = map(vec![
		(0, uint(2)),
		(1, Value::Bytes(store.context().genesis_hash.to_vec())),
		(2, Value::Text(product_id.to_owned())),
		(3, Value::Text(context.to_owned())),
		(4, Value::Text(audience.to_owned())),
		(5, uint(epoch.into())),
		(6, Value::Bytes(incarnation.to_vec())),
		(7, Value::Bool(continuity)),
	]);
	let context_bytes = Dto::<SubjectContextV2>::from_value(context_value)
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?
		.canonical()
		.to_vec();
	let salt: [u8; 32] = Sha256::digest(SUBJECT_KDF_DOMAIN).into();
	let mut seed = [0; 32];
	Hkdf::<Sha256>::new(Some(&salt), &master_seed)
		.expand(&context_bytes, &mut seed)
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	let signing = SigningKey::from_bytes(&seed);
	let public_key = signing.verifying_key().to_bytes();
	let mut subject_input = Vec::with_capacity(SUBJECT_ID_DOMAIN.len() + 64);
	subject_input.extend_from_slice(SUBJECT_ID_DOMAIN);
	subject_input.extend_from_slice(&incarnation);
	subject_input.extend_from_slice(&public_key);
	let subject = sp_crypto_hashing::blake2_256(&subject_input);
	Ok(DerivedSubjectV2 { subject, public_key, seed, epoch, incarnation, continuity })
}

#[allow(clippy::too_many_arguments)]
fn subject_proof(
	genesis_hash: [u8; 32],
	product_id: &str,
	context: &str,
	audience: &str,
	derived: &DerivedSubjectV2,
	challenge: &[u8],
	issued_at: u64,
	expires_at: u64,
	nonce: [u8; 16],
) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
	let proof_value = map(vec![
		(0, uint(2)),
		(1, Value::Bytes(genesis_hash.to_vec())),
		(2, Value::Text(product_id.to_owned())),
		(3, Value::Text(context.to_owned())),
		(4, Value::Text(audience.to_owned())),
		(5, uint(derived.epoch.into())),
		(6, Value::Bytes(derived.incarnation.to_vec())),
		(7, Value::Bool(derived.continuity)),
		(8, Value::Bytes(derived.subject.to_vec())),
		(9, Value::Bytes(derived.public_key.to_vec())),
		(10, Value::Bytes(challenge.to_vec())),
		(11, uint(issued_at)),
		(12, uint(expires_at)),
		(13, Value::Bytes(nonce.to_vec())),
	]);
	let proof = Dto::<SubjectProofV2>::from_value(proof_value)
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	let mut signed = Vec::with_capacity(SUBJECT_PROOF_DOMAIN.len() + proof.canonical().len());
	signed.extend_from_slice(SUBJECT_PROOF_DOMAIN);
	signed.extend_from_slice(proof.canonical());
	let signature = SigningKey::from_bytes(&derived.seed).sign(&signed).to_bytes();
	Dto::<SubjectProofEnvelopeV2>::from_value(map(vec![
		(0, proof.value().clone()),
		(1, Value::Bytes(signature.to_vec())),
	]))
	.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)
	.map(|envelope| envelope.canonical().to_vec())
}

fn challenge_key(
	genesis_hash: [u8; 32],
	audience: &str,
	incarnation: [u8; 32],
	epoch: u32,
	challenge: &[u8],
) -> [u8; 32] {
	effect_hash(&[
		CHALLENGE_DOMAIN,
		&genesis_hash,
		audience.as_bytes(),
		&incarnation,
		&epoch.to_be_bytes(),
		&Sha256::digest(challenge),
	])
}

fn claims_commitment(claims: &[String]) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(b"cord.identity.humanity.claims.v2");
	for claim in claims {
		hash.update((claim.len() as u64).to_be_bytes());
		hash.update(claim.as_bytes());
	}
	hash.finalize().into()
}

fn read_execution<A: Production, R: Production>(
	request_id: [u8; 16],
	result: Value,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
	execution::<A, R>(request_id, result, false)
}

fn consent_execution<A: Production, P: Production, R: Production>(
	request_id: [u8; 16],
	result: Value,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
	let accepted_payload = map(vec![(0, uint(0))]);
	Dto::<A>::from_value(accepted_payload.clone())
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	let progress_payload = map(vec![(0, uint(1))]);
	Dto::<P>::from_value(progress_payload.clone())
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	Dto::<R>::from_value(result.clone())
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	let events = vec![
		accepted_event(request_id, 0, accepted_payload)?,
		progress_event(request_id, 1, progress_payload)?,
		result_event(request_id, 2, result)?,
	];
	Ok(execution_from_events(events)?)
}

fn authority_result<E: Production>(
	request_id: [u8; 16],
	operation: u16,
	result: Result<HostExecutionV2, IdentityAuthorityErrorV2>,
) -> Result<HostExecutionV2, HostExecutionErrorV2> {
	match result {
		Ok(execution) => Ok(execution),
		Err(error) => error_execution::<E>(request_id, operation, error)
			.map_err(|_| HostExecutionErrorV2::Registry),
	}
}

fn error_execution<E: Production>(
	request_id: [u8; 16],
	operation: u16,
	error: IdentityAuthorityErrorV2,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
	let code = error.code(operation) as u16;
	let operation = OPERATIONS
		.iter()
		.find(|binding| binding.code == operation)
		.ok_or(IdentityAuthorityErrorV2::Corrupt)?;
	if !operation.allowed_errors.contains(&code) {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	let binding = ERRORS
		.iter()
		.find(|binding| binding.code == code)
		.ok_or(IdentityAuthorityErrorV2::Corrupt)?;
	let payload = map(vec![
		(0, uint(code.into())),
		(1, Value::Text(binding.name.into())),
		(2, Value::Bool(binding.retryable)),
		(3, Value::Map(Vec::new())),
	]);
	Dto::<E>::from_value(payload.clone()).map_err(|_| IdentityAuthorityErrorV2::Corrupt)?;
	let event = Dto::<ErrorEventV2>::from_value(event_value(request_id, 0, 3, payload))
		.map_err(|_| IdentityAuthorityErrorV2::Corrupt)?
		.canonical()
		.to_vec();
	Ok(HostExecutionV2 {
		terminal_response_hash: Some(Sha256::digest(&event).into()),
		events: vec![event],
	})
}

fn execution<A: Production, R: Production>(
	request_id: [u8; 16],
	result: Value,
	_progress: bool,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
	let accepted_payload = map(vec![(0, uint(0))]);
	Dto::<A>::from_value(accepted_payload.clone())
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	Dto::<R>::from_value(result.clone())
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)?;
	let events = vec![
		accepted_event(request_id, 0, accepted_payload)?,
		result_event(request_id, 1, result)?,
	];
	Ok(execution_from_events(events)?)
}

fn execution_from_events(
	events: Vec<Vec<u8>>,
) -> Result<HostExecutionV2, IdentityAuthorityErrorV2> {
	let terminal = events.last().ok_or(IdentityAuthorityErrorV2::Corrupt)?;
	Dto::<ResultEventV2>::decode(terminal).map_err(|_| IdentityAuthorityErrorV2::Corrupt)?;
	Ok(HostExecutionV2 { terminal_response_hash: Some(Sha256::digest(terminal).into()), events })
}

fn accepted_event(
	request_id: [u8; 16],
	sequence: u64,
	payload: Value,
) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
	Dto::<AcceptedEventV2>::from_value(event_value(request_id, sequence, 0, payload))
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)
		.map(|event| event.canonical().to_vec())
}

fn progress_event(
	request_id: [u8; 16],
	sequence: u64,
	payload: Value,
) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
	Dto::<ProgressEventV2>::from_value(event_value(request_id, sequence, 1, payload))
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)
		.map(|event| event.canonical().to_vec())
}

fn result_event(
	request_id: [u8; 16],
	sequence: u64,
	payload: Value,
) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
	Dto::<ResultEventV2>::from_value(event_value(request_id, sequence, 2, payload))
		.map_err(|_| IdentityAuthorityErrorV2::AuthorityUnavailable)
		.map(|event| event.canonical().to_vec())
}

fn event_value(request_id: [u8; 16], sequence: u64, kind: u64, payload: Value) -> Value {
	map(vec![
		(0, uint(2)),
		(1, Value::Bytes(request_id.to_vec())),
		(2, uint(sequence)),
		(3, uint(kind)),
		(4, payload),
	])
}

fn execution_hash(execution: &HostExecutionV2) -> [u8; 32] {
	let parts = execution.events.iter().map(Vec::as_slice).collect::<Vec<_>>();
	effect_hash(&parts)
}

fn effect_hash(parts: &[&[u8]]) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(EFFECT_DOMAIN);
	for part in parts {
		hash.update((part.len() as u64).to_be_bytes());
		hash.update(part);
	}
	hash.finalize().into()
}

fn identity_receipt(
	commitment: [u8; 32],
	valid_until: u64,
	finalized: FinalizedIdentityEffectV2,
) -> Value {
	map(vec![(0, Value::Bytes(commitment.to_vec())), (1, uint(valid_until)), (2, fin(finalized))])
}

fn fin(finalized: FinalizedIdentityEffectV2) -> Value {
	map(vec![(0, uint(finalized.block_number)), (1, Value::Bytes(finalized.block_hash.to_vec()))])
}

fn payload(frame: &Value) -> Result<&Value, IdentityAuthorityErrorV2> {
	field(frame, 8)
}

fn field(value: &Value, wanted: u64) -> Result<&Value, IdentityAuthorityErrorV2> {
	let Value::Map(fields) = value else { return Err(IdentityAuthorityErrorV2::Corrupt) };
	fields
		.iter()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
				.then_some(value)
		})
		.ok_or(IdentityAuthorityErrorV2::Corrupt)
}

fn optional_field(value: &Value, wanted: u64) -> Result<Option<&Value>, IdentityAuthorityErrorV2> {
	let Value::Map(fields) = value else { return Err(IdentityAuthorityErrorV2::Corrupt) };
	Ok(fields.iter().find_map(|(key, value)| {
		matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
			.then_some(value)
	}))
}

fn fixed_field(
	value: &Value,
	wanted: u64,
	length: usize,
) -> Result<&[u8], IdentityAuthorityErrorV2> {
	let Value::Bytes(bytes) = field(value, wanted)? else {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	};
	if bytes.len() != length {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	Ok(bytes)
}

fn optional_fixed_field(
	value: &Value,
	wanted: u64,
	length: usize,
) -> Result<Option<&[u8]>, IdentityAuthorityErrorV2> {
	let Some(value) = optional_field(value, wanted)? else { return Ok(None) };
	let Value::Bytes(bytes) = value else { return Err(IdentityAuthorityErrorV2::Corrupt) };
	if bytes.len() != length {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	Ok(Some(bytes))
}

fn bytes_field(value: &Value, wanted: u64) -> Result<&[u8], IdentityAuthorityErrorV2> {
	let Value::Bytes(bytes) = field(value, wanted)? else {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	};
	Ok(bytes)
}

fn text_field(value: &Value, wanted: u64) -> Result<&str, IdentityAuthorityErrorV2> {
	let Value::Text(text) = field(value, wanted)? else {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	};
	Ok(text)
}

fn text_array(value: &Value, wanted: u64) -> Result<Vec<String>, IdentityAuthorityErrorV2> {
	let Value::Array(values) = field(value, wanted)? else {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	};
	values
		.iter()
		.map(|value| match value {
			Value::Text(value) => Ok(value.clone()),
			_ => Err(IdentityAuthorityErrorV2::Corrupt),
		})
		.collect()
}

fn uint_field(value: &Value, wanted: u64) -> Result<u64, IdentityAuthorityErrorV2> {
	let Value::Integer(value) = field(value, wanted)? else {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	};
	u64::try_from(*value).map_err(|_| IdentityAuthorityErrorV2::Corrupt)
}

fn optional_uint_field(
	value: &Value,
	wanted: u64,
) -> Result<Option<u64>, IdentityAuthorityErrorV2> {
	let Some(value) = optional_field(value, wanted)? else { return Ok(None) };
	let Value::Integer(value) = value else { return Err(IdentityAuthorityErrorV2::Corrupt) };
	u64::try_from(*value).map(Some).map_err(|_| IdentityAuthorityErrorV2::Corrupt)
}

fn uint(value: u64) -> Value {
	Value::Integer(value.into())
}

fn map(fields: Vec<(u64, Value)>) -> Value {
	Value::Map(
		fields
			.into_iter()
			.map(|(key, value)| (Value::Integer(key.into()), value))
			.collect(),
	)
}

fn same_operation(left: &DurableIdentityOperationV2, right: &DurableIdentityOperationV2) -> bool {
	left.operation_id == right.operation_id &&
		left.operation == right.operation &&
		left.request_hash == right.request_hash &&
		left.grant_id == right.grant_id &&
		left.external_effect_id == right.external_effect_id &&
		left.effect_hash == right.effect_hash &&
		left.finalized_number == right.finalized_number &&
		left.finalized_hash == right.finalized_hash &&
		left.events == right.events
}

fn fresh_recovery_root(
	entropy: &mut impl IdentityRecoveryEntropyV2,
	current_tombstone: [u8; 32],
	retired_roots: &BTreeSet<[u8; 32]>,
) -> Result<([u8; 32], [u8; 32]), IdentityAuthorityErrorV2> {
	for _ in 0..RECOVERY_ENTROPY_ATTEMPTS {
		let mut seed = [0; 32];
		let mut incarnation = [0; 32];
		entropy.fill(&mut seed)?;
		entropy.fill(&mut incarnation)?;
		let tombstone = recovery_tombstone(seed, incarnation);
		if seed != [0; 32] &&
			incarnation != [0; 32] &&
			tombstone != current_tombstone &&
			!retired_roots.contains(&tombstone)
		{
			return Ok((seed, incarnation));
		}
	}
	Err(IdentityAuthorityErrorV2::RecoveryEntropyFailed)
}

fn recovery_tombstone(seed: [u8; 32], incarnation: [u8; 32]) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(seed);
	hash.update(incarnation);
	hash.finalize().into()
}

fn recovery_records(
	operation_id: [u8; 16],
	seed: [u8; 32],
	incarnation: [u8; 32],
	receipt_signing_seed: [u8; 32],
	previous_root: Option<[u8; 32]>,
	installed_at: u64,
) -> Result<(Vec<u8>, Vec<u8>), IdentityAuthorityErrorV2> {
	if operation_id == [0; 16] ||
		seed == [0; 32] ||
		incarnation == [0; 32] ||
		receipt_signing_seed == [0; 32]
	{
		return Err(IdentityAuthorityErrorV2::RecoveryEntropyFailed);
	}
	let mut install_fields = vec![
		(0, uint(2)),
		(1, Value::Bytes(operation_id.to_vec())),
		(2, Value::Bytes(seed.to_vec())),
		(3, Value::Bytes(incarnation.to_vec())),
		(4, uint(0)),
		(5, Value::Bool(false)),
	];
	if let Some(previous_root) = previous_root {
		install_fields.push((6, Value::Bytes(previous_root.to_vec())));
	}
	install_fields.push((7, uint(installed_at)));
	let install = Dto::<RecoveryInstallV2>::from_value(map(install_fields))
		.map_err(|_| IdentityAuthorityErrorV2::RecoveryInstallFailed)?;

	let root: [u8; 32] = Sha256::digest(seed).into();
	let mut receipt_fields = vec![
		(0, uint(2)),
		(1, Value::Bytes(operation_id.to_vec())),
		(2, Value::Bytes(root.to_vec())),
		(3, Value::Bytes(incarnation.to_vec())),
		(4, uint(0)),
		(5, Value::Bool(false)),
	];
	if let Some(previous_root) = previous_root {
		receipt_fields.push((6, Value::Bytes(previous_root.to_vec())));
	}
	receipt_fields.push((7, uint(installed_at)));
	let unsigned_bytes = canonical_value(&map(receipt_fields.clone()))?;
	let mut signed = Vec::with_capacity(RECOVERY_RECEIPT_DOMAIN.len() + unsigned_bytes.len());
	signed.extend_from_slice(RECOVERY_RECEIPT_DOMAIN);
	signed.extend_from_slice(&unsigned_bytes);
	let signature = SigningKey::from_bytes(&receipt_signing_seed).sign(&signed).to_bytes();
	receipt_fields.push((8, Value::Bytes(signature.to_vec())));
	let receipt = Dto::<RecoveryReceiptV2>::from_value(map(receipt_fields))
		.map_err(|_| IdentityAuthorityErrorV2::RecoveryInstallFailed)?;
	Ok((install.canonical().to_vec(), receipt.canonical().to_vec()))
}

fn canonical_value(value: &Value) -> Result<Vec<u8>, IdentityAuthorityErrorV2> {
	let mut bytes = Vec::new();
	ciborium::ser::into_writer(value, &mut bytes)
		.map_err(|_| IdentityAuthorityErrorV2::RecoveryInstallFailed)?;
	Ok(bytes)
}

fn recovery_install_error(error: IdentityAuthorityErrorV2) -> IdentityAuthorityErrorV2 {
	match error {
		IdentityAuthorityErrorV2::Unavailable |
		IdentityAuthorityErrorV2::Corrupt |
		IdentityAuthorityErrorV2::Full => IdentityAuthorityErrorV2::RecoveryInstallFailed,
		other => other,
	}
}

fn validate_state_key(state_key: [u8; 32]) -> Result<(), IdentityAuthorityErrorV2> {
	if state_key == [0; 32] {
		return Err(IdentityAuthorityErrorV2::KeystoreUnavailable);
	}
	Ok(())
}

fn validate_recovery_receipt_signing_seed(
	recovery_receipt_signing_seed: [u8; 32],
) -> Result<(), IdentityAuthorityErrorV2> {
	if recovery_receipt_signing_seed == [0; 32] {
		return Err(IdentityAuthorityErrorV2::KeystoreUnavailable);
	}
	Ok(())
}

fn recovery_receipt_public_key(recovery_receipt_signing_seed: [u8; 32]) -> [u8; 32] {
	SigningKey::from_bytes(&recovery_receipt_signing_seed)
		.verifying_key()
		.to_bytes()
}

fn validate_keystore(keystore: IdentityKeystoreMaterialV2) -> Result<(), IdentityAuthorityErrorV2> {
	validate_state_key(keystore.state_key)?;
	validate_recovery_receipt_signing_seed(keystore.recovery_receipt_signing_seed)?;
	if keystore.subject_master_seed == [0; 32] || keystore.recovery_incarnation == [0; 32] {
		return Err(IdentityAuthorityErrorV2::KeystoreUnavailable);
	}
	Ok(())
}

fn grants_map(
	grants: Vec<IdentityGrantRecordV2>,
	incarnation: [u8; 32],
) -> Result<BTreeMap<[u8; 32], IdentityGrantRecordV2>, IdentityAuthorityErrorV2> {
	if grants.len() > MAX_GRANTS {
		return Err(IdentityAuthorityErrorV2::Full);
	}
	let mut output = BTreeMap::new();
	for grant in grants {
		if grant.recovery_incarnation != incarnation {
			return Err(IdentityAuthorityErrorV2::OldIncarnation);
		}
		if grant.id == [0; 32] ||
			grant.product_id.is_empty() ||
			grant.product_id.len() > 128 ||
			grant
				.audience
				.as_ref()
				.is_some_and(|audience| audience.is_empty() || audience.len() > 256) ||
			output.insert(grant.id, grant).is_some()
		{
			return Err(IdentityAuthorityErrorV2::Corrupt);
		}
	}
	Ok(output)
}

fn state_key(
	root_key: [u8; 32],
	context: IdentityAuthorityContextV2,
) -> Result<[u8; 32], IdentityAuthorityErrorV2> {
	let salt: [u8; 32] = Sha256::digest(STATE_KEY_DOMAIN).into();
	let mut info = Vec::with_capacity(STATE_KEY_DOMAIN.len() + 64);
	info.extend_from_slice(STATE_KEY_DOMAIN);
	info.extend_from_slice(&context.profile_id);
	info.extend_from_slice(&context.genesis_hash);
	let mut key = [0; 32];
	Hkdf::<Sha256>::new(Some(&salt), &root_key)
		.expand(&info, &mut key)
		.map_err(|_| IdentityAuthorityErrorV2::KeystoreUnavailable)?;
	Ok(key)
}

fn state_aad(context: IdentityAuthorityContextV2) -> Vec<u8> {
	let mut aad = Vec::with_capacity(STATE_AAD_DOMAIN.len() + 64);
	aad.extend_from_slice(STATE_AAD_DOMAIN);
	aad.extend_from_slice(&context.profile_id);
	aad.extend_from_slice(&context.genesis_hash);
	aad
}

fn state_nonce(revision: u64, plaintext: &[u8]) -> [u8; 24] {
	let mut hash = Sha256::new();
	hash.update(STATE_NONCE_DOMAIN);
	hash.update(revision.to_be_bytes());
	hash.update(Sha256::digest(plaintext));
	hash.finalize()[..24].try_into().expect("SHA-256 has 24 bytes")
}

fn load_state(
	path: &Path,
	key: &[u8; 32],
	aad: &[u8],
	context: IdentityAuthorityContextV2,
) -> Result<DurableIdentityStateV2, IdentityAuthorityErrorV2> {
	let metadata = fs::symlink_metadata(path).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_STATE_BYTES
	{
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	let mut bytes = Vec::with_capacity(metadata.len() as usize);
	File::open(path)
		.and_then(|mut file| file.read_to_end(&mut bytes))
		.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	let plaintext = decrypt(&bytes, key, aad)?;
	let state = DurableIdentityStateV2::decode(&mut plaintext.as_slice())
		.map_err(|_| IdentityAuthorityErrorV2::Corrupt)?;
	if state.encode() != plaintext {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	state.validate(context)?;
	Ok(state)
}

fn quarantine(root: &Path, path: &Path) -> Result<(), IdentityAuthorityErrorV2> {
	let bytes = fs::read(path).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	let marker = root.join(QUARANTINE_MARKER);
	if !marker.exists() {
		let mut file = OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(&marker)
			.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		set_private_file(&file)?;
		file.write_all(&Sha256::digest(&bytes))
			.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		file.sync_all().map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		sync_dir(root)?;
	}
	let name = format!("{}.identity", hex::encode(Sha256::digest(&bytes)));
	fs::rename(path, root.join(QUARANTINE).join(name))
		.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	sync_dir(&root.join(QUARANTINE))?;
	sync_dir(root)?;
	Ok(())
}

fn cleanup_stale_temporaries(root: &Path) -> Result<(), IdentityAuthorityErrorV2> {
	let prefix = format!(".{STATE}.");
	let mut removed = false;
	for entry in fs::read_dir(root).map_err(|_| IdentityAuthorityErrorV2::Unavailable)? {
		let entry = entry.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		let name = entry.file_name();
		let name = name.to_string_lossy();
		if !name.starts_with(&prefix) || !name.ends_with(".tmp") {
			continue;
		}
		let metadata = entry.metadata().map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		if !metadata.is_file() ||
			entry
				.file_type()
				.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?
				.is_symlink()
		{
			return Err(IdentityAuthorityErrorV2::Corrupt);
		}
		fs::remove_file(entry.path()).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		removed = true;
	}
	if removed {
		sync_dir(root)?;
	}
	Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), IdentityAuthorityErrorV2> {
	fs::create_dir_all(path).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	let metadata = fs::symlink_metadata(path).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	if metadata.file_type().is_symlink() || !metadata.is_dir() {
		return Err(IdentityAuthorityErrorV2::Corrupt);
	}
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;
		fs::set_permissions(path, fs::Permissions::from_mode(0o700))
			.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	}
	Ok(())
}

fn set_private_file(file: &File) -> Result<(), IdentityAuthorityErrorV2> {
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;
		file.set_permissions(fs::Permissions::from_mode(0o600))
			.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
	use std::{
		collections::VecDeque,
		sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
	};

	use super::*;
	use crate::product_sdk::host_v2::execution::{HostRequestMetaV2, ProviderOutboxContextV2};

	fn context() -> IdentityAuthorityContextV2 {
		IdentityAuthorityContextV2 { profile_id: [1; 32], genesis_hash: [2; 32] }
	}

	fn keystore() -> IdentityKeystoreMaterialV2 {
		IdentityKeystoreMaterialV2 {
			state_key: [3; 32],
			recovery_receipt_signing_seed: [7; 32],
			subject_master_seed: [4; 32],
			recovery_incarnation: [5; 32],
			epoch: 7,
			continuity: true,
		}
	}

	fn grant() -> IdentityGrantRecordV2 {
		IdentityGrantRecordV2 {
			id: [6; 32],
			product_id: b"festival".to_vec(),
			operation: 1102,
			recovery_incarnation: [5; 32],
			expires_at: 200,
			audience: Some(b"festival.example".to_vec()),
			revoked: false,
		}
	}

	fn operation() -> DurableIdentityOperationV2 {
		DurableIdentityOperationV2 {
			operation_id: [7; 16],
			operation: 1102,
			request_hash: [8; 32],
			grant_id: [6; 32],
			external_effect_id: [16; 32],
			effect_hash: [9; 32],
			finalized_number: 100,
			finalized_hash: [10; 32],
			consent_receipt: [11; 32],
			consent_recorded: true,
			events: vec![b"accepted".to_vec(), b"result".to_vec()],
			completed: false,
		}
	}

	struct ScriptedRecoveryEntropyV2 {
		values: VecDeque<Result<[u8; 32], ()>>,
		calls: usize,
	}

	impl ScriptedRecoveryEntropyV2 {
		fn new(values: impl IntoIterator<Item = [u8; 32]>) -> Self {
			Self { values: values.into_iter().map(Ok).collect(), calls: 0 }
		}
	}

	impl IdentityRecoveryEntropyV2 for ScriptedRecoveryEntropyV2 {
		fn fill(&mut self, output: &mut [u8; 32]) -> Result<(), IdentityAuthorityErrorV2> {
			self.calls += 1;
			match self.values.pop_front() {
				Some(Ok(value)) => {
					*output = value;
					Ok(())
				},
				_ => Err(IdentityAuthorityErrorV2::RecoveryEntropyFailed),
			}
		}
	}

	#[test]
	fn required_keystore_and_grant_deliveries_fail_closed() {
		let root = tempfile::tempdir().unwrap();
		assert!(matches!(
			IdentityAuthorityStoreV2::open(root.path(), context(), None, Some(vec![grant()])),
			Err(IdentityAuthorityErrorV2::KeystoreUnavailable)
		));
		assert!(matches!(
			IdentityAuthorityStoreV2::open(root.path(), context(), Some(keystore()), None),
			Err(IdentityAuthorityErrorV2::GrantsUnavailable)
		));
	}

	#[test]
	fn encrypted_state_restores_exact_prepared_and_completed_effects() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		store.prepare_operation(operation(), Some([12; 32])).unwrap();
		drop(store);

		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		assert_eq!(store.operation([7; 16]).unwrap(), Some(operation()));
		store.prepare_operation(operation(), Some([12; 32])).unwrap();
		store
			.complete_operation(
				[7; 16],
				[13; 32],
				101,
				[14; 32],
				vec![b"accepted".to_vec(), b"result-final".to_vec()],
			)
			.unwrap();
		drop(store);

		let restored = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap()
		.operation([7; 16])
		.unwrap()
		.unwrap();
		assert!(restored.completed);
		assert_eq!(restored.effect_hash, [13; 32]);
		assert_eq!(restored.finalized_number, 101);
		assert_eq!(restored.events[1], b"result-final");

		let mut changed = operation();
		changed.request_hash = [15; 32];
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		assert_eq!(
			store.prepare_operation(changed, None),
			Err(IdentityAuthorityErrorV2::EffectConflict)
		);
	}

	#[test]
	fn authenticated_corruption_is_quarantined_without_starting_fresh() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let state_path = store.root.join(STATE);
		drop(store);
		let mut bytes = fs::read(&state_path).unwrap();
		*bytes.last_mut().unwrap() ^= 1;
		fs::write(&state_path, bytes).unwrap();
		assert!(matches!(
			IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(keystore()),
				Some(vec![grant()]),
			),
			Err(IdentityAuthorityErrorV2::Corrupt)
		));
		assert!(!state_path.exists());
		assert_eq!(fs::read_dir(root.path().join(ROOT).join(QUARANTINE)).unwrap().count(), 1);
		assert!(root.path().join(ROOT).join(QUARANTINE_MARKER).exists());
		assert!(matches!(
			IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(keystore()),
				Some(vec![grant()]),
			),
			Err(IdentityAuthorityErrorV2::Corrupt)
		));
		IdentityAuthorityStoreV2::authorize_recovery_install(root.path()).unwrap();
		IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
	}

	#[test]
	fn stale_atomic_temporary_is_removed_before_retrying_state() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let temporary = store.root.join(format!(".{STATE}.{}.tmp", 99));
		fs::write(&temporary, b"interrupted").unwrap();
		drop(store);
		let restored = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		assert!(!temporary.exists());
		assert!(restored.grant([6; 32]).unwrap().is_some());
	}

	#[test]
	fn recovery_install_and_receipt_match_the_frozen_canonical_vector() {
		let (install, receipt) =
			recovery_records([0x44; 16], [0x91; 32], [0x92; 32], [0x93; 32], Some([0x11; 32]), 100)
				.unwrap();
		let vectors: serde_json::Value =
			serde_json::from_str(include_str!("../../../../docs/specs/identity-v2.vectors.json"))
				.unwrap();
		let vector = vectors["executable_vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|vector| vector["id"] == "recovery-install-v2")
			.unwrap();
		let crypto = &vector["crypto"];
		let public_key: [u8; 32] = hex::decode(crypto["receipt_public_key_hex"].as_str().unwrap())
			.unwrap()
			.try_into()
			.unwrap();
		let signed = hex::decode(crypto["receipt_signed_bytes_hex"].as_str().unwrap()).unwrap();
		let signature = Signature::from_slice(
			&hex::decode(crypto["receipt_signature_hex"].as_str().unwrap()).unwrap(),
		)
		.unwrap();
		assert_eq!(recovery_receipt_public_key([0x93; 32]), public_key);
		VerifyingKey::from_bytes(&public_key)
			.unwrap()
			.verify(&signed, &signature)
			.unwrap();
		assert_eq!(hex::encode(install), vector["canonical_cbor_hex"]);
		assert_eq!(hex::encode(receipt), vector["exact_response_cbor_hex"]);
	}

	#[test]
	fn unproven_recovery_is_fresh_idempotent_and_restart_safe_without_grant_inheritance() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let mut no_entropy = ScriptedRecoveryEntropyV2::new([]);
		assert_eq!(
			store.recover_unproven_with([0; 16], 100, &mut no_entropy),
			Err(IdentityAuthorityErrorV2::RecoveryInstallFailed)
		);
		assert_eq!(no_entropy.calls, 0);
		assert!(store.grant([6; 32]).unwrap().is_some());
		store.prepare_operation(operation(), Some([12; 32])).unwrap();

		let mut entropy = ScriptedRecoveryEntropyV2::new([[0x91; 32], [0x92; 32]]);
		let receipt = store.recover_unproven_with([0x44; 16], 100, &mut entropy).unwrap();
		assert_eq!(entropy.calls, 2);
		assert_eq!(store.root_material().unwrap(), ([0x91; 32], [0x92; 32], 0, false));
		assert_eq!(store.grant([6; 32]), Err(IdentityAuthorityErrorV2::OldIncarnation));
		assert!(store.operation([7; 16]).unwrap().is_none());
		assert!(!store.challenge_consumed([12; 32]).unwrap());
		assert_eq!(store.retired_root_count().unwrap(), 1);

		let mut unused_entropy = ScriptedRecoveryEntropyV2::new([]);
		assert_eq!(
			store.recover_unproven_with([0x44; 16], 999, &mut unused_entropy).unwrap(),
			receipt
		);
		assert_eq!(unused_entropy.calls, 0);
		drop(store);

		let mut wrong_receipt_authority = keystore();
		wrong_receipt_authority.recovery_receipt_signing_seed = [8; 32];
		assert!(matches!(
			IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(wrong_receipt_authority),
				Some(vec![]),
			),
			Err(IdentityAuthorityErrorV2::KeystoreUnavailable)
		));

		let reopened =
			IdentityAuthorityStoreV2::open(root.path(), context(), Some(keystore()), Some(vec![]))
				.unwrap();
		let mut restart_entropy = ScriptedRecoveryEntropyV2::new([]);
		assert_eq!(
			reopened.recover_unproven_with([0x44; 16], 1_000, &mut restart_entropy).unwrap(),
			receipt
		);
		assert_eq!(restart_entropy.calls, 0);
		assert!(matches!(
			IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(keystore()),
				Some(vec![grant()]),
			),
			Err(IdentityAuthorityErrorV2::OldIncarnation)
		));
	}

	#[test]
	fn recovery_retries_a_retired_root_collision() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let mut first = ScriptedRecoveryEntropyV2::new([[0x61; 32], [0x62; 32]]);
		store.recover_unproven_with([0x4a; 16], 100, &mut first).unwrap();

		let mut second = ScriptedRecoveryEntropyV2::new([[4; 32], [5; 32], [0x63; 32], [0x64; 32]]);
		store.recover_unproven_with([0x4b; 16], 101, &mut second).unwrap();
		assert_eq!(second.calls, 4);
		assert_eq!(store.root_material().unwrap(), ([0x63; 32], [0x64; 32], 0, false));
		assert_eq!(store.retired_root_count().unwrap(), 2);
	}

	#[test]
	fn entropy_failure_persists_a_fail_closed_recovery_barrier_and_retry_can_finish() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let mut zeros = ScriptedRecoveryEntropyV2::new([[0; 32]; 6]);
		assert_eq!(
			store.recover_unproven_with([0x45; 16], 100, &mut zeros),
			Err(IdentityAuthorityErrorV2::RecoveryEntropyFailed)
		);
		assert_eq!(zeros.calls, 6);
		assert_eq!(store.grant([6; 32]), Err(IdentityAuthorityErrorV2::OldIncarnation));
		assert_eq!(store.root_material().unwrap(), ([4; 32], [5; 32], 7, true));
		drop(store);

		let reopened =
			IdentityAuthorityStoreV2::open(root.path(), context(), Some(keystore()), Some(vec![]))
				.unwrap();
		let mut fresh = ScriptedRecoveryEntropyV2::new([[0x61; 32], [0x62; 32]]);
		reopened.recover_unproven_with([0x45; 16], 101, &mut fresh).unwrap();
		assert_eq!(reopened.root_material().unwrap(), ([0x61; 32], [0x62; 32], 0, false));
	}

	#[test]
	fn recovery_retries_collisions_and_install_failure_never_commits_partial_material() {
		for point in [TestPersistFaultPointV2::BeforeWrite, TestPersistFaultPointV2::AfterFileSync]
		{
			let root = tempfile::tempdir().unwrap();
			let store = IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(keystore()),
				Some(vec![grant()]),
			)
			.unwrap();
			store.fail_persist_after(1, point);
			let mut entropy =
				ScriptedRecoveryEntropyV2::new([[4; 32], [5; 32], [0x71; 32], [0x72; 32]]);
			assert_eq!(
				store.recover_unproven_with([0x46; 16], 100, &mut entropy),
				Err(IdentityAuthorityErrorV2::RecoveryInstallFailed)
			);
			assert_eq!(entropy.calls, 4);
			assert_eq!(store.root_material().unwrap(), ([4; 32], [5; 32], 7, true));
			assert_eq!(store.grant([6; 32]), Err(IdentityAuthorityErrorV2::OldIncarnation));
			drop(store);

			let reopened = IdentityAuthorityStoreV2::open(
				root.path(),
				context(),
				Some(keystore()),
				Some(vec![]),
			)
			.unwrap();
			let mut retry = ScriptedRecoveryEntropyV2::new([[0x73; 32], [0x74; 32]]);
			reopened.recover_unproven_with([0x46; 16], 101, &mut retry).unwrap();
			assert_eq!(reopened.root_material().unwrap(), ([0x73; 32], [0x74; 32], 0, false));
		}
	}

	#[test]
	fn lost_success_after_atomic_rename_replays_the_committed_receipt() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		store.fail_persist_after(1, TestPersistFaultPointV2::AfterRename);
		let mut entropy = ScriptedRecoveryEntropyV2::new([[0x75; 32], [0x76; 32]]);
		assert_eq!(
			store.recover_unproven_with([0x49; 16], 100, &mut entropy),
			Err(IdentityAuthorityErrorV2::RecoveryInstallFailed)
		);
		assert_eq!(store.root_material().unwrap(), ([0x75; 32], [0x76; 32], 0, false));

		let mut unused_entropy = ScriptedRecoveryEntropyV2::new([]);
		let receipt = store.recover_unproven_with([0x49; 16], 999, &mut unused_entropy).unwrap();
		assert_eq!(unused_entropy.calls, 0);
		drop(store);

		let reopened =
			IdentityAuthorityStoreV2::open(root.path(), context(), Some(keystore()), Some(vec![]))
				.unwrap();
		let mut restart_entropy = ScriptedRecoveryEntropyV2::new([]);
		assert_eq!(
			reopened.recover_unproven_with([0x49; 16], 1_000, &mut restart_entropy).unwrap(),
			receipt
		);
		assert_eq!(restart_entropy.calls, 0);
	}

	#[test]
	fn retired_root_capacity_blocks_recovery_before_entropy() {
		let root = tempfile::tempdir().unwrap();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let current = recovery_tombstone([4; 32], [5; 32]);
		store
			.mutate(|state| {
				for index in 0..MAX_RETIRED_ROOTS as u32 {
					let mut candidate = [0; 32];
					candidate[..4].copy_from_slice(&index.to_be_bytes());
					if candidate == current {
						candidate[31] = 1;
					}
					state.retired_roots.insert(candidate);
				}
				Ok(())
			})
			.unwrap();
		let mut entropy = ScriptedRecoveryEntropyV2::new([[0x81; 32], [0x82; 32]]);
		assert_eq!(
			store.recover_unproven_with([0x47; 16], 100, &mut entropy),
			Err(IdentityAuthorityErrorV2::RetiredSetFull)
		);
		assert_eq!(entropy.calls, 0);
		assert_eq!(store.root_material().unwrap(), ([4; 32], [5; 32], 7, true));
		assert_eq!(store.retired_root_count().unwrap(), MAX_RETIRED_ROOTS);
		assert_eq!(store.grant([6; 32]), Err(IdentityAuthorityErrorV2::OldIncarnation));
	}

	#[test]
	fn disconnected_recoveries_from_one_backup_install_distinct_roots() {
		let left_root = tempfile::tempdir().unwrap();
		let right_root = tempfile::tempdir().unwrap();
		let left = IdentityAuthorityStoreV2::open(
			left_root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let right = IdentityAuthorityStoreV2::open(
			right_root.path(),
			context(),
			Some(keystore()),
			Some(vec![grant()]),
		)
		.unwrap();
		let mut left_entropy = ScriptedRecoveryEntropyV2::new([[0x91; 32], [0x92; 32]]);
		let mut right_entropy = ScriptedRecoveryEntropyV2::new([[0xa1; 32], [0xa2; 32]]);
		left.recover_unproven_with([0x48; 16], 100, &mut left_entropy).unwrap();
		right.recover_unproven_with([0x48; 16], 100, &mut right_entropy).unwrap();
		assert_ne!(left.root_material().unwrap().0, right.root_material().unwrap().0);
		assert!(!left.root_material().unwrap().3);
		assert!(!right.root_material().unwrap().3);
	}

	#[derive(Clone)]
	struct RuntimeFixture {
		available: Arc<AtomicBool>,
		head: Arc<AtomicU64>,
	}

	impl FinalizedIdentitySourceV2 for RuntimeFixture {
		fn head(&mut self) -> Result<FinalizedIdentityEffectV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(runtime_finality(self.head.load(Ordering::SeqCst)))
		}

		fn profile(
			&mut self,
			_subject: [u8; 32],
			_fields: &[String],
			_at: Option<[u8; 32]>,
		) -> Result<FinalizedProfileAuthorityV2, IdentityAuthorityErrorV2> {
			Ok(FinalizedProfileAuthorityV2 {
				commitment: [21; 32],
				valid_until: 150,
				finalized: finality(),
			})
		}

		fn humanity_status(
			&mut self,
			_subject: [u8; 32],
			_at: Option<[u8; 32]>,
		) -> Result<FinalizedHumanityAuthorityV2, IdentityAuthorityErrorV2> {
			Ok(humanity_authority_at(self.head.load(Ordering::SeqCst)))
		}

		fn humanity_proof(
			&mut self,
			_product_id: &str,
			_audience: &str,
			_claims: &[String],
		) -> Result<FinalizedHumanityAuthorityV2, IdentityAuthorityErrorV2> {
			Ok(humanity_authority_at(self.head.load(Ordering::SeqCst)))
		}

		fn entitlement(
			&mut self,
			_subject: [u8; 32],
			scope: &str,
			_at: Option<[u8; 32]>,
		) -> Result<FinalizedEntitlementAuthorityV2, IdentityAuthorityErrorV2> {
			Ok(FinalizedEntitlementAuthorityV2 {
				allowed: true,
				scope: scope.into(),
				policy_version: 3,
				expires_at: 150,
				fresh_until: 140,
				finalized: finality(),
			})
		}

		fn is_finalized(
			&mut self,
			finality: FinalizedIdentityEffectV2,
		) -> Result<bool, IdentityAuthorityErrorV2> {
			Ok(finality == super::tests::finality() ||
				finality == signing_finality() ||
				finality == runtime_finality(finality.block_number))
		}
	}

	#[derive(Clone)]
	struct HostFixture {
		available: Arc<AtomicBool>,
		calls: Arc<AtomicUsize>,
		effects_applied: Arc<AtomicUsize>,
		fail_after_effect_once: Arc<AtomicBool>,
		effects: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], HostProfileDisclosureV2)>>>,
	}

	impl HostIdentityDeliveryV2 for HostFixture {
		fn account(
			&mut self,
			_session: &str,
		) -> Result<HostAccountSessionV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(HostAccountSessionV2 { account: [31; 32], expires_at: 150 })
		}

		fn recover_or_disclose(
			&mut self,
			effect_id: [u8; 32],
			request: &ProfileDisclosureRequestV2,
		) -> Result<HostProfileDisclosureV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			if effect_id != request.effect_id {
				return Err(IdentityAuthorityErrorV2::EffectConflict);
			}
			self.calls.fetch_add(1, Ordering::SeqCst);
			let binding = effect_hash(&[
				&request.operation_id,
				&request.request_hash,
				&request.grant_id,
				&request.consent_receipt,
				&request.expires_at.to_be_bytes(),
				&request.intent_finality.block_number.to_be_bytes(),
				&request.intent_finality.block_hash,
			]);
			let mut effects = self.effects.lock().unwrap();
			if let Some((existing_binding, disclosure)) = effects.get(&effect_id) {
				return if *existing_binding == binding {
					Ok(disclosure.clone())
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			let disclosure = HostProfileDisclosureV2 {
				commitment: [32; 32],
				valid_until: 130,
				finalized: request.intent_finality,
			};
			effects.insert(effect_id, (binding, disclosure.clone()));
			self.effects_applied.fetch_add(1, Ordering::SeqCst);
			if self.fail_after_effect_once.swap(false, Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(disclosure)
		}
	}

	#[derive(Clone)]
	struct ConsentFixture {
		available: Arc<AtomicBool>,
		calls: Arc<AtomicUsize>,
		receipts: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], [u8; 32])>>>,
	}

	impl FreshIdentityConsentV2 for ConsentFixture {
		fn recover_or_consume(
			&mut self,
			effect_id: [u8; 32],
			request: &IdentityConsentRequestV2,
		) -> Result<[u8; 32], IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			if effect_id != request.effect_id {
				return Err(IdentityAuthorityErrorV2::EffectConflict);
			}
			let binding = effect_hash(&[
				&request.operation_id,
				&request.operation.to_be_bytes(),
				&request.request_hash,
				&request.grant_id,
				&request.effect_hash,
				&request.finalized.block_number.to_be_bytes(),
				&request.finalized.block_hash,
			]);
			let mut receipts = self.receipts.lock().unwrap();
			if let Some((existing_binding, receipt)) = receipts.get(&effect_id) {
				return if *existing_binding == binding {
					Ok(*receipt)
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			self.calls.fetch_add(1, Ordering::SeqCst);
			let receipt = effect_hash(&[
				b"consent",
				&request.operation_id,
				&request.grant_id,
				&request.effect_hash,
			]);
			receipts.insert(effect_id, (binding, receipt));
			Ok(receipt)
		}
	}

	#[derive(Clone)]
	struct SignerFixture {
		available: Arc<AtomicBool>,
		fail_after_effect_once: Arc<AtomicBool>,
		calls: Arc<AtomicUsize>,
		effects_applied: Arc<AtomicUsize>,
		effects: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], FinalizedTransactionEffectV2)>>>,
	}

	impl FinalizedTransactionSignerV2 for SignerFixture {
		fn recover_or_sign_and_finalize(
			&mut self,
			effect_id: [u8; 32],
			request: &TransactionSigningRequestV2,
		) -> Result<FinalizedTransactionEffectV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			if effect_id != request.effect_id {
				return Err(IdentityAuthorityErrorV2::EffectConflict);
			}
			self.calls.fetch_add(1, Ordering::SeqCst);
			let binding = effect_hash(&[
				&request.operation_id,
				&request.grant_id,
				&request.genesis_hash,
				&request.payload_hash,
				&request.policy_hash,
				&request.consent_receipt,
				&request.expires_at.to_be_bytes(),
				&request.intent_finality.block_number.to_be_bytes(),
				&request.intent_finality.block_hash,
			]);
			let mut effects = self.effects.lock().unwrap();
			if let Some((existing_binding, effect)) = effects.get(&effect_id) {
				return if *existing_binding == binding {
					Ok(effect.clone())
				} else {
					Err(IdentityAuthorityErrorV2::EffectConflict)
				};
			}
			let effect = FinalizedTransactionEffectV2 {
				transaction_hash: effect_hash(&[
					b"transaction",
					&request.payload_hash,
					&request.policy_hash,
					&request.consent_receipt,
				]),
				finalized: signing_finality(),
			};
			effects.insert(effect_id, (binding, effect.clone()));
			self.effects_applied.fetch_add(1, Ordering::SeqCst);
			if self.fail_after_effect_once.swap(false, Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(effect)
		}
	}

	#[derive(Clone)]
	struct FixtureControls {
		runtime_available: Arc<AtomicBool>,
		runtime_head: Arc<AtomicU64>,
		host_available: Arc<AtomicBool>,
		host_calls: Arc<AtomicUsize>,
		host_effects_applied: Arc<AtomicUsize>,
		host_fail_after_effect_once: Arc<AtomicBool>,
		host_effects: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], HostProfileDisclosureV2)>>>,
		consent_available: Arc<AtomicBool>,
		consent_calls: Arc<AtomicUsize>,
		consent_receipts: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], [u8; 32])>>>,
		signer_available: Arc<AtomicBool>,
		signer_fail_after_effect_once: Arc<AtomicBool>,
		signer_calls: Arc<AtomicUsize>,
		signer_effects_applied: Arc<AtomicUsize>,
		signer_effects: Arc<Mutex<BTreeMap<[u8; 32], ([u8; 32], FinalizedTransactionEffectV2)>>>,
	}

	impl Default for FixtureControls {
		fn default() -> Self {
			Self {
				runtime_available: Arc::new(AtomicBool::new(true)),
				runtime_head: Arc::new(AtomicU64::new(100)),
				host_available: Arc::new(AtomicBool::new(true)),
				host_calls: Arc::new(AtomicUsize::new(0)),
				host_effects_applied: Arc::new(AtomicUsize::new(0)),
				host_fail_after_effect_once: Arc::new(AtomicBool::new(false)),
				host_effects: Arc::new(Mutex::new(BTreeMap::new())),
				consent_available: Arc::new(AtomicBool::new(true)),
				consent_calls: Arc::new(AtomicUsize::new(0)),
				consent_receipts: Arc::new(Mutex::new(BTreeMap::new())),
				signer_available: Arc::new(AtomicBool::new(true)),
				signer_fail_after_effect_once: Arc::new(AtomicBool::new(false)),
				signer_calls: Arc::new(AtomicUsize::new(0)),
				signer_effects_applied: Arc::new(AtomicUsize::new(0)),
				signer_effects: Arc::new(Mutex::new(BTreeMap::new())),
			}
		}
	}

	type FixtureAuthority =
		DurableIdentityAuthorityV2<RuntimeFixture, HostFixture, ConsentFixture, SignerFixture>;

	fn authority(root: &Path, controls: &FixtureControls) -> FixtureAuthority {
		DurableIdentityAuthorityV2::open(
			root,
			context(),
			Some(keystore()),
			Some(all_grants()),
			RuntimeFixture {
				available: Arc::clone(&controls.runtime_available),
				head: Arc::clone(&controls.runtime_head),
			},
			HostFixture {
				available: Arc::clone(&controls.host_available),
				calls: Arc::clone(&controls.host_calls),
				effects_applied: Arc::clone(&controls.host_effects_applied),
				fail_after_effect_once: Arc::clone(&controls.host_fail_after_effect_once),
				effects: Arc::clone(&controls.host_effects),
			},
			ConsentFixture {
				available: Arc::clone(&controls.consent_available),
				calls: Arc::clone(&controls.consent_calls),
				receipts: Arc::clone(&controls.consent_receipts),
			},
			SignerFixture {
				available: Arc::clone(&controls.signer_available),
				fail_after_effect_once: Arc::clone(&controls.signer_fail_after_effect_once),
				calls: Arc::clone(&controls.signer_calls),
				effects_applied: Arc::clone(&controls.signer_effects_applied),
				effects: Arc::clone(&controls.signer_effects),
			},
		)
		.unwrap()
	}

	fn finality() -> FinalizedIdentityEffectV2 {
		FinalizedIdentityEffectV2 { block_number: 100, block_hash: [41; 32] }
	}

	fn runtime_finality(block_number: u64) -> FinalizedIdentityEffectV2 {
		if block_number == 100 {
			return finality();
		}
		FinalizedIdentityEffectV2 {
			block_number,
			block_hash: effect_hash(&[b"fixture-finality", &block_number.to_be_bytes()]),
		}
	}

	fn signing_finality() -> FinalizedIdentityEffectV2 {
		FinalizedIdentityEffectV2 { block_number: 101, block_hash: [42; 32] }
	}

	fn humanity_authority_at(block_number: u64) -> FinalizedHumanityAuthorityV2 {
		FinalizedHumanityAuthorityV2 {
			status: 1,
			fresh_until: 150,
			commitment: [43; 32],
			finalized: runtime_finality(block_number),
		}
	}

	fn grant_id(operation: u16) -> [u8; 32] {
		[operation as u8; 32]
	}

	fn all_grants() -> Vec<IdentityGrantRecordV2> {
		[1100, 1101, 1102, 1103, 1104, 1105, 1106, 1200]
			.into_iter()
			.map(|operation| IdentityGrantRecordV2 {
				id: grant_id(operation),
				product_id: b"festival".to_vec(),
				operation,
				recovery_incarnation: keystore().recovery_incarnation,
				expires_at: 200,
				audience: matches!(operation, 1102 | 1104 | 1105)
					.then(|| b"festival.example".to_vec()),
				revoked: false,
			})
			.collect()
	}

	fn frame<P: Production>(
		operation: u16,
		request_id: [u8; 16],
		operation_id: Option<[u8; 16]>,
		payload: Value,
	) -> Dto<P> {
		let mut fields = vec![
			(0, uint(2)),
			(1, Value::Bytes(request_id.to_vec())),
			(2, Value::Text("festival".into())),
			(3, uint(operation.into())),
			(4, Value::Bytes(grant_id(operation).to_vec())),
		];
		if let Some(operation_id) = operation_id {
			fields.push((5, Value::Bytes(operation_id.to_vec())));
		}
		fields.extend([(7, uint(130)), (8, payload)]);
		Dto::<P>::from_value(map(fields)).unwrap()
	}

	fn meta(request_id: [u8; 16], operation_id: Option<[u8; 16]>) -> HostRequestMetaV2 {
		HostRequestMetaV2 { request_id, operation_id, deadline_block: 130 }
	}

	fn outbox() -> ProviderOutboxContextV2 {
		ProviderOutboxContextV2 {
			outbox_id: [51; 16],
			generation: 0,
			intended_cursor: 0,
			negotiated_tuple: [52; 32],
			provider_id: [53; 32],
			provider_endpoint_hash: [54; 32],
			created_at: 100,
			authority_expires_at: 130,
			terminal_block: 100,
			prepare_nonce: [1; 24],
			mark_sent_nonce: [2; 24],
			install_nonce: [3; 24],
			mark_ack_nonce: [4; 24],
			confirm_nonce: [5; 24],
			compact_nonce: [6; 24],
		}
	}

	fn assert_execution(execution: &HostExecutionV2, expected_events: usize) {
		assert_eq!(execution.events.len(), expected_events);
		for (sequence, event) in execution.events.iter().enumerate() {
			let decoded = Dto::<super::super::generated::EventV2>::decode(event).unwrap();
			assert_eq!(uint_field(decoded.value(), 2).unwrap(), sequence as u64);
		}
		Dto::<ResultEventV2>::decode(execution.events.last().unwrap()).unwrap();
	}

	fn assert_error(execution: &HostExecutionV2, operation: u16, expected: ErrorCode) {
		assert_eq!(execution.events.len(), 1);
		let event = Dto::<ErrorEventV2>::decode(&execution.events[0]).unwrap();
		let payload = field(event.value(), 4).unwrap();
		assert_eq!(uint_field(payload, 0).unwrap(), expected as u64);
		let binding = ERRORS.iter().find(|binding| binding.code == expected as u16).unwrap();
		assert!(OPERATIONS
			.iter()
			.find(|binding| binding.code == operation)
			.unwrap()
			.allowed_errors
			.contains(&(expected as u16)));
		assert_eq!(text_field(payload, 1).unwrap(), binding.name);
		assert_eq!(field(payload, 2).unwrap(), &Value::Bool(binding.retryable));
		assert_eq!(
			execution.terminal_response_hash,
			Some(Sha256::digest(&execution.events[0]).into())
		);
	}

	#[test]
	fn every_authority_failure_uses_an_allowed_generated_error_binding() {
		let common = [
			(IdentityAuthorityErrorV2::KeystoreUnavailable, ErrorCode::HostOutboxUnavailable),
			(IdentityAuthorityErrorV2::GrantsUnavailable, ErrorCode::GrantRequired),
			(IdentityAuthorityErrorV2::Corrupt, ErrorCode::HostOutboxCorrupt),
			(IdentityAuthorityErrorV2::Full, ErrorCode::HostOutboxFull),
			(IdentityAuthorityErrorV2::Unavailable, ErrorCode::HostOutboxUnavailable),
			(IdentityAuthorityErrorV2::GrantRequired, ErrorCode::GrantRequired),
			(IdentityAuthorityErrorV2::GrantScopeDenied, ErrorCode::GrantScopeDenied),
			(IdentityAuthorityErrorV2::GrantExpired, ErrorCode::GrantExpired),
			(IdentityAuthorityErrorV2::GrantRevoked, ErrorCode::GrantRevoked),
			(IdentityAuthorityErrorV2::DeadlineExpired, ErrorCode::RequestDeadlineExpired),
			(IdentityAuthorityErrorV2::AudienceInvalid, ErrorCode::IdentityAudienceInvalid),
			(IdentityAuthorityErrorV2::ChallengeReplay, ErrorCode::IdentityChallengeReplay),
			(IdentityAuthorityErrorV2::ProofExpired, ErrorCode::IdentityProofExpired),
			(IdentityAuthorityErrorV2::OldIncarnation, ErrorCode::IdentityOldIncarnation),
			(IdentityAuthorityErrorV2::EpochInvalid, ErrorCode::IdentityEpochInvalid),
		];
		for (error, expected) in common {
			let execution = error_execution::<IdentityAccountError>([99; 16], 1100, error).unwrap();
			assert_error(&execution, 1100, expected);
		}
		for (operation, expected) in [
			(1100, ErrorCode::IdentityAuthorityUnavailable),
			(1102, ErrorCode::IdentityAuthorityUnavailable),
			(1103, ErrorCode::IdentityAuthorityUnavailable),
			(1104, ErrorCode::IdentityAuthorityUnavailable),
			(1106, ErrorCode::IdentityAuthorityUnavailable),
			(1200, ErrorCode::IdentityAuthorityUnavailable),
		] {
			let execution = error_execution::<IdentityAccountError>(
				[98; 16],
				operation,
				IdentityAuthorityErrorV2::AuthorityUnavailable,
			)
			.unwrap();
			assert_error(&execution, operation, expected);
		}
		let conflict = error_execution::<TransactionSignError>(
			[97; 16],
			1200,
			IdentityAuthorityErrorV2::EffectConflict,
		)
		.unwrap();
		assert_error(&conflict, 1200, ErrorCode::IdentityEffectConflict);
	}

	#[test]
	fn concrete_authority_executes_seven_isolated_identity_operations_and_signing() {
		let root = tempfile::tempdir().unwrap();
		let controls = FixtureControls::default();
		let mut backend = authority(root.path(), &controls);
		let outbox = outbox();

		let account = frame::<IdentityAccountFrame>(
			1100,
			[1; 16],
			None,
			map(vec![(0, Value::Text("selected".into()))]),
		);
		assert_execution(
			&backend
				.identity_account(HostCallV2 {
					frame: &account,
					authority: &grant_id(1100),
					meta: meta([1; 16], None),
					outbox: &outbox,
				})
				.unwrap(),
			2,
		);

		let profile = frame::<IdentityProfileReadFrame>(
			1101,
			[2; 16],
			None,
			map(vec![
				(0, Value::Bytes([61; 32].to_vec())),
				(1, Value::Array(vec![Value::Text("display".into())])),
			]),
		);
		assert_execution(
			&backend
				.identity_profile_read(HostCallV2 {
					frame: &profile,
					authority: &grant_id(1101),
					meta: meta([2; 16], None),
					outbox: &outbox,
				})
				.unwrap(),
			2,
		);

		let disclosure_id = [3; 16];
		let disclosure = frame::<IdentityProfileDiscloseFrame>(
			1102,
			[3; 16],
			Some(disclosure_id),
			map(vec![
				(0, Value::Text("festival.example".into())),
				(1, Value::Array(vec![Value::Text("email".into())])),
				(2, Value::Text("ticket".into())),
				(3, uint(140)),
			]),
		);
		let disclosed = backend
			.identity_profile_disclose(HostCallV2 {
				frame: &disclosure,
				authority: &grant_id(1102),
				meta: meta([3; 16], Some(disclosure_id)),
				outbox: &outbox,
			})
			.unwrap();
		assert_execution(&disclosed, 3);

		let humanity_status = frame::<IdentityHumanityStatusFrame>(
			1103,
			[4; 16],
			None,
			map(vec![(0, Value::Bytes([62; 32].to_vec()))]),
		);
		assert_execution(
			&backend
				.identity_humanity_status(HostCallV2 {
					frame: &humanity_status,
					authority: &grant_id(1103),
					meta: meta([4; 16], None),
					outbox: &outbox,
				})
				.unwrap(),
			2,
		);

		let proof_id = [5; 16];
		let humanity_proof = frame::<IdentityHumanityProveFrame>(
			1104,
			[5; 16],
			Some(proof_id),
			map(vec![
				(0, Value::Text("festival.example".into())),
				(1, Value::Bytes(vec![63; 16])),
				(2, uint(120)),
				(3, Value::Array(vec![Value::Text("adult".into())])),
			]),
		);
		assert_execution(
			&backend
				.identity_humanity_prove(HostCallV2 {
					frame: &humanity_proof,
					authority: &grant_id(1104),
					meta: meta([5; 16], Some(proof_id)),
					outbox: &outbox,
				})
				.unwrap(),
			3,
		);
		let repeated_challenge = frame::<IdentityHumanityProveFrame>(
			1104,
			[55; 16],
			Some([55; 16]),
			map(vec![
				(0, Value::Text("festival.example".into())),
				(1, Value::Bytes(vec![63; 16])),
				(2, uint(120)),
				(3, Value::Array(vec![Value::Text("adult".into())])),
			]),
		);
		let challenge_error = backend
			.identity_humanity_prove(HostCallV2 {
				frame: &repeated_challenge,
				authority: &grant_id(1104),
				meta: meta([55; 16], Some([55; 16])),
				outbox: &outbox,
			})
			.unwrap();
		assert_error(&challenge_error, 1104, ErrorCode::IdentityChallengeReplay);
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 2);

		let subject = frame::<IdentitySubjectDeriveFrame>(
			1105,
			[6; 16],
			None,
			map(vec![
				(0, Value::Text("festival".into())),
				(1, Value::Text("attendee".into())),
				(2, Value::Text("festival.example".into())),
			]),
		);
		assert_execution(
			&backend
				.identity_subject_derive(HostCallV2 {
					frame: &subject,
					authority: &grant_id(1105),
					meta: meta([6; 16], None),
					outbox: &outbox,
				})
				.unwrap(),
			2,
		);

		let entitlement = frame::<IdentityEntitlementsReadFrame>(
			1106,
			[7; 16],
			None,
			map(vec![
				(0, Value::Bytes([64; 32].to_vec())),
				(1, Value::Text("festival.entry".into())),
			]),
		);
		assert_execution(
			&backend
				.identity_entitlements_read(HostCallV2 {
					frame: &entitlement,
					authority: &grant_id(1106),
					meta: meta([7; 16], None),
					outbox: &outbox,
				})
				.unwrap(),
			2,
		);

		let signing_id = [8; 16];
		let signing = frame::<TransactionSignFrame>(
			1200,
			[8; 16],
			Some(signing_id),
			map(vec![
				(0, Value::Bytes([65; 32].to_vec())),
				(1, Value::Bytes([66; 32].to_vec())),
				(2, uint(120)),
			]),
		);
		assert_execution(
			&backend
				.transaction_sign(HostCallV2 {
					frame: &signing,
					authority: &grant_id(1200),
					meta: meta([8; 16], Some(signing_id)),
					outbox: &outbox,
				})
				.unwrap(),
			3,
		);
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 3);

		let grant_error = backend
			.identity_account(HostCallV2 {
				frame: &account,
				authority: &grant_id(1101),
				meta: meta([1; 16], None),
				outbox: &outbox,
			})
			.unwrap();
		assert_error(&grant_error, 1100, ErrorCode::GrantScopeDenied);

		let replay = backend
			.identity_profile_disclose(HostCallV2 {
				frame: &disclosure,
				authority: &grant_id(1102),
				meta: meta([3; 16], Some(disclosure_id)),
				outbox: &outbox,
			})
			.unwrap();
		assert_eq!(replay.events, disclosed.events);
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 3);
	}

	#[test]
	fn signing_lost_success_and_identity_consent_replay_survive_restart() {
		let root = tempfile::tempdir().unwrap();
		let controls = FixtureControls::default();
		controls.signer_fail_after_effect_once.store(true, Ordering::SeqCst);
		let outbox = outbox();
		let operation_id = [71; 16];
		let signing = frame::<TransactionSignFrame>(
			1200,
			[72; 16],
			Some(operation_id),
			map(vec![
				(0, Value::Bytes([73; 32].to_vec())),
				(1, Value::Bytes([74; 32].to_vec())),
				(2, uint(120)),
			]),
		);
		let mut backend = authority(root.path(), &controls);
		let first = backend
			.transaction_sign(HostCallV2 {
				frame: &signing,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			})
			.unwrap();
		assert_error(&first, 1200, ErrorCode::IdentityAuthorityUnavailable);
		drop(backend);
		controls.runtime_head.store(101, Ordering::SeqCst);

		let mut backend = authority(root.path(), &controls);
		let recovered = backend
			.transaction_sign(HostCallV2 {
				frame: &signing,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			})
			.unwrap();
		drop(backend);
		let mut backend = authority(root.path(), &controls);
		let replayed = backend
			.transaction_sign(HostCallV2 {
				frame: &signing,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			})
			.unwrap();
		assert_eq!(recovered.events, replayed.events);
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 1);
		assert_eq!(controls.signer_calls.load(Ordering::SeqCst), 2);
		assert_eq!(controls.signer_effects_applied.load(Ordering::SeqCst), 1);
		assert_eq!(controls.signer_effects.lock().unwrap().len(), 1);

		let changed = frame::<TransactionSignFrame>(
			1200,
			[72; 16],
			Some(operation_id),
			map(vec![
				(0, Value::Bytes([75; 32].to_vec())),
				(1, Value::Bytes([74; 32].to_vec())),
				(2, uint(120)),
			]),
		);
		let conflict = backend
			.transaction_sign(HostCallV2 {
				frame: &changed,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			})
			.unwrap();
		assert_error(&conflict, 1200, ErrorCode::IdentityEffectConflict);
	}

	#[test]
	fn disclosure_lost_success_recovers_one_stable_external_effect_after_restart() {
		let root = tempfile::tempdir().unwrap();
		let controls = FixtureControls::default();
		controls.host_fail_after_effect_once.store(true, Ordering::SeqCst);
		let operation_id = [76; 16];
		let request = frame::<IdentityProfileDiscloseFrame>(
			1102,
			[77; 16],
			Some(operation_id),
			map(vec![
				(0, Value::Text("festival.example".into())),
				(1, Value::Array(vec![Value::Text("email".into())])),
				(2, Value::Text("ticket".into())),
				(3, uint(140)),
			]),
		);
		let mut backend = authority(root.path(), &controls);
		let failed = backend
			.identity_profile_disclose(HostCallV2 {
				frame: &request,
				authority: &grant_id(1102),
				meta: meta([77; 16], Some(operation_id)),
				outbox: &outbox(),
			})
			.unwrap();
		assert_error(&failed, 1102, ErrorCode::IdentityAuthorityUnavailable);
		let stable_effect_id = backend
			.core
			.lock()
			.unwrap()
			.store
			.operation(operation_id)
			.unwrap()
			.unwrap()
			.external_effect_id;
		assert!(controls.host_effects.lock().unwrap().contains_key(&stable_effect_id));
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 1);
		assert_eq!(controls.host_calls.load(Ordering::SeqCst), 1);
		assert_eq!(controls.host_effects_applied.load(Ordering::SeqCst), 1);
		drop(backend);
		controls.runtime_head.store(101, Ordering::SeqCst);

		let mut backend = authority(root.path(), &controls);
		let recovered = backend
			.identity_profile_disclose(HostCallV2 {
				frame: &request,
				authority: &grant_id(1102),
				meta: meta([77; 16], Some(operation_id)),
				outbox: &outbox(),
			})
			.unwrap();
		assert_execution(&recovered, 3);
		assert_eq!(controls.consent_calls.load(Ordering::SeqCst), 1);
		assert_eq!(controls.host_calls.load(Ordering::SeqCst), 2);
		assert_eq!(controls.host_effects_applied.load(Ordering::SeqCst), 1);
		let replay = backend
			.identity_profile_disclose(HostCallV2 {
				frame: &request,
				authority: &grant_id(1102),
				meta: meta([77; 16], Some(operation_id)),
				outbox: &outbox(),
			})
			.unwrap();
		assert_eq!(recovered.events, replay.events);
		assert_eq!(controls.host_calls.load(Ordering::SeqCst), 2);
	}

	#[test]
	fn consent_reservation_survives_pre_and_post_consent_persist_faults() {
		fn proof(request_id: [u8; 16], operation_id: [u8; 16]) -> Dto<IdentityHumanityProveFrame> {
			frame::<IdentityHumanityProveFrame>(
				1104,
				request_id,
				Some(operation_id),
				map(vec![
					(0, Value::Text("festival.example".into())),
					(1, Value::Bytes(vec![91; 16])),
					(2, uint(120)),
					(3, Value::Array(vec![Value::Text("adult".into())])),
				]),
			)
		}

		let before_root = tempfile::tempdir().unwrap();
		let before_controls = FixtureControls::default();
		let mut before = authority(before_root.path(), &before_controls);
		before
			.core
			.lock()
			.unwrap()
			.store
			.fail_persist_after(0, TestPersistFaultPointV2::BeforeWrite);
		let operation_id = [92; 16];
		let request = proof([92; 16], operation_id);
		let failed = before
			.identity_humanity_prove(HostCallV2 {
				frame: &request,
				authority: &grant_id(1104),
				meta: meta([92; 16], Some(operation_id)),
				outbox: &outbox(),
			})
			.unwrap();
		assert_error(&failed, 1104, ErrorCode::HostOutboxUnavailable);
		assert_eq!(before_controls.consent_calls.load(Ordering::SeqCst), 0);
		assert!(before.core.lock().unwrap().store.operation(operation_id).unwrap().is_none());
		assert_execution(
			&before
				.identity_humanity_prove(HostCallV2 {
					frame: &request,
					authority: &grant_id(1104),
					meta: meta([92; 16], Some(operation_id)),
					outbox: &outbox(),
				})
				.unwrap(),
			3,
		);
		assert_eq!(before_controls.consent_calls.load(Ordering::SeqCst), 1);

		let after_root = tempfile::tempdir().unwrap();
		let after_controls = FixtureControls::default();
		let mut after = authority(after_root.path(), &after_controls);
		after
			.core
			.lock()
			.unwrap()
			.store
			.fail_persist_after(1, TestPersistFaultPointV2::AfterFileSync);
		let operation_id = [93; 16];
		let request = proof([93; 16], operation_id);
		let failed = after
			.identity_humanity_prove(HostCallV2 {
				frame: &request,
				authority: &grant_id(1104),
				meta: meta([93; 16], Some(operation_id)),
				outbox: &outbox(),
			})
			.unwrap();
		assert_error(&failed, 1104, ErrorCode::HostOutboxUnavailable);
		assert_eq!(after_controls.consent_calls.load(Ordering::SeqCst), 1);
		assert_eq!(after_controls.consent_receipts.lock().unwrap().len(), 1);
		let reserved = after.core.lock().unwrap().store.operation(operation_id).unwrap().unwrap();
		assert!(!reserved.consent_recorded);
		assert!(after_root.path().join(ROOT).read_dir().unwrap().all(|entry| !entry
			.unwrap()
			.file_name()
			.to_string_lossy()
			.ends_with(".tmp")));
		drop(after);
		after_controls.runtime_head.store(101, Ordering::SeqCst);
		let mut after = authority(after_root.path(), &after_controls);
		assert_execution(
			&after
				.identity_humanity_prove(HostCallV2 {
					frame: &request,
					authority: &grant_id(1104),
					meta: meta([93; 16], Some(operation_id)),
					outbox: &outbox(),
				})
				.unwrap(),
			3,
		);
		assert_eq!(after_controls.consent_calls.load(Ordering::SeqCst), 1);
		assert_eq!(after_controls.consent_receipts.lock().unwrap().len(), 1);
	}

	#[test]
	fn contextual_subjects_are_domain_separated_and_missing_authorities_fail_closed() {
		let root = tempfile::tempdir().unwrap();
		let controls = FixtureControls::default();
		let store = IdentityAuthorityStoreV2::open(
			root.path(),
			context(),
			Some(keystore()),
			Some(all_grants()),
		)
		.unwrap();
		let first =
			derive_subject(&store, "festival", "attendee", "festival.example", None).unwrap();
		let second =
			derive_subject(&store, "festival-two", "attendee", "festival.example", None).unwrap();
		assert_ne!(first.subject, second.subject);
		assert_ne!(first.public_key, second.public_key);
		drop(store);

		controls.runtime_available.store(false, Ordering::SeqCst);
		let mut backend = authority(root.path(), &controls);
		let account = frame::<IdentityAccountFrame>(
			1100,
			[81; 16],
			None,
			map(vec![(0, Value::Text("selected".into()))]),
		);
		let unavailable = backend
			.identity_account(HostCallV2 {
				frame: &account,
				authority: &grant_id(1100),
				meta: meta([81; 16], None),
				outbox: &outbox(),
			})
			.unwrap();
		assert_error(&unavailable, 1100, ErrorCode::IdentityAuthorityUnavailable);
	}
}

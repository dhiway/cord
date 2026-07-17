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
use sha2::{Digest, Sha256};

use crate::product_sdk::host_outbox::{decrypt, encrypt, sync_dir, HostOutboxError};

use super::{
	codec::Dto,
	execution::{
		FinalizedIdentityRuntimeV2, HostCallV2, HostExecutionErrorV2, HostExecutionV2,
		HostIdentityAuthorityV2, HostSigningAuthorityV2,
	},
	generated::{
		AcceptedEventV2, IdentityAccountAccepted, IdentityAccountFrame, IdentityAccountResult,
		IdentityEntitlementsReadAccepted, IdentityEntitlementsReadFrame,
		IdentityEntitlementsReadResult, IdentityHumanityProveAccepted, IdentityHumanityProveFrame,
		IdentityHumanityProveProgress, IdentityHumanityProveResult, IdentityHumanityStatusAccepted,
		IdentityHumanityStatusFrame, IdentityHumanityStatusResult, IdentityProfileDiscloseAccepted,
		IdentityProfileDiscloseFrame, IdentityProfileDiscloseProgress,
		IdentityProfileDiscloseResult, IdentityProfileReadAccepted, IdentityProfileReadFrame,
		IdentityProfileReadResult, IdentitySubjectDeriveAccepted, IdentitySubjectDeriveFrame,
		IdentitySubjectDeriveResult, Production, ProgressEventV2, ResultEventV2, SubjectContextV2,
		SubjectProofEnvelopeV2, SubjectProofV2, TransactionSignAccepted, TransactionSignFrame,
		TransactionSignProgress, TransactionSignResult,
	},
};

const STATE_VERSION: u8 = 2;
const ROOT: &str = "identity-authority-v2";
const STATE: &str = "state.identity";
const QUARANTINE: &str = "quarantine";
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
const PROOF_MAX_BLOCKS: u64 = 128;

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
	#[error("IDENTITY_AUDIENCE_INVALID")]
	AudienceInvalid,
	#[error("IDENTITY_CHALLENGE_REPLAY")]
	ChallengeReplay,
	#[error("IDENTITY_OLD_INCARNATION")]
	OldIncarnation,
	#[error("IDENTITY_EPOCH_INVALID")]
	EpochInvalid,
	#[error("IDENTITY_AUTHORITY_UNAVAILABLE")]
	AuthorityUnavailable,
	#[error("IDENTITY_EFFECT_CONFLICT")]
	EffectConflict,
}

impl IdentityAuthorityErrorV2 {
	fn backend(self) -> HostExecutionErrorV2 {
		let name = match self {
			Self::KeystoreUnavailable => "HOST_IDENTITY_KEYSTORE_UNAVAILABLE",
			Self::GrantsUnavailable => "HOST_IDENTITY_GRANTS_UNAVAILABLE",
			Self::Corrupt => "HOST_IDENTITY_STATE_CORRUPT",
			Self::Full => "HOST_IDENTITY_STATE_FULL",
			Self::Unavailable => "HOST_IDENTITY_STATE_UNAVAILABLE",
			Self::GrantRequired => "GRANT_REQUIRED",
			Self::GrantScopeDenied => "GRANT_SCOPE_DENIED",
			Self::GrantExpired => "GRANT_EXPIRED",
			Self::GrantRevoked => "GRANT_REVOKED",
			Self::AudienceInvalid => "IDENTITY_AUDIENCE_INVALID",
			Self::ChallengeReplay => "IDENTITY_CHALLENGE_REPLAY",
			Self::OldIncarnation => "IDENTITY_OLD_INCARNATION",
			Self::EpochInvalid => "IDENTITY_EPOCH_INVALID",
			Self::AuthorityUnavailable => "IDENTITY_AUTHORITY_UNAVAILABLE",
			Self::EffectConflict => "IDENTITY_EFFECT_CONFLICT",
		};
		HostExecutionErrorV2::Backend(name)
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
pub(crate) struct IdentityConsentRequestV2 {
	pub(crate) operation_id: [u8; 16],
	pub(crate) operation: u16,
	pub(crate) grant_id: [u8; 32],
	pub(crate) request_hash: [u8; 32],
	pub(crate) effect_hash: [u8; 32],
	pub(crate) finalized: FinalizedIdentityEffectV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransactionSigningRequestV2 {
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

pub(crate) trait HostIdentityDeliveryV2: Send {
	fn account(&mut self, session: &str) -> Result<HostAccountSessionV2, IdentityAuthorityErrorV2>;
	fn disclose(
		&mut self,
		product_id: &str,
		audience: &str,
		fields: &[String],
		purpose: &str,
		expires_at: u64,
		finalized: FinalizedIdentityEffectV2,
	) -> Result<HostProfileDisclosureV2, IdentityAuthorityErrorV2>;
}

pub(crate) trait FreshIdentityConsentV2: Send {
	fn consume(
		&mut self,
		request: &IdentityConsentRequestV2,
	) -> Result<[u8; 32], IdentityAuthorityErrorV2>;
}

pub(crate) trait FinalizedTransactionSignerV2: Send {
	fn sign_and_finalize(
		&mut self,
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
	pub(crate) effect_hash: [u8; 32],
	pub(crate) finalized_number: u64,
	pub(crate) finalized_hash: [u8; 32],
	pub(crate) consent_receipt: [u8; 32],
	pub(crate) events: Vec<Vec<u8>>,
	pub(crate) completed: bool,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct DurableIdentityStateV2 {
	version: u8,
	revision: u64,
	profile_id: [u8; 32],
	genesis_hash: [u8; 32],
	subject_master_seed: [u8; 32],
	recovery_incarnation: [u8; 32],
	epoch: u32,
	continuity: bool,
	grants: BTreeMap<[u8; 32], IdentityGrantRecordV2>,
	operations: BTreeMap<[u8; 16], DurableIdentityOperationV2>,
	challenges: BTreeSet<[u8; 32]>,
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
			self.grants.len() > MAX_GRANTS ||
			self.operations.len() > MAX_OPERATIONS ||
			self.challenges.len() > MAX_CHALLENGES
		{
			return Err(IdentityAuthorityErrorV2::Corrupt);
		}
		for (id, grant) in &self.grants {
			if *id != grant.id || grant.product_id.is_empty() || grant.product_id.len() > 128 {
				return Err(IdentityAuthorityErrorV2::Corrupt);
			}
		}
		for (id, operation) in &self.operations {
			if *id != operation.operation_id || operation.events.len() > 3 {
				return Err(IdentityAuthorityErrorV2::Corrupt);
			}
		}
		Ok(())
	}
}

pub(crate) struct IdentityAuthorityStoreV2 {
	root: PathBuf,
	context: IdentityAuthorityContextV2,
	key: [u8; 32],
	aad: Vec<u8>,
	state: RwLock<DurableIdentityStateV2>,
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
		validate_keystore(keystore)?;
		let grants = grants_map(grants, keystore.recovery_incarnation)?;
		let root = root.as_ref().join(ROOT);
		ensure_directory(&root)?;
		ensure_directory(&root.join(QUARANTINE))?;
		let key = state_key(keystore.state_key, context)?;
		let aad = state_aad(context);
		let path = root.join(STATE);
		let state = if path.exists() {
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
				grants: grants.clone(),
				operations: BTreeMap::new(),
				challenges: BTreeSet::new(),
			}
		};
		if state.subject_master_seed != keystore.subject_master_seed ||
			state.recovery_incarnation != keystore.recovery_incarnation ||
			state.epoch != keystore.epoch ||
			state.continuity != keystore.continuity
		{
			return Err(IdentityAuthorityErrorV2::OldIncarnation);
		}
		state.validate(context)?;
		let store = Self { root, context, key, aad, state: RwLock::new(state) };
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

	pub(crate) fn context(&self) -> IdentityAuthorityContextV2 {
		self.context
	}

	pub(crate) fn root_material(
		&self,
	) -> Result<([u8; 32], [u8; 32], u32, bool), IdentityAuthorityErrorV2> {
		let state = self.read()?;
		Ok((state.subject_master_seed, state.recovery_incarnation, state.epoch, state.continuity))
	}

	pub(crate) fn grant(
		&self,
		grant_id: [u8; 32],
	) -> Result<Option<IdentityGrantRecordV2>, IdentityAuthorityErrorV2> {
		Ok(self.read()?.grants.get(&grant_id).cloned())
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
		self.persist(&next)?;
		*current = next;
		Ok(output)
	}

	fn persist(&self, state: &DurableIdentityStateV2) -> Result<(), IdentityAuthorityErrorV2> {
		let plaintext = state.encode();
		if plaintext.len() as u64 > MAX_STATE_BYTES {
			return Err(IdentityAuthorityErrorV2::Full);
		}
		let nonce = state_nonce(state.revision, &plaintext);
		let envelope = encrypt(&plaintext, &self.key, nonce, &self.aad)?;
		let target = self.root.join(STATE);
		let temporary = self.root.join(format!(".{STATE}.{}.tmp", state.revision));
		let mut file = OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(&temporary)
			.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		set_private_file(&file)?;
		file.write_all(&envelope).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		file.sync_all().map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		fs::rename(&temporary, &target).map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
		sync_dir(&self.root)?;
		Ok(())
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
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		let disclosure = core.host.disclose(
			&authorized.product_id,
			&audience,
			&fields,
			purpose,
			expires_at,
			authorized.head,
		)?;
		validate_query_finality(&mut core.runtime, authorized.head, None, disclosure.finalized)?;
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
		complete_pure_consent(&mut core, &authorized, execution, None)
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
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
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
			return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
		}
		let operation_id =
			authorized.operation_id.ok_or(IdentityAuthorityErrorV2::GrantScopeDenied)?;
		let intent_effect = effect_hash(&[
			&payload_hash,
			&policy_hash,
			&expires_at.to_be_bytes(),
			&authorized.head.block_number.to_be_bytes(),
			&authorized.head.block_hash,
		]);
		let prepared = match core.store.operation(operation_id)? {
			Some(record) => {
				validate_replay_record(&record, &authorized)?;
				record
			},
			None => {
				let consent_receipt = core.consent.consume(&IdentityConsentRequestV2 {
					operation_id,
					operation: authorized.operation,
					grant_id: authorized.grant.id,
					request_hash: authorized.request_hash,
					effect_hash: intent_effect,
					finalized: authorized.head,
				})?;
				let record = DurableIdentityOperationV2 {
					operation_id,
					operation: authorized.operation,
					request_hash: authorized.request_hash,
					grant_id: authorized.grant.id,
					effect_hash: intent_effect,
					finalized_number: authorized.head.block_number,
					finalized_hash: authorized.head.block_hash,
					consent_receipt,
					events: Vec::new(),
					completed: false,
				};
				core.store.prepare_operation(record.clone(), None)?;
				record
			},
		};
		if prepared.effect_hash != intent_effect {
			return Err(IdentityAuthorityErrorV2::EffectConflict);
		}
		let genesis_hash = core.store.context().genesis_hash;
		let effect = core.signer.sign_and_finalize(&TransactionSigningRequestV2 {
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
		})?;
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
		self.execute_account(call).map_err(IdentityAuthorityErrorV2::backend)
	}

	fn identity_humanity_status(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityStatusFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		self.execute_humanity_status(call).map_err(IdentityAuthorityErrorV2::backend)
	}

	fn identity_entitlements_read(
		&mut self,
		call: HostCallV2<'_, IdentityEntitlementsReadFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		self.execute_entitlements_read(call).map_err(IdentityAuthorityErrorV2::backend)
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
		self.execute_profile_read(call).map_err(IdentityAuthorityErrorV2::backend)
	}

	fn identity_profile_disclose(
		&mut self,
		call: HostCallV2<'_, IdentityProfileDiscloseFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		self.execute_profile_disclose(call).map_err(IdentityAuthorityErrorV2::backend)
	}

	fn identity_humanity_prove(
		&mut self,
		call: HostCallV2<'_, IdentityHumanityProveFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		self.execute_humanity_prove(call).map_err(IdentityAuthorityErrorV2::backend)
	}

	fn identity_subject_derive(
		&mut self,
		call: HostCallV2<'_, IdentitySubjectDeriveFrame>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		self.execute_subject_derive(call).map_err(IdentityAuthorityErrorV2::backend)
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
		self.execute_transaction_sign(call).map_err(IdentityAuthorityErrorV2::backend)
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
		return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
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
	if record.events.is_empty() {
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
	if let Some(challenge) = challenge {
		if core.store.challenge_consumed(challenge)? {
			return Err(IdentityAuthorityErrorV2::ChallengeReplay);
		}
	}
	let effect_hash = execution_hash(&execution);
	let consent_receipt = core.consent.consume(&IdentityConsentRequestV2 {
		operation_id,
		operation: authorized.operation,
		grant_id: authorized.grant.id,
		request_hash: authorized.request_hash,
		effect_hash,
		finalized: authorized.head,
	})?;
	core.store.prepare_operation(
		DurableIdentityOperationV2 {
			operation_id,
			operation: authorized.operation,
			request_hash: authorized.request_hash,
			grant_id: authorized.grant.id,
			effect_hash,
			finalized_number: authorized.head.block_number,
			finalized_hash: authorized.head.block_hash,
			consent_receipt,
			events: execution.events.clone(),
			completed: false,
		},
		challenge,
	)?;
	core.store.complete_operation(
		operation_id,
		effect_hash,
		authorized.head.block_number,
		authorized.head.block_hash,
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
		left.effect_hash == right.effect_hash &&
		left.finalized_number == right.finalized_number &&
		left.finalized_hash == right.finalized_hash &&
		left.consent_receipt == right.consent_receipt &&
		left.events == right.events
}

fn validate_keystore(keystore: IdentityKeystoreMaterialV2) -> Result<(), IdentityAuthorityErrorV2> {
	if keystore.state_key == [0; 32] ||
		keystore.subject_master_seed == [0; 32] ||
		keystore.recovery_incarnation == [0; 32]
	{
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
		if grant.id == [0; 32] ||
			grant.product_id.is_empty() ||
			grant.product_id.len() > 128 ||
			grant.recovery_incarnation != incarnation ||
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
	let name = format!("{}.identity", hex::encode(Sha256::digest(&bytes)));
	fs::rename(path, root.join(QUARANTINE).join(name))
		.map_err(|_| IdentityAuthorityErrorV2::Unavailable)?;
	sync_dir(&root.join(QUARANTINE))?;
	sync_dir(root)?;
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
	use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

	use super::*;
	use crate::product_sdk::host_v2::execution::{HostRequestMetaV2, ProviderOutboxContextV2};

	fn context() -> IdentityAuthorityContextV2 {
		IdentityAuthorityContextV2 { profile_id: [1; 32], genesis_hash: [2; 32] }
	}

	fn keystore() -> IdentityKeystoreMaterialV2 {
		IdentityKeystoreMaterialV2 {
			state_key: [3; 32],
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
			effect_hash: [9; 32],
			finalized_number: 100,
			finalized_hash: [10; 32],
			consent_receipt: [11; 32],
			events: vec![b"accepted".to_vec(), b"result".to_vec()],
			completed: false,
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
	}

	#[derive(Clone)]
	struct RuntimeFixture {
		available: Arc<AtomicBool>,
	}

	impl FinalizedIdentitySourceV2 for RuntimeFixture {
		fn head(&mut self) -> Result<FinalizedIdentityEffectV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(finality())
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
			Ok(humanity_authority())
		}

		fn humanity_proof(
			&mut self,
			_product_id: &str,
			_audience: &str,
			_claims: &[String],
		) -> Result<FinalizedHumanityAuthorityV2, IdentityAuthorityErrorV2> {
			Ok(humanity_authority())
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
			Ok(finality == super::tests::finality() || finality == signing_finality())
		}
	}

	#[derive(Clone)]
	struct HostFixture {
		available: Arc<AtomicBool>,
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

		fn disclose(
			&mut self,
			_product_id: &str,
			_audience: &str,
			_fields: &[String],
			_purpose: &str,
			_expires_at: u64,
			finalized: FinalizedIdentityEffectV2,
		) -> Result<HostProfileDisclosureV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(HostProfileDisclosureV2 { commitment: [32; 32], valid_until: 130, finalized })
		}
	}

	#[derive(Clone)]
	struct ConsentFixture {
		available: Arc<AtomicBool>,
		calls: Arc<AtomicUsize>,
	}

	impl FreshIdentityConsentV2 for ConsentFixture {
		fn consume(
			&mut self,
			request: &IdentityConsentRequestV2,
		) -> Result<[u8; 32], IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			self.calls.fetch_add(1, Ordering::SeqCst);
			Ok(effect_hash(&[
				b"consent",
				&request.operation_id,
				&request.grant_id,
				&request.effect_hash,
			]))
		}
	}

	#[derive(Clone)]
	struct SignerFixture {
		available: Arc<AtomicBool>,
		fail_after_effect_once: Arc<AtomicBool>,
		calls: Arc<AtomicUsize>,
		effects: Arc<Mutex<BTreeMap<[u8; 16], FinalizedTransactionEffectV2>>>,
	}

	impl FinalizedTransactionSignerV2 for SignerFixture {
		fn sign_and_finalize(
			&mut self,
			request: &TransactionSigningRequestV2,
		) -> Result<FinalizedTransactionEffectV2, IdentityAuthorityErrorV2> {
			if !self.available.load(Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			self.calls.fetch_add(1, Ordering::SeqCst);
			let mut effects = self.effects.lock().unwrap();
			if let Some(effect) = effects.get(&request.operation_id) {
				return Ok(effect.clone());
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
			effects.insert(request.operation_id, effect.clone());
			if self.fail_after_effect_once.swap(false, Ordering::SeqCst) {
				return Err(IdentityAuthorityErrorV2::AuthorityUnavailable);
			}
			Ok(effect)
		}
	}

	#[derive(Clone)]
	struct FixtureControls {
		runtime_available: Arc<AtomicBool>,
		host_available: Arc<AtomicBool>,
		consent_available: Arc<AtomicBool>,
		consent_calls: Arc<AtomicUsize>,
		signer_available: Arc<AtomicBool>,
		signer_fail_after_effect_once: Arc<AtomicBool>,
		signer_calls: Arc<AtomicUsize>,
		signer_effects: Arc<Mutex<BTreeMap<[u8; 16], FinalizedTransactionEffectV2>>>,
	}

	impl Default for FixtureControls {
		fn default() -> Self {
			Self {
				runtime_available: Arc::new(AtomicBool::new(true)),
				host_available: Arc::new(AtomicBool::new(true)),
				consent_available: Arc::new(AtomicBool::new(true)),
				consent_calls: Arc::new(AtomicUsize::new(0)),
				signer_available: Arc::new(AtomicBool::new(true)),
				signer_fail_after_effect_once: Arc::new(AtomicBool::new(false)),
				signer_calls: Arc::new(AtomicUsize::new(0)),
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
			RuntimeFixture { available: Arc::clone(&controls.runtime_available) },
			HostFixture { available: Arc::clone(&controls.host_available) },
			ConsentFixture {
				available: Arc::clone(&controls.consent_available),
				calls: Arc::clone(&controls.consent_calls),
			},
			SignerFixture {
				available: Arc::clone(&controls.signer_available),
				fail_after_effect_once: Arc::clone(&controls.signer_fail_after_effect_once),
				calls: Arc::clone(&controls.signer_calls),
				effects: Arc::clone(&controls.signer_effects),
			},
		)
		.unwrap()
	}

	fn finality() -> FinalizedIdentityEffectV2 {
		FinalizedIdentityEffectV2 { block_number: 100, block_hash: [41; 32] }
	}

	fn signing_finality() -> FinalizedIdentityEffectV2 {
		FinalizedIdentityEffectV2 { block_number: 101, block_hash: [42; 32] }
	}

	fn humanity_authority() -> FinalizedHumanityAuthorityV2 {
		FinalizedHumanityAuthorityV2 {
			status: 1,
			fresh_until: 150,
			commitment: [43; 32],
			finalized: finality(),
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
		assert!(matches!(
			backend.identity_humanity_prove(HostCallV2 {
				frame: &repeated_challenge,
				authority: &grant_id(1104),
				meta: meta([55; 16], Some([55; 16])),
				outbox: &outbox,
			}),
			Err(HostExecutionErrorV2::Backend("IDENTITY_CHALLENGE_REPLAY"))
		));
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

		assert!(matches!(
			backend.identity_account(HostCallV2 {
				frame: &account,
				authority: &grant_id(1101),
				meta: meta([1; 16], None),
				outbox: &outbox,
			}),
			Err(HostExecutionErrorV2::Backend("GRANT_SCOPE_DENIED"))
		));

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
		assert!(backend
			.transaction_sign(HostCallV2 {
				frame: &signing,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			})
			.is_err());
		drop(backend);

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
		assert!(matches!(
			backend.transaction_sign(HostCallV2 {
				frame: &changed,
				authority: &grant_id(1200),
				meta: meta([72; 16], Some(operation_id)),
				outbox: &outbox,
			}),
			Err(HostExecutionErrorV2::Backend("IDENTITY_EFFECT_CONFLICT"))
		));
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
		assert!(matches!(
			backend.identity_account(HostCallV2 {
				frame: &account,
				authority: &grant_id(1100),
				meta: meta([81; 16], None),
				outbox: &outbox(),
			}),
			Err(HostExecutionErrorV2::Backend("IDENTITY_AUTHORITY_UNAVAILABLE"))
		));
	}
}

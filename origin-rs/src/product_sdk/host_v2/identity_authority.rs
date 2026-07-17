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
	sync::RwLock,
};

use codec::{Decode, Encode};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

use crate::product_sdk::host_outbox::{decrypt, encrypt, sync_dir, HostOutboxError};

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
	use super::*;

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
}

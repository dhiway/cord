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

//! Canonical checkpoint-v2 and manifest-deletion background coordination.

use std::{
	fs,
	io::{Read as _, Seek as _, SeekFrom, Write as _},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use async_trait::async_trait;
use codec::Encode;
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sp_core::H256;
use tokio::{
	io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
	sync::Mutex,
	time::{interval, MissedTickBehavior},
};

const MANIFEST_DELETION_DEDUPE_TAIL_BYTES: u64 = 64 * 1024;
const MAX_JSONL_RECORD_BYTES: u64 = 1024 * 1024;
const MANIFEST_DELETION_OUTBOX_FILE: &str = "provider-submissions-v3.jsonl";
const MANIFEST_DELETION_OUTBOX_LOCK_FILE: &str = "provider-submissions-v3.jsonl.lock";

use crate::{
	observability::{emit_failure, ProviderFailureCode},
	storage::{PendingDeletion, PendingRootSubmission},
	BucketId, ChainAuthority, DeletionDuty, ProviderService,
};

const MAX_CHECKPOINT_DUTY_PAGES_PER_POLL: usize = 4_096;
const MAX_MANIFEST_DELETIONS_PER_POLL: usize = 128;

/// Native provider deletion acknowledgement queued after finalized authorization and byte removal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContentDeletionSubmission {
	/// Agreement whose provider must acknowledge content deletion.
	pub agreement_id: String,
	/// Deleted raw-content commitment.
	pub content_commitment: String,
	/// Finalized block that authorized deletion.
	pub authorized_at: String,
	/// Provider root after appending the tombstone leaf.
	pub tombstone_root: String,
	/// Exact tombstone leaf appended by the preceding provider-root call.
	pub tombstone_leaf: String,
	/// Provider root sequence which must already be finalized on Orbis.
	pub root_sequence: u64,
	/// Canonical tombstone leaf index.
	pub leaf_index: u64,
	/// Total leaves covered by the committed root.
	pub leaf_count: u64,
	/// Bounded leaf-to-root sibling hashes.
	pub inclusion_proof: Vec<String>,
}

/// Idempotent canonical manifest-deletion acknowledgement for the governed signer pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestDeletionSubmission {
	/// Canonical manifest digest used as the outbox idempotency key.
	pub manifest: String,
	/// Canonical bucket identifier from the finalized duty.
	pub bucket_id: String,
	/// Provider CID digest that was durably tombstoned.
	pub provider_commitment: String,
	/// Deterministic local deletion evidence hash.
	pub evidence_hash: String,
	/// Governed checkpoint at which the manifest was tombstoned.
	pub tombstoned_at: u32,
	/// Finalized provider service key which signed the acknowledgement digest.
	pub service_key: String,
	/// Ed25519 signature over `cord/storage/deletion-ack/v1` runtime digest.
	pub signature: String,
	/// Exact finalized runtime duty fingerprint retained for replay auditing.
	pub duty_fingerprint: String,
}

/// Provider-authenticated append-only root which must finalize before a deletion acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderRootSubmission {
	/// Monotonic provider-authenticated submission sequence.
	pub sequence: u64,
	/// Exact leaf values appended and folded by the runtime accumulator.
	pub appended_leaves: Vec<String>,
	/// Locally derived expected root retained for finalized-result audit.
	pub expected_root: String,
	/// Locally derived expected leaf count retained for finalized-result audit.
	pub expected_leaf_count: u64,
}

/// Versioned typed provider transaction request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum ProviderSubmission {
	/// Native `StorageProvider::acknowledge_manifest_deletion` request.
	ManifestDeletion(ManifestDeletionSubmission),
}

/// Canonical manifest-deletion seam consumed by the metadata-derived Orbis finality lane.
#[async_trait]
pub(crate) trait ManifestDeletionSubmitter: Send + Sync + 'static {
	/// Validate any local durable recovery owned by this submitter without mutating it.
	fn prepare_startup(
		&self,
		_provider_root: &Path,
		_prepared_root: Option<&fs::File>,
	) -> Result<ManifestDeletionStartupPlan, String> {
		Ok(ManifestDeletionStartupPlan::default())
	}

	/// Durably accept one idempotent canonical manifest-deletion acknowledgement.
	async fn submit_manifest_deletion(
		&self,
		_request: ManifestDeletionSubmission,
	) -> Result<(), String> {
		Err("canonical manifest deletion outbox unavailable".into())
	}
}

/// Type-erased startup recovery for a manifest-deletion submitter.
#[derive(Default)]
pub(crate) struct ManifestDeletionStartupPlan {
	jsonl: Option<PreparedJsonlStartup>,
}

impl ManifestDeletionStartupPlan {
	pub(crate) fn arm(
		self,
		root: &crate::bounded_io::LockedDirectory,
	) -> Result<ArmedManifestDeletionStartupPlan, String> {
		let jsonl = self.jsonl.map(|jsonl| jsonl.arm(root)).transpose()?;
		Ok(ArmedManifestDeletionStartupPlan { jsonl })
	}
}

pub(crate) struct ArmedManifestDeletionStartupPlan {
	jsonl: Option<ArmedJsonlStartup>,
}

impl ArmedManifestDeletionStartupPlan {
	pub(crate) fn apply(self) -> Result<(), String> {
		if let Some(jsonl) = self.jsonl {
			jsonl.apply()?;
		}
		Ok(())
	}

	pub(crate) fn rollback(self) -> Result<(), String> {
		if let Some(jsonl) = self.jsonl {
			jsonl.rollback()?;
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreparedFileGuard {
	Missing,
	Present { identity: crate::bounded_io::FileIdentity, length: u64 },
}

struct PreparedJsonlStartup {
	lock: PreparedFileGuard,
	lock_temps: Vec<crate::bounded_io::PreparedRegularFile>,
	source: PreparedFileGuard,
	source_digest: Option<[u8; 32]>,
	tail_start: u64,
	expected_tail: Vec<u8>,
	truncate_to: u64,
	qualified: Arc<AtomicBool>,
	root_capability: Arc<std::sync::Mutex<Option<InstalledOutboxRoot>>>,
}

struct InstalledOutboxRoot {
	directory: fs::File,
	lock_file: fs::File,
	lock: PreparedFileGuard,
}

impl PreparedJsonlStartup {
	fn arm(
		self,
		root: &crate::bounded_io::LockedDirectory,
	) -> Result<ArmedJsonlStartup, String> {
		let directory = root.file().try_clone().map_err(|error| error.to_string())?;
		let lock = acquire_prepared_outbox_lock(&directory, &self.lock)?;
		crate::bounded_io::validate_prepared_regular_files_at(&directory, &self.lock_temps)
			.map_err(|error| error.to_string())?;
		let validated = validate_prepared_jsonl_guard(
					&directory,
					&self.source,
					self.source_digest,
					self.tail_start,
					&self.expected_tail,
				);
		if let Err(error) = validated {
			return match lock.rollback_created() {
				Ok(()) => Err(error),
				Err(cleanup) =>
					Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
			}
		}
		Ok(ArmedJsonlStartup { prepared: self, lock, directory })
	}
}

struct ArmedJsonlStartup {
	prepared: PreparedJsonlStartup,
	lock: AcquiredPreparedOutboxLock,
	directory: fs::File,
}

impl ArmedJsonlStartup {
	fn apply(mut self) -> Result<(), String> {
		let installed_lock = self.lock.file().try_clone().map_err(|error| error.to_string())?;
		let mut root_capability = match self.prepared.root_capability.lock() {
			Ok(root_capability) => root_capability,
			Err(_) => return match self.lock.rollback_created() {
				Ok(()) => Err("manifest deletion outbox root capability lock was poisoned".into()),
				Err(cleanup) => Err(format!(
					"manifest deletion outbox root capability lock was poisoned; created outbox lock cleanup failed: {cleanup}",
				)),
			},
		};
		if let Err(error) = self.lock.validate_canonical() {
			return match self.lock.rollback_created() {
				Ok(()) => Err(error),
				Err(cleanup) =>
					Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
			}
		}
		if let Err(error) = crate::bounded_io::remove_validated_owned_lock_artifacts_at(
			&self.directory,
			&self.prepared.lock_temps,
		)
		.map_err(|error| error.to_string())
		{
			return match self.lock.rollback_created() {
				Ok(()) => Err(error),
				Err(cleanup) => Err(format!("{error}; outbox lock residue cleanup failed: {cleanup}")),
			}
		}
		let applied = apply_prepared_jsonl_guard(
			&self.directory,
			&self.prepared.source,
			self.prepared.source_digest,
			self.prepared.tail_start,
			&self.prepared.expected_tail,
			self.prepared.truncate_to,
		);
		if let Err(error) = applied {
			return match self.lock.rollback_created() {
				Ok(()) => Err(error),
				Err(cleanup) =>
					Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
			}
		}
		if let Err(error) = self.lock.validate_canonical() {
			return match restore_prepared_jsonl_source(
				&self.directory,
				&self.prepared.source,
				self.prepared.tail_start,
				&self.prepared.expected_tail,
			) {
				Ok(()) => Err(error),
				Err(rollback) => Err(format!("{error}; outbox source rollback failed: {rollback}")),
			}
		}
		let lock = self.lock.canonical_guard();
		FileExt::unlock(self.lock.file()).map_err(|error| error.to_string())?;
		*root_capability = Some(InstalledOutboxRoot {
			directory: self.directory,
			lock_file: installed_lock,
			lock,
		});
		self.lock.preserve_created();
		self.prepared.qualified.store(true, Ordering::Release);
		Ok(())
	}

	fn rollback(self) -> Result<(), String> {
		self.lock.rollback_created()
	}
}

/// Private P4 cutover bearer for the legacy DiskStore append journal.
///
/// No production worker or HTTP route invokes this seam. It remains private so P4 can replace the
/// object mutation and completion unit atomically without publishing nonexistent Commons calls.
#[async_trait]
pub(crate) trait LegacyDiskCompletionSubmitter: Send + Sync {
	async fn submit_root(&self, request: ProviderRootSubmission) -> Result<(), String>;
	async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String>;
}

/// Append-only JSONL outbox for metadata-valid canonical manifest-deletion acknowledgements.
pub(crate) struct JsonlManifestDeletionOutbox {
	path: PathBuf,
	write_lock: Mutex<()>,
	qualified: Arc<AtomicBool>,
	root_capability: Arc<std::sync::Mutex<Option<InstalledOutboxRoot>>>,
	#[cfg(test)]
	fail_next_directory_sync: std::sync::atomic::AtomicBool,
}

impl JsonlManifestDeletionOutbox {
	/// Create the canonical direct-child outbox for one validated provider root.
	pub(crate) fn for_provider_root(root: impl AsRef<Path>) -> Self {
		Self::new(root.as_ref().join(MANIFEST_DELETION_OUTBOX_FILE))
	}

	/// Create an outbox at `path`; parent directories are created on first submission.
	///
	/// A raw path remains inert unless `ProviderService` validates it as the canonical direct
	/// child.
	pub(crate) fn new(path: impl AsRef<Path>) -> Self {
		Self {
			path: path.as_ref().to_path_buf(),
			write_lock: Mutex::new(()),
			qualified: Arc::new(AtomicBool::new(false)),
			root_capability: Arc::new(std::sync::Mutex::new(None)),
			#[cfg(test)]
			fail_next_directory_sync: std::sync::atomic::AtomicBool::new(false),
		}
	}
}

#[async_trait]
impl ManifestDeletionSubmitter for JsonlManifestDeletionOutbox {
	fn prepare_startup(
		&self,
		provider_root: &Path,
		prepared_root: Option<&fs::File>,
	) -> Result<ManifestDeletionStartupPlan, String> {
		validate_canonical_outbox_path(provider_root, &self.path)?;
		prepare_jsonl_manifest_deletion_outbox(
			prepared_root,
			Arc::clone(&self.qualified),
			Arc::clone(&self.root_capability),
		)
	}

	async fn submit_manifest_deletion(
		&self,
		request: ManifestDeletionSubmission,
	) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		if !self.qualified.load(Ordering::Acquire) {
			return Err("manifest deletion outbox startup has not been applied".into())
		}
		let (directory, lock_file, expected_lock) = self.installed_root_capability()?;
		let process_lock = self.lock_outbox(&directory, lock_file, &expected_lock).await?;
		repair_incomplete_jsonl_tail(&directory, &process_lock).await?;
		let existing = read_bounded_jsonl_tail(
			&directory,
			&process_lock,
			MANIFEST_DELETION_DEDUPE_TAIL_BYTES,
		)
		.await?;
		for line in existing.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
			let Ok(ProviderSubmission::ManifestDeletion(previous)) =
				serde_json::from_slice::<ProviderSubmission>(line)
			else {
				continue;
			};
			if previous.manifest == request.manifest {
				return if previous == request {
					process_lock.validate()
				} else {
					Err("manifest deletion outbox replay changed its payload".into())
				};
			}
		}
		let mut encoded = serde_json::to_vec(&ProviderSubmission::ManifestDeletion(request))
			.map_err(|error| error.to_string())?;
		self.append_locked(&directory, &process_lock, &mut encoded).await?;
		process_lock.validate()
	}
}

fn prepare_jsonl_manifest_deletion_outbox(
	prepared_root: Option<&fs::File>,
	qualified: Arc<AtomicBool>,
	root_capability: Arc<std::sync::Mutex<Option<InstalledOutboxRoot>>>,
) -> Result<ManifestDeletionStartupPlan, String> {
	let (lock, lock_temps, source, source_digest, tail_start, expected_tail, truncate_to) =
		match prepared_root {
			Some(root) => {
				let lock_temps = collect_owned_lock_temps(root)?;
				let lock = open_optional_guarded_file_at(
					root,
					MANIFEST_DELETION_OUTBOX_LOCK_FILE,
					false,
				)?
				.map_or(PreparedFileGuard::Missing, |(_, guard)| guard);
				let (source, digest, tail_start, tail, truncate_to) =
					prepare_jsonl_source_guard_at(root)?;
				(lock, lock_temps, source, digest, tail_start, tail, truncate_to)
			},
			None => (
				PreparedFileGuard::Missing,
				Vec::new(),
				PreparedFileGuard::Missing,
				None,
				0,
				Vec::new(),
				0,
			),
		};
	Ok(ManifestDeletionStartupPlan {
		jsonl: Some(PreparedJsonlStartup {
			lock,
			lock_temps,
			source,
			source_digest,
			tail_start,
			expected_tail,
			truncate_to,
			qualified,
			root_capability,
		}),
	})
}

fn validate_canonical_outbox_path(provider_root: &Path, path: &Path) -> Result<(), String> {
	if path != provider_root.join(MANIFEST_DELETION_OUTBOX_FILE) ||
		path.parent() != Some(provider_root) ||
		path.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_DELETION_OUTBOX_FILE)
	{
		return Err("manifest deletion outbox must be the canonical provider-root child".into())
	}
	Ok(())
}

fn collect_owned_lock_temps(
	directory: &fs::File,
) -> Result<Vec<crate::bounded_io::PreparedRegularFile>, String> {
	const PREFIX: &str = ".provider-lock.create-";
	let mut temps = Vec::new();
	for entry in crate::bounded_io::list_directory(directory).map_err(|error| error.to_string())? {
		let name = entry.name.to_string_lossy();
		let Some(random) = name.strip_prefix(PREFIX) else { continue };
		if random.len() != 32 ||
			!random.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) ||
			entry.length != 0
		{
			return Err("manifest deletion outbox lock residue is invalid".into())
		}
		temps.push(entry.into_regular_guard().map_err(|error| error.to_string())?);
		if temps.len() > 8 {
			return Err("manifest deletion outbox lock residue set is too large".into())
		}
	}
	Ok(temps)
}

fn prepare_jsonl_source_guard_at(
	directory: &fs::File,
) -> Result<(PreparedFileGuard, Option<[u8; 32]>, u64, Vec<u8>, u64), String> {
	Ok(match open_optional_guarded_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE,
		false,
	)? {
		None => (PreparedFileGuard::Missing, None, 0, Vec::new(), 0),
		Some((mut file, guard)) => {
			let digest = jsonl_source_digest(&mut file, &guard)?;
			let (tail_start, expected_tail, truncate_to) = jsonl_tail_view(&mut file, &guard)?;
			validate_open_file(&file, &guard)?;
			validate_file_guard_at(directory, MANIFEST_DELETION_OUTBOX_FILE, &guard)?;
			(guard, Some(digest), tail_start, expected_tail, truncate_to)
		},
	})
}

fn jsonl_source_digest(file: &mut fs::File, guard: &PreparedFileGuard) -> Result<[u8; 32], String> {
	let PreparedFileGuard::Present { length, .. } = guard else {
		return Err("manifest deletion outbox guard has no source".into())
	};
	file.seek(SeekFrom::Start(0)).map_err(|error| error.to_string())?;
	let mut remaining = *length;
	let mut hasher = blake3::Hasher::new();
	let mut buffer = [0u8; 64 * 1024];
	while remaining > 0 {
		let width = usize::try_from(remaining.min(buffer.len() as u64))
			.map_err(|_| "manifest deletion outbox digest width exceeds platform bounds")?;
		let read = file.read(&mut buffer[..width]).map_err(|error| error.to_string())?;
		if read == 0 {
			return Err("manifest deletion outbox changed while hashing".into())
		}
		hasher.update(&buffer[..read]);
		remaining -= read as u64;
	}
	Ok(*hasher.finalize().as_bytes())
}

fn jsonl_tail_view(
	file: &mut fs::File,
	guard: &PreparedFileGuard,
) -> Result<(u64, Vec<u8>, u64), String> {
	let PreparedFileGuard::Present { length, .. } = guard else {
		return Err("manifest deletion outbox guard has no source".into())
	};
	let start = length.saturating_sub(MAX_JSONL_RECORD_BYTES.saturating_add(1));
	let width: usize = (length - start)
		.try_into()
		.map_err(|_| "manifest deletion outbox tail exceeds the platform bound")?;
	let mut tail = vec![0u8; width];
	file.seek(SeekFrom::Start(start)).map_err(|error| error.to_string())?;
	file.read_exact(&mut tail).map_err(|error| error.to_string())?;
	let truncate_to = if tail.last() == Some(&b'\n') || tail.is_empty() {
		*length
	} else {
		match tail.iter().rposition(|byte| *byte == b'\n') {
			Some(index) => start + index as u64 + 1,
			None if start == 0 => 0,
			None => return Err("incomplete outbox record exceeds the bounded line limit".into()),
		}
	};
	Ok((start, tail, truncate_to))
}

fn open_optional_guarded_file_at(
	directory: &fs::File,
	name: &str,
	write: bool,
) -> Result<Option<(fs::File, PreparedFileGuard)>, String> {
	let Some(file) = crate::bounded_io::open_optional_regular_file_at(
		directory,
		name.as_ref(),
		write,
	)
	.map_err(|error| error.to_string())?
	else {
		return Ok(None)
	};
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	let guard = PreparedFileGuard::Present {
		identity: crate::bounded_io::file_identity(&metadata),
		length: metadata.len(),
	};
	validate_file_guard_at(directory, name, &guard)?;
	Ok(Some((file, guard)))
}

fn validate_prepared_jsonl_guard(
	directory: &fs::File,
	guard: &PreparedFileGuard,
	expected_digest: Option<[u8; 32]>,
	tail_start: u64,
	expected_tail: &[u8],
) -> Result<(), String> {
	let PreparedFileGuard::Present { length, .. } = guard else {
		return crate::bounded_io::entry_missing_at(directory, MANIFEST_DELETION_OUTBOX_FILE.as_ref())
			.map_err(|error| error.to_string())
			.and_then(|missing| missing.then_some(()).ok_or_else(|| {
				"manifest deletion outbox changed after startup validation".into()
			}))
	};
	let expected_digest = expected_digest
		.ok_or("manifest deletion outbox startup source digest is missing")?;
	let mut file = crate::bounded_io::open_optional_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		false,
	)
	.map_err(|error| error.to_string())?
	.ok_or("manifest deletion outbox changed after startup validation")?;
	validate_open_file(&file, guard)?;
	if jsonl_source_digest(&mut file, guard)? != expected_digest {
		return Err("manifest deletion outbox changed after startup validation".into())
	}
	let expected_width: usize = (length - tail_start)
		.try_into()
		.map_err(|_| "manifest deletion outbox tail exceeds the platform bound")?;
	if expected_width != expected_tail.len() {
		return Err("manifest deletion outbox startup tail guard is invalid".into())
	}
	let mut tail = vec![0u8; expected_width];
	file.seek(SeekFrom::Start(tail_start)).map_err(|error| error.to_string())?;
	file.read_exact(&mut tail).map_err(|error| error.to_string())?;
	if tail != expected_tail {
		return Err("manifest deletion outbox tail changed after startup validation".into())
	}
	validate_file_guard_at(directory, MANIFEST_DELETION_OUTBOX_FILE, guard)
}

fn apply_prepared_jsonl_guard(
	directory: &fs::File,
	guard: &PreparedFileGuard,
	expected_digest: Option<[u8; 32]>,
	tail_start: u64,
	expected_tail: &[u8],
	truncate_to: u64,
) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = guard else {
		return crate::bounded_io::entry_missing_at(directory, MANIFEST_DELETION_OUTBOX_FILE.as_ref())
			.map_err(|error| error.to_string())
			.and_then(|missing| missing.then_some(()).ok_or_else(|| {
				"manifest deletion outbox changed after startup validation".into()
			}))
	};
	let expected_digest = expected_digest
		.ok_or("manifest deletion outbox startup source digest is missing")?;
	let mut file = crate::bounded_io::open_optional_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		true,
	)
	.map_err(|error| error.to_string())?
	.ok_or("manifest deletion outbox changed after startup validation")?;
	validate_open_file(&file, guard)?;
	if jsonl_source_digest(&mut file, guard)? != expected_digest {
		return Err("manifest deletion outbox changed after startup validation".into())
	}
	let expected_width: usize = (length - tail_start)
		.try_into()
		.map_err(|_| "manifest deletion outbox tail exceeds the platform bound")?;
	if expected_width != expected_tail.len() {
		return Err("manifest deletion outbox startup tail guard is invalid".into())
	}
	let mut tail = vec![0u8; expected_width];
	file.seek(SeekFrom::Start(tail_start)).map_err(|error| error.to_string())?;
	file.read_exact(&mut tail).map_err(|error| error.to_string())?;
	if tail != expected_tail {
		return Err("manifest deletion outbox tail changed after startup validation".into())
	}
	validate_file_guard_at(directory, MANIFEST_DELETION_OUTBOX_FILE, guard)?;
	if truncate_to < *length {
		file.set_len(truncate_to).map_err(|error| error.to_string())?;
		file.sync_all().map_err(|error| error.to_string())?;
		crate::bounded_io::sync_directory(directory).map_err(|error| error.to_string())?;
	}
	crate::bounded_io::validate_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		*identity,
		truncate_to,
	)
	.map_err(|error| error.to_string())
}

fn restore_prepared_jsonl_source(
	directory: &fs::File,
	guard: &PreparedFileGuard,
	tail_start: u64,
	expected_tail: &[u8],
) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = guard else { return Ok(()) };
	let mut file = crate::bounded_io::open_optional_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		true,
	)
	.map_err(|error| error.to_string())?
	.ok_or("manifest deletion outbox disappeared before rollback")?;
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	if crate::bounded_io::file_identity(&metadata) != *identity || metadata.len() > *length {
		return Err("manifest deletion outbox changed before rollback".into())
	}
	crate::bounded_io::validate_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		*identity,
		metadata.len(),
	)
	.map_err(|error| error.to_string())?;
	file.set_len(tail_start).map_err(|error| error.to_string())?;
	file.seek(SeekFrom::Start(tail_start)).map_err(|error| error.to_string())?;
	file.write_all(expected_tail).map_err(|error| error.to_string())?;
	file.sync_all().map_err(|error| error.to_string())?;
	crate::bounded_io::sync_directory(directory).map_err(|error| error.to_string())?;
	crate::bounded_io::validate_regular_file_at(
		directory,
		MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
		*identity,
		*length,
	)
	.map_err(|error| error.to_string())
}

fn validate_open_file(file: &fs::File, expected: &PreparedFileGuard) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = expected else {
		return Err("manifest deletion outbox expected a missing file".into())
	};
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	if !metadata.file_type().is_file() ||
		crate::bounded_io::file_identity(&metadata) != *identity ||
		metadata.len() != *length
	{
		return Err("manifest deletion outbox changed after startup validation".into())
	}
	Ok(())
}

fn validate_file_guard_at(
	directory: &fs::File,
	name: &str,
	expected: &PreparedFileGuard,
) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = expected else {
		return Err("manifest deletion outbox expected a missing file".into())
	};
	crate::bounded_io::validate_regular_file_at(directory, name.as_ref(), *identity, *length)
		.map_err(|_| "manifest deletion outbox changed after startup validation".into())
}

struct AcquiredPreparedOutboxLock {
	file: AcquiredPreparedLockFile,
	directory: fs::File,
	canonical: PreparedFileGuard,
}

enum AcquiredPreparedLockFile {
	Owned(crate::bounded_io::OwnedLockedRegularFile),
	Existing(fs::File),
}

impl AcquiredPreparedOutboxLock {
	fn file(&self) -> &fs::File {
		match &self.file {
			AcquiredPreparedLockFile::Owned(file) => file.file(),
			AcquiredPreparedLockFile::Existing(file) => file,
		}
	}

	fn validate_canonical(&self) -> Result<(), String> {
		validate_open_file(self.file(), &self.canonical)?;
		validate_file_guard_at(
			&self.directory,
			MANIFEST_DELETION_OUTBOX_LOCK_FILE,
			&self.canonical,
		)
	}

	fn canonical_guard(&self) -> PreparedFileGuard {
		self.canonical.clone()
	}

	fn rollback_created(self) -> Result<(), String> {
		match self.file {
			AcquiredPreparedLockFile::Owned(file) => file.rollback().map_err(|error| error.to_string()),
			AcquiredPreparedLockFile::Existing(_) => Ok(()),
		}
	}

	fn preserve_created(&mut self) {
		if let AcquiredPreparedLockFile::Owned(file) = &mut self.file {
			file.preserve();
		}
	}
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum PreparedLockFailure {
	Publish(crate::bounded_io::OwnedLockPublishStage),
	Transfer,
}

fn acquire_prepared_outbox_lock(
	directory: &fs::File,
	expected: &PreparedFileGuard,
) -> Result<AcquiredPreparedOutboxLock, String> {
	acquire_prepared_outbox_lock_inner(directory, expected, None)
}

fn acquire_prepared_outbox_lock_inner(
	directory: &fs::File,
	expected: &PreparedFileGuard,
	#[cfg(test)] failure: Option<PreparedLockFailure>,
	#[cfg(not(test))] _failure: Option<()>,
) -> Result<AcquiredPreparedOutboxLock, String> {
	let owned_directory = directory.try_clone().map_err(|error| error.to_string())?;
	let file = match expected {
		PreparedFileGuard::Missing => {
			let file = crate::bounded_io::create_and_lock_regular_file_at_with_hook(
				directory,
				MANIFEST_DELETION_OUTBOX_LOCK_FILE.as_ref(),
				|stage| {
					#[cfg(not(test))]
					let _ = stage;
					#[cfg(test)]
					if failure == Some(PreparedLockFailure::Publish(stage)) {
						return Err(crate::ContentError::IntegrityFailed)
					}
					Ok(())
				},
			)
			.map_err(|error| error.to_string())?;
			#[cfg(test)]
			if failure == Some(PreparedLockFailure::Transfer) {
				return Err("injected prepared outbox lock transfer failure".into())
			}
			AcquiredPreparedLockFile::Owned(file)
		},
		PreparedFileGuard::Present { .. } => AcquiredPreparedLockFile::Existing(
			crate::bounded_io::open_optional_regular_file_at(
				directory,
				MANIFEST_DELETION_OUTBOX_LOCK_FILE.as_ref(),
				true,
			)
			.map_err(|error| error.to_string())?
			.ok_or("manifest deletion outbox lock changed after startup validation")?,
		),
	};
	let file_ref = match &file {
		AcquiredPreparedLockFile::Owned(file) => file.file(),
		AcquiredPreparedLockFile::Existing(file) => file,
	};
	if matches!(&file, AcquiredPreparedLockFile::Existing(_)) {
		FileExt::try_lock_exclusive(file_ref).map_err(|error| {
			format!("manifest deletion outbox lock is already held: {error}")
		})?;
	}
	let metadata = file_ref.metadata().map_err(|error| error.to_string())?;
	let actual = PreparedFileGuard::Present {
		identity: crate::bounded_io::file_identity(&metadata),
		length: metadata.len(),
	};
	let validation = validate_open_file(file_ref, &actual)
		.and_then(|()| {
			if matches!(expected, PreparedFileGuard::Present { .. }) && &actual != expected {
				return Err("manifest deletion outbox lock changed after startup validation".into())
			}
			validate_file_guard_at(directory, MANIFEST_DELETION_OUTBOX_LOCK_FILE, &actual)
		});
	let acquired = AcquiredPreparedOutboxLock {
		file,
		directory: owned_directory,
		canonical: actual,
	};
	if let Err(error) = validation {
		return match acquired.rollback_created() {
			Ok(()) => Err(error),
			Err(cleanup) => Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
		}
	}
	Ok(acquired)
}

async fn read_bounded_jsonl_tail(
	directory: &fs::File,
	lock: &AcquiredOutboxLock,
	limit: u64,
) -> Result<Vec<u8>, String> {
	lock.validate()?;
	let directory = directory.try_clone().map_err(|error| error.to_string())?;
	let file = tokio::task::spawn_blocking(move || {
		crate::bounded_io::open_optional_regular_file_at(
			&directory,
			MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
			false,
		)
		.map_err(|error| error.to_string())
	})
		.await
		.map_err(|error| error.to_string())??;
	let mut file = match file {
		Some(file) => tokio::fs::File::from_std(file),
		None => {
			lock.validate()?;
			return Ok(Vec::new())
		},
	};
	let len = file.metadata().await.map_err(|error| error.to_string())?.len();
	let start = len.saturating_sub(limit);
	file.seek(std::io::SeekFrom::Start(start))
		.await
		.map_err(|error| error.to_string())?;
	let mut bytes = Vec::with_capacity((len - start) as usize);
	file.take(limit)
		.read_to_end(&mut bytes)
		.await
		.map_err(|error| error.to_string())?;
	if start > 0 {
		if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
			bytes.drain(..=newline);
		} else {
			bytes.clear();
		}
	}
	lock.validate()?;
	Ok(bytes)
}

impl JsonlManifestDeletionOutbox {
	fn installed_root_capability(
		&self,
	) -> Result<(fs::File, fs::File, PreparedFileGuard), String> {
		let root_capability = self.root_capability
			.lock()
			.map_err(|_| "manifest deletion outbox root capability lock was poisoned")?;
		let installed = root_capability.as_ref()
			.ok_or_else(|| "manifest deletion outbox root capability is unavailable".to_string())?;
		Ok((
			installed.directory.try_clone().map_err(|error| error.to_string())?,
			installed.lock_file.try_clone().map_err(|error| error.to_string())?,
			installed.lock.clone(),
		))
	}

	async fn lock_outbox(
		&self,
		directory: &fs::File,
		lock: fs::File,
		expected: &PreparedFileGuard,
	) -> Result<AcquiredOutboxLock, String> {
		let directory = directory.try_clone().map_err(|error| error.to_string())?;
		let expected = expected.clone();
		tokio::task::spawn_blocking(move || {
			FileExt::try_lock_exclusive(&lock)
				.map_err(|error| format!("manifest deletion outbox lock is already held: {error}"))?;
			let acquired = AcquiredOutboxLock { file: lock, directory, expected };
			acquired.validate()?;
			Ok(acquired)
		})
		.await
		.map_err(|error| error.to_string())?
	}

	/// Append while the caller holds the shared process lock for the complete dedupe decision.
	async fn append_locked(
		&self,
		directory: &fs::File,
		lock: &AcquiredOutboxLock,
		encoded: &mut Vec<u8>,
	) -> Result<(), String> {
		if encoded.len() as u64 > MAX_JSONL_RECORD_BYTES {
			return Err("outbox record exceeds the bounded line limit".into());
		}
		encoded.push(b'\n');
		lock.validate()?;
		let directory = directory.try_clone().map_err(|error| error.to_string())?;
		let sync_directory = directory.try_clone().map_err(|error| error.to_string())?;
		let file = tokio::task::spawn_blocking(move || {
			crate::bounded_io::open_append_regular_file_at(
				&directory,
				MANIFEST_DELETION_OUTBOX_FILE.as_ref(),
			)
			.map_err(|error| error.to_string())
		})
			.await
			.map_err(|error| error.to_string())??;
		let mut file = tokio::fs::File::from_std(file);
		let original_len = file.metadata().await.map_err(|error| error.to_string())?.len();
		file.write_all(&encoded).await.map_err(|error| error.to_string())?;
		file.sync_all().await.map_err(|error| error.to_string())?;
		if let Err(error) = lock.validate() {
			file.set_len(original_len).await.map_err(|rollback| {
				format!("{error}; outbox append rollback failed: {rollback}")
			})?;
			file.sync_all().await.map_err(|rollback| {
				format!("{error}; outbox append rollback sync failed: {rollback}")
			})?;
			return Err(error)
		}
		#[cfg(test)]
		if self.fail_next_directory_sync.swap(false, std::sync::atomic::Ordering::SeqCst) {
			return Err("injected parent directory sync failure".into());
		}
		tokio::task::spawn_blocking(move || {
			crate::bounded_io::sync_directory(&sync_directory).map_err(|error| error.to_string())
		})
		.await
		.map_err(|error| error.to_string())??;
		lock.validate()
	}
}

struct AcquiredOutboxLock {
	file: fs::File,
	directory: fs::File,
	expected: PreparedFileGuard,
}

impl Drop for AcquiredOutboxLock {
	fn drop(&mut self) {
		let _ = FileExt::unlock(&self.file);
	}
}

impl AcquiredOutboxLock {
	fn validate(&self) -> Result<(), String> {
		validate_open_file(&self.file, &self.expected)?;
		validate_file_guard_at(
			&self.directory,
			MANIFEST_DELETION_OUTBOX_LOCK_FILE,
			&self.expected,
		)
		.map_err(|_| "manifest deletion outbox lock changed after startup qualification".into())
	}
}

#[cfg(test)]
fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
	let mut value = path.as_os_str().to_os_string();
	value.push(suffix);
	PathBuf::from(value)
}

async fn repair_incomplete_jsonl_tail(
	directory: &fs::File,
	lock: &AcquiredOutboxLock,
) -> Result<(), String> {
	lock.validate()?;
	let directory = directory.try_clone().map_err(|error| error.to_string())?;
	let (guard, tail_start, expected_tail) = tokio::task::spawn_blocking(move || {
		let (guard, digest, tail_start, expected_tail, truncate_to) =
			prepare_jsonl_source_guard_at(&directory)?;
		apply_prepared_jsonl_guard(
			&directory,
			&guard,
			digest,
			tail_start,
			&expected_tail,
			truncate_to,
		)?;
		Ok::<_, String>((guard, tail_start, expected_tail))
	})
	.await
	.map_err(|error| error.to_string())??;
	if let Err(error) = lock.validate() {
		return match restore_prepared_jsonl_source(
			&lock.directory,
			&guard,
			tail_start,
			&expected_tail,
		) {
			Ok(()) => Err(error),
			Err(rollback) => Err(format!("{error}; outbox repair rollback failed: {rollback}")),
		}
	}
	Ok(())
}

/// Bounded canonical runtime-duty cadence.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
	/// Finalized checkpoint-v2 and manifest-deletion duty intake cadence.
	pub checkpoint_duty_interval: Duration,
}

impl Default for WorkerConfig {
	fn default() -> Self {
		Self { checkpoint_duty_interval: Duration::from_secs(6) }
	}
}

/// Run only canonical checkpoint-v2 intake and manifest-deletion intake until cancelled.
pub async fn run_workers<A: ChainAuthority>(
	service: Arc<ProviderService<A>>,
	config: WorkerConfig,
) {
	let mut runtime_duties = interval(config.checkpoint_duty_interval);
	runtime_duties.set_missed_tick_behavior(MissedTickBehavior::Skip);
	loop {
		runtime_duties.tick().await;
		let (checkpoint, deletion) = poll_canonical_runtime_duties_once(&service).await;
		if checkpoint.is_err() {
			emit_failure(ProviderFailureCode::CheckpointDutyIntakeFailed);
		}
		if deletion.is_err() {
			emit_failure(ProviderFailureCode::ManifestDeletionFailed);
		}
	}
}

async fn poll_canonical_runtime_duties_once<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> (Result<usize, String>, Result<usize, String>) {
	let checkpoint = poll_checkpoint_duties_once(service).await;
	let deletion = poll_manifest_deletions_once(service).await;
	(checkpoint, deletion)
}

/// Poll and durably stage one complete fixed-finalized-state checkpoint-duty snapshot.
///
/// Each non-terminal page is fsynced before its exact cursor is used. A retry or restart resumes
/// the same finalized hash; pending duties become visible only after the terminal page is
/// installed.
pub async fn poll_checkpoint_duties_once<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<usize, String> {
	let mut installed = 0usize;
	for _ in 0..MAX_CHECKPOINT_DUTY_PAGES_PER_POLL {
		let request = service
			.store()
			.checkpoint_duty_resume_request()
			.map_err(|error| error.to_string())?;
		let page = service
			.authority()
			.checkpoint_duties(request)
			.await
			.map_err(|error| error.to_string())?;
		installed = installed.saturating_add(page.duties.len());
		if service
			.store()
			.stage_checkpoint_duty_page(page)
			.map_err(|error| error.to_string())?
		{
			return Ok(installed);
		}
	}
	Err("checkpoint duty page bound exceeded".into())
}

/// Stage or resume one bounded fixed-finalized page, then durably hand off that page's duties.
pub async fn poll_manifest_deletions_once<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<usize, String> {
	let mut duties = service
		.store()
		.pending_manifest_deletions(MAX_MANIFEST_DELETIONS_PER_POLL)
		.map_err(|error| error.to_string())?;
	if duties.is_empty() {
		let request = service
			.store()
			.deletion_duty_resume_request()
			.map_err(|error| error.to_string())?;
		let page = service
			.authority()
			.deletion_duties(request)
			.await
			.map_err(|error| error.to_string())?;
		service
			.store()
			.stage_deletion_duty_page(page)
			.map_err(|error| error.to_string())?;
		duties = service
			.store()
			.pending_manifest_deletions(MAX_MANIFEST_DELETIONS_PER_POLL)
			.map_err(|error| error.to_string())?;
	}
	let mut completed = 0usize;
	for duty in duties {
		process_manifest_deletion(service, &duty).await?;
		service
			.store()
			.complete_manifest_deletion(&duty.manifest, &duty.duty_fingerprint)
			.map_err(|error| error.to_string())?;
		completed = completed.saturating_add(1);
	}
	Ok(completed)
}

async fn process_manifest_deletion<A: ChainAuthority>(
	service: &ProviderService<A>,
	duty: &DeletionDuty,
) -> Result<(), String> {
	let manifest = decode_prefixed_hash(&duty.manifest, "manifest")?;
	let bucket = decode_prefixed_hash(&duty.bucket_id, "bucket")?;
	let provider_commitment =
		decode_prefixed_hash(&duty.provider_commitment, "provider commitment")?;
	let evidence = service
		.checkpoint_stack()
		.tombstone_manifest(
			manifest,
			BucketId::from_bytes(bucket),
			provider_commitment,
			duty.tombstoned_at,
		)
		.map_err(|error| error.to_string())?;
	let digest = manifest_deletion_ack_digest(
		H256::from(*evidence.bucket_id.as_bytes()),
		evidence.manifest,
		H256::from(evidence.evidence_hash),
		evidence.tombstoned_at,
	);
	let (service_key, signature) = service.sign_manifest_deletion_digest(digest);
	service
		.outbox()
		.submit_manifest_deletion(ManifestDeletionSubmission {
			manifest: duty.manifest.clone(),
			bucket_id: duty.bucket_id.clone(),
			provider_commitment: duty.provider_commitment.clone(),
			evidence_hash: format!("0x{}", hex::encode(evidence.evidence_hash)),
			tombstoned_at: duty.tombstoned_at,
			service_key: format!("0x{}", hex::encode(service_key)),
			signature: format!("0x{}", hex::encode(signature)),
			duty_fingerprint: duty.duty_fingerprint.clone(),
		})
		.await
}

pub(crate) fn manifest_deletion_ack_digest(
	bucket_id: H256,
	manifest: [u8; 32],
	evidence_hash: H256,
	tombstoned_at: u32,
) -> [u8; 32] {
	sp_crypto_hashing::blake2_256(
		&(b"cord/storage/deletion-ack/v1", bucket_id, manifest, evidence_hash, tombstoned_at)
			.encode(),
	)
}

fn decode_prefixed_hash(value: &str, label: &str) -> Result<[u8; 32], String> {
	let raw = value.strip_prefix("0x").ok_or_else(|| format!("{label} is not 0x-prefixed"))?;
	if raw.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(format!("{label} is not canonical lowercase hex"));
	}
	hex::decode(raw)
		.map_err(|error| error.to_string())?
		.try_into()
		.map_err(|_| format!("{label} is not 32 bytes"))
}

pub(crate) fn deletion_submission(
	pending: &PendingDeletion,
) -> Result<ContentDeletionSubmission, String> {
	Ok(ContentDeletionSubmission {
		agreement_id: pending.agreement_id.clone(),
		content_commitment: format!("0x{}", pending.commitment),
		authorized_at: pending.authorized_at.clone(),
		tombstone_root: format!("0x{}", pending.tombstone_root),
		root_sequence: pending.root_sequence,
		tombstone_leaf: format!("0x{}", pending.tombstone_leaf),
		leaf_index: pending.leaf_index,
		leaf_count: pending.leaf_count,
		inclusion_proof: pending.inclusion_proof.iter().map(|hash| format!("0x{hash}")).collect(),
	})
}

/// Flush every journaled root in ascending sequence order while the caller holds the service's
/// shared root/outbox ordering lock. A deletion root and acknowledgement are one indivisible
/// ordering unit; failures stop the scan before any later sequence can be queued or cleared.
pub(crate) async fn flush_pending_submissions(
	store: &crate::DiskStore,
	outbox: &dyn LegacyDiskCompletionSubmitter,
) -> Result<(), String> {
	let deletions = store.pending_deletions().map_err(|error| error.to_string())?;
	for pending in store.pending_root_submissions().map_err(|error| error.to_string())? {
		if let Some(deletion) = deletions.iter().find(|item| item.root_sequence == pending.sequence)
		{
			outbox.submit_deletion(deletion_submission(deletion)?).await?;
			store.complete_delete(&deletion.commitment).map_err(|error| error.to_string())?;
			store
				.complete_root_submission(pending.sequence)
				.map_err(|error| error.to_string())?;
		} else {
			outbox.submit_root(root_submission(&pending)).await?;
			store
				.complete_root_submission(pending.sequence)
				.map_err(|error| error.to_string())?;
		}
	}
	Ok(())
}

pub(crate) fn root_submission(pending: &PendingRootSubmission) -> ProviderRootSubmission {
	ProviderRootSubmission {
		sequence: pending.sequence,
		appended_leaves: pending.appended_leaves.iter().map(|leaf| format!("0x{leaf}")).collect(),
		expected_root: format!("0x{}", pending.expected_root),
		expected_leaf_count: pending.expected_leaf_count,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::{
		atomic::{AtomicBool, AtomicUsize, Ordering},
		Mutex as StdMutex,
	};

	use crate::{
		storage::CommitInput, AgreementAuthorization, ChainError, ChallengeBatch, DiskStore,
		NodeProfile,
	};
	use sp_core::{crypto::AccountId32, Pair as _};

	#[derive(Default)]
	struct FaultOutbox {
		log: StdMutex<Vec<String>>,
		fail_next_root: AtomicBool,
		partial_delete_once: AtomicBool,
	}

	struct NoopAuthority;

	#[derive(Default)]
	struct CanonicalOnlyAuthority {
		checkpoint_calls: AtomicUsize,
		deletion_calls: AtomicUsize,
		legacy_challenge_calls: AtomicUsize,
	}

	fn manifest_deletion(byte: &str) -> ManifestDeletionSubmission {
		ManifestDeletionSubmission {
			manifest: format!("0x{}", byte.repeat(32)),
			bucket_id: format!("0x{}", "22".repeat(32)),
			provider_commitment: format!("0x{}", "33".repeat(32)),
			evidence_hash: format!("0x{}", "44".repeat(32)),
			tombstoned_at: 70,
			service_key: format!("0x{}", "55".repeat(32)),
			signature: format!("0x{}", "66".repeat(64)),
			duty_fingerprint: format!("0x{}", "77".repeat(32)),
		}
	}

	fn qualify(outbox: &JsonlManifestDeletionOutbox) {
		let root = outbox.path.parent().unwrap();
		let locked = lock_test_root(root);
		outbox
			.prepare_startup(root, Some(locked.file()))
			.unwrap()
			.arm(&locked)
			.unwrap()
			.apply()
			.unwrap();
	}

	fn lock_test_root(root: &Path) -> crate::bounded_io::LockedDirectory {
		crate::bounded_io::prepare_directory_path(root)
			.unwrap()
			.lock_existing()
			.unwrap()
	}

	fn direct_test_plan(
		root: &crate::bounded_io::LockedDirectory,
		qualified: Arc<AtomicBool>,
	) -> ManifestDeletionStartupPlan {
		prepare_jsonl_manifest_deletion_outbox(
			Some(root.file()),
			qualified,
			Arc::new(std::sync::Mutex::new(None)),
		)
		.unwrap()
	}

	fn direct_test_plan_result(
		root: &crate::bounded_io::LockedDirectory,
	) -> Result<ManifestDeletionStartupPlan, String> {
		prepare_jsonl_manifest_deletion_outbox(
			Some(root.file()),
			Arc::new(AtomicBool::new(false)),
			Arc::new(std::sync::Mutex::new(None)),
		)
	}

	#[test]
	fn jsonl_startup_prepare_is_read_only_and_apply_truncates_only_the_torn_tail() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("provider-submissions-v3.jsonl");
		let durable = b"complete-record\n";
		let mut original = durable.to_vec();
		original.extend_from_slice(b"torn-tail");
		fs::write(&path, &original).unwrap();

		let qualified = Arc::new(AtomicBool::new(false));
		let locked = lock_test_root(temp.path());
		let plan = direct_test_plan(&locked, Arc::clone(&qualified));

		assert_eq!(fs::read(&path).unwrap(), original);
		let armed = plan.arm(&locked).unwrap();
		assert_eq!(fs::read(&path).unwrap(), original);
		armed.apply().unwrap();
		assert_eq!(fs::read(path).unwrap(), durable);
		assert!(qualified.load(Ordering::Acquire));
	}

	#[test]
	fn startup_recovers_exact_crash_shaped_owned_lock_residue() {
		let temp = tempfile::tempdir().unwrap();
		let residue = temp
			.path()
			.join(".provider-lock.create-0123456789abcdef0123456789abcdef");
		fs::write(&residue, b"").unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		let locked = lock_test_root(temp.path());
		let plan = outbox.prepare_startup(temp.path(), Some(locked.file())).unwrap();
		assert!(residue.is_file());

		plan.arm(&locked).unwrap().apply().unwrap();

		assert!(!residue.exists());
		assert!(temp.path().join(MANIFEST_DELETION_OUTBOX_LOCK_FILE).is_file());
		assert!(outbox.qualified.load(Ordering::Acquire));
	}

	#[cfg(unix)]
	#[test]
	fn jsonl_startup_prepare_rejects_symlink_and_nonregular_paths() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let target = temp.path().join("target.jsonl");
		fs::write(&target, b"durable\n").unwrap();
		let link = temp.path().join("provider-submissions-v3.jsonl");
		symlink(&target, &link).unwrap();
		let locked = lock_test_root(temp.path());
		assert!(direct_test_plan_result(&locked).is_err());
		assert_eq!(fs::read(target).unwrap(), b"durable\n");

		let nonregular = tempfile::tempdir().unwrap();
		fs::create_dir(nonregular.path().join(MANIFEST_DELETION_OUTBOX_FILE)).unwrap();
		let locked = lock_test_root(nonregular.path());
		assert!(direct_test_plan_result(&locked).is_err());
	}

	#[tokio::test]
	async fn public_jsonl_submit_is_inert_until_the_private_startup_plan_is_applied() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("provider-submissions-v3.jsonl");
		let outbox = JsonlManifestDeletionOutbox::new(&path);

		assert_eq!(
			outbox.submit_manifest_deletion(manifest_deletion("11")).await.unwrap_err(),
			"manifest deletion outbox startup has not been applied",
		);
		assert!(!path.exists());
		assert!(!suffixed_path(&path, ".lock").exists());

		qualify(&outbox);
		outbox.submit_manifest_deletion(manifest_deletion("11")).await.unwrap();
		assert!(fs::read(path).unwrap().ends_with(b"\n"));
	}

	#[test]
	fn armed_jsonl_holds_the_consumer_lock_and_keeps_the_published_lock_permanent() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		let lock_path = suffixed_path(&outbox.path, ".lock");
		let locked = lock_test_root(temp.path());
		let armed = outbox
			.prepare_startup(temp.path(), Some(locked.file()))
			.unwrap()
			.arm(&locked)
			.unwrap();
		let contender = fs::OpenOptions::new().read(true).write(true).open(&lock_path).unwrap();

		assert!(FileExt::try_lock_exclusive(&contender).is_err());
		armed.rollback().unwrap();
		assert!(lock_path.exists());
		assert!(!outbox.qualified.load(Ordering::Acquire));
	}

	#[test]
	fn held_jsonl_startup_lock_fails_without_waiting() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		let locked = lock_test_root(temp.path());
		let held = outbox
			.prepare_startup(temp.path(), Some(locked.file()))
			.unwrap()
			.arm(&locked)
			.unwrap();
		let contender = outbox.prepare_startup(temp.path(), Some(locked.file())).unwrap();
		let contender_root = locked.file().try_clone().unwrap();
		let (sender, receiver) = std::sync::mpsc::channel();
		let thread = std::thread::spawn(move || {
			let result = acquire_prepared_outbox_lock(&contender_root, &contender.jsonl.unwrap().lock)
				.map(|lock| lock.rollback_created());
			sender.send(result).unwrap();
		});

		let result = match receiver.recv_timeout(Duration::from_secs(2)) {
			Ok(result) => result,
			Err(error) => {
				held.rollback().unwrap();
				thread.join().unwrap();
				panic!("contended startup lock did not fail promptly: {error}");
			},
		};
		assert!(result.unwrap_err().contains("already held"));
		held.rollback().unwrap();
		thread.join().unwrap();
	}

	#[test]
	fn three_missing_lock_contenders_never_unlink_the_winner() {
		let temp = tempfile::tempdir().unwrap();
		let locked = lock_test_root(temp.path());
		let barrier = Arc::new(std::sync::Barrier::new(3));
		let mut threads = Vec::new();
		for _ in 0..3 {
			let directory = locked.file().try_clone().unwrap();
			let barrier = Arc::clone(&barrier);
			threads.push(std::thread::spawn(move || {
				barrier.wait();
				acquire_prepared_outbox_lock(&directory, &PreparedFileGuard::Missing)
			}));
		}
		let mut winners = Vec::new();
		for thread in threads {
			if let Ok(lock) = thread.join().unwrap() {
				winners.push(lock);
			}
		}
		assert_eq!(winners.len(), 1);
		assert!(crate::bounded_io::regular_file_exists_at(
			locked.file(),
			MANIFEST_DELETION_OUTBOX_LOCK_FILE.as_ref(),
		)
		.unwrap());
		assert!(!crate::bounded_io::list_directory(locked.file())
			.unwrap()
			.iter()
			.any(|entry| entry.name.to_string_lossy().starts_with(".provider-lock.create-")));
		winners.pop().unwrap().rollback_created().unwrap();
	}

	#[test]
	fn owned_lock_failures_after_publish_keep_the_canonical_lock_permanent() {
		for failure in [
			PreparedLockFailure::Publish(crate::bounded_io::OwnedLockPublishStage::AfterRename),
			PreparedLockFailure::Publish(
				crate::bounded_io::OwnedLockPublishStage::BeforeDirectorySync,
			),
			PreparedLockFailure::Transfer,
		] {
			let temp = tempfile::tempdir().unwrap();
			let locked = lock_test_root(temp.path());
			let result = acquire_prepared_outbox_lock_inner(
				locked.file(),
				&PreparedFileGuard::Missing,
				Some(failure),
			);
			assert!(result.is_err());
			assert!(temp.path().join(MANIFEST_DELETION_OUTBOX_LOCK_FILE).is_file());
			assert!(!crate::bounded_io::list_directory(locked.file())
				.unwrap()
				.iter()
				.any(|entry| entry.name.to_string_lossy().starts_with(".provider-lock.create-")));

			let (_, expected) = open_optional_guarded_file_at(
				locked.file(),
				MANIFEST_DELETION_OUTBOX_LOCK_FILE,
				false,
			)
			.unwrap()
			.unwrap();
			acquire_prepared_outbox_lock(locked.file(), &expected)
				.unwrap()
				.rollback_created()
				.unwrap();
		}
	}

	#[test]
	fn startup_lock_displacement_fails_before_source_repair_or_qualification() {
		let temp = tempfile::tempdir().unwrap();
		let source = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let lock = temp.path().join(MANIFEST_DELETION_OUTBOX_LOCK_FILE);
		let displaced = temp.path().join("displaced-lock");
		let original = b"durable\ntorn";
		fs::write(&source, original).unwrap();
		fs::write(&lock, b"").unwrap();
		let qualified = Arc::new(AtomicBool::new(false));
		let locked = lock_test_root(temp.path());
		let armed = direct_test_plan(&locked, Arc::clone(&qualified)).arm(&locked).unwrap();
		fs::rename(&lock, &displaced).unwrap();
		fs::write(&lock, b"replacement").unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(source).unwrap(), original);
		assert_eq!(fs::read(lock).unwrap(), b"replacement");
		assert!(!qualified.load(Ordering::Acquire));
	}

	#[tokio::test]
	async fn qualified_consumer_rejects_lock_replacement_before_repair_read_or_append() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		qualify(&outbox);
		let source = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let lock = temp.path().join(MANIFEST_DELETION_OUTBOX_LOCK_FILE);
		let displaced = temp.path().join("qualified-displaced-lock");
		let original = b"durable\ntorn";
		fs::write(&source, original).unwrap();
		fs::rename(&lock, &displaced).unwrap();
		let displaced_contender = fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(&displaced)
			.unwrap();
		FileExt::try_lock_exclusive(&displaced_contender).unwrap();
		fs::write(&lock, b"replacement").unwrap();

		assert!(outbox.submit_manifest_deletion(manifest_deletion("91")).await.is_err());
		assert_eq!(fs::read(source).unwrap(), original);
		assert_eq!(fs::read(lock).unwrap(), b"replacement");
		FileExt::unlock(&displaced_contender).unwrap();
	}

	#[tokio::test]
	async fn qualified_exact_lock_fd_is_unlocked_when_idle_and_released_after_each_guard() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		qualify(&outbox);
		let lock_path = temp.path().join(MANIFEST_DELETION_OUTBOX_LOCK_FILE);
		let contender = fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(&lock_path)
			.unwrap();
		FileExt::try_lock_exclusive(&contender).unwrap();
		FileExt::unlock(&contender).unwrap();

		let (directory, lock_file, expected) = outbox.installed_root_capability().unwrap();
		let guard = outbox.lock_outbox(&directory, lock_file, &expected).await.unwrap();
		assert!(FileExt::try_lock_exclusive(&contender).is_err());
		drop(guard);
		FileExt::try_lock_exclusive(&contender).unwrap();
		FileExt::unlock(&contender).unwrap();
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn root_path_substitution_never_redirects_startup_or_live_outbox_io() {
		use std::os::unix::fs::symlink;

		for stage in ["before-arm", "before-apply", "after-qualification"] {
			let parent = tempfile::tempdir().unwrap();
			let root = parent.path().join("provider");
			fs::create_dir(&root).unwrap();
			let moved = parent.path().join("provider-moved");
			let external = tempfile::tempdir().unwrap();
			let outbox = JsonlManifestDeletionOutbox::for_provider_root(&root);
			let locked = lock_test_root(&root);
			let mut plan = Some(outbox.prepare_startup(&root, Some(locked.file())).unwrap());
			let mut armed = None;
			if stage != "before-arm" {
				armed = Some(plan.take().unwrap().arm(&locked).unwrap());
			}
			if stage == "after-qualification" {
				armed.take().unwrap().apply().unwrap();
			}
			fs::rename(&root, &moved).unwrap();
			symlink(external.path(), &root).unwrap();
			if stage == "before-arm" {
				plan.take().unwrap().arm(&locked).unwrap().apply().unwrap();
			} else if stage == "before-apply" {
				armed.take().unwrap().apply().unwrap();
			}
			if stage == "after-qualification" {
				outbox.submit_manifest_deletion(manifest_deletion("81")).await.unwrap();
				assert!(fs::read(moved.join(MANIFEST_DELETION_OUTBOX_FILE)).unwrap().ends_with(b"\n"));
			}
			assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0, "{stage}");
			assert!(moved.join(MANIFEST_DELETION_OUTBOX_LOCK_FILE).is_file());
		}
	}

	#[test]
	fn jsonl_startup_rejects_same_inode_prefix_mutation_outside_unchanged_tail() {
		for mutate_after_arm in [false, true] {
			let temp = tempfile::tempdir().unwrap();
			let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
			let mut bytes = vec![b'a'; MAX_JSONL_RECORD_BYTES as usize + 8 * 1024];
			*bytes.last_mut().unwrap() = b'\n';
			let tail_start = bytes.len() - (MAX_JSONL_RECORD_BYTES as usize + 1);
			let expected_tail = bytes[tail_start..].to_vec();
			fs::write(&path, &bytes).unwrap();
			let qualified = Arc::new(AtomicBool::new(false));
			let locked = lock_test_root(temp.path());
			let plan = direct_test_plan(&locked, Arc::clone(&qualified));
			let mut plan = Some(plan);
			let armed = mutate_after_arm.then(|| plan.take().unwrap().arm(&locked).unwrap());
			let mut source = fs::OpenOptions::new().write(true).open(&path).unwrap();
			source.seek(SeekFrom::Start(0)).unwrap();
			source.write_all(b"z").unwrap();
			source.sync_all().unwrap();
			drop(source);

			let result = match armed {
				Some(armed) => armed.apply(),
				None => plan
					.take()
					.unwrap()
					.arm(&locked)
					.and_then(ArmedManifestDeletionStartupPlan::apply),
			};
			assert!(result.is_err(), "mutate_after_arm={mutate_after_arm}");
			let current = fs::read(&path).unwrap();
			assert_eq!(current.len(), bytes.len());
			assert_eq!(&current[tail_start..], expected_tail);
			assert!(!suffixed_path(&path, ".lock").exists());
			assert!(!qualified.load(Ordering::Acquire));
		}
	}

	#[cfg(unix)]
	#[test]
	fn jsonl_startup_accepts_only_the_fixed_direct_child_of_the_validated_root() {
		use std::os::unix::fs::symlink;

		let root = tempfile::tempdir().unwrap();
		let outside = tempfile::tempdir().unwrap();
		let safe = JsonlManifestDeletionOutbox::for_provider_root(root.path());
		assert!(safe.prepare_startup(root.path(), None).is_ok());

		for path in [
			outside.path().join(MANIFEST_DELETION_OUTBOX_FILE),
			root.path().join("nested").join(MANIFEST_DELETION_OUTBOX_FILE),
			root.path().join("other.jsonl"),
		] {
			assert!(JsonlManifestDeletionOutbox::new(path)
				.prepare_startup(root.path(), None)
				.is_err());
		}

		let alias = outside.path().join("provider-alias");
		symlink(root.path(), &alias).unwrap();
		assert!(JsonlManifestDeletionOutbox::new(alias.join(MANIFEST_DELETION_OUTBOX_FILE))
			.prepare_startup(root.path(), None)
			.is_err());
	}

	#[test]
	fn every_jsonl_startup_state_rejects_a_changed_source_before_qualification() {
		for (name, initial, replacement) in [
			("missing", None, b"created-after-prepare".as_slice()),
			("empty", Some(b"".as_slice()), b"".as_slice()),
			("complete", Some(b"complete\n".as_slice()), b"replaced\n".as_slice()),
			("torn", Some(b"complete\ntorn".as_slice()), b"replaced\ntorn".as_slice()),
		] {
			let temp = tempfile::tempdir().unwrap();
			let path = temp.path().join("provider-submissions-v3.jsonl");
			if let Some(initial) = initial {
				fs::write(&path, initial).unwrap();
			}
			let qualified = Arc::new(AtomicBool::new(false));
			let locked = lock_test_root(temp.path());
			let plan = direct_test_plan(&locked, Arc::clone(&qualified));
			if path.exists() {
				fs::remove_file(&path).unwrap();
			}
			fs::write(&path, replacement).unwrap();

			assert!(plan.arm(&locked).is_err(), "{name}");
			assert_eq!(fs::read(&path).unwrap(), replacement, "{name}");
			assert!(!suffixed_path(&path, ".lock").exists(), "{name}");
			assert!(!qualified.load(Ordering::Acquire), "{name}");
		}
	}

	#[cfg(unix)]
	#[test]
	fn startup_apply_rejects_source_and_lock_symlink_swaps_without_touching_targets() {
		use std::os::unix::fs::symlink;

		for swap_lock in [false, true] {
			let temp = tempfile::tempdir().unwrap();
			let path = temp.path().join("provider-submissions-v3.jsonl");
			let lock = suffixed_path(&path, ".lock");
			fs::write(&path, b"durable\ntorn").unwrap();
			if swap_lock {
				fs::write(&lock, b"").unwrap();
			}
			let qualified = Arc::new(AtomicBool::new(false));
			let locked = lock_test_root(temp.path());
			let plan = direct_test_plan(&locked, Arc::clone(&qualified));
			let swapped = if swap_lock { &lock } else { &path };
			let target = temp.path().join(if swap_lock { "lock-target" } else { "source-target" });
			let target_bytes = if swap_lock { b"".as_slice() } else { b"external-safe".as_slice() };
			fs::write(&target, target_bytes).unwrap();
			fs::remove_file(swapped).unwrap();
			symlink(&target, swapped).unwrap();

			assert!(plan.arm(&locked).is_err());
			assert_eq!(fs::read(&target).unwrap(), target_bytes);
			if !swap_lock {
				assert!(!lock.exists());
			}
			assert!(!qualified.load(Ordering::Acquire));
		}
	}

	#[test]
	fn provider_manifest_deletion_signature_is_accepted_by_runtime_pallet() {
		use origin_commons_runtime::{Runtime, RuntimeOrigin, StorageProvider, System};
		use pallet_orbis_storage_control_primitives::CommitmentState;
		use pallet_orbis_storage_provider::{
			AssignedProvidersOf, CanonicalManifestRecord, CanonicalManifests,
			GovernedFinalizedCheckpoint, ManifestDeletionAcknowledgements,
			ManifestDeletionRequirements, OrganizationRefOf, ProviderOrganizationRefV1,
			ProviderRecord, ProviderStatus, Providers, ServiceKeyRecord,
		};
		use sp_core::ed25519;

		sp_io::TestExternalities::new_empty().execute_with(|| {
			System::set_block_number(90);
			GovernedFinalizedCheckpoint::<Runtime>::put(90);
			let provider = AccountId32::new([0x17; 32]);
			let pair = ed25519::Pair::from_seed(&[0x27; 32]);
			let organization: OrganizationRefOf<Runtime> = ProviderOrganizationRefV1 {
				entity_id: vec![1].try_into().unwrap(),
				attestation_id: H256::repeat_byte(1),
				schema_id: H256::repeat_byte(2),
				sla_commitment: H256::repeat_byte(3),
				sla_version: 1,
				valid_from: 1,
				valid_until: 1_000,
				rotation_predecessor: None,
			};
			Providers::<Runtime>::insert(
				provider.clone(),
				ProviderRecord {
					endpoint: vec![1].try_into().unwrap(),
					organization,
					service_key: ServiceKeyRecord {
						active: pair.public(),
						active_version: 1,
						previous: None,
						pending: None,
						pending_version: None,
						pending_effective_at: None,
					},
					capacity_bytes: 1_000,
					allocated_bytes: 0,
					pending_bytes: 0,
					status: ProviderStatus::Active,
					last_heartbeat: 90,
					authority_validated_at: Some(90),
				},
			);
			let manifest = [0x37; 32];
			let bucket_id = H256::repeat_byte(0x47);
			CanonicalManifests::<Runtime>::insert(
				manifest,
				CanonicalManifestRecord {
					bucket_id,
					provider_commitment: Some([0x57; 32]),
					state: CommitmentState::Tombstoned,
					checkpoint: Some(70),
					tombstoned_at: Some(80),
				},
			);
			let required: AssignedProvidersOf<Runtime> = vec![provider.clone()].try_into().unwrap();
			ManifestDeletionRequirements::<Runtime>::insert(manifest, required);
			let evidence_hash = H256::repeat_byte(0x67);
			let digest = manifest_deletion_ack_digest(bucket_id, manifest, evidence_hash, 80);
			let signature = pair.sign(&digest);
			let result = StorageProvider::acknowledge_manifest_deletion(
				RuntimeOrigin::signed(provider.clone()),
				manifest,
				evidence_hash,
				pair.public(),
				signature.clone(),
			);
			assert!(result.is_ok(), "runtime rejected provider-produced signature: {result:?}");
			assert!(ManifestDeletionAcknowledgements::<Runtime>::contains_key(manifest, &provider));
			let events = System::events().len();
			assert!(StorageProvider::acknowledge_manifest_deletion(
				RuntimeOrigin::signed(provider.clone()),
				manifest,
				evidence_hash,
				pair.public(),
				signature.clone(),
			)
			.is_ok());
			assert_eq!(System::events().len(), events);
			assert!(StorageProvider::acknowledge_manifest_deletion(
				RuntimeOrigin::signed(provider),
				manifest,
				H256::repeat_byte(0x68),
				pair.public(),
				signature,
			)
			.is_err());
		});
	}

	#[async_trait]
	impl ChainAuthority for NoopAuthority {
		async fn authorize_commit(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
			_bytes: u64,
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn authorize_delete(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn challenge_duties(
			&self,
			_after_block: Option<u32>,
		) -> Result<ChallengeBatch, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn checkpoint_duties(
			&self,
			_request: Option<crate::CheckpointDutyPageRequest>,
		) -> Result<crate::CheckpointDutyBatch, ChainError> {
			Err(ChainError::Decode("injected checkpoint duty decode failure".into()))
		}
	}

	#[async_trait]
	impl ChainAuthority for CanonicalOnlyAuthority {
		async fn authorize_commit(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
			_bytes: u64,
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("legacy object commit invoked".into()))
		}

		async fn authorize_delete(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("legacy object delete invoked".into()))
		}

		async fn challenge_duties(
			&self,
			_after_block: Option<u32>,
		) -> Result<ChallengeBatch, ChainError> {
			self.legacy_challenge_calls.fetch_add(1, Ordering::SeqCst);
			Err(ChainError::Rejected("legacy challenge worker invoked".into()))
		}

		async fn checkpoint_duties(
			&self,
			_request: Option<crate::CheckpointDutyPageRequest>,
		) -> Result<crate::CheckpointDutyBatch, ChainError> {
			self.checkpoint_calls.fetch_add(1, Ordering::SeqCst);
			Ok(crate::CheckpointDutyBatch {
				finalized_hash: format!("0x{}", "10".repeat(32)),
				finalized_number: 71,
				provider: profile().provider,
				snapshot_checkpoint: 70,
				requested_cursor: None,
				next_cursor: None,
				duties: Vec::new(),
			})
		}

		async fn deletion_duties(
			&self,
			_request: Option<crate::DeletionDutyPageRequest>,
		) -> Result<crate::DeletionDutyBatch, ChainError> {
			self.deletion_calls.fetch_add(1, Ordering::SeqCst);
			Ok(crate::DeletionDutyBatch {
				finalized_hash: format!("0x{}", "20".repeat(32)),
				finalized_number: 71,
				provider: profile().provider,
				snapshot_checkpoint: 70,
				requested_cursor: None,
				next_cursor: None,
				duties: Vec::new(),
			})
		}
	}

	impl FaultOutbox {
		fn log(&self) -> Vec<String> {
			self.log.lock().unwrap().clone()
		}
	}

	#[async_trait]
	impl LegacyDiskCompletionSubmitter for FaultOutbox {
		async fn submit_root(&self, request: ProviderRootSubmission) -> Result<(), String> {
			if self.fail_next_root.swap(false, Ordering::SeqCst) {
				return Err("injected root failure".into());
			}
			self.log.lock().unwrap().push(format!("root-{}", request.sequence));
			Ok(())
		}

		async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String> {
			self.log.lock().unwrap().push(format!("root-{}", request.root_sequence));
			if self.partial_delete_once.swap(false, Ordering::SeqCst) {
				return Err("injected failure after deletion root".into());
			}
			self.log.lock().unwrap().push(format!("ack-{}", request.root_sequence));
			Ok(())
		}
	}

	#[async_trait]
	impl ManifestDeletionSubmitter for FaultOutbox {}

	fn profile() -> NodeProfile {
		let service_key = sp_core::ed25519::Pair::from_seed(&[7u8; 32]).public();
		NodeProfile {
			provider: "01".repeat(32),
			endpoint: "http://127.0.0.1:8080".into(),
			service_key: hex::encode(service_key.0),
			region: None,
		}
	}

	fn authorization(commitment: [u8; 32], bytes: u64) -> AgreementAuthorization {
		AgreementAuthorization {
			finalized_hash: format!("0x{}", "11".repeat(32)),
			agreement_id: format!("0x{}", hex::encode(commitment)),
			provider: profile().provider,
			container_ref: format!("0x{}", "03".repeat(32)),
			bytes,
			expires_at: 100,
		}
	}

	fn commit(store: &DiskStore, value: u8) -> (String, AgreementAuthorization) {
		let bytes = vec![value];
		let commitment = DiskStore::content_commitment(&bytes);
		let authorization = authorization(commitment, bytes.len() as u64);
		let record = store
			.commit(CommitInput {
				commitment,
				authorization: authorization.clone(),
				bucket: None,
				key: None,
				bytes,
			})
			.unwrap();
		(record.commitment, authorization)
	}

	fn service(store: Arc<DiskStore>) -> ProviderService<NoopAuthority> {
		ProviderService::new_preopened(
			store,
			Arc::new(NoopAuthority),
			sp_core::ed25519::Pair::from_seed(&[7u8; 32]),
			Arc::new(FaultOutbox::default()),
		)
		.unwrap()
	}

	#[tokio::test]
	async fn checkpoint_duty_authority_failure_cannot_advance_durable_intake() {
		let temp = tempfile::tempdir().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let service = service(store.clone());
		assert!(poll_checkpoint_duties_once(&service).await.is_err());
		assert!(store.checkpoint_duty_resume_request().unwrap().is_none());
		assert!(store.checkpoint_duty_watermark().unwrap().is_none());
		assert!(store.pending_checkpoint_duties().unwrap().is_empty());
	}

	#[tokio::test]
	async fn production_duty_tick_never_invokes_legacy_challenge_or_completion_seams() {
		let temp = tempfile::tempdir().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let authority = Arc::new(CanonicalOnlyAuthority::default());
		let outbox = Arc::new(FaultOutbox::default());
		let service = ProviderService::new_preopened(
			store,
			Arc::clone(&authority),
			sp_core::ed25519::Pair::from_seed(&[7u8; 32]),
			outbox.clone(),
		)
		.unwrap();

		let (checkpoints, deletions) = poll_canonical_runtime_duties_once(&service).await;
		assert_eq!(checkpoints.unwrap(), 0);
		assert_eq!(deletions.unwrap(), 0);
		assert_eq!(authority.checkpoint_calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.deletion_calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.legacy_challenge_calls.load(Ordering::SeqCst), 0);
		assert!(outbox.log().is_empty());
	}

	#[tokio::test]
	async fn manifest_deletion_outbox_replay_is_idempotent_and_conflicts_fail_closed() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let outbox = JsonlManifestDeletionOutbox::new(&path);
		qualify(&outbox);
		let request = ManifestDeletionSubmission {
			manifest: format!("0x{}", "11".repeat(32)),
			bucket_id: format!("0x{}", "22".repeat(32)),
			provider_commitment: format!("0x{}", "33".repeat(32)),
			evidence_hash: format!("0x{}", "44".repeat(32)),
			tombstoned_at: 70,
			service_key: format!("0x{}", "55".repeat(32)),
			signature: format!("0x{}", "66".repeat(64)),
			duty_fingerprint: format!("0x{}", "77".repeat(32)),
		};
		outbox.submit_manifest_deletion(request.clone()).await.unwrap();
		outbox.submit_manifest_deletion(request.clone()).await.unwrap();
		assert_eq!(tokio::fs::read_to_string(&path).await.unwrap().lines().count(), 1);
		let mut conflict = request;
		conflict.evidence_hash = format!("0x{}", "88".repeat(32));
		assert!(outbox.submit_manifest_deletion(conflict).await.is_err());
		assert_eq!(tokio::fs::read_to_string(path).await.unwrap().lines().count(), 1);
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
	async fn independent_manifest_deletion_producers_dedupe_under_one_process_lock() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let request = manifest_deletion("11");
		let producer_count = 16;
		let barrier = Arc::new(tokio::sync::Barrier::new(producer_count));
		let mut tasks = Vec::with_capacity(producer_count);
		for _ in 0..producer_count {
			let barrier = barrier.clone();
			let request = request.clone();
			let outbox = JsonlManifestDeletionOutbox::new(&path);
			qualify(&outbox);
			tasks.push(tokio::spawn(async move {
				barrier.wait().await;
				outbox.submit_manifest_deletion(request).await
			}));
		}

		for task in tasks {
			task.await.unwrap().unwrap();
		}
		assert_eq!(tokio::fs::read_to_string(path).await.unwrap().lines().count(), 1);
	}

	#[tokio::test]
	async fn manifest_deletion_submission_is_bounded_with_large_unrelated_outbox_prefix() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let prefix = b"{}\n".repeat(700_000);
		tokio::fs::write(&path, &prefix).await.unwrap();
		let outbox = JsonlManifestDeletionOutbox::new(&path);
		qualify(&outbox);
		let request = ManifestDeletionSubmission {
			manifest: format!("0x{}", "11".repeat(32)),
			bucket_id: format!("0x{}", "22".repeat(32)),
			provider_commitment: format!("0x{}", "33".repeat(32)),
			evidence_hash: format!("0x{}", "44".repeat(32)),
			tombstoned_at: 70,
			service_key: format!("0x{}", "55".repeat(32)),
			signature: format!("0x{}", "66".repeat(64)),
			duty_fingerprint: format!("0x{}", "77".repeat(32)),
		};
		outbox.submit_manifest_deletion(request.clone()).await.unwrap();
		let once = tokio::fs::metadata(&path).await.unwrap().len();
		outbox.submit_manifest_deletion(request).await.unwrap();
		assert_eq!(tokio::fs::metadata(path).await.unwrap().len(), once);
		assert!(once > prefix.len() as u64);
	}

	#[tokio::test]
	async fn producer_repairs_torn_final_submission_before_manifest_retry() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let outbox = JsonlManifestDeletionOutbox::new(&path);
		qualify(&outbox);
		outbox.submit_manifest_deletion(manifest_deletion("11")).await.unwrap();
		let mut file = tokio::fs::OpenOptions::new().append(true).open(&path).await.unwrap();
		file.write_all(b"{\"kind\":\"provider_root\"").await.unwrap();
		file.sync_all().await.unwrap();
		outbox.submit_manifest_deletion(manifest_deletion("12")).await.unwrap();
		let submissions: Vec<_> = tokio::fs::read_to_string(&path)
			.await
			.unwrap()
			.lines()
			.map(|line| serde_json::from_str::<ProviderSubmission>(line).unwrap())
			.collect();
		assert_eq!(submissions.len(), 2);
		assert!(
			matches!(&submissions[0], ProviderSubmission::ManifestDeletion(request) if request.manifest == format!("0x{}", "11".repeat(32)))
		);
		assert!(
			matches!(&submissions[1], ProviderSubmission::ManifestDeletion(request) if request.manifest == format!("0x{}", "12".repeat(32)))
		);
	}

	#[tokio::test]
	async fn producer_never_reports_success_before_parent_directory_sync() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(MANIFEST_DELETION_OUTBOX_FILE);
		let outbox = JsonlManifestDeletionOutbox::new(&path);
		qualify(&outbox);
		outbox.fail_next_directory_sync.store(true, Ordering::SeqCst);
		let result = outbox.submit_manifest_deletion(manifest_deletion("11")).await;
		assert_eq!(result.unwrap_err(), "injected parent directory sync failure");
		assert!(tokio::fs::read(&path).await.unwrap().ends_with(b"\n"));
	}

	#[tokio::test]
	async fn failed_root_is_flushed_before_a_later_commit_or_delete_sequence() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let outbox = FaultOutbox::default();
		outbox.fail_next_root.store(true, Ordering::SeqCst);
		let (_first, _) = commit(&store, 1);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		assert_eq!(store.pending_root_submissions().unwrap()[0].sequence, 1);

		// This is the pre-mutation flush performed by /commit and /delete.
		flush_pending_submissions(&store, &outbox).await.unwrap();
		let (second, second_authorization) = commit(&store, 2);
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2"]);

		store.prepare_delete(&second, &second_authorization).unwrap();
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "root-3", "ack-3"]);
		assert!(store.pending_root_submissions().unwrap().is_empty());
		assert!(store.pending_deletions().unwrap().is_empty());
	}

	#[tokio::test]
	async fn failed_root_is_flushed_before_delete_is_created() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let outbox = FaultOutbox::default();
		outbox.fail_next_root.store(true, Ordering::SeqCst);
		let (commitment, authorization) = commit(&store, 4);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		flush_pending_submissions(&store, &outbox).await.unwrap();
		store.prepare_delete(&commitment, &authorization).unwrap();
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "ack-2"]);
	}

	#[tokio::test]
	async fn partial_deletion_submission_recovers_after_restart_before_later_sequence() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = FaultOutbox::default();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let (commitment, authorization) = commit(&store, 5);
		flush_pending_submissions(&store, &outbox).await.unwrap();
		store.prepare_delete(&commitment, &authorization).unwrap();
		outbox.partial_delete_once.store(true, Ordering::SeqCst);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		assert_eq!(outbox.log(), vec!["root-1", "root-2"]);
		assert_eq!(store.pending_root_submissions().unwrap()[0].sequence, 2);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		flush_pending_submissions(&reopened, &outbox).await.unwrap();
		commit(&reopened, 6);
		flush_pending_submissions(&reopened, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "root-2", "ack-2", "root-3"]);
		assert!(reopened.pending_root_submissions().unwrap().is_empty());
	}

	#[test]
	fn shared_deletion_vector_recomputes_leaf_proof_and_root_cryptographically() {
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../../docs/sdk/vectors/storage-provider-deletion-v1.json");
		let vector: serde_json::Value =
			serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
		let hash = |value: &str| -> [u8; 32] {
			hex::decode(value.trim_start_matches("0x")).unwrap().try_into().unwrap()
		};
		let input = &vector["input"];
		let agreement = H256::from(hash(input["agreement_id"].as_str().unwrap()));
		let content = H256::from(hash(input["content_commitment"].as_str().unwrap()));
		let provider = AccountId32::new(hash(input["provider_account_id32"].as_str().unwrap()));
		let nonce = input["deletion_nonce"].as_str().unwrap().parse::<u64>().unwrap();
		let tombstone = sp_crypto_hashing::blake2_256(
			&(b"orbis/provider-deletion-leaf/v1", agreement, content, &provider, nonce).encode(),
		);
		assert_eq!(
			hex::encode(tombstone),
			vector["expected"]["tombstone_leaf"].as_str().unwrap().trim_start_matches("0x")
		);
		let leaves: Vec<[u8; 32]> = vector["append_only_accumulator"]["sequence_1"]
			["appended_leaves"]
			.as_array()
			.unwrap()
			.iter()
			.map(|value| hash(value.as_str().unwrap()))
			.collect();
		assert_eq!(leaves[1], tombstone);
		let proof = crate::merkle::proof(&leaves, 1).unwrap();
		let expected_proof: Vec<[u8; 32]> = vector["expected"]["inclusion_proof"]
			.as_array()
			.unwrap()
			.iter()
			.map(|value| hash(value.as_str().unwrap()))
			.collect();
		assert_eq!(proof, expected_proof);
		assert_eq!(
			crate::merkle::root(&leaves),
			hash(vector["expected"]["tombstone_root"].as_str().unwrap())
		);
	}
}

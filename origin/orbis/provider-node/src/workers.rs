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
	io::{Read as _, Seek as _, SeekFrom},
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
#[cfg(unix)]
use rustix::fs::{self as unix_fs, Mode, OFlags};
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

use crate::{
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
pub trait ManifestDeletionSubmitter: Send + Sync + 'static {
	/// Validate any local durable recovery owned by this submitter without mutating it.
	fn prepare_startup(
		&self,
		_provider_root: &Path,
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
pub struct ManifestDeletionStartupPlan {
	jsonl: Option<PreparedJsonlStartup>,
}

impl ManifestDeletionStartupPlan {
	pub(crate) fn arm(self) -> Result<ArmedManifestDeletionStartupPlan, String> {
		let jsonl = self.jsonl.map(PreparedJsonlStartup::arm).transpose()?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
	device: u64,
	inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreparedFileGuard {
	Missing,
	Present { identity: FileIdentity, length: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparedProviderRootGuard {
	Missing,
	Present(FileIdentity),
}

struct PreparedJsonlStartup {
	provider_root: PathBuf,
	provider_root_guard: PreparedProviderRootGuard,
	path: PathBuf,
	lock: PreparedFileGuard,
	source: PreparedFileGuard,
	tail_start: u64,
	expected_tail: Vec<u8>,
	truncate_to: u64,
	qualified: Arc<AtomicBool>,
}

impl PreparedJsonlStartup {
	fn arm(self) -> Result<ArmedJsonlStartup, String> {
		let root_identity =
			validate_provider_root_for_arm(&self.provider_root, self.provider_root_guard)?;
		let lock = acquire_prepared_outbox_lock(&self.path, &self.lock)?;
		let validated = validate_provider_root_identity(&self.provider_root, root_identity)
			.and_then(|()| {
				validate_prepared_jsonl_guard(
					&self.path,
					&self.source,
					self.tail_start,
					&self.expected_tail,
				)
			});
		if let Err(error) = validated {
			return match lock.rollback_created() {
				Ok(()) => Err(error),
				Err(cleanup) =>
					Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
			}
		}
		Ok(ArmedJsonlStartup { prepared: self, lock })
	}
}

struct ArmedJsonlStartup {
	prepared: PreparedJsonlStartup,
	lock: AcquiredPreparedOutboxLock,
}

impl ArmedJsonlStartup {
	fn apply(mut self) -> Result<(), String> {
		let applied = apply_prepared_jsonl_guard(
			&self.prepared.path,
			&self.prepared.source,
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
pub struct JsonlManifestDeletionOutbox {
	path: PathBuf,
	write_lock: Mutex<()>,
	qualified: Arc<AtomicBool>,
	#[cfg(test)]
	fail_next_directory_sync: std::sync::atomic::AtomicBool,
}

impl JsonlManifestDeletionOutbox {
	/// Create the canonical direct-child outbox for one validated provider root.
	pub fn for_provider_root(root: impl AsRef<Path>) -> Self {
		Self::new(root.as_ref().join(MANIFEST_DELETION_OUTBOX_FILE))
	}

	/// Create an outbox at `path`; parent directories are created on first submission.
	///
	/// A raw path remains inert unless `ProviderService` validates it as the canonical direct
	/// child.
	pub fn new(path: impl AsRef<Path>) -> Self {
		Self {
			path: path.as_ref().to_path_buf(),
			write_lock: Mutex::new(()),
			qualified: Arc::new(AtomicBool::new(false)),
			#[cfg(test)]
			fail_next_directory_sync: std::sync::atomic::AtomicBool::new(false),
		}
	}
}

#[async_trait]
impl ManifestDeletionSubmitter for JsonlManifestDeletionOutbox {
	fn prepare_startup(&self, provider_root: &Path) -> Result<ManifestDeletionStartupPlan, String> {
		validate_canonical_outbox_path(provider_root, &self.path)?;
		prepare_jsonl_manifest_deletion_outbox(&self.path, Arc::clone(&self.qualified))
	}

	async fn submit_manifest_deletion(
		&self,
		request: ManifestDeletionSubmission,
	) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		if !self.qualified.load(Ordering::Acquire) {
			return Err("manifest deletion outbox startup has not been applied".into())
		}
		validate_optional_regular_file(&self.path, "manifest deletion outbox")?;
		let _process_lock = self.lock_outbox().await?;
		repair_incomplete_jsonl_tail(&self.path).await?;
		let existing =
			read_bounded_jsonl_tail(&self.path, MANIFEST_DELETION_DEDUPE_TAIL_BYTES).await?;
		for line in existing.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
			let Ok(ProviderSubmission::ManifestDeletion(previous)) =
				serde_json::from_slice::<ProviderSubmission>(line)
			else {
				continue;
			};
			if previous.manifest == request.manifest {
				return if previous == request {
					Ok(())
				} else {
					Err("manifest deletion outbox replay changed its payload".into())
				};
			}
		}
		let mut encoded = serde_json::to_vec(&ProviderSubmission::ManifestDeletion(request))
			.map_err(|error| error.to_string())?;
		self.append_locked(&mut encoded).await
	}
}

fn prepare_jsonl_manifest_deletion_outbox(
	path: &Path,
	qualified: Arc<AtomicBool>,
) -> Result<ManifestDeletionStartupPlan, String> {
	let provider_root = path
		.parent()
		.ok_or("manifest deletion outbox has no provider root")?
		.to_path_buf();
	let provider_root_guard = prepare_provider_root_guard(&provider_root)?;
	let lock_path = suffixed_path(path, ".lock");
	let lock = open_optional_guarded_file(&lock_path, false)?
		.map_or(PreparedFileGuard::Missing, |(_, guard)| guard);
	let (source, tail_start, expected_tail, truncate_to) = prepare_jsonl_source_guard(path)?;
	Ok(ManifestDeletionStartupPlan {
		jsonl: Some(PreparedJsonlStartup {
			provider_root,
			provider_root_guard,
			path: path.to_path_buf(),
			lock,
			source,
			tail_start,
			expected_tail,
			truncate_to,
			qualified,
		}),
	})
}

fn prepare_provider_root_guard(root: &Path) -> Result<PreparedProviderRootGuard, String> {
	match fs::symlink_metadata(root) {
		Ok(metadata) if metadata.is_dir() =>
			Ok(PreparedProviderRootGuard::Present(file_identity(&metadata)?)),
		Ok(_) => Err("manifest deletion outbox parent is not a provider directory".into()),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound =>
			Ok(PreparedProviderRootGuard::Missing),
		Err(error) => Err(error.to_string()),
	}
}

fn validate_provider_root_for_arm(
	root: &Path,
	expected: PreparedProviderRootGuard,
) -> Result<FileIdentity, String> {
	let metadata = fs::symlink_metadata(root)
		.map_err(|_| "manifest deletion outbox provider root is unavailable at startup arm")?;
	if !metadata.is_dir() || metadata.file_type().is_symlink() {
		return Err("manifest deletion outbox parent is not a provider directory".into())
	}
	let identity = file_identity(&metadata)?;
	if let PreparedProviderRootGuard::Present(prepared) = expected {
		if prepared != identity {
			return Err("manifest deletion outbox provider root changed after validation".into())
		}
	}
	Ok(identity)
}

fn validate_provider_root_identity(root: &Path, expected: FileIdentity) -> Result<(), String> {
	let metadata = fs::symlink_metadata(root).map_err(|error| error.to_string())?;
	if !metadata.is_dir() ||
		metadata.file_type().is_symlink() ||
		file_identity(&metadata)? != expected
	{
		return Err("manifest deletion outbox provider root changed during startup arm".into())
	}
	Ok(())
}

fn validate_canonical_outbox_path(provider_root: &Path, path: &Path) -> Result<(), String> {
	if path != provider_root.join(MANIFEST_DELETION_OUTBOX_FILE) ||
		path.parent() != Some(provider_root) ||
		path.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_DELETION_OUTBOX_FILE)
	{
		return Err("manifest deletion outbox must be the canonical provider-root child".into())
	}
	match fs::symlink_metadata(provider_root) {
		Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() =>
			Err("manifest deletion outbox parent is not a provider directory".into()),
		Ok(_) => Ok(()),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error.to_string()),
	}
}

fn prepare_jsonl_source_guard(
	path: &Path,
) -> Result<(PreparedFileGuard, u64, Vec<u8>, u64), String> {
	Ok(match open_optional_guarded_file(path, false)? {
		None => (PreparedFileGuard::Missing, 0, Vec::new(), 0),
		Some((mut file, guard)) => {
			let (tail_start, expected_tail, truncate_to) = jsonl_tail_view(&mut file, &guard)?;
			(guard, tail_start, expected_tail, truncate_to)
		},
	})
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

fn open_optional_guarded_file(
	path: &Path,
	write: bool,
) -> Result<Option<(fs::File, PreparedFileGuard)>, String> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if metadata.file_type().is_file() => {},
		Ok(_) => return Err("manifest deletion outbox path is not a regular file".into()),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => return Err(error.to_string()),
	}
	let file = open_existing_no_follow(path, write)?;
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	if !metadata.file_type().is_file() {
		return Err("manifest deletion outbox path is not a regular file".into())
	}
	let guard =
		PreparedFileGuard::Present { identity: file_identity(&metadata)?, length: metadata.len() };
	validate_path_guard(path, &guard)?;
	Ok(Some((file, guard)))
}

fn validate_prepared_jsonl_guard(
	path: &Path,
	guard: &PreparedFileGuard,
	tail_start: u64,
	expected_tail: &[u8],
) -> Result<(), String> {
	let PreparedFileGuard::Present { length, .. } = guard else {
		return match fs::symlink_metadata(path) {
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
			_ => Err("manifest deletion outbox changed after startup validation".into()),
		}
	};
	let mut file = open_existing_no_follow(path, false)?;
	validate_open_file(&file, guard)?;
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
	validate_path_guard(path, guard)
}

fn apply_prepared_jsonl_guard(
	path: &Path,
	guard: &PreparedFileGuard,
	tail_start: u64,
	expected_tail: &[u8],
	truncate_to: u64,
) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = guard else {
		return match fs::symlink_metadata(path) {
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
			_ => Err("manifest deletion outbox changed after startup validation".into()),
		}
	};
	let mut file = open_existing_no_follow(path, true)?;
	validate_open_file(&file, guard)?;
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
	validate_path_guard(path, guard)?;
	if truncate_to < *length {
		file.set_len(truncate_to).map_err(|error| error.to_string())?;
		file.sync_all().map_err(|error| error.to_string())?;
		sync_parent_directory_blocking(path)?;
	}
	validate_path_identity(path, *identity, truncate_to)
}

fn validate_optional_regular_file(path: &Path, label: &str) -> Result<(), String> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if metadata.file_type().is_file() => Ok(()),
		Ok(_) => Err(format!("{label} is not a regular file")),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error.to_string()),
	}
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Result<FileIdentity, String> {
	use std::os::unix::fs::MetadataExt;
	Ok(FileIdentity { device: metadata.dev(), inode: metadata.ino() })
}

#[cfg(windows)]
fn file_identity(metadata: &fs::Metadata) -> Result<FileIdentity, String> {
	use std::os::windows::fs::MetadataExt;
	Ok(FileIdentity {
		device: u64::from(
			metadata
				.volume_serial_number()
				.ok_or("manifest deletion outbox has no volume identity")?,
		),
		inode: metadata.file_index().ok_or("manifest deletion outbox has no file identity")?,
	})
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_metadata: &fs::Metadata) -> Result<FileIdentity, String> {
	Err("manifest deletion outbox file identity is unsupported on this platform".into())
}

fn validate_open_file(file: &fs::File, expected: &PreparedFileGuard) -> Result<(), String> {
	let PreparedFileGuard::Present { identity, length } = expected else {
		return Err("manifest deletion outbox expected a missing file".into())
	};
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	if !metadata.file_type().is_file() ||
		file_identity(&metadata)? != *identity ||
		metadata.len() != *length
	{
		return Err("manifest deletion outbox changed after startup validation".into())
	}
	Ok(())
}

fn validate_path_guard(path: &Path, expected: &PreparedFileGuard) -> Result<(), String> {
	let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
	let PreparedFileGuard::Present { identity, length } = expected else {
		return Err("manifest deletion outbox expected a missing file".into())
	};
	if !metadata.file_type().is_file() ||
		file_identity(&metadata)? != *identity ||
		metadata.len() != *length
	{
		return Err("manifest deletion outbox changed after startup validation".into())
	}
	Ok(())
}

fn validate_path_identity(
	path: &Path,
	expected_identity: FileIdentity,
	expected_length: u64,
) -> Result<(), String> {
	validate_path_guard(
		path,
		&PreparedFileGuard::Present { identity: expected_identity, length: expected_length },
	)
}

#[cfg(unix)]
fn open_existing_no_follow(path: &Path, write: bool) -> Result<fs::File, String> {
	let access = if write { OFlags::RDWR } else { OFlags::RDONLY };
	unix_fs::open(path, access | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty())
		.map(fs::File::from)
		.map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn open_existing_no_follow(path: &Path, write: bool) -> Result<fs::File, String> {
	let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
	if !before.file_type().is_file() {
		return Err("manifest deletion outbox path is not a regular file".into())
	}
	let file = fs::OpenOptions::new()
		.read(true)
		.write(write)
		.open(path)
		.map_err(|error| error.to_string())?;
	let identity = file_identity(&file.metadata().map_err(|error| error.to_string())?)?;
	if file_identity(&before)? != identity {
		return Err("manifest deletion outbox changed while opening".into())
	}
	validate_path_identity(path, identity, before.len())?;
	Ok(file)
}

struct AcquiredPreparedOutboxLock {
	_file: fs::File,
	path: PathBuf,
	created: Option<(FileIdentity, u64)>,
	rollback_on_drop: bool,
}

impl AcquiredPreparedOutboxLock {
	fn cleanup_created(&mut self) -> Result<(), String> {
		let Some((identity, length)) = self.created else {
			self.rollback_on_drop = false;
			return Ok(())
		};
		// Keep the owned handle locked through unlink and parent fsync; a pre-existing lock is
		// never represented by `created` and therefore can never reach this cleanup path.
		validate_path_identity(&self.path, identity, length)?;
		fs::remove_file(&self.path).map_err(|error| error.to_string())?;
		sync_parent_directory_blocking(&self.path)?;
		self.rollback_on_drop = false;
		Ok(())
	}

	fn rollback_created(mut self) -> Result<(), String> {
		self.cleanup_created()
	}

	fn preserve_created(&mut self) {
		self.rollback_on_drop = false;
	}
}

impl Drop for AcquiredPreparedOutboxLock {
	fn drop(&mut self) {
		if self.rollback_on_drop {
			let _ = self.cleanup_created();
		}
	}
}

fn acquire_prepared_outbox_lock(
	outbox: &Path,
	expected: &PreparedFileGuard,
) -> Result<AcquiredPreparedOutboxLock, String> {
	let path = suffixed_path(outbox, ".lock");
	let (file, created) = match expected {
		PreparedFileGuard::Missing => {
			let file = create_new_lock_no_follow(&path)?;
			let metadata = file.metadata().map_err(|error| error.to_string())?;
			(file, Some((file_identity(&metadata)?, metadata.len())))
		},
		PreparedFileGuard::Present { .. } => (open_existing_no_follow(&path, true)?, None),
	};
	if let Err(error) = FileExt::lock_exclusive(&file) {
		if let Some((identity, length)) = created {
			if validate_path_identity(&path, identity, length).is_ok() {
				drop(file);
				let _ = fs::remove_file(&path);
				let _ = sync_parent_directory_blocking(&path);
			}
		}
		return Err(error.to_string())
	}
	let validation = match expected {
		PreparedFileGuard::Missing =>
			file.metadata().map_err(|error| error.to_string()).and_then(|metadata| {
				file_identity(&metadata)
					.and_then(|identity| validate_path_identity(&path, identity, metadata.len()))
			}),
		PreparedFileGuard::Present { .. } =>
			validate_open_file(&file, expected).and_then(|()| validate_path_guard(&path, expected)),
	};
	let acquired = AcquiredPreparedOutboxLock {
		_file: file,
		path,
		created,
		rollback_on_drop: created.is_some(),
	};
	if let Err(error) = validation {
		return match acquired.rollback_created() {
			Ok(()) => Err(error),
			Err(cleanup) => Err(format!("{error}; created outbox lock cleanup failed: {cleanup}")),
		}
	}
	Ok(acquired)
}

#[cfg(unix)]
fn create_new_lock_no_follow(path: &Path) -> Result<fs::File, String> {
	unix_fs::open(
		path,
		OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::RUSR | Mode::WUSR,
	)
	.map(fs::File::from)
	.map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn create_new_lock_no_follow(path: &Path) -> Result<fs::File, String> {
	fs::OpenOptions::new()
		.create_new(true)
		.read(true)
		.write(true)
		.open(path)
		.map_err(|error| error.to_string())
}

#[cfg(unix)]
fn open_or_create_lock_no_follow(path: &Path) -> Result<fs::File, String> {
	let file = unix_fs::open(
		path,
		OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::RUSR | Mode::WUSR,
	)
	.map(fs::File::from)
	.map_err(|error| error.to_string())?;
	if !file.metadata().map_err(|error| error.to_string())?.file_type().is_file() {
		return Err("manifest deletion outbox lock is not a regular file".into())
	}
	Ok(file)
}

#[cfg(not(unix))]
fn open_or_create_lock_no_follow(path: &Path) -> Result<fs::File, String> {
	match open_optional_guarded_file(path, true)? {
		Some((file, _)) => Ok(file),
		None => create_new_lock_no_follow(path),
	}
}

#[cfg(unix)]
fn open_append_no_follow(path: &Path) -> Result<fs::File, String> {
	let file = unix_fs::open(
		path,
		OFlags::WRONLY | OFlags::APPEND | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::RUSR | Mode::WUSR,
	)
	.map(fs::File::from)
	.map_err(|error| error.to_string())?;
	let metadata = file.metadata().map_err(|error| error.to_string())?;
	if !metadata.file_type().is_file() {
		return Err("manifest deletion outbox is not a regular file".into())
	}
	validate_path_identity(path, file_identity(&metadata)?, metadata.len())?;
	Ok(file)
}

#[cfg(not(unix))]
fn open_append_no_follow(path: &Path) -> Result<fs::File, String> {
	match open_optional_guarded_file(path, true)? {
		Some((file, _)) => Ok(file),
		None => fs::OpenOptions::new()
			.create_new(true)
			.append(true)
			.open(path)
			.map_err(|error| error.to_string()),
	}
}

fn sync_parent_directory_blocking(path: &Path) -> Result<(), String> {
	let parent = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or(Path::new("."));
	fs::File::open(parent)
		.and_then(|directory| directory.sync_all())
		.map_err(|error| error.to_string())
}

async fn read_bounded_jsonl_tail(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
	let path = path.to_path_buf();
	let file = tokio::task::spawn_blocking(move || open_optional_guarded_file(&path, false))
		.await
		.map_err(|error| error.to_string())??;
	let mut file = match file {
		Some((file, _)) => tokio::fs::File::from_std(file),
		None => return Ok(Vec::new()),
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
	Ok(bytes)
}

impl JsonlManifestDeletionOutbox {
	async fn lock_outbox(&self) -> Result<std::fs::File, String> {
		if let Some(parent) = self.path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(|error| error.to_string())?;
		}
		let lock_path = suffixed_path(&self.path, ".lock");
		validate_optional_regular_file(&lock_path, "manifest deletion outbox lock")?;
		tokio::task::spawn_blocking(move || {
			let lock = open_or_create_lock_no_follow(&lock_path)?;
			FileExt::lock_exclusive(&lock).map_err(|error| error.to_string())?;
			let metadata = lock.metadata().map_err(|error| error.to_string())?;
			validate_path_identity(&lock_path, file_identity(&metadata)?, metadata.len())?;
			Ok(lock)
		})
		.await
		.map_err(|error| error.to_string())?
	}

	/// Append while the caller holds the shared process lock for the complete dedupe decision.
	async fn append_locked(&self, encoded: &mut Vec<u8>) -> Result<(), String> {
		if encoded.len() as u64 > MAX_JSONL_RECORD_BYTES {
			return Err("outbox record exceeds the bounded line limit".into());
		}
		encoded.push(b'\n');
		let path = self.path.clone();
		let file = tokio::task::spawn_blocking(move || open_append_no_follow(&path))
			.await
			.map_err(|error| error.to_string())??;
		let mut file = tokio::fs::File::from_std(file);
		file.write_all(&encoded).await.map_err(|error| error.to_string())?;
		file.sync_all().await.map_err(|error| error.to_string())?;
		#[cfg(test)]
		if self.fail_next_directory_sync.swap(false, std::sync::atomic::Ordering::SeqCst) {
			return Err("injected parent directory sync failure".into());
		}
		sync_parent_directory(&self.path).await
	}
}

fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
	let mut value = path.as_os_str().to_os_string();
	value.push(suffix);
	PathBuf::from(value)
}

async fn sync_parent_directory(path: &Path) -> Result<(), String> {
	let parent = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or(Path::new("."));
	let directory = tokio::fs::File::open(parent).await.map_err(|error| error.to_string())?;
	directory.sync_all().await.map_err(|error| error.to_string())
}

async fn repair_incomplete_jsonl_tail(path: &Path) -> Result<(), String> {
	let path = path.to_path_buf();
	tokio::task::spawn_blocking(move || {
		let (guard, tail_start, expected_tail, truncate_to) = prepare_jsonl_source_guard(&path)?;
		apply_prepared_jsonl_guard(&path, &guard, tail_start, &expected_tail, truncate_to)
	})
	.await
	.map_err(|error| error.to_string())?
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
			eprintln!("checkpoint-v2 duty intake failed");
		}
		if deletion.is_err() {
			eprintln!("manifest deletion duty processing failed");
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
		outbox.prepare_startup(root).unwrap().arm().unwrap().apply().unwrap();
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
		let plan = prepare_jsonl_manifest_deletion_outbox(&path, Arc::clone(&qualified)).unwrap();

		assert_eq!(fs::read(&path).unwrap(), original);
		let armed = plan.arm().unwrap();
		assert_eq!(fs::read(&path).unwrap(), original);
		armed.apply().unwrap();
		assert_eq!(fs::read(path).unwrap(), durable);
		assert!(qualified.load(Ordering::Acquire));
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
		assert!(prepare_jsonl_manifest_deletion_outbox(&link, Arc::new(AtomicBool::new(false)),)
			.is_err());
		assert_eq!(fs::read(target).unwrap(), b"durable\n");

		let directory = temp.path().join("nonregular");
		fs::create_dir(&directory).unwrap();
		assert!(prepare_jsonl_manifest_deletion_outbox(
			&directory,
			Arc::new(AtomicBool::new(false)),
		)
		.is_err());
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
	fn armed_jsonl_holds_the_consumer_lock_and_rolls_back_an_owned_missing_lock() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(temp.path());
		let lock_path = suffixed_path(&outbox.path, ".lock");
		let armed = outbox.prepare_startup(temp.path()).unwrap().arm().unwrap();
		let contender = fs::OpenOptions::new().read(true).write(true).open(&lock_path).unwrap();

		assert!(FileExt::try_lock_exclusive(&contender).is_err());
		armed.rollback().unwrap();
		assert!(!lock_path.exists());
		assert!(!outbox.qualified.load(Ordering::Acquire));
	}

	#[cfg(unix)]
	#[test]
	fn jsonl_arm_accepts_a_materialized_root_but_rejects_a_symlink_substitution() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("provider");
		let outbox = JsonlManifestDeletionOutbox::for_provider_root(&root);
		let plan = outbox.prepare_startup(&root).unwrap();
		fs::create_dir(&root).unwrap();
		plan.arm().unwrap().rollback().unwrap();

		fs::remove_dir(&root).unwrap();
		let target = temp.path().join("target");
		fs::create_dir(&target).unwrap();
		let plan = outbox.prepare_startup(&root).unwrap();
		symlink(&target, &root).unwrap();
		assert!(plan.arm().is_err());
		assert!(!target.join("provider-submissions-v3.jsonl.lock").exists());
	}

	#[cfg(unix)]
	#[test]
	fn jsonl_startup_accepts_only_the_fixed_direct_child_of_the_validated_root() {
		use std::os::unix::fs::symlink;

		let root = tempfile::tempdir().unwrap();
		let outside = tempfile::tempdir().unwrap();
		let safe = JsonlManifestDeletionOutbox::for_provider_root(root.path());
		assert!(safe.prepare_startup(root.path()).is_ok());

		for path in [
			outside.path().join(MANIFEST_DELETION_OUTBOX_FILE),
			root.path().join("nested").join(MANIFEST_DELETION_OUTBOX_FILE),
			root.path().join("other.jsonl"),
		] {
			assert!(JsonlManifestDeletionOutbox::new(path).prepare_startup(root.path()).is_err());
		}

		let alias = outside.path().join("provider-alias");
		symlink(root.path(), &alias).unwrap();
		assert!(JsonlManifestDeletionOutbox::new(alias.join(MANIFEST_DELETION_OUTBOX_FILE))
			.prepare_startup(root.path())
			.is_err());
		assert!(JsonlManifestDeletionOutbox::for_provider_root(&alias)
			.prepare_startup(&alias)
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
			let plan =
				prepare_jsonl_manifest_deletion_outbox(&path, Arc::clone(&qualified)).unwrap();
			if path.exists() {
				fs::remove_file(&path).unwrap();
			}
			fs::write(&path, replacement).unwrap();

			assert!(plan.arm().is_err(), "{name}");
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
			let plan =
				prepare_jsonl_manifest_deletion_outbox(&path, Arc::clone(&qualified)).unwrap();
			let swapped = if swap_lock { &lock } else { &path };
			let target = temp.path().join(if swap_lock { "lock-target" } else { "source-target" });
			let target_bytes = if swap_lock { b"".as_slice() } else { b"external-safe".as_slice() };
			fs::write(&target, target_bytes).unwrap();
			fs::remove_file(swapped).unwrap();
			symlink(&target, swapped).unwrap();

			assert!(plan.arm().is_err());
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

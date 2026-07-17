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

//! Metadata-first bounded reads for durable provider control-plane records.

use std::{
	ffi::OsString,
	fs::{self, File},
	io::{Read, Write},
	os::unix::ffi::OsStringExt,
	path::{Component, Path, PathBuf},
};

use fs4::FileExt;
use rand::{rngs::OsRng, RngCore};
use rustix::{
	fs::{self as unix_fs, AtFlags, FileType, Mode, OFlags, RenameFlags},
	io::Errno as UnixErrno,
};

use crate::ContentError;

/// Stable identity of an opened durable filesystem object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
	device: u64,
	inode: u64,
}

/// Bytes and path identity captured by one bounded, no-follow regular-file read.
pub(crate) struct RegularFileSnapshot {
	pub(crate) bytes: Vec<u8>,
	pub(crate) identity: FileIdentity,
	pub(crate) length: u64,
}

pub(crate) struct DirectoryEntry {
	pub(crate) name: OsString,
	pub(crate) file_type: FileType,
	pub(crate) identity: FileIdentity,
	pub(crate) length: u64,
}

/// Exact regular-file identity retained between provider startup prepare and apply.
pub(crate) struct PreparedRegularFile {
	pub(crate) name: OsString,
	identity: FileIdentity,
	length: u64,
}

/// A no-follow parent-directory capability for one provider-root entry.
pub(crate) struct PreparedDirectoryPath {
	parent: File,
	name: OsString,
}

/// An exclusively locked provider-root capability whose pathname is identity-bound.
pub(crate) struct LockedDirectory {
	parent: File,
	name: OsString,
	directory: File,
	identity: FileIdentity,
	rollback_owned_empty_on_drop: bool,
}

impl PreparedDirectoryPath {
	/// Open and exclusively lock the existing final directory without following any component.
	pub(crate) fn lock_existing(self) -> Result<LockedDirectory, ContentError> {
		let directory = open_directory_at(&self.parent, &self.name)?;
		let identity = file_identity(&directory.metadata().map_err(io_error)?);
		let locked = LockedDirectory {
			parent: self.parent,
			name: self.name,
			directory,
			identity,
			rollback_owned_empty_on_drop: false,
		};
		locked.lock_and_validate()?;
		Ok(locked)
	}

	/// Create, identity-bind and exclusively lock a previously missing final directory.
	pub(crate) fn create_and_lock(self) -> Result<LockedDirectory, ContentError> {
		match unix_fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW) {
			Err(UnixErrno::NOENT) => {},
			_ => return Err(ContentError::IntegrityFailed),
		}
		let temporary = create_private_directory_name(&self.parent)?;
		let created = unix_fs::statat(&self.parent, &temporary, AtFlags::SYMLINK_NOFOLLOW)
			.map_err(io_error)?;
		let identity = file_identity_from_stat(&created);
		let directory = match open_directory_at(&self.parent, temporary.as_ref()) {
			Ok(directory) => directory,
			Err(error) => {
				rollback_created_directory(&self.parent, &temporary, identity);
				return Err(error)
			},
		};
		if file_identity(&directory.metadata().map_err(io_error)?) != identity {
			rollback_created_directory(&self.parent, &temporary, identity);
			return Err(ContentError::IntegrityFailed)
		}
		let mut locked = LockedDirectory {
			parent: self.parent,
			name: temporary,
			directory,
			identity,
			rollback_owned_empty_on_drop: true,
		};
		locked.lock_and_validate()?;
		unix_fs::renameat_with(
			&locked.parent,
			&locked.name,
			&locked.parent,
			&self.name,
			RenameFlags::NOREPLACE,
		)
		.map_err(io_error)?;
		locked.name = self.name;
		locked.validate_path_identity()?;
		unix_fs::fsync(&locked.parent).map_err(io_error)?;
		Ok(locked)
	}

	pub(crate) fn is_missing(&self) -> Result<bool, ContentError> {
		match unix_fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW) {
			Err(UnixErrno::NOENT) => Ok(true),
			Ok(_) => Ok(false),
			Err(error) => Err(io_error(error)),
		}
	}
}

impl LockedDirectory {
	fn lock_and_validate(&self) -> Result<(), ContentError> {
		self.directory.try_lock_exclusive().map_err(io_error)?;
		self.validate_path_identity()
	}

	pub(crate) fn file(&self) -> &File {
		&self.directory
	}

	pub(crate) fn validate_path_identity(&self) -> Result<(), ContentError> {
		let stat = unix_fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW)
			.map_err(io_error)?;
		if FileType::from_raw_mode(stat.st_mode) != FileType::Directory ||
			file_identity_from_stat(&stat) != self.identity ||
			file_identity(&self.directory.metadata().map_err(io_error)?) != self.identity
		{
			return Err(ContentError::IntegrityFailed)
		}
		Ok(())
	}

	pub(crate) fn preserve_owned(&mut self) {
		self.rollback_owned_empty_on_drop = false;
	}
}

impl Drop for LockedDirectory {
	fn drop(&mut self) {
		if self.rollback_owned_empty_on_drop {
			rollback_created_directory(&self.parent, &self.name, self.identity);
		}
	}
}

/// Resolve every existing ancestor without following symlinks and retain the parent capability.
pub(crate) fn prepare_directory_path(path: &Path) -> Result<PreparedDirectoryPath, ContentError> {
	let name = path.file_name().ok_or(ContentError::IntegrityFailed)?.to_os_string();
	let parent_path = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or_else(|| Path::new("."));
	let mut directory = if parent_path.is_absolute() {
		open_directory_path(Path::new("/"))?
	} else {
		open_directory_path(Path::new("."))?
	};
	for component in parent_path.components() {
		match component {
			Component::RootDir | Component::CurDir => {},
			Component::Normal(name) => directory = open_directory_at(&directory, name)?,
			Component::ParentDir | Component::Prefix(_) =>
				return Err(ContentError::IntegrityFailed),
		}
	}
	Ok(PreparedDirectoryPath { parent: directory, name })
}

/// Read one regular durable record without allocating beyond its declared hard limit.
pub(crate) fn read_regular_file(
	path: impl AsRef<Path>,
	max_bytes: u64,
) -> Result<Vec<u8>, ContentError> {
	Ok(read_regular_file_snapshot(path, max_bytes)?.bytes)
}

/// Capture a bounded regular file together with the exact opened object identity.
pub(crate) fn read_regular_file_snapshot(
	path: impl AsRef<Path>,
	max_bytes: u64,
) -> Result<RegularFileSnapshot, ContentError> {
	let path = path.as_ref();
	let path_metadata = fs::symlink_metadata(path).map_err(io_error)?;
	if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
		return Err(ContentError::IntegrityFailed)
	}
	let file = open_regular_file_nofollow(path)?;
	let metadata = file.metadata().map_err(io_error)?;
	if !metadata.is_file()
		|| file_identity(&path_metadata) != file_identity(&metadata)
		|| metadata.len() > max_bytes
	{
		return Err(ContentError::IntegrityFailed);
	}
	let capacity = usize::try_from(metadata.len()).map_err(|_| ContentError::IntegrityFailed)?;
	let mut bytes = Vec::with_capacity(capacity);
	let mut bounded = file.take(max_bytes.checked_add(1).ok_or(ContentError::IntegrityFailed)?);
	bounded.read_to_end(&mut bytes).map_err(io_error)?;
	if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > max_bytes {
		return Err(ContentError::IntegrityFailed);
	}
	let current = fs::symlink_metadata(path).map_err(io_error)?;
	if current.file_type().is_symlink()
		|| !current.is_file()
		|| file_identity(&current) != file_identity(&metadata)
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(RegularFileSnapshot {
		bytes,
		identity: file_identity(&metadata),
		length: metadata.len(),
	})
}

/// Capture a bounded regular file relative to an already validated directory capability.
pub(crate) fn read_regular_file_snapshot_at(
	directory: &File,
	name: &std::ffi::OsStr,
	max_bytes: u64,
) -> Result<RegularFileSnapshot, ContentError> {
	let fd = unix_fs::openat(
		directory,
		name,
		OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	let file = File::from(fd);
	let metadata = file.metadata().map_err(io_error)?;
	if !metadata.is_file() || metadata.len() > max_bytes {
		return Err(ContentError::IntegrityFailed)
	}
	let capacity = usize::try_from(metadata.len()).map_err(|_| ContentError::IntegrityFailed)?;
	let mut bytes = Vec::with_capacity(capacity);
	let mut bounded = file.take(max_bytes.checked_add(1).ok_or(ContentError::IntegrityFailed)?);
	bounded.read_to_end(&mut bytes).map_err(io_error)?;
	if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > max_bytes {
		return Err(ContentError::IntegrityFailed)
	}
	let stat = unix_fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile ||
		file_identity_from_stat(&stat) != file_identity(&metadata)
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(RegularFileSnapshot { bytes, identity: file_identity(&metadata), length: metadata.len() })
}

pub(crate) fn entry_missing_at(
	directory: &File,
	name: &std::ffi::OsStr,
) -> Result<bool, ContentError> {
	match unix_fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
		Err(UnixErrno::NOENT) => Ok(true),
		Ok(_) => Ok(false),
		Err(error) => Err(io_error(error)),
	}
}

pub(crate) fn regular_file_exists_at(
	directory: &File,
	name: &std::ffi::OsStr,
) -> Result<bool, ContentError> {
	match unix_fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
		Err(UnixErrno::NOENT) => Ok(false),
		Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => Ok(true),
		Ok(_) => Err(ContentError::IntegrityFailed),
		Err(error) => Err(io_error(error)),
	}
}

pub(crate) fn list_directory(directory: &File) -> Result<Vec<DirectoryEntry>, ContentError> {
	let mut entries = Vec::new();
	let mut stream = rustix::fs::Dir::read_from(directory).map_err(io_error)?;
	for entry in &mut stream {
		let entry = entry.map_err(io_error)?;
		let bytes = entry.file_name().to_bytes();
		if bytes == b"." || bytes == b".." {
			continue
		}
		let stat = unix_fs::statat(directory, entry.file_name(), AtFlags::SYMLINK_NOFOLLOW)
			.map_err(io_error)?;
		entries.push(DirectoryEntry {
			name: OsString::from_vec(bytes.to_vec()),
			file_type: FileType::from_raw_mode(stat.st_mode),
			identity: file_identity_from_stat(&stat),
			length: stat.st_size as u64,
		});
	}
	Ok(entries)
}

impl DirectoryEntry {
	pub(crate) fn into_regular_guard(self) -> Result<PreparedRegularFile, ContentError> {
		if self.file_type != FileType::RegularFile {
			return Err(ContentError::IntegrityFailed)
		}
		Ok(PreparedRegularFile {
			name: self.name,
			identity: self.identity,
			length: self.length,
		})
	}
}

pub(crate) fn open_prepared_regular_file_at(
	directory: &File,
	guard: &PreparedRegularFile,
) -> Result<File, ContentError> {
	let file = open_regular_file_at(directory, &guard.name)?;
	let metadata = file.metadata().map_err(io_error)?;
	if !metadata.is_file() ||
		metadata.len() != guard.length ||
		file_identity(&metadata) != guard.identity
	{
		return Err(ContentError::IntegrityFailed)
	}
	let stat = unix_fs::statat(directory, &guard.name, AtFlags::SYMLINK_NOFOLLOW)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile ||
		stat.st_size as u64 != guard.length ||
		file_identity_from_stat(&stat) != guard.identity
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(file)
}

pub(crate) fn open_regular_file_at(
	directory: &File,
	name: &std::ffi::OsStr,
) -> Result<File, ContentError> {
	unix_fs::openat(
		directory,
		name,
		OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)
	.map(File::from)
	.map_err(|_| ContentError::IntegrityFailed)
}

pub(crate) fn open_directory_at(
	directory: &File,
	name: &std::ffi::OsStr,
) -> Result<File, ContentError> {
	unix_fs::openat(
		directory,
		name,
		OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)
	.map(File::from)
	.map_err(|_| ContentError::IntegrityFailed)
}

pub(crate) fn create_directory_at(
	parent: &File,
	name: &std::ffi::OsStr,
) -> Result<File, ContentError> {
	if !entry_missing_at(parent, name)? {
		return Err(ContentError::IntegrityFailed)
	}
	let temporary = create_private_directory_name(parent)?;
	let created = unix_fs::statat(parent, &temporary, AtFlags::SYMLINK_NOFOLLOW)
		.map_err(io_error)?;
	let identity = file_identity_from_stat(&created);
	let directory = match open_directory_at(parent, temporary.as_ref()) {
		Ok(directory) => directory,
		Err(error) => {
			rollback_created_directory(parent, &temporary, identity);
			return Err(error)
		},
	};
	if file_identity(&directory.metadata().map_err(io_error)?) != identity {
		rollback_created_directory(parent, &temporary, identity);
		return Err(ContentError::IntegrityFailed)
	}
	if let Err(error) = unix_fs::renameat_with(
		parent,
		&temporary,
		parent,
		name,
		RenameFlags::NOREPLACE,
	) {
		rollback_created_directory(parent, &temporary, identity);
		return Err(io_error(error))
	}
	let stat = unix_fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io_error)?;
	if file_identity_from_stat(&stat) != identity {
		return Err(ContentError::IntegrityFailed)
	}
	unix_fs::fsync(parent).map_err(io_error)?;
	Ok(directory)
}

fn open_directory_path(path: &Path) -> Result<File, ContentError> {
	unix_fs::open(
		path,
		OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)
	.map(File::from)
	.map_err(|_| ContentError::IntegrityFailed)
}

fn file_identity_from_stat(stat: &rustix::fs::Stat) -> FileIdentity {
	FileIdentity { device: stat.st_dev as u64, inode: stat.st_ino as u64 }
}

fn create_private_directory_name(parent: &File) -> Result<OsString, ContentError> {
	for _ in 0..16 {
		let mut random = [0u8; 16];
		OsRng.fill_bytes(&mut random);
		let name = OsString::from(format!(".provider-root.create-{}", hex::encode(random)));
		match unix_fs::mkdirat(parent, &name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
			Ok(()) => return Ok(name),
			Err(UnixErrno::EXIST) => {},
			Err(error) => return Err(io_error(error)),
		}
	}
	Err(ContentError::IntegrityFailed)
}

fn rollback_created_directory(parent: &File, name: &std::ffi::OsStr, identity: FileIdentity) {
	let Ok(stat) = unix_fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) else { return };
	if FileType::from_raw_mode(stat.st_mode) != FileType::Directory ||
		file_identity_from_stat(&stat) != identity
	{
		return
	}
	if unix_fs::unlinkat(parent, name, AtFlags::REMOVEDIR).is_ok() {
		let _ = unix_fs::fsync(parent);
	}
}

/// Remove prepared artifacts relative to a held directory capability.
pub(crate) fn remove_validated_temp_artifacts_at(
	directory: &File,
	temp_artifacts: &[PreparedRegularFile],
) -> Result<(), ContentError> {
	for artifact in temp_artifacts {
		open_prepared_regular_file_at(directory, artifact)?;
	}
	for artifact in temp_artifacts {
		open_prepared_regular_file_at(directory, artifact)?;
		match unix_fs::unlinkat(directory, &artifact.name, AtFlags::empty()) {
			Ok(()) | Err(UnixErrno::NOENT) => {},
			Err(error) => return Err(io_error(error)),
		}
	}
	if !temp_artifacts.is_empty() {
		unix_fs::fsync(directory).map_err(io_error)?;
	}
	Ok(())
}

pub(crate) fn validate_prepared_regular_files_at(
	directory: &File,
	files: &[PreparedRegularFile],
) -> Result<(), ContentError> {
	for file in files {
		open_prepared_regular_file_at(directory, file)?;
	}
	Ok(())
}

/// Atomically replace a regular file relative to one held directory capability.
pub(crate) fn write_atomic_at(
	directory: &File,
	name: &std::ffi::OsStr,
	temporary: &std::ffi::OsStr,
	bytes: &[u8],
) -> Result<(), ContentError> {
	let result = (|| {
		let fd = unix_fs::openat(
			directory,
			temporary,
			OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
			Mode::RUSR | Mode::WUSR,
		)
		.map_err(io_error)?;
		let mut file = File::from(fd);
		file.write_all(bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		unix_fs::renameat(directory, temporary, directory, name).map_err(io_error)?;
		unix_fs::fsync(directory).map_err(io_error)
	})();
	if result.is_err() {
		let _ = unix_fs::unlinkat(directory, temporary, AtFlags::empty());
		let _ = unix_fs::fsync(directory);
	}
	result
}

fn open_regular_file_nofollow(path: &Path) -> Result<File, ContentError> {
	unix_fs::open(
		path,
		OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)
	.map(File::from)
	.map_err(|_| ContentError::IntegrityFailed)
}

pub(crate) fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
	use std::os::unix::fs::MetadataExt as _;

	FileIdentity { device: metadata.dev(), inode: metadata.ino() }
}

/// Recognize only the crash artifact shape emitted by the durable JSON writers.
pub(crate) fn is_json_temp_artifact(name: &str) -> bool {
	let Some((record, process_id)) = name.rsplit_once(".tmp-") else {
		return false
	};
	!record.is_empty() &&
		record.ends_with(".json") &&
		!process_id.is_empty() &&
		process_id.bytes().all(|byte| byte.is_ascii_digit())
}

/// Remove crash artifacts only after the caller has validated its complete recovery view.
pub(crate) fn remove_validated_temp_artifacts(
	root: &Path,
	temp_artifacts: &[PathBuf],
) -> Result<(), ContentError> {
	if temp_artifacts.is_empty() {
		return Ok(())
	}
	for artifact in temp_artifacts {
		if artifact.parent() != Some(root) {
			return Err(ContentError::IntegrityFailed)
		}
	}
	for artifact in temp_artifacts {
		match fs::remove_file(artifact) {
			Ok(()) => {},
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
			Err(error) => return Err(io_error(error)),
		}
	}
	File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)
}

/// Inspect one optional durable directory without creating it.
pub(crate) fn optional_directory_exists(path: &Path) -> Result<bool, ContentError> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() =>
			Err(ContentError::IntegrityFailed),
		Ok(_) => Ok(true),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(io_error(error)),
	}
}

/// Materialize a directory which was absent from a completely validated startup view.
pub(crate) fn create_prepared_directory(path: &Path, missing: bool) -> Result<(), ContentError> {
	if !missing {
		return Ok(())
	}
	fs::create_dir(path).map_err(io_error)?;
	let parent = path.parent().ok_or(ContentError::IntegrityFailed)?;
	File::open(parent).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sparse_oversized_record_is_rejected_before_reading() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("oversized.json");
		let file = File::create(&path).unwrap();
		file.set_len(4097).unwrap();
		assert_eq!(read_regular_file(path, 4096).unwrap_err(), ContentError::IntegrityFailed);
	}

	#[test]
	fn normal_record_reopens_exactly() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("record.json");
		std::fs::write(&path, b"durable").unwrap();
		assert_eq!(read_regular_file(path, 8).unwrap(), b"durable");
	}

	#[test]
	fn bare_relative_provider_root_uses_the_current_directory_as_parent() {
		let prepared = prepare_directory_path(Path::new("data")).unwrap();
		assert!(prepared.is_missing().is_ok());
	}

	#[cfg(unix)]
	#[test]
	fn optional_directory_rejects_live_and_dangling_symlinks() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let live_target = temp.path().join("live-target");
		fs::create_dir(&live_target).unwrap();
		let live = temp.path().join("live");
		symlink(&live_target, &live).unwrap();
		let dangling = temp.path().join("dangling");
		symlink(temp.path().join("missing-target"), &dangling).unwrap();

		assert_eq!(optional_directory_exists(&live), Err(ContentError::IntegrityFailed));
		assert_eq!(optional_directory_exists(&dangling), Err(ContentError::IntegrityFailed));
	}

	#[cfg(unix)]
	#[test]
	fn regular_file_read_rejects_live_and_dangling_symlinks_without_external_reads() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let external = temp.path().join("external.json");
		let external_bytes = b"external-durable-record";
		fs::write(&external, external_bytes).unwrap();
		let live = temp.path().join("live.json");
		symlink(&external, &live).unwrap();
		let missing = temp.path().join("missing.json");
		let dangling = temp.path().join("dangling.json");
		symlink(&missing, &dangling).unwrap();

		assert_eq!(read_regular_file(&live, 1024), Err(ContentError::IntegrityFailed));
		assert!(read_regular_file(&dangling, 1024).is_err());
		assert_eq!(fs::read(external).unwrap(), external_bytes);
		assert!(!missing.exists());
	}
}

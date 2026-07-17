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
	fs::{self, File},
	io::Read,
	path::{Path, PathBuf},
};

use crate::ContentError;

/// Read one regular durable record without allocating beyond its declared hard limit.
pub(crate) fn read_regular_file(
	path: impl AsRef<Path>,
	max_bytes: u64,
) -> Result<Vec<u8>, ContentError> {
	let file = File::open(path).map_err(io_error)?;
	let metadata = file.metadata().map_err(io_error)?;
	if !metadata.is_file() || metadata.len() > max_bytes {
		return Err(ContentError::IntegrityFailed);
	}
	let capacity = usize::try_from(metadata.len()).map_err(|_| ContentError::IntegrityFailed)?;
	let mut bytes = Vec::with_capacity(capacity);
	let mut bounded = file.take(max_bytes.checked_add(1).ok_or(ContentError::IntegrityFailed)?);
	bounded.read_to_end(&mut bytes).map_err(io_error)?;
	if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > max_bytes {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(bytes)
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
	let exists = path.try_exists().map_err(io_error)?;
	if exists && !fs::metadata(path).map_err(io_error)?.is_dir() {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(exists)
}

/// Materialize a directory which was absent from a completely validated startup view.
pub(crate) fn create_prepared_directory(path: &Path, missing: bool) -> Result<(), ContentError> {
	if !missing {
		return Ok(())
	}
	fs::create_dir_all(path).map_err(io_error)?;
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
}

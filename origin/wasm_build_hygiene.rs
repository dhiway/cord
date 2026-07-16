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

use std::{
	env,
	ffi::OsString,
	path::{Path, PathBuf},
	process::Command,
};

fn canonical(path: PathBuf, base: &Path, name: &str) -> PathBuf {
	let path = if path.is_absolute() { path } else { base.join(path) };
	path.canonicalize()
		.unwrap_or_else(|error| panic!("failed to resolve {name} {}: {error}", path.display()))
}

fn remap(from: &Path, to: &str) -> String {
	let from = from.to_string_lossy();
	assert!(
		!from.chars().any(char::is_whitespace),
		"sanitized Wasm build path contains whitespace: {from}"
	);
	format!("--remap-path-prefix={from}={to}")
}

/// Select the workspace lock and sanitize host paths in local Wasm builds.
///
/// Production reproducibility authority belongs to the pinned srtool release entrypoint.
pub fn configure() {
	let manifest_dir =
		PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
	let workspace_root = manifest_dir
		.ancestors()
		.find(|path| path.join("HEADER-GPL3").is_file())
		.expect("runtime must be built from the CORD workspace")
		.to_path_buf();
	let workspace_root = canonical(workspace_root, &manifest_dir, "workspace root");

	let home = env::var_os("HOME").map(PathBuf::from);
	let cargo_home = env::var_os("CARGO_HOME")
		.map(PathBuf::from)
		.or_else(|| home.as_ref().map(|path| path.join(".cargo")))
		.expect("CARGO_HOME or HOME is required for a sanitized runtime build");
	let cargo_home = canonical(cargo_home, &workspace_root, "Cargo home");

	let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
	let output = Command::new(&rustc)
		.args(["--print", "sysroot"])
		.output()
		.unwrap_or_else(|error| panic!("failed to query rustc sysroot: {error}"));
	assert!(output.status.success(), "rustc --print sysroot failed");
	let sysroot = String::from_utf8(output.stdout).expect("rustc sysroot is UTF-8");
	let sysroot = canonical(PathBuf::from(sysroot.trim()), &workspace_root, "rustc sysroot");

	let rustflags = [
		remap(&workspace_root, "/cord"),
		remap(&cargo_home, "/cargo"),
		remap(&sysroot, "/rust-toolchain"),
	]
	.join(" ");

	// Keep wasm-builder on the canonical workspace/Cargo.lock, pass the path contract to its
	// nested Cargo invocation, and prevent encoded caller flags from taking precedence.
	env::set_var("WASM_BUILD_WORKSPACE_HINT", &workspace_root);
	env::set_var("WASM_BUILD_RUSTFLAGS", rustflags);
	env::remove_var("CARGO_ENCODED_RUSTFLAGS");
	println!("cargo:rerun-if-env-changed=CARGO_HOME");
	println!("cargo:rerun-if-env-changed=HOME");
	println!("cargo:rerun-if-env-changed=RUSTC");
}

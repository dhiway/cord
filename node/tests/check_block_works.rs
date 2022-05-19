// This file is part of Cord – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// Cord is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cord is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cord. If not, see <https://www.gnu.org/licenses/>.

#![cfg(unix)]

use assert_cmd::cargo::cargo_bin;
use std::process::Command;
use tempfile::tempdir;

pub mod common;

#[tokio::test]
async fn check_block_works() {
	let base_path = tempdir().expect("could not create a temp dir");

	common::run_node_for_a_while(base_path.path(), &["--dev"]).await;

	let status = Command::new(cargo_bin("cord"))
		.args(&["check-block", "--dev", "-d"])
		.arg(base_path.path())
		.arg("1")
		.status()
		.unwrap();
	assert!(status.success());
}

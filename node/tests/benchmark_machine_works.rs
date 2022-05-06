// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use assert_cmd::cargo::cargo_bin;
use std::process::Command;

/// Tests that the `benchmark machine` command works for the substrate dev runtime.
#[test]
fn benchmark_machine_works() {
	let status = Command::new(cargo_bin("cord"))
		.args(["benchmark", "machine", "--dev"])
		.args(["--verify-duration", "0.1"])
		.status()
		.unwrap();

	assert!(status.success());
}

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
#![cfg(feature = "runtime-benchmarks")]

use assert_cmd::cargo::cargo_bin;
use std::process::Command;

// use std::process::Command;
static RUNTIMES: &[&str] = &["base", "plus"];

/// `benchmark pallet` works for the different combinations of `steps` and
/// `repeat`.
#[test]
fn benchmark_pallet_works() {
	for runtime in RUNTIMES {
		let runtime = format!("dev-braid-{}", runtime);

		// Some invalid combinations:
		benchmark_pallet(&runtime, 0, 10, false);
		benchmark_pallet(&runtime, 1, 10, false);
		// ... and some valid:
		benchmark_pallet(&runtime, 2, 1, true);
		benchmark_pallet(&runtime, 50, 20, true);
		benchmark_pallet(&runtime, 20, 50, true);
	}
}

fn benchmark_pallet(runtime: &str, steps: u32, repeat: u32, should_work: bool) {
	let status = Command::new(cargo_bin("cord"))
		.args(["benchmark", "pallet", "--chain", runtime])
		// Use the `addition` benchmark since is the fastest.
		.args(["--pallet", "frame-benchmarking", "--extrinsic", "addition"])
		.args(["--steps", &format!("{}", steps), "--repeat", &format!("{}", repeat)])
		.args([
			"--wasm-execution=compiled",
			"--no-storage-info",
			"--no-median-slopes",
			"--no-min-squares",
			"--heap-pages=4096",
		])
		.status()
		.unwrap();

	assert_eq!(status.success(), should_work);
}

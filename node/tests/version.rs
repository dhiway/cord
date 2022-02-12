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
use platforms::*;
use regex::Regex;
use std::process::Command;

fn expected_regex() -> Regex {
	Regex::new(r"^cord (\d+\.\d+\.\d+(?:-.+?)?)-([a-f\d]+|unknown)-(.+?)-(.+?)(?:-(.+))?$").unwrap()
}

#[test]
fn version_is_full() {
	let expected = expected_regex();
	let output = Command::new(cargo_bin("cord")).args(&["--version"]).output().unwrap();

	assert!(output.status.success(), "command returned with non-success exit code");

	let output = String::from_utf8_lossy(&output.stdout).trim().to_owned();
	let captures = expected.captures(output.as_str()).expect("could not parse version in output");

	assert_eq!(&captures[1], env!("CARGO_PKG_VERSION"));
	assert_eq!(&captures[3], TARGET_ARCH.as_str());
	assert_eq!(&captures[4], TARGET_OS.as_str());
	assert_eq!(captures.get(5).map(|x| x.as_str()), TARGET_ENV.map(|x| x.as_str()));
}

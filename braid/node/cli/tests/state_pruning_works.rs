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
use tempfile::tempdir;
mod common;

// use cord_braid_cli_test_utils as common;

#[tokio::test]
#[cfg(unix)]
async fn remember_state_pruning_works() {
	let base_path = tempdir().expect("could not create a temp dir");

	// First run with `--state-pruning=archive`.
	common::run_node_for_a_while(
		base_path.path(),
		&["--dev", "--state-pruning=archive", "--no-hardware-benchmarks"],
	)
	.await;

	// Then run again without specifying the state pruning.
	// This should load state pruning settings from the db.
	common::run_node_for_a_while(base_path.path(), &["--dev", "--no-hardware-benchmarks"]).await;
}

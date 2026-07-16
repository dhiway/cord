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

use std::{collections::BTreeSet, fs, path::PathBuf};

use origin_orbis_provider::run_three_provider_recovery_evidence;
use tempfile::TempDir;

const ARTIFACT_NAME: &str = "ac5-three-provider-recovery-v1.json";

#[tokio::test]
async fn deterministic_failover() {
	let artifact = artifact_path();
	if artifact.exists() {
		fs::remove_file(&artifact).expect("remove stale AC5 evidence artifact");
	}
	let providers = TempDir::new().expect("create isolated provider roots");
	let evidence = run_three_provider_recovery_evidence(providers.path())
		.await
		.expect("complete deterministic three-provider recovery");

	assert_eq!(evidence.provider_count, 3);
	assert_eq!(evidence.provider_roots.len(), evidence.provider_count);
	assert_eq!(evidence.provider_roots.iter().collect::<BTreeSet<_>>().len(), 1);
	let selected = evidence
		.eligible_sources
		.iter()
		.min_by_key(|source| source.order)
		.expect("at least one eligible failover source");
	assert!(evidence.selection_results.iter().all(|provider| provider == &selected.provider));
	assert_eq!(evidence.corrupt_read_observations.len(), 1);
	assert!(evidence.corrupt_read_observations.iter().all(|observation| {
		observation.result == "rejected_integrity_failed" && observation.bytes_returned == 0
	}));
	assert_eq!(
		evidence.convergence_observed_block - evidence.failure_detected_block,
		evidence.recovery_steps
	);
	assert!(evidence.recovery_steps <= 200);
	assert!(evidence.recovery_network_requests > 0);

	let encoded = serde_json::to_vec_pretty(&evidence).expect("encode raw AC5 observations");
	fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("create target/debug");
	let temporary = artifact.with_extension("json.tmp");
	fs::write(&temporary, encoded).expect("write temporary AC5 evidence artifact");
	fs::rename(&temporary, &artifact).expect("publish AC5 evidence artifact atomically");
}

fn artifact_path() -> PathBuf {
	let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|path| path.parent())
		.and_then(|path| path.parent())
		.expect("provider crate is nested under the workspace")
		.to_path_buf();
	workspace.join("target").join("debug").join(ARTIFACT_NAME)
}

// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

use substrate_build_script_utils::{generate_cargo_keys, rerun_if_git_head_changed};

fn main() {
	generate_cargo_keys();

	rerun_if_git_head_changed();
}

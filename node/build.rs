/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

use substrate_build_script_utils::{generate_cargo_keys, rerun_if_git_head_changed};

fn main() {
	generate_cargo_keys();

	rerun_if_git_head_changed();
}

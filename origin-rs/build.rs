use std::{env, path::PathBuf};

fn main() {
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let required = ["metadata/origin.scale", "metadata/origin-hub.scale"];
	for rel in required {
		let path = manifest_dir.join(rel);
		if !path.exists() {
			panic!(
				"Missing metadata file: {}. Run `subxt metadata --url <node-url> --output {}` first.",
				path.display(),
				path.display()
			);
		}
		println!("cargo:rerun-if-changed={}", path.display());
	}
}

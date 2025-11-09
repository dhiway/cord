use std::{env, path::PathBuf};

fn main() {
	let metadata_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("metadata/cord.scale");
	if !metadata_path.exists() {
		panic!(
			"Missing metadata file: {}. Run `subxt metadata --url <node-url> --output {}` first.",
			metadata_path.display(),
			metadata_path.display()
		);
	}
	println!("cargo:rerun-if-changed={}", metadata_path.display());
}

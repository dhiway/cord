use assert_cmd::cargo::cargo_bin;
use std::process::Command;
use tempfile::tempdir;

/// Tests that the `benchmark overhead` command works for the substrate dev runtime.
#[test]
fn benchmark_overhead_works() {
	let tmp_dir = tempdir().expect("could not create a temp dir");
	let base_path = tmp_dir.path();

	// Only put 10 extrinsics into the block otherwise it takes forever to build it
	// especially for a non-release build.
	let status = Command::new(cargo_bin("cord"))
		.args(&["benchmark", "overhead", "--dev", "-d"])
		.arg(base_path)
		.arg("--weight-path")
		.arg(base_path)
		.args(["--warmup", "10", "--repeat", "10"])
		.args(["--add", "100", "--mul", "1.2", "--metric", "p75"])
		.args(["--max-ext-per-block", "10"])
		.status()
		.unwrap();
	assert!(status.success());

	// Weight files have been created.
	assert!(base_path.join("block_weights.rs").exists());
	assert!(base_path.join("extrinsic_weights.rs").exists());
}

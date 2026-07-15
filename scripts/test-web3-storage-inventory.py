#!/usr/bin/env python3
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Hostile fixtures for deterministic Web3 Storage census and AC1 validation."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

from web3_storage_semantic import rust_surfaces, typescript_surfaces


ROOT = Path(__file__).resolve().parents[1]
BUILDER_PATH = ROOT / "scripts/build-web3-storage-inventory.py"
VALIDATOR = ROOT / "scripts/validate-web3-storage-parity.py"


def load_builder():
	spec = importlib.util.spec_from_file_location("web3_inventory_fixture_builder", BUILDER_PATH)
	if spec is None or spec.loader is None:
		raise AssertionError("could not load builder")
	module = importlib.util.module_from_spec(spec)
	sys.modules[spec.name] = module
	spec.loader.exec_module(module)
	return module


BUILDER = load_builder()


def load_validator():
	spec = importlib.util.spec_from_file_location("web3_inventory_fixture_validator", VALIDATOR)
	if spec is None or spec.loader is None:
		raise AssertionError("could not load validator")
	module = importlib.util.module_from_spec(spec)
	sys.modules[spec.name] = module
	spec.loader.exec_module(module)
	return module


VALIDATOR_MODULE = load_validator()


def run(*command: str, cwd: Path, check: bool = True) -> subprocess.CompletedProcess:
	return subprocess.run(command, cwd=cwd, text=True, capture_output=True, check=check)


def init_repo(path: Path) -> None:
	path.mkdir(parents=True)
	run("git", "init", "-q", cwd=path)
	run("git", "config", "user.name", "fixture", cwd=path)
	run("git", "config", "user.email", "fixture@example.invalid", cwd=path)


def commit_all(path: Path, message: str = "fixture") -> str:
	run("git", "add", "-A", cwd=path)
	run("git", "commit", "-q", "-m", message, cwd=path)
	return run("git", "rev-parse", "HEAD", cwd=path).stdout.strip()


def independent_root(repo: Path) -> str:
	raw = subprocess.check_output(["git", "-C", os.fspath(repo), "ls-files", "-s", "-z"])
	entries = []
	for value in raw.split(b"\0"):
		if not value:
			continue
		metadata, path = value.split(b"\t", 1)
		mode, object_hex, stage = metadata.split()
		assert stage == b"0" and mode != b"160000"
		object_id = bytes.fromhex(object_hex.decode("ascii"))
		entries.append((path, mode, object_id))
	payload = b""
	for path, mode, object_id in sorted(entries, key=lambda item: item[0]):
		payload += (
			struct.pack(">Q", len(path)) + path
			+ struct.pack(">I", len(mode)) + mode
			+ struct.pack(">I", len(object_id)) + object_id
			+ struct.pack(">I", 0)
		)
	return hashlib.sha256(payload).hexdigest()


def test_length_framed_root_and_raw_paths(workspace: Path) -> None:
	repo = workspace / "plain"
	init_repo(repo)
	(repo / "alpha").write_text("a", encoding="utf-8")
	(repo / "line\nbreak").write_text("b", encoding="utf-8")
	(repo / "é").write_text("c", encoding="utf-8")
	sha = commit_all(repo)
	entries, root = BUILDER.census(repo, sha)
	assert len(entries) == 3
	assert root.hex() == independent_root(repo)
	assert [entry.path for entry in entries] == sorted(entry.path for entry in entries)


def test_recursive_gitlink_binding(workspace: Path) -> None:
	child = workspace / "child"
	init_repo(child)
	(child / "nested.txt").write_text("child", encoding="utf-8")
	child_sha = commit_all(child)
	parent = workspace / "parent"
	init_repo(parent)
	(parent / "root.txt").write_text("root", encoding="utf-8")
	run("git", "-c", "protocol.file.allow=always", "submodule", "add", "-q", os.fspath(child), "vendor/child", cwd=parent)
	parent_sha = commit_all(parent)
	entries, first_root = BUILDER.census(parent, parent_sha)
	paths = [entry.path for entry in entries]
	assert b"vendor/child" in paths and b"vendor/child/nested.txt" in paths
	gitlink = next(entry for entry in entries if entry.path == b"vendor/child")
	assert gitlink.object_id.hex() == child_sha and len(gitlink.submodule_root) == 32
	assert BUILDER.census(parent, parent_sha)[1] == first_root


def test_escaping_symlink_rejected(workspace: Path) -> None:
	repo = workspace / "symlink"
	init_repo(repo)
	(repo.parent / "outside").write_text("outside", encoding="utf-8")
	os.symlink("../outside", repo / "escape")
	sha = commit_all(repo)
	try:
		BUILDER.census(repo, sha)
	except BUILDER.InventoryError as error:
		assert "escapes source root" in str(error)
	else:
		raise AssertionError("escaping tracked symlink was accepted")


def test_artifact_identity_is_framed_by_nuls() -> None:
	first = BUILDER.artifact_id("a", "bc", "d", "e", "f")
	second = BUILDER.artifact_id("ab", "c", "d", "e", "f")
	assert first != second
	assert first == BUILDER.artifact_id("a", "bc", "d", "e", "f")


def test_hostile_rust_semantic_fixture() -> None:
	source = r'''
#[cfg(feature = "alpha")]
pub enum PublicChoice {
    #[cfg_attr(feature = "beta", deprecated)]
    First,
    Second(u8),
}
#[pallet::storage]
pub type Records<T> = StorageValue<_, u32>;
#[pallet::constant]
type Limit: Get<u32>;
#[pallet::event]
pub enum Event<T> { Created { who: T }, Removed }
#[pallet::error]
pub enum Error<T> { Missing, Full }
#[pallet::call]
impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    pub fn create(origin: OriginFor<T>) -> DispatchResult { Ok(()) }
}
#[pallet::hooks]
impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_initialize(_n: BlockNumberFor<T>) -> Weight { Weight::zero() }
}
#[cfg(feature = "std")]
impl DirectoryNode {
    pub fn shared_name(&self) {}
    #[cfg(feature = "nested")]
    pub fn nested_gate(&self) {}
}
#[cfg(feature = "std")]
impl FileManifest {
    pub fn shared_name(&self) {}
}
sp_api::decl_runtime_apis! {
    #[cfg(feature = "runtime-api")]
    pub trait DemoApi {
        fn first(value: u32) -> u32;
        #[cfg(feature = "second")]
        fn second() -> bool;
    }
}
'''
	items = {(item.kind, item.symbol, item.predicate) for item in rust_surfaces(source)}
	expected = {
		("rust-public", "enum:PublicChoice", 'cfg(feature="alpha")'),
		("rust-enum-variant", "PublicChoice::First", 'cfg(feature="alpha")&&cfg_attr(feature="beta",deprecated)'),
		("rust-enum-variant", "PublicChoice::Second", 'cfg(feature="alpha")'),
		("frame-storage", "Records", "always"), ("frame-constant", "Limit", "always"),
		("frame-event", "Event::Created", "always"), ("frame-event", "Event::Removed", "always"),
		("frame-error", "Error::Missing", "always"), ("frame-error", "Error::Full", "always"),
		("rust-public", "DirectoryNode::shared_name", 'cfg(feature="std")'),
		("rust-public", "DirectoryNode::nested_gate", 'cfg(feature="nested")&&cfg(feature="std")'),
		("rust-public", "FileManifest::shared_name", 'cfg(feature="std")'),
		("frame-call", "Pallet<T>::create", "always"),
		("frame-hook", "<Pallet<T>asHooks<BlockNumberFor<T>>>::on_initialize", "always"),
		("runtime-api-trait", "DemoApi", 'cfg(feature="runtime-api")'),
		("runtime-api-method", "DemoApi::first", 'cfg(feature="runtime-api")'),
		("runtime-api-method", "DemoApi::second", 'cfg(feature="runtime-api")&&cfg(feature="second")'),
	}
	missing = expected - items
	assert not missing, f"syntax-aware Rust fixture omissions: {sorted(missing)}"
	with tempfile.TemporaryDirectory(prefix="web3-storage-rust-oracle-") as temporary:
		root = Path(temporary); path = "hostile.rs"
		(root / path).write_text(source, encoding="utf-8")
		artifacts = [
			{
				"path": path, "kind": item.kind, "qualified_symbol": item.symbol,
				"feature_predicate": item.predicate,
			}
			for item in rust_surfaces(source)
		]
		checks, oracle_missing = VALIDATOR_MODULE.independent_semantic_oracle(root, artifacts)
		assert checks > 0 and not oracle_missing, oracle_missing
		omitted = [item for item in artifacts if item["qualified_symbol"] != "FileManifest::shared_name"]
		_checks, omission_missing = VALIDATOR_MODULE.independent_semantic_oracle(root, omitted)
		assert any("FileManifest::shared_name" in item for item in omission_missing), omission_missing
		tampered = [dict(item) for item in artifacts]
		nested = next(item for item in tampered if item["qualified_symbol"] == "DirectoryNode::nested_gate")
		nested["feature_predicate"] = "always"
		_checks, predicate_missing = VALIDATOR_MODULE.independent_semantic_oracle(root, tampered)
		assert any("DirectoryNode::nested_gate" in item for item in predicate_missing), predicate_missing


def test_hostile_typescript_export_and_route_fixture(workspace: Path) -> None:
	source = '''
export {
  Alpha as RenamedAlpha,
  type Beta as RenamedBeta,
} from "./named";
export * from "./star";
export * as namespace from "./namespace";
export { default as DefaultThing } from "./default";
export default class LocalDefault {}
export async function upload() { return providerFetch(base, "/upload", { method: "POST" }); }
'''
	items = {(item.kind, item.symbol) for item in typescript_surfaces(source)}
	expected = {
		("typescript-reexport", "RenamedAlpha<-./named:Alpha"),
		("typescript-reexport", "RenamedBeta<-./named:Beta"),
		("typescript-reexport", "*<-./star"),
		("typescript-reexport", "namespace<-./namespace"),
		("typescript-reexport", "DefaultThing<-./default:default"),
		("typescript-public", "default"), ("typescript-public", "class:LocalDefault"),
		("typescript-public", "function:upload"), ("typescript-route", "/upload=>providerFetch"),
	}
	assert not expected - items, f"TypeScript fixture omissions: {sorted(expected - items)}"
	root = workspace / "semantic-ts"; (root / "src").mkdir(parents=True)
	(root / "package.json").write_text(json.dumps({
		"name": "fixture", "exports": {".": {"types": "./src/index.ts", "import": "./src/index.ts"}}
	}))
	(root / "src/index.ts").write_text('export { Alpha as PublicAlpha } from "./named";\n')
	(root / "src/named.ts").write_text('export const Alpha = 1;\n')
	tracked = {"package.json", "src/index.ts", "src/named.ts"}
	resolved = BUILDER.resolved_typescript_artifacts(root, tracked)
	symbols = {item[2] for item in resolved if item[1] == "typescript-resolved-export"}
	assert symbols == {"fixture:.:default.import:PublicAlpha", "fixture:.:default.types:PublicAlpha"}


def test_checked_inventory_and_authorization_contract(workspace: Path) -> None:
	ratification_path = ROOT / "docs/specs/p0-ratification-v1.toml"
	ratification_text = ratification_path.read_text(encoding="utf-8")
	ratification = VALIDATOR_MODULE.tomllib.loads(ratification_text)
	architect_id = ratification["architect_approval"]
	critic_id = ratification["critic_approval"]
	assert ratification["status"] == "accepted" and ratification["p0_gate_status"] == "pass"
	assert re.fullmatch(r"P0-ARCH-[0-9]{8}-RATIFY-V[1-9][0-9]*", architect_id)
	assert re.fullmatch(r"P0-CRITIC-[0-9]{8}-RATIFY-V[1-9][0-9]*", critic_id)
	assert "BLOCK" not in architect_id and "BLOCK" not in critic_id
	ledger_bytes = (ROOT / "docs/specs/web3-storage-capability-ledger-v1.json").read_bytes()
	inventory_bytes = (ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json").read_bytes()
	assert ratification["ledger_sha256"] == hashlib.sha256(ledger_bytes).hexdigest()
	assert ratification["inventory_sha256"] == hashlib.sha256(inventory_bytes).hexdigest()

	common = [
		sys.executable, os.fspath(VALIDATOR),
		"--manifest", os.fspath(ROOT / "docs/specs/web3-storage-upstream-sources-v1.toml"),
		"--ledger", os.fspath(ROOT / "docs/specs/web3-storage-capability-ledger-v1.json"),
		"--inventory", os.fspath(ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json"),
	]
	accepted = subprocess.run(common, cwd=ROOT, text=True, capture_output=True)
	assert accepted.returncode == 0, accepted.stdout + accepted.stderr
	value = json.loads(accepted.stdout)
	assert value["status"] == "pass" and value["p1_authorized"] is True
	assert value["ratification_accepted"] is True
	assert value["architect_approval"] == architect_id
	assert value["critic_approval"] == critic_id
	assert value["feature_complete"] is False and value["production_ready"] is False
	assert value["semantic_oracle_missing"] == 0 and value["semantic_oracle_checks"] > 0
	assert value["unvisited"] == value["unmapped"] == value["unknown"] == 0

	pending_ratification = workspace / "pending-ratification.toml"
	pending_ratification.write_text(
		ratification_text.replace(
			'status = "accepted"', 'status = "pending"', 1,
		),
		encoding="utf-8",
	)
	pending = subprocess.run(
		common + ["--ratification", os.fspath(pending_ratification), "--p0-audit"],
		cwd=ROOT, text=True, capture_output=True,
	)
	assert pending.returncode == 0, pending.stdout + pending.stderr
	pending_value = json.loads(pending.stdout)
	assert pending_value["status"] == "pending-ratification"
	assert pending_value["ratification_accepted"] is False
	assert pending_value["p1_authorized"] is False
	assert pending_value["feature_complete"] is False
	assert pending_value["production_ready"] is False

	def assert_ratification_rejected(path: Path) -> None:
		result = subprocess.run(
			common + ["--ratification", os.fspath(path), "--p0-audit"],
			cwd=ROOT, text=True, capture_output=True,
		)
		assert result.returncode == 0, result.stdout + result.stderr
		report = json.loads(result.stdout)
		assert report["status"] == "pending-ratification"
		assert report["ratification_accepted"] is False
		assert report["p1_authorized"] is False
		assert report["feature_complete"] is False
		assert report["production_ready"] is False

	blocked_ratification = workspace / "blocked-ratification.toml"
	blocked_ratification.write_text(
		ratification_text.replace(
			f'architect_approval = "{architect_id}"',
			'architect_approval = "P0-ARCH-19700101-BLOCK-V1"',
			1,
		),
		encoding="utf-8",
	)
	assert_ratification_rejected(blocked_ratification)

	mismatched_ratification = workspace / "mismatched-ratification.toml"
	mismatched_ratification.write_text(
		ratification_text.replace(
			f'ledger_sha256 = "{ratification["ledger_sha256"]}"',
			'ledger_sha256 = "' + "0" * 64 + '"',
			1,
		),
		encoding="utf-8",
	)
	assert_ratification_rejected(mismatched_ratification)

	tampered_path = workspace / "tampered.json"
	tampered = json.loads((ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json").read_text())
	tampered["artifacts"][0]["ledger_row_id"] = "WSI-NOT-A-ROW"
	tampered_path.write_text(json.dumps(tampered), encoding="utf-8")
	rejected = subprocess.run(
		common[:-1] + [os.fspath(tampered_path), "--p0-audit"], cwd=ROOT, text=True, capture_output=True,
	)
	assert rejected.returncode == 1, rejected.stdout + rejected.stderr
	assert json.loads(rejected.stdout)["unmapped"] == 1

	semantic_path = workspace / "semantic-omission.json"
	semantic = json.loads((ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json").read_text())
	removed = next(
		item for item in semantic["artifacts"]
		if item["kind"] == "frame-call" and item["path"] == "pallet/src/lib.rs"
	)
	semantic["artifacts"].remove(removed)
	semantic["artifact_ids"].remove(removed["id"])
	semantic["artifact_count"] -= 1
	semantic_path.write_text(json.dumps(semantic), encoding="utf-8")
	omission = subprocess.run(
		common[:-1] + [os.fspath(semantic_path)], cwd=ROOT, text=True, capture_output=True,
	)
	assert omission.returncode == 1, omission.stdout + omission.stderr
	omission_report = json.loads(omission.stdout)
	assert omission_report["semantic_oracle_missing"] > 0
	assert any("independent semantic oracle omissions" in error for error in omission_report["errors"])

	ledger_path = workspace / "implemented-ledger.json"
	inventory_path = workspace / "implemented-inventory.json"
	ledger = json.loads((ROOT / "docs/specs/web3-storage-capability-ledger-v1.json").read_text())
	provider = next(row for row in ledger["rows"] if row["id"] == "WSI-PROVIDER-BYTE-PLANE")
	provider["maturity"] = "implemented"
	provider["implementation_state"] = "implemented"
	ledger_bytes = (json.dumps(ledger, indent=2, sort_keys=True) + "\n").encode()
	ledger_path.write_bytes(ledger_bytes)
	implemented_inventory = json.loads(
		(ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json").read_text()
	)
	implemented_inventory["ledger_sha256"] = hashlib.sha256(ledger_bytes).hexdigest()
	inventory_path.write_text(json.dumps(implemented_inventory), encoding="utf-8")
	unsupported_claim = subprocess.run(
		[
			sys.executable, os.fspath(VALIDATOR), "--ledger", os.fspath(ledger_path),
			"--inventory", os.fspath(inventory_path),
		],
		cwd=ROOT, text=True, capture_output=True,
	)
	assert unsupported_claim.returncode == 1, unsupported_claim.stdout + unsupported_claim.stderr
	assert "lacks CORD symbol/test evidence" in unsupported_claim.stdout

	tampered_decision = workspace / "web3-storage-blocked-conflicts-v1.json"
	decision = json.loads((ROOT / "docs/specs/web3-storage-blocked-conflicts-v1.json").read_text())
	decision["version"] = 2
	tampered_decision.write_text(json.dumps(decision), encoding="utf-8")
	decision_drift = subprocess.run(
		common + ["--decisions-json", os.fspath(tampered_decision)],
		cwd=ROOT, text=True, capture_output=True,
	)
	assert decision_drift.returncode == 1, decision_drift.stdout + decision_drift.stderr
	assert "decision packet JSON hash drift" in decision_drift.stdout


def main() -> int:
	with tempfile.TemporaryDirectory(prefix="web3-storage-inventory-") as temporary:
		workspace = Path(temporary)
		test_length_framed_root_and_raw_paths(workspace)
		test_recursive_gitlink_binding(workspace)
		test_escaping_symlink_rejected(workspace)
		test_artifact_identity_is_framed_by_nuls()
		test_hostile_rust_semantic_fixture()
		test_hostile_typescript_export_and_route_fixture(workspace)
		test_checked_inventory_and_authorization_contract(workspace)
	print("PASS: census, Rust/FRAME/runtime-API/TS semantics, independent oracle, ratification and false-implementation fixtures")
	return 0


if __name__ == "__main__":
	raise SystemExit(main())

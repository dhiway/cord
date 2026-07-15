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

"""Build the pinned, whole-repository Parity Web3 Storage inventory.

The enumerator is deliberately fail closed: every tracked path is visited, every
artifact has one classification and one ledger row, and gitlinks are bound to
their recorded object IDs and recursively length-framed census roots.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
try:
	import tomllib
except ImportError:  # Python 3.9 used by supported macOS developer hosts.
	import tomli as tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional

from web3_storage_semantic import package_surfaces, rust_surfaces, typescript_surfaces


ROOT = Path(__file__).resolve().parents[1]
ENUMERATOR_VERSION = 3
CLASSIFICATIONS = {
	"public-product", "public-operator", "runtime-consensus", "provider-protocol",
	"reference-flow", "design-only", "guidance", "workspace-config", "lockfile",
	"internal", "generated", "test-only", "approved-out-of-scope",
}


class InventoryError(RuntimeError):
	"""An input is incomplete, ambiguous, unsafe, or different from the pin."""


@dataclass(frozen=True)
class CensusEntry:
	path: bytes
	mode: bytes
	object_id: bytes
	submodule_root: bytes = b""

	def framed(self) -> bytes:
		return (
			struct.pack(">Q", len(self.path)) + self.path
			+ struct.pack(">I", len(self.mode)) + self.mode
			+ struct.pack(">I", len(self.object_id)) + self.object_id
			+ struct.pack(">I", len(self.submodule_root)) + self.submodule_root
		)


def run_bytes(repo: Path, *args: str) -> bytes:
	result = subprocess.run(
		["git", "-C", os.fspath(repo), *args], capture_output=True, check=False,
	)
	if result.returncode:
		raise InventoryError(
			f"git {' '.join(args)} failed in {repo}: "
			f"{result.stderr.decode('utf-8', 'replace').strip()}"
		)
	return result.stdout


def git_text(repo: Path, *args: str) -> str:
	return run_bytes(repo, *args).decode("utf-8").strip()


def _tracked_records(repo: Path) -> list[tuple[bytes, bytes, bytes]]:
	paths = [value for value in run_bytes(repo, "ls-files", "-z").split(b"\0") if value]
	stage = [value for value in run_bytes(repo, "ls-files", "-s", "-z").split(b"\0") if value]
	records: list[tuple[bytes, bytes, bytes]] = []
	for value in stage:
		metadata, separator, path = value.partition(b"\t")
		parts = metadata.split()
		if not separator or len(parts) != 3 or parts[2] != b"0":
			raise InventoryError(f"unsupported index entry: {value!r}")
		mode, object_hex, _stage = parts
		try:
			object_id = bytes.fromhex(object_hex.decode("ascii"))
		except (ValueError, UnicodeDecodeError) as error:
			raise InventoryError(f"invalid object ID for {path!r}") from error
		records.append((path, mode, object_id))
	if sorted(paths) != sorted(path for path, _mode, _object in records):
		raise InventoryError("git ls-files -z and staged census disagree")
	return records


def _reject_escaping_symlink(repo: Path, path: bytes) -> None:
	try:
		relative = Path(path.decode("utf-8"))
	except UnicodeDecodeError as error:
		raise InventoryError(f"tracked path is not UTF-8: {path!r}") from error
	target_text = os.readlink(repo / relative)
	target = Path(target_text)
	if target.is_absolute():
		raise InventoryError(f"tracked symlink escapes source root: {relative}")
	resolved = (repo / relative.parent / target).resolve(strict=False)
	try:
		resolved.relative_to(repo.resolve(strict=True))
	except ValueError as error:
		raise InventoryError(f"tracked symlink escapes source root: {relative}") from error


def census(repo: Path, expected_sha: Optional[str] = None) -> tuple[list[CensusEntry], bytes]:
	if expected_sha is not None and git_text(repo, "rev-parse", "HEAD") != expected_sha:
		raise InventoryError(f"source HEAD does not equal pinned commit {expected_sha}")
	entries: list[CensusEntry] = []
	for path, mode, object_id in _tracked_records(repo):
		if mode == b"120000":
			_reject_escaping_symlink(repo, path)
		if mode != b"160000":
			entries.append(CensusEntry(path, mode, object_id))
			continue
		relative = Path(path.decode("utf-8"))
		submodule = repo / relative
		if not submodule.is_dir() or not (submodule / ".git").exists():
			raise InventoryError(f"gitlink is not initialized: {relative}")
		recorded_sha = object_id.hex()
		children, child_root = census(submodule, recorded_sha)
		entries.append(CensusEntry(path, mode, object_id, child_root))
		for child in children:
			entries.append(CensusEntry(path + b"/" + child.path, child.mode, child.object_id, child.submodule_root))
	entries.sort(key=lambda item: item.path)
	root = hashlib.sha256(b"".join(item.framed() for item in entries)).digest()
	return entries, root


def _matches(path: str, selector: dict[str, object]) -> bool:
	return (
		path in selector.get("exact_paths", [])
		or any(path == prefix.rstrip("/") or path.startswith(prefix) for prefix in selector.get("prefixes", []))
		or any(fnmatch.fnmatchcase(path, pattern) for pattern in selector.get("globs", []))
	)


def ledger_row_for(path: str, rows: list[dict[str, object]]) -> str:
	matches = [str(row["id"]) for row in rows if _matches(path, row["selector"])]
	if len(matches) != 1:
		raise InventoryError(f"tracked path must map to exactly one ledger row: {path}: {matches}")
	return matches[0]


def classify(path: str) -> str:
	"""Classify a path through an exhaustive, ordered policy with no unknown default."""
	name = Path(path).name
	parts = Path(path).parts
	if path == "CLAUDE.md" or path.startswith(".claude/"):
		return "guidance"
	if name.startswith("README") or name == "CONTRIBUTING.md":
		return "guidance"
	if name in {"Cargo.lock", "pnpm-lock.yaml"}:
		return "lockfile"
	if path.endswith(".scale") or "/weights/" in path or "/src/contract/photos-abi.ts" in path:
		return "generated"
	if (
		"tests" in parts or "e2e" in parts or "/test" in path or name.endswith((".test.ts", ".spec.ts"))
		or name in {"mock.rs", "benchmarking.rs", "weights.rs"}
	):
		return "test-only"
	if path.startswith(("precompiles/", "examples/contracts/", "runtimes/", "chain-specs/", "zombienet")):
		return "approved-out-of-scope" if path.startswith(("precompiles/", "examples/contracts/")) else "runtime-consensus"
	if path.startswith(("pallet/", "primitives/")):
		return "runtime-consensus"
	if path.startswith(("provider-node/", "provider/")):
		return "provider-protocol"
	if path.startswith("user-interfaces/provider/"):
		return "public-operator"
	if path.startswith(("client/", "packages/", "storage-interfaces/")):
		return "public-product"
	if path.startswith(("examples/", "user-interfaces/")):
		return "reference-flow"
	if path.startswith("docs/design/") or path.endswith(("DESIGN.md", "ARCHITECTURE.md")):
		return "design-only"
	if path.startswith("docs/") or name == "FILE_SYSTEM_QUICKSTART.md":
		return "guidance"
	if (
		path.startswith((".github/", ".config/", "templates/"))
		or name in {"Cargo.toml", "package.json", "pnpm-workspace.yaml", "rust-toolchain.toml", "justfile", "deny.toml"}
		or name.endswith(("tsconfig.json", "vite.config.ts", "playwright.config.ts", "vitest.config.ts"))
	):
		return "workspace-config"
	# This is an explicit repository-internal classification for the finite paths
	# admitted by a ledger selector; paths without a selector fail before here.
	return "internal"


def artifact_id(repo_sha: str, path: str, kind: str, symbol: str, predicate: str) -> str:
	payload = "\0".join((repo_sha, path, kind, symbol, predicate)).encode("utf-8")
	return hashlib.sha256(payload).hexdigest()


def semantic_artifacts(path: str, data: bytes) -> Iterable[tuple[str, str, str, str]]:
	"""Yield kind, qualified symbol, feature predicate and evidence."""
	try:
		text = data.decode("utf-8")
	except UnicodeDecodeError:
		return
	if path.endswith(".rs"):
		for item in rust_surfaces(text):
			yield item.kind, item.symbol, item.predicate, f"{path}:{item.line}"
	elif path.endswith((".ts", ".tsx", ".js", ".mjs")):
		for item in typescript_surfaces(text):
			yield item.kind, item.symbol, item.predicate, f"{path}:{item.line}"
	elif path.lower().endswith(".md"):
		for match in re.finditer(r"(?m)^(#{1,6})\s+(.+?)\s*$", text):
			yield "markdown-heading", match.group(2), "always", f"{path}:{text.count(chr(10), 0, match.start()) + 1}"
		for match in re.finditer(r"(?ms)^```([^\n]*)\n(.*?)^```", text):
			body = match.group(2)
			if re.search(r"(?i)\b(api|schema|route|extrinsic|GET /|POST /|pub fn|interface)\b", body):
				yield "documented-api", f"fence:{text.count(chr(10), 0, match.start()) + 1}:{match.group(1).strip() or 'plain'}", "always", f"{path}:{text.count(chr(10), 0, match.start()) + 1}"


def cargo_artifacts(source: Path) -> list[tuple[str, str, str, str, str]]:
	command = ["cargo", "metadata", "--all-features", "--format-version", "1", "--no-deps"]
	result = subprocess.run(command, cwd=source, text=True, capture_output=True, check=False)
	if result.returncode:
		raise InventoryError(f"cargo metadata failed: {result.stderr.strip()}")
	metadata = json.loads(result.stdout)
	items: list[tuple[str, str, str, str, str]] = []
	for package in metadata.get("packages", []):
		manifest = Path(package["manifest_path"]).resolve().relative_to(source.resolve()).as_posix()
		items.append((manifest, "cargo-package", package["name"], "always", manifest))
		for feature in sorted(package.get("features", {})):
			items.append((manifest, "cargo-feature", f"{package['name']}:{feature}", feature, manifest))
		for target in package.get("targets", []):
			items.append((manifest, "cargo-target", f"{package['name']}:{target['name']}:{','.join(target['kind'])}", "always", manifest))
	return items


def npm_artifacts(path: str, data: bytes) -> Iterable[tuple[str, str, str, str]]:
	if not path.endswith("package.json"):
		return
	for item in package_surfaces(data):
		yield item.kind, item.symbol, item.predicate, f"{path}:{item.line}"


def npm_workspace_artifacts(source: Path, tracked_paths: set[str]) -> list[tuple[str, str, str, str, str]]:
	"""Resolve the finite pnpm workspace without depending on a YAML package."""
	workspace_file = source / "pnpm-workspace.yaml"
	if not workspace_file.exists():
		return []
	patterns: list[str] = []
	in_packages = False
	for raw_line in workspace_file.read_text(encoding="utf-8").splitlines():
		line = raw_line.split("#", 1)[0].rstrip()
		if not line:
			continue
		if not line.startswith((" ", "\t")):
			in_packages = line == "packages:"
			continue
		if in_packages:
			match = re.match(r"\s*-\s*['\"]?([^'\"]+)['\"]?\s*$", line)
			if not match:
				raise InventoryError(f"unsupported pnpm workspace entry: {raw_line}")
			patterns.append(match.group(1))
	resolved = {
		path.relative_to(source).as_posix()
		for pattern in patterns
		for path in source.glob(pattern + "/package.json")
	}
	tracked_packages = {path for path in tracked_paths if path.endswith("/package.json")}
	if resolved != tracked_packages:
		raise InventoryError(
			"pnpm workspace resolution drift: "
			f"missing={sorted(tracked_packages - resolved)} extra={sorted(resolved - tracked_packages)}"
		)
	return [
		(path, "npm-workspace", str(Path(path).parent), "always", "pnpm-workspace.yaml")
		for path in sorted(resolved)
	]


def _resolve_ts_module(current: str, specifier: str, tracked: set[str]) -> Optional[str]:
	if not specifier.startswith("."):
		return None
	base = (Path(current).parent / specifier).as_posix()
	if base.endswith(".js"): base = base[:-3]
	candidates = [base, base + ".ts", base + ".tsx", base + "/index.ts", base + "/index.tsx"]
	return next((candidate for candidate in candidates if candidate in tracked), None)


def resolved_typescript_artifacts(source: Path, tracked: set[str]) -> list[tuple[str, str, str, str, str]]:
	"""Resolve local TS export graphs, package conditions and provider route edges."""
	modules: dict[str, list[object]] = {}
	server_routes: dict[str, list[str]] = {}
	client_routes: list[tuple[str, object]] = []
	for path in sorted(item for item in tracked if Path(item).suffix in {".ts", ".tsx", ".js", ".mjs"}):
		items = typescript_surfaces((source / path).read_text(encoding="utf-8"))
		modules[path] = items
		client_routes.extend((path, item) for item in items if item.kind == "typescript-route")
	for path in sorted(item for item in tracked if item.endswith(".rs")):
		for item in rust_surfaces((source / path).read_text(encoding="utf-8")):
			if item.kind == "http-route": server_routes.setdefault(item.symbol.split("=>", 1)[0], []).append(f"{path}:{item.symbol}")

	cache: dict[str, dict[str, str]] = {}
	def exports(path: str, stack: tuple[str, ...] = ()) -> dict[str, str]:
		if path in cache: return cache[path]
		if path in stack: raise InventoryError(f"TypeScript export cycle: {' -> '.join(stack + (path,))}")
		value: dict[str, str] = {}
		for item in modules.get(path, []):
			if item.kind == "typescript-public":
				name = item.symbol if item.symbol == "default" else item.symbol.split(":", 1)[-1]
				value[name] = path
			elif item.kind == "typescript-reexport":
				exported, target = item.symbol.split("<-", 1)
				if target.startswith("local:"):
					value[exported] = path; continue
				if ":" in target and not target.startswith("."):
					continue
				if ":" in target:
					specifier, imported = target.rsplit(":", 1)
				else:
					specifier, imported = target, "*"
				resolved = _resolve_ts_module(path, specifier, tracked)
				if resolved is None: continue
				resolved_exports = exports(resolved, stack + (path,))
				if exported == "*":
					value.update({name: origin for name, origin in resolved_exports.items() if name != "default"})
				elif imported == "*": value[exported] = resolved
				elif imported in resolved_exports: value[exported] = resolved_exports[imported]
		cache[path] = value
		return value

	result: list[tuple[str, str, str, str, str]] = []
	for package_json in sorted(item for item in tracked if item.endswith("package.json")):
		package = json.loads((source / package_json).read_text(encoding="utf-8")); package_name = package.get("name", package_json)
		export_map = package.get("exports", {})
		if isinstance(export_map, str): export_map = {".": export_map}
		if not isinstance(export_map, dict): continue
		for subpath in sorted(export_map):
			from web3_storage_semantic import package_export_leaves
			for condition, target in package_export_leaves(export_map[subpath]):
				base = (Path(package_json).parent / target).as_posix()
				resolved = _resolve_ts_module(package_json, target, tracked) or (base if base in tracked else None)
				if resolved is None: continue
				for name, origin in sorted(exports(resolved).items()):
					result.append((origin, "typescript-resolved-export", f"{package_name}:{subpath}:{condition}:{name}", "always", f"{package_json}->{resolved}->{origin}"))
	for path, item in client_routes:
		route = item.symbol.split("=>", 1)[0]
		for server in sorted(server_routes.get(route, [])):
			result.append((path, "route-graph", f"{route}=>{server}", "always", f"{path}:{item.line}"))
	return result


def build(source: Path, manifest_path: Path, ledger_path: Path) -> dict[str, object]:
	manifest_bytes = manifest_path.read_bytes()
	manifest = tomllib.loads(manifest_bytes.decode("utf-8"))
	ledger_bytes = ledger_path.read_bytes()
	ledger = json.loads(ledger_bytes)
	pin = manifest["source"]["commit"]
	if git_text(source, "rev-parse", "HEAD") != pin:
		raise InventoryError(f"source HEAD does not equal pinned commit {pin}")
	entries, census_root = census(source, pin)
	rows = ledger["rows"]
	artifacts: dict[str, dict[str, object]] = {}

	def add(path: str, kind: str, symbol: str, predicate: str, evidence: str) -> None:
		row = ledger_row_for(path, rows)
		identifier = artifact_id(pin, path, kind, symbol, predicate)
		item = {
			"id": identifier, "path": path, "kind": kind, "qualified_symbol": symbol,
			"feature_predicate": predicate, "classification": classify(path),
			"ledger_row_id": row, "evidence": [evidence],
		}
		if identifier in artifacts:
			existing = artifacts[identifier]
			without_evidence = {key: value for key, value in existing.items() if key != "evidence"}
			if without_evidence != {key: value for key, value in item.items() if key != "evidence"}:
				raise InventoryError(f"artifact ID collision: {identifier}")
			existing["evidence"] = sorted(set(existing["evidence"] + [evidence]))
			return
		artifacts[identifier] = item

	for entry in entries:
		path = entry.path.decode("utf-8")
		add(path, "file", path, "always", f"git:{entry.object_id.hex()}")
		if classify(path) == "guidance":
			add(path, "guidance", path, "always", path)
		# A gitlink itself is visited above; recursively prefixed files are read from
		# their initialized checkout path just like ordinary tracked files.
		if entry.mode == b"160000":
			continue
		file_path = source / path
		if file_path.is_symlink():
			data = os.readlink(file_path).encode("utf-8")
		else:
			data = file_path.read_bytes()
		for kind, symbol, predicate, evidence in semantic_artifacts(path, data):
			add(path, kind, symbol, predicate, evidence)
		for kind, symbol, predicate, evidence in npm_artifacts(path, data):
			add(path, kind, symbol, predicate, evidence)

	for path, kind, symbol, predicate, evidence in cargo_artifacts(source):
		add(path, kind, symbol, predicate, evidence)
	tracked_paths = {entry.path.decode("utf-8") for entry in entries}
	for path, kind, symbol, predicate, evidence in npm_workspace_artifacts(source, tracked_paths):
		add(path, kind, symbol, predicate, evidence)
	for path, kind, symbol, predicate, evidence in resolved_typescript_artifacts(source, tracked_paths):
		add(path, kind, symbol, predicate, evidence)

	ordered = sorted(artifacts.values(), key=lambda item: item["id"])
	return {
		"schema_version": 1,
		"enumerator_version": ENUMERATOR_VERSION,
		"repository_url": manifest["source"]["repository"],
		"repo_sha": pin,
		"manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
		"ledger_sha256": hashlib.sha256(ledger_bytes).hexdigest(),
		"census_count": len(entries),
		"census_root": census_root.hex(),
		"artifact_count": len(ordered),
		"artifact_ids": [item["id"] for item in ordered],
		"artifacts": ordered,
	}


def default_source(manifest_path: Path) -> Path:
	manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
	configured = os.environ.get("WEB3_STORAGE_PIN", manifest["source"]["reference_checkout"])
	path = Path(configured).expanduser()
	return (ROOT / path).resolve() if not path.is_absolute() else path.resolve()


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument("--manifest", type=Path, default=ROOT / "docs/specs/web3-storage-upstream-sources-v1.toml")
	parser.add_argument("--ledger", type=Path, default=ROOT / "docs/specs/web3-storage-capability-ledger-v1.json")
	parser.add_argument("--source", "--repository", dest="source", type=Path)
	parser.add_argument("--out", type=Path, default=ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json")
	args = parser.parse_args()
	try:
		source = args.source.resolve() if args.source else default_source(args.manifest)
		inventory = build(source, args.manifest.resolve(), args.ledger.resolve())
		encoded = json.dumps(inventory, indent=2, sort_keys=True) + "\n"
		args.out.parent.mkdir(parents=True, exist_ok=True)
		temporary = args.out.with_suffix(args.out.suffix + ".tmp")
		temporary.write_text(encoded, encoding="utf-8")
		temporary.replace(args.out)
		print(json.dumps({
			"status": "pass", "census_count": inventory["census_count"],
			"census_root": inventory["census_root"], "artifact_count": inventory["artifact_count"],
			"out": os.fspath(args.out),
		}, sort_keys=True))
		return 0
	except (InventoryError, KeyError, ValueError, OSError, json.JSONDecodeError) as error:
		print(json.dumps({"status": "fail", "error": str(error)}, sort_keys=True), file=sys.stderr)
		return 1


if __name__ == "__main__":
	raise SystemExit(main())

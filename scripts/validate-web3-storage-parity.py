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

"""Validate AC1 inventory/ledger structure and block conflicted implementation."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sys
try:
	import tomllib
except ImportError:  # Python 3.9 used by supported macOS developer hosts.
	import tomli as tomllib
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BUILDER = ROOT / "scripts/build-web3-storage-inventory.py"
REQUIRED_ROW_FIELDS = {
	"id", "title", "selector", "classification_scope", "upstream_transition", "maturity",
	"cord_authority", "cord_path", "cord_transition", "invariant", "bounds", "failure_codes",
	"vector_or_test", "disposition", "owner", "dependencies", "deletion_targets", "rationale",
}
ALLOWED_DISPOSITIONS = {"retain", "refactor", "replace-clean-room", "approved-excluded"}
ALLOWED_MATURITY = {
	"prototype-implemented", "prototype-partial", "design-only", "blocked-conflict", "implemented",
}


def _oracle_strip_comments(text: str) -> str:
	"""Remove Rust comments without changing offsets; strings remain opaque."""
	value = list(text); index, state, block_depth = 0, "code", 0
	while index < len(text):
		if state == "code":
			if text.startswith("//", index):
				value[index:index + 2] = "  "; index += 2; state = "line"; continue
			if text.startswith("/*", index):
				value[index:index + 2] = "  "; index += 2; state = "block"; block_depth = 1; continue
			if text[index] == '"' or (
				text[index] == "'" and re.match(r"'(?:\\.|[^\\'])'", text[index:])
			):
				state = text[index]; index += 1; continue
		elif state == "line":
			if text[index] == "\n": state = "code"
			else: value[index] = " "
			index += 1; continue
		elif state == "block":
			if text.startswith("/*", index): value[index:index + 2] = "  "; index += 2; block_depth += 1; continue
			if text.startswith("*/", index):
				value[index:index + 2] = "  "; index += 2; block_depth -= 1
				if not block_depth: state = "code"
				continue
			if text[index] != "\n": value[index] = " "
			index += 1; continue
		else:
			if text[index] == "\\": index += 2; continue
			if text[index] == state: state = "code"
			index += 1; continue
		index += 1
	return "".join(value)


def _oracle_matching_brace(text: str, start: int) -> int | None:
	depth, index, state = 0, start, "code"
	while index < len(text):
		char = text[index]
		if state == "code":
			if char == '"' or (char == "'" and re.match(r"'(?:\\.|[^\\'])'", text[index:])): state = char
			elif char == "{": depth += 1
			elif char == "}":
				depth -= 1
				if depth == 0: return index
		else:
			if char == "\\": index += 2; continue
			if char == state: state = "code"
		index += 1
	return None


def _oracle_canonical(value: str) -> str:
	"""Remove insignificant whitespace while preserving quoted cfg values."""
	result, index, state = [], 0, "code"
	while index < len(value):
		char = value[index]
		if state == "code":
			if char in {'"', "'"}: result.append(char); state = char
			elif not char.isspace(): result.append(char)
		else:
			result.append(char)
			if char == "\\" and index + 1 < len(value):
				index += 1; result.append(value[index])
			elif char == state: state = "code"
		index += 1
	return "".join(result)


def _oracle_leading_cfg(text: str, position: int) -> list[str]:
	match = re.search(r"((?:#\s*\[[^\]]*\]\s*)+)$", text[:position])
	if not match: return []
	result = []
	for attribute in re.findall(r"#\s*\[([^\]]*)\]", match.group(1)):
		canonical = _oracle_canonical(attribute)
		if canonical.startswith(("cfg(", "cfg_attr(")): result.append(canonical)
	return result


def _oracle_declaration_start(text: str, keyword_start: int) -> int:
	visibility = re.search(r"\bpub(?:\s*\([^)]*\))?\s*$", text[:keyword_start])
	return visibility.start() if visibility else keyword_start


def _oracle_predicate(*groups: list[str]) -> str:
	parts = sorted({item for group in groups for item in group})
	return "&&".join(parts) if parts else "always"


def _oracle_impl_label(header: str) -> str:
	"""Independent textual normalization matching the public self/trait notation."""
	value = header.strip()
	if value.startswith("<"):
		depth = 0
		for index, char in enumerate(value):
			if char == "<": depth += 1
			elif char == ">":
				depth -= 1
				if depth == 0: value = value[index + 1:].lstrip(); break
	# The pinned corpus uses ordinary type headers; split `where`/`for` only at
	# whitespace-delimited top level, conservatively tracking generic brackets.
	tokens = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|::|->|[^\s]", value)
	angle = round_depth = square_depth = 0; where = for_index = None
	for index, token in enumerate(tokens):
		if token == "<": angle += 1
		elif token == ">" and angle: angle -= 1
		elif token == "(": round_depth += 1
		elif token == ")" and round_depth: round_depth -= 1
		elif token == "[": square_depth += 1
		elif token == "]" and square_depth: square_depth -= 1
		elif angle == round_depth == square_depth == 0:
			if token == "where" and where is None: where = index
			elif token == "for" and for_index is None: for_index = index
	end = len(tokens) if where is None else where
	if for_index is None or for_index >= end: return "".join(tokens[:end])
	return f"<{''.join(tokens[for_index + 1:end])}as{''.join(tokens[:for_index])}>"


def _independent_modules(clean: str) -> list[tuple[int, int, str, list[str]]]:
	modules = []
	for match in re.finditer(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", clean):
		opening = clean.find("{", match.start()); close = _oracle_matching_brace(clean, opening)
		if close is not None:
			modules.append((
				opening, close, match.group(1),
				_oracle_leading_cfg(clean, _oracle_declaration_start(clean, match.start())),
			))
	return modules


def _independent_impl_methods(text: str) -> set[tuple[str, str]]:
	"""Lower-bound model for public associated methods and inherited cfg."""
	clean = _oracle_strip_comments(text); result: set[tuple[str, str]] = set()
	modules = _independent_modules(clean)
	for match in re.finditer(r"\bimpl(?:\s*<[^{};]*?>)?\s+", clean):
		opening = clean.find("{", match.end())
		semicolon = clean.find(";", match.end(), opening if opening >= 0 else len(clean))
		if opening < 0 or semicolon >= 0: continue
		close = _oracle_matching_brace(clean, opening)
		if close is None: continue
		header = clean[match.end():opening]
		label = _oracle_impl_label(header)
		containing = sorted((module for module in modules if module[0] < match.start() < module[1]), key=lambda item: item[0])
		prefix = "::".join(module[2] for module in containing)
		impl_cfg = _oracle_leading_cfg(clean, match.start())
		module_cfg = [item for module in containing for item in module[3]]
		body = clean[opening + 1:close]
		for method in re.finditer(r"\bpub(?:\s*\([^)]*\))?\s+(?:(?:async|unsafe|const|extern|default)\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)", body):
			absolute = opening + 1 + method.start()
			# Only direct impl members belong to this block.
			if clean[opening + 1:absolute].count("{") != clean[opening + 1:absolute].count("}"): continue
			symbol = f"{label}::{method.group(1)}"
			if prefix: symbol = f"{prefix}::{symbol}"
			result.add((symbol, _oracle_predicate(module_cfg, impl_cfg, _oracle_leading_cfg(clean, absolute))))
	return result


def _independent_trait_methods(text: str) -> set[tuple[str, str]]:
	"""Independent lower bound for qualified trait-associated method surfaces."""
	clean = _oracle_strip_comments(text); modules = _independent_modules(clean)
	result: set[tuple[str, str]] = set()
	for match in re.finditer(r"\btrait\s+([A-Za-z_][A-Za-z0-9_]*)", clean):
		opening = clean.find("{", match.end()); semicolon = clean.find(";", match.end(), opening if opening >= 0 else len(clean))
		if opening < 0 or semicolon >= 0: continue
		close = _oracle_matching_brace(clean, opening)
		if close is None: continue
		containing = sorted((module for module in modules if module[0] < match.start() < module[1]), key=lambda item: item[0])
		prefix = "::".join(module[2] for module in containing)
		module_cfg = [item for module in containing for item in module[3]]
		trait_cfg = _oracle_leading_cfg(clean, _oracle_declaration_start(clean, match.start()))
		body = clean[opening + 1:close]
		for method in re.finditer(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", body):
			absolute = opening + 1 + method.start()
			if clean[opening + 1:absolute].count("{") != clean[opening + 1:absolute].count("}"): continue
			symbol = f"{match.group(1)}::{method.group(1)}"
			if prefix: symbol = f"{prefix}::{symbol}"
			result.add((symbol, _oracle_predicate(module_cfg, trait_cfg, _oracle_leading_cfg(clean, absolute))))
	return result


def independent_semantic_oracle(source: Path, artifacts: list[dict[str, object]]) -> tuple[int, list[str]]:
	"""Conservative, independent lower-bound checks against the structural parser.

	The oracle intentionally uses a separate textual/counting model.  It cannot add
	artifacts; it only catches omissions in the syntax-aware enumerator.
	"""
	by_path: dict[str, Counter[str]] = {}
	by_path_symbols: dict[str, set[tuple[str, str]]] = {}
	for item in artifacts:
		path = str(item.get("path", "")); kind = str(item.get("kind", "")); symbol = str(item.get("qualified_symbol", ""))
		by_path.setdefault(path, Counter())[kind] += 1
		by_path_symbols.setdefault(path, set()).add((kind, symbol))
	checks, missing = 0, []
	for path in sorted(by_path):
		absolute = source / path
		if path.endswith(".rs") and absolute.is_file():
			text = absolute.read_text(encoding="utf-8")
			associated_methods = _independent_impl_methods(text) | _independent_trait_methods(text)
			for symbol, feature_predicate in sorted(associated_methods):
				checks += 1
				match = next((
					item for item in artifacts
					if item.get("path") == path and item.get("kind") == "rust-public"
					and item.get("qualified_symbol") == symbol
					and item.get("feature_predicate") == feature_predicate
				), None)
				if match is None: missing.append(f"{path}:qualified-impl-method:{symbol}:{feature_predicate}")
			clean = _oracle_strip_comments(text); modules = _independent_modules(clean)
			for enum_match in re.finditer(r"\bpub(?:\s*\([^)]*\))?\s+enum\s+([A-Za-z_][A-Za-z0-9_]*)", clean):
				name = enum_match.group(1)
				prefix = "::".join(
					module[2] for module in sorted(
						(item for item in modules if item[0] < enum_match.start() < item[1]), key=lambda item: item[0],
					)
				)
				symbol = f"enum:{name}" if not prefix else f"{prefix}::enum:{name}"
				checks += 1
				if ("rust-public", symbol) not in by_path_symbols[path]: missing.append(f"{path}:public-enum:{symbol}")
			lower_bounds = {
				"frame-call": len(re.findall(r"#\s*\[\s*pallet::call_index", text)),
				"frame-storage": len(re.findall(r"#\s*\[\s*pallet::storage\s*\]", text)),
				"frame-constant": len(re.findall(r"#\s*\[\s*pallet::constant\s*\]", text)),
				"http-route": len(re.findall(r"\.route\s*\(", text)),
			}
			for kind, minimum in lower_bounds.items():
				checks += 1
				if by_path[path][kind] < minimum: missing.append(f"{path}:{kind}:{by_path[path][kind]}<{minimum}")
			cfg_public = len(re.findall(r"#\s*\[\s*cfg(?:_attr)?\s*\([^]]+\)\s*\]\s*pub", text, re.S))
			if cfg_public:
				checks += 1
				semantic = [item for item in artifacts if item.get("path") == path and item.get("kind") in {"rust-public", "rust-struct-field"}]
				if sum(item.get("feature_predicate") != "always" for item in semantic) < cfg_public:
					missing.append(f"{path}:cfg-public-predicates")
		elif path.endswith((".ts", ".tsx", ".js", ".mjs")) and absolute.is_file():
			text = absolute.read_text(encoding="utf-8")
			export_statements = sum(
				1 for match in re.finditer(r"(?m)^\s*export\b", text)
				if text[:match.start()].count("`") % 2 == 0
			)
			checks += 1
			if by_path[path]["typescript-public"] + by_path[path]["typescript-reexport"] < export_statements:
				missing.append(f"{path}:typescript-exports")
		elif path.endswith("package.json") and absolute.is_file():
			value = json.loads(absolute.read_text(encoding="utf-8")); exports = value.get("exports", {})
			if exports:
				from web3_storage_semantic import package_export_leaves
				if isinstance(exports, str): exports = {".": exports}
				minimum = sum(len(package_export_leaves(exports[key])) for key in exports)
				checks += 1
				if by_path[path]["npm-export-condition"] != minimum: missing.append(f"{path}:package-export-conditions")
	return checks, missing


def load_builder():
	spec = importlib.util.spec_from_file_location("cord_web3_storage_inventory", BUILDER)
	if spec is None or spec.loader is None:
		raise RuntimeError("cannot load inventory builder")
	module = importlib.util.module_from_spec(spec)
	sys.modules[spec.name] = module
	spec.loader.exec_module(module)
	return module


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(args: argparse.Namespace) -> tuple[dict[str, object], int]:
	errors: list[str] = []
	manifest = tomllib.loads(args.manifest.read_text(encoding="utf-8"))
	ledger = json.loads(args.ledger.read_text(encoding="utf-8"))
	inventory = json.loads(args.inventory.read_text(encoding="utf-8"))
	decision_packet = json.loads(args.decisions_json.read_text(encoding="utf-8"))
	ratification = tomllib.loads(args.ratification.read_text(encoding="utf-8"))
	builder = load_builder()

	if inventory.get("schema_version") != 1 or inventory.get("enumerator_version") != 3:
		errors.append("inventory schema/enumerator version is not schema 1 / enumerator 3")
	if ledger.get("schema_version") != 1:
		errors.append("ledger schema version is not 1")
	if inventory.get("repo_sha") != manifest["source"]["commit"]:
		errors.append("inventory repository pin differs from source manifest")
	if inventory.get("repository_url") != manifest["source"]["repository"]:
		errors.append("inventory repository URL differs from source manifest")
	if inventory.get("manifest_sha256") != sha256(args.manifest):
		errors.append("source manifest hash drift")
	if inventory.get("ledger_sha256") != sha256(args.ledger):
		errors.append("capability ledger hash drift")
	packet_ref = ledger.get("decision_packet", {})
	if packet_ref.get("machine_sha256") != sha256(args.decisions_json):
		errors.append("decision packet JSON hash drift")
	if packet_ref.get("markdown_sha256") != sha256(args.decisions_markdown):
		errors.append("decision packet Markdown hash drift")
	try:
		decision_json_relative = args.decisions_json.resolve().relative_to(ROOT).as_posix()
	except ValueError:
		decision_json_relative = "outside-cord"
	try:
		decision_markdown_relative = args.decisions_markdown.resolve().relative_to(ROOT).as_posix()
	except ValueError:
		decision_markdown_relative = "outside-cord"
	if packet_ref.get("machine_path") != decision_json_relative:
		errors.append("decision packet JSON path drift")
	if packet_ref.get("markdown_path") != decision_markdown_relative:
		errors.append("decision packet Markdown path drift")
	if decision_packet.get("version") != 1:
		errors.append("decision packet version is not 1")
	if decision_packet.get("upstream", {}).get("commit") != manifest["source"]["commit"]:
		errors.append("decision packet upstream pin drift")

	rows = ledger.get("rows", [])
	row_ids = [row.get("id") for row in rows]
	if not rows:
		errors.append("empty capability ledger")
	if len(row_ids) != len(set(row_ids)):
		errors.append("duplicate capability ledger row ID")
	if any(not isinstance(identifier, str) or not re.fullmatch(r"WSI-[A-Z0-9-]+", identifier) for identifier in row_ids):
		errors.append("invalid capability ledger row ID")
	for row in rows:
		missing = sorted(REQUIRED_ROW_FIELDS - set(row))
		if missing:
			errors.append(f"ledger row {row.get('id')} missing fields: {missing}")
		if row.get("disposition") not in ALLOWED_DISPOSITIONS:
			errors.append(f"ledger row {row.get('id')} has invalid disposition")
		if row.get("maturity") not in ALLOWED_MATURITY:
			errors.append(f"ledger row {row.get('id')} has invalid maturity")
		if row.get("disposition") == "approved-excluded" and not row.get("approved_adr"):
			errors.append(f"excluded ledger row {row.get('id')} lacks approved ADR")
		for field in REQUIRED_ROW_FIELDS - {"id", "selector", "classification_scope"}:
			if row.get(field) in (None, "", []):
				errors.append(f"ledger row {row.get('id')} has empty {field}")

	row_by_id = {row["id"]: row for row in rows if isinstance(row.get("id"), str)}
	decisions = {item["id"]: item for item in decision_packet.get("decisions", [])}
	for decision_id in ("WSI-GUIDANCE-DESIGN", "WSI-PROVIDER-BYTE-PLANE"):
		if decision_id not in decisions or decision_id not in row_by_id:
			errors.append(f"missing decision/ledger row: {decision_id}")
			continue
		decision = decisions[decision_id]
		row = row_by_id[decision_id]
		if (
			row.get("decision") != decision.get("decision")
			or row.get("decision_evidence") != decision.get("evidence")
			or row.get("decision_unblock_requires") != decision.get("unblock_requires")
			or row.get("blocking_conflict_resolved_as") != decision.get("blocking_conflict_resolved_as")
		):
			errors.append(f"decision packet evidence/resolution drift: {decision_id}")
	guidance = row_by_id.get("WSI-GUIDANCE-DESIGN", {})
	if (
		guidance.get("maturity") != "design-only"
		or guidance.get("disposition") != "refactor"
		or guidance.get("implementation_authority") != "none"
		or guidance.get("runtime_capability_claim") is not False
		or set(guidance.get("classification_scope", [])) != {"guidance", "design-only"}
	):
		errors.append("guidance/design decision packet is not applied exactly")
	provider = row_by_id.get("WSI-PROVIDER-BYTE-PLANE", {})
	provider_is_implemented = (
		provider.get("maturity") == "implemented" or provider.get("implementation_state") == "implemented"
	)
	if not provider_is_implemented and (
		provider.get("maturity") != "prototype-partial"
		or provider.get("disposition") != "replace-clean-room"
		or provider.get("implementation_state") != "not-implemented"
		or provider.get("parity_status") != "not-parity-complete"
		or provider.get("runtime_capability_claim") is not False
		or provider.get("frozen_minimum") != decisions.get("WSI-PROVIDER-BYTE-PLANE", {}).get("frozen_minimum")
	):
		errors.append("provider P0 decision packet is not applied exactly")
	if provider.get("implementation_authority") != decisions.get("WSI-PROVIDER-BYTE-PLANE", {}).get("implementation_authority"):
		errors.append("provider implementation authority differs from the decision packet")
	if provider.get("decision_implementation_claim_requires") != decisions.get("WSI-PROVIDER-BYTE-PLANE", {}).get("implementation_claim_requires"):
		errors.append("provider implementation claim requirements differ from the decision packet")
	if len(provider.get("required_p1_tests", [])) != 11:
		errors.append("provider row does not preserve all eleven required P1 tests")
	for test in provider.get("required_p1_tests", []):
		if not test.get("id") or test.get("phase") != "P1" or not test.get("requirement"):
			errors.append("provider row has an incomplete required P1 test")
	if (
		ledger.get("p0_decisions_resolved") is not True
		or ledger.get("p1_authorized") is not False
		or ledger.get("feature_complete") is not False
		or ledger.get("production_ready") is not False
	):
		errors.append("pending P0 ledger overclaims P1 authorization, completion or readiness")

	for row in rows:
		implemented = row.get("maturity") == "implemented" or row.get("implementation_state") == "implemented"
		if not implemented:
			continue
		symbols = row.get("cord_symbol_evidence", [])
		tests = row.get("cord_test_evidence", [])
		if not symbols or not tests:
			errors.append(f"implemented row {row['id']} lacks CORD symbol/test evidence")
			continue
		for evidence in symbols + tests:
			path = evidence.get("path", "") if isinstance(evidence, dict) else ""
			name = evidence.get("symbol") or evidence.get("test") if isinstance(evidence, dict) else None
			if not path or not name or Path(path).is_absolute() or not (ROOT / path).is_file():
				errors.append(f"implemented row {row['id']} has invalid CORD evidence")
				continue
			try:
				content = (ROOT / path).read_text(encoding="utf-8")
			except UnicodeDecodeError:
				errors.append(f"implemented row {row['id']} cites non-text CORD evidence")
				continue
			if name not in content:
				errors.append(f"implemented row {row['id']} cites absent CORD symbol/test")
		if row.get("id") == "WSI-PROVIDER-BYTE-PLANE":
			required_ids = {test["id"] for test in row.get("required_p1_tests", [])}
			passing_ids = {
				evidence.get("requirement_id") for evidence in tests
				if isinstance(evidence, dict) and evidence.get("status") == "pass"
			}
			if passing_ids != required_ids:
				errors.append("implemented provider row lacks passing evidence for every required P1 test")

	artifacts = inventory.get("artifacts", [])
	artifact_ids = [item.get("id") for item in artifacts]
	if len(artifact_ids) != len(set(artifact_ids)):
		errors.append("duplicate artifact ID")
	if artifact_ids != sorted(artifact_ids):
		errors.append("artifact render order is unstable")
	if inventory.get("artifact_ids") != artifact_ids or inventory.get("artifact_count") != len(artifacts):
		errors.append("artifact count/list mismatch")
	artifact_id_set = set(artifact_ids)
	for row in rows:
		for conflict in row.get("resolved_conflicts", []):
			claim = conflict.get("claim_artifact_id")
			counter = conflict.get("counterevidence_artifact_id")
			if claim == counter or claim not in artifact_id_set or counter not in artifact_id_set:
				errors.append(f"resolved ledger row {row.get('id')} has invalid conflict artifact IDs")
			if (
				conflict.get("decision_packet_markdown_sha256") != packet_ref.get("markdown_sha256")
				or conflict.get("decision_packet_machine_sha256") != packet_ref.get("machine_sha256")
				or not conflict.get("claim_evidence") or not conflict.get("counterevidence")
				or not conflict.get("resolution")
			):
				errors.append(f"resolved ledger row {row.get('id')} lost conflict evidence or packet binding")
	for row in rows:
		if row.get("maturity") != "blocked-conflict":
			continue
		conflicts = row.get("conflicts", [])
		if not conflicts:
			errors.append(f"blocked ledger row {row.get('id')} lacks conflict evidence")
		for conflict in conflicts:
			claim = conflict.get("claim_artifact_id")
			counter = conflict.get("counterevidence_artifact_id")
			if claim == counter or claim not in artifact_id_set or counter not in artifact_id_set:
				errors.append(f"blocked ledger row {row.get('id')} has invalid conflict artifact IDs")
			for field in ("claim_evidence", "counterevidence", "resolution_gate"):
				if not conflict.get(field):
					errors.append(f"blocked ledger row {row.get('id')} conflict lacks {field}")

	mapping_counts: Counter[str] = Counter()
	file_paths: set[str] = set()
	multi_classified = 0
	unmapped = 0
	bad_ids = 0
	bad_evidence = 0
	for item in artifacts:
		path = item.get("path", "")
		if (
			item.get("classification") not in builder.CLASSIFICATIONS
			or item.get("classification") != builder.classify(path)
		):
			multi_classified += 1
		if not isinstance(item.get("evidence"), list) or not item.get("evidence"):
			bad_evidence += 1
		matches = [row["id"] for row in rows if builder._matches(path, row["selector"])]
		if len(matches) != 1 or item.get("ledger_row_id") not in matches:
			unmapped += 1
		else:
			row = next(candidate for candidate in rows if candidate["id"] == matches[0])
			if item.get("classification") not in row.get("classification_scope", []):
				multi_classified += 1
			mapping_counts[matches[0]] += 1
		expected_id = builder.artifact_id(
			inventory.get("repo_sha", ""), path, item.get("kind", ""),
			item.get("qualified_symbol", ""), item.get("feature_predicate", ""),
		)
		if item.get("id") != expected_id:
			bad_ids += 1
		if item.get("kind") == "file":
			file_paths.add(path)

	missing_rows = sorted(set(row_ids) - set(mapping_counts))
	if missing_rows:
		errors.append(f"ledger rows have no artifacts: {missing_rows}")
	if multi_classified:
		errors.append(f"invalid/unclassified artifact classifications: {multi_classified}")
	if unmapped:
		errors.append(f"artifacts without exactly one row mapping: {unmapped}")
	if bad_ids:
		errors.append(f"unstable/invalid artifact IDs: {bad_ids}")
	if bad_evidence:
		errors.append(f"artifacts without explicit evidence: {bad_evidence}")

	source = args.source.resolve() if args.source else builder.default_source(args.manifest)
	try:
		entries, root = builder.census(source, manifest["source"]["commit"])
	except Exception as error:  # builder gives a precise fail-closed error
		errors.append(f"source census failed: {error}")
		entries, root = [], b""
	tracked_paths = {entry.path.decode("utf-8") for entry in entries}
	entry_by_path = {entry.path.decode("utf-8"): entry for entry in entries}
	bad_file_evidence = 0
	for item in artifacts:
		if item.get("kind") != "file" or item.get("path") not in entry_by_path:
			continue
		expected = [f"git:{entry_by_path[item['path']].object_id.hex()}"]
		if item.get("evidence") != expected:
			bad_file_evidence += 1
	if bad_file_evidence:
		errors.append(f"file artifacts with object evidence drift: {bad_file_evidence}")
	oracle_checks, oracle_missing = independent_semantic_oracle(source, artifacts)
	if oracle_missing:
		errors.append(f"independent semantic oracle omissions: {oracle_missing[:10]} (total {len(oracle_missing)})")
	unvisited = sorted(tracked_paths - file_paths)
	extra_files = sorted(file_paths - tracked_paths)
	if inventory.get("census_count") != len(entries) or inventory.get("census_root") != root.hex():
		errors.append("tracked census count/root drift")
	if unvisited:
		errors.append(f"unvisited tracked paths: {unvisited[:10]} (total {len(unvisited)})")
	if extra_files:
		errors.append(f"inventory file artifacts not in census: {extra_files[:10]} (total {len(extra_files)})")

	blocked_rows = sorted(row["id"] for row in rows if row.get("maturity") == "blocked-conflict")
	unknown = sum(1 for row in rows if row.get("maturity") not in ALLOWED_MATURITY)
	architect_id = ratification.get("architect_approval", "")
	critic_id = ratification.get("critic_approval", "")
	ratification_accepted = (
		ratification.get("status") == "accepted"
		and ratification.get("p0_gate_status") == "pass"
		and isinstance(architect_id, str) and architect_id.startswith("P0-ARCH-") and "BLOCK" not in architect_id
		and isinstance(critic_id, str) and critic_id.startswith("P0-CRITIC-") and "BLOCK" not in critic_id
		and ratification.get("ledger_sha256") == sha256(args.ledger)
		and ratification.get("inventory_sha256") == sha256(args.inventory)
	)
	p1_authorized = not errors and not blocked_rows and ratification_accepted
	summary: dict[str, object] = {
		"schema_version": 1,
		"status": "fail" if errors else "blocked" if blocked_rows else "pass" if p1_authorized else "pending-ratification",
		"phase": "P0",
		"repo_sha": inventory.get("repo_sha"),
		"census_count": len(entries),
		"census_root": root.hex(),
		"artifact_count": len(artifacts),
		"semantic_oracle_checks": oracle_checks,
		"semantic_oracle_missing": len(oracle_missing),
		"ledger_rows": len(rows),
		"unvisited": len(unvisited),
		"missing": len(extra_files),
		"duplicate": len(artifact_ids) - len(set(artifact_ids)),
		"multi_classified": multi_classified,
		"unmapped": unmapped,
		"unknown": unknown,
		"blocked": len(blocked_rows),
		"blocked_rows": blocked_rows,
		"p0_decisions_resolved": not errors and not blocked_rows,
		"ratification_accepted": ratification_accepted,
		"architect_approval": architect_id,
		"critic_approval": critic_id,
		"p1_authorized": p1_authorized,
		"authorization_scope": "P1 unauthorized pending accepted independent ratification" if not p1_authorized else "P1 may begin only",
		"feature_complete": False,
		"production_ready": False,
		"errors": errors,
	}
	if errors:
		return summary, 1
	if blocked_rows and not args.p0_audit:
		return summary, 2
	return summary, 0


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument("--manifest", type=Path, default=ROOT / "docs/specs/web3-storage-upstream-sources-v1.toml")
	parser.add_argument("--ledger", type=Path, default=ROOT / "docs/specs/web3-storage-capability-ledger-v1.json")
	parser.add_argument("--inventory", type=Path, default=ROOT / "docs/specs/web3-storage-upstream-inventory-v1.json")
	parser.add_argument("--decisions-json", type=Path, default=ROOT / "docs/specs/web3-storage-blocked-conflicts-v1.json")
	parser.add_argument("--decisions-markdown", type=Path, default=ROOT / "docs/specs/web3-storage-blocked-conflicts-v1.md")
	parser.add_argument("--ratification", type=Path, default=ROOT / "docs/specs/p0-ratification-v1.toml")
	parser.add_argument("--source", "--repository", dest="source", type=Path)
	parser.add_argument(
		"--p0-audit", action="store_true",
		help="return zero for a structurally complete P0 ledger while reporting implementation_authorized=false",
	)
	parser.add_argument("--out", type=Path)
	args = parser.parse_args()
	try:
		summary, code = validate(args)
	except (OSError, KeyError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
		summary, code = {"schema_version": 1, "status": "fail", "errors": [str(error)]}, 1
	encoded = json.dumps(summary, indent=2, sort_keys=True) + "\n"
	if args.out:
		args.out.parent.mkdir(parents=True, exist_ok=True)
		temporary = args.out.with_suffix(args.out.suffix + ".tmp")
		temporary.write_text(encoded, encoding="utf-8")
		temporary.replace(args.out)
	print(encoded, end="")
	return code


if __name__ == "__main__":
	raise SystemExit(main())

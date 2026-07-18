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

"""Prove disposition-driven storage and Identity deletion mechanically."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json, canonical_bytes, sha256_bytes, sha256_file


HISTORICAL_ROOTS = ("docs/adr/", "docs/evidence/", "docs/specs/")
ITEM_FIELDS = {
    "id", "path", "symbol", "kind", "replacement_consumer", "replacement_owner",
    "replacement_test", "replacement_gate", "slice", "order", "status",
    "prerequisites", "post_delete_scans", "surface_locator", "owns_obsolete",
}
ITEM_KINDS = {
    "public-route", "host-operation", "provider-route", "provider-worker", "runtime-config",
    "runtime-api", "runtime-api-method", "signed-extension", "node-service", "pallet-call", "pallet-event",
    "pallet-storage", "cargo-crate", "runtime-pallet-index", "generated-descriptor", "fixture",
    "public-package", "contract-fixture",
    "obsolete-reference", "obsolete-file",
}
SLICES = {"P4_OBJECT", "P4_PROVIDER", "P4_DRIVE", "P4_S3", "P4_RUNTIME", "P5"}
TEXT_SUFFIXES = {
    ".rs", ".toml", ".ts", ".tsx", ".js", ".mjs", ".json", ".md", ".yml", ".yaml",
    ".sh", ".py", ".graphql", ".proto", ".sol", ".contract",
}


def tracked_and_untracked(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return sorted(
        {value.decode("utf-8", "strict") for value in result.stdout.split(b"\0") if value},
        key=lambda value: value.encode("utf-8"),
    )


def allowlist(manifest: dict[str, Any], root: Path) -> tuple[set[tuple[str, int, str]], list[str]]:
    allowed: set[tuple[str, int, str]] = set()
    errors: list[str] = []
    for row in manifest.get("historical_allow", []):
        path = row.get("path", "")
        if not path.startswith(HISTORICAL_ROOTS):
            errors.append(f"historical allowlist path is active/compiled: {path}")
            continue
        absolute = root / path
        if not absolute.is_file():
            errors.append(f"historical allowlist path is missing: {path}")
            continue
        lines = absolute.read_text(encoding="utf-8", errors="replace").splitlines()
        start = int(row.get("line_start", 0))
        end = int(row.get("line_end", 0))
        item_id = row.get("item_id", "")
        if start < 1 or end < start or end > len(lines):
            errors.append(f"invalid historical line range: {path}:{start}-{end}")
            continue
        for line in range(start, end + 1):
            allowed.add((path, line, item_id))
    return allowed, errors


def cargo_packages(root: Path) -> set[str]:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode("utf-8", "replace"))
    return {package["name"] for package in json.loads(result.stdout)["packages"]}


def npm_packages(root: Path, paths: list[str]) -> set[str]:
    names = set()
    for path in paths:
        if Path(path).name != "package.json":
            continue
        try:
            document = json.loads((root / path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(document.get("name"), str):
            names.add(document["name"])
    return names


def exact_surface_exists(row: dict[str, Any], root: Path) -> bool:
    path = root / row["path"]
    if not path.is_file():
        return False
    locator = row["surface_locator"]
    symbol = row["symbol"]
    if locator == "file":
        return symbol == f"file:{row['path']}"
    text = path.read_text(encoding="utf-8", errors="replace")
    if locator == "token":
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", symbol):
            return re.search(rf"(?<![A-Za-z0-9_]){re.escape(symbol)}(?![A-Za-z0-9_])", text) is not None
        return symbol in text
    if locator == "cargo-package":
        try:
            return tomllib.loads(text).get("package", {}).get("name") == symbol
        except tomllib.TOMLDecodeError:
            return False
    if locator == "rust-pallet-call":
        match = re.fullmatch(r"Pallet::call\[(\d+)\]::([A-Za-z_][A-Za-z0-9_]*)", symbol)
        return bool(match and re.search(
            rf"#\[pallet::call_index\({match.group(1)}\)\][\s\S]{{0,300}}?pub fn\s+{re.escape(match.group(2))}\b",
            text,
        ))
    if locator == "rust-pallet-event":
        name = symbol.removeprefix("Event::")
        return symbol.startswith("Event::") and re.search(rf"(?m)^\s*{re.escape(name)}\s*(?:\{{|,)", text) is not None
    if locator == "rust-pallet-storage":
        name = symbol.removeprefix("Storage::")
        return symbol.startswith("Storage::") and re.search(
            rf"#\[pallet::storage\][\s\S]{{0,400}}?pub(?:\(super\))?\s+type\s+{re.escape(name)}\b",
            text,
        ) is not None
    if locator == "rust-symbol":
        return re.search(rf"\b(?:fn|struct|enum|trait|type)\s+{re.escape(symbol)}\b", text) is not None
    if locator == "http-route":
        method, separator, route = symbol.partition(" ")
        return bool(separator and re.search(
            rf"\(Method::{re.escape(method)},\s*\"{re.escape(route)}\"\)", text
        ))
    if locator == "rust-runtime-api-method":
        trait, separator, method = symbol.rpartition("::")
        return bool(separator and re.search(
            rf"impl\s+{re.escape(trait)}(?:<[^{{]+)?\s+for\s+Runtime\s*\{{[\s\S]*?"
            rf"(?m:^\s*fn\s+{re.escape(method)}\s*\()",
            text,
        ))
    if locator == "ts-export":
        name = symbol.removeprefix("export::")
        return symbol.startswith("export::") and re.search(
            rf"(?m)^export\s+(?:interface|type|const|function|class)\s+{re.escape(name)}\b",
            text,
        ) is not None
    if locator == "rust-export":
        name = symbol.removeprefix("rust-export::")
        return symbol.startswith("rust-export::") and re.search(
            rf"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+{re.escape(name)}\s*\(", text
        ) is not None
    if locator == "ts-host-route":
        object_name, separator, method = symbol.partition("::")
        return bool(separator and re.search(
            rf"export const\s+{re.escape(object_name)}\s*=\s*\{{[\s\S]*?"
            rf"(?m:^\s*{re.escape(method)}\s*\()",
            text,
        ))
    if locator == "ts-host-operation":
        return re.search(rf'(?m)^\s*"{re.escape(symbol)}"\s*:', text) is not None
    return False


def source_paths(root: Path) -> list[str]:
    """Return current repository source paths without consulting deletion declarations."""
    try:
        return tracked_and_untracked(root)
    except (OSError, subprocess.SubprocessError):
        ignored = {".git", "target", "node_modules", "dist", "__pycache__"}
        return sorted(
            path.relative_to(root).as_posix()
            for path in root.rglob("*")
            if path.is_file() and not any(part in ignored for part in path.relative_to(root).parts)
        )


def rust_block(text: str, opening: int) -> str:
    brace = text.index("{", opening)
    depth = 0
    for cursor in range(brace, len(text)):
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        if depth == 0:
            return text[brace + 1:cursor]
    raise ValueError("unclosed Rust block")


def current_surface_census(root: Path) -> set[tuple[str, str, str]]:
    """Independently enumerate every incumbent deletion surface from current sources."""
    surfaces: set[tuple[str, str, str]] = set()
    paths = source_paths(root)

    # Token/file/package incumbents are derived from source contents and package manifests,
    # independently of [[item]] declarations.
    token_categories = {
        "public-package": {
            "tokens": ("PeopleLite", "personhood", "resources"),
            "roots": (
                "product-sdk/**", "origin-rs/src/product_sdk/**", "docs/Developer.md",
                "docs/cord-features.md", "docs/architecture/domains/**",
            ),
        },
        "obsolete-reference": {
            "tokens": ("HopPromotion", "TransactionStorage"),
            "roots": ("origin/**", "product-sdk/**", "origin-rs/**", "node/**"),
        },
    }
    for relative in paths:
        path = root / relative
        if path.suffix not in TEXT_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for kind, category in token_categories.items():
            if not any(fnmatch.fnmatchcase(relative, pattern) for pattern in category["roots"]):
                continue
            for token in category["tokens"]:
                if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", text):
                    surfaces.add((relative, kind, token))

    incumbent_roots = (
        "origin/orbis/pallets/hop-promotion/",
        "origin/orbis/pallets/transaction-storage/",
    )
    for relative in paths:
        if relative.startswith(incumbent_roots):
            surfaces.add((relative, "obsolete-file", f"file:{relative}"))
            if Path(relative).name == "Cargo.toml":
                try:
                    name = tomllib.loads((root / relative).read_text())["package"]["name"]
                except (OSError, KeyError, tomllib.TOMLDecodeError):
                    continue
                surfaces.add((relative, "cargo-crate", name))

    pallet_path = "origin/orbis/pallets/transaction-storage/src/lib.rs"
    pallet_source = root / pallet_path
    pallet = pallet_source.read_text(encoding="utf-8") if pallet_source.exists() else ""
    for match in re.finditer(
        r"#\[pallet::call_index\((\d+)\)\][\s\S]{0,300}?pub fn\s+([A-Za-z_][A-Za-z0-9_]*)",
        pallet,
    ):
        index, name = match.groups()
        surfaces.add((pallet_path, "pallet-call", f"Pallet::call[{index}]::{name}"))
    event_start = pallet.find("pub enum Event<T: Config>")
    body = rust_block(pallet, event_start) if event_start >= 0 else ""
    depth = 0
    for line in re.sub(r"///.*", "", body).splitlines():
        stripped = line.strip()
        if depth == 0:
            match = re.match(r"([A-Z][A-Za-z0-9_]*)\s*(?:\{|,)", stripped)
            if match:
                surfaces.add((pallet_path, "pallet-event", f"Event::{match.group(1)}"))
        depth += line.count("{") - line.count("}")
    for match in re.finditer(
        r"#\[pallet::storage\][\s\S]{0,400}?pub(?:\(super\))?\s+type\s+([A-Za-z_][A-Za-z0-9_]*)",
        pallet,
    ):
        surfaces.add((pallet_path, "pallet-storage", f"Storage::{match.group(1)}"))

    runtime_path = "origin/orbis/runtime/src/lib.rs"
    runtime = (root / runtime_path).read_text(encoding="utf-8")
    for name, crate, index in re.findall(
        r"(?m)^\s*(TransactionStorage|HopPromotion):\s*([A-Za-z0-9_]+)\s*=\s*(\d+),",
        runtime,
    ):
        surfaces.add((runtime_path, "runtime-pallet-index", f"{name}: {crate} = {index}"))
    for match in re.finditer(
        r"impl\s+((?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*StorageApi[A-Za-z0-9_]*)"
        r"(?:<[^\{]+)?\s+for\s+Runtime\s*\{",
        runtime,
    ):
        trait = match.group(1)
        surfaces.add((runtime_path, "runtime-api", trait))
        for method in re.findall(r"(?m)^\s*fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", rust_block(runtime, match.start())):
            surfaces.add((runtime_path, "runtime-api-method", f"{trait}::{method}"))
    for alias, target in re.findall(
        r"(?m)^\s*type\s+(LongTermStorageDataStore[A-Za-z0-9_]*)\s*=\s*(TransactionStorage)\s*;",
        runtime,
    ):
        surfaces.add((runtime_path, "runtime-config", f"{alias} = {target}"))
    if re.search(r"pallet_orbis_transaction_storage::extension::ValidateStorageCalls\s*<", runtime):
        surfaces.add((runtime_path, "signed-extension", "ValidateStorageCalls"))

    extension_path = "origin/orbis/pallets/transaction-storage/src/extension.rs"
    extension_source = root / extension_path
    if extension_source.exists():
        extension = extension_source.read_text(encoding="utf-8")
        for name in re.findall(
            r"(?m)^pub\s+(?:struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)",
            extension,
        ):
            surfaces.add((extension_path, "signed-extension", name))

    proof_root = root / "origin/orbis/node/src/proof_campaign"
    for path in sorted(proof_root.glob("*.rs")):
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(
            r"(?m)^\s*pub\s+(?:async\s+)?(?:fn|struct|enum|trait)\s+([A-Za-z_][A-Za-z0-9_]*)",
            text,
        ):
            surfaces.add((relative, "node-service", match.group(1)))

    api_path = "origin/orbis/provider-node/src/api.rs"
    api = (root / api_path).read_text(encoding="utf-8")
    for method, route in re.findall(r"\(Method::([A-Z]+),\s*\"([^\"]+)\"\)", api):
        surfaces.add((api_path, "provider-route", f"{method} {route}"))

    descriptor_root = root / "product-sdk/packages/descriptors/generated"
    for path in descriptor_root.glob("*"):
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            surfaces.add((relative, "generated-descriptor", f"file:{relative}"))

    # Public TS SDK exports of the two incumbent facades are deletion surfaces.
    for relative in (
        "product-sdk/packages/origin-sdk-personhood/src/index.ts",
        "product-sdk/packages/origin-sdk-resources/src/index.ts",
    ):
        path = root / relative
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for name in re.findall(
            r"(?m)^export\s+(?:interface|type|const|function|class)\s+([A-Za-z_][A-Za-z0-9_]*)",
            text,
        ):
            surfaces.add((relative, "public-route", f"export::{name}"))

    rust_sdk_path = "origin-rs/src/product_sdk/transport.rs"
    rust_sdk = (root / rust_sdk_path).read_text(encoding="utf-8")
    for name in re.findall(
        r"(?m)^pub\s+(?:async\s+)?fn\s+((?:read|submit|prepare|identity_)[A-Za-z0-9_]*(?:personhood|storage)[A-Za-z0-9_]*)",
        rust_sdk,
    ):
        surfaces.add((rust_sdk_path, "public-route", f"rust-export::{name}"))
    for trait_body in re.findall(r"pub trait FinalizedReadBinding[^\{]*\{([\s\S]*?)\n\}", rust_sdk):
        for name in re.findall(r"(?m)^\s*async fn\s+([A-Za-z0-9_]*(?:personhood|storage)[A-Za-z0-9_]*)", trait_body):
            surfaces.add((rust_sdk_path, "public-route", f"rust-export::{name}"))

    host_routes_path = "product-sdk/packages/descriptors/src/identity-host-routes.ts"
    host_routes_file = root / host_routes_path
    host_routes = host_routes_file.read_text(encoding="utf-8") if host_routes_file.is_file() else ""
    marker = host_routes.find("export const personhoodHostRoutes")
    if marker >= 0:
        for name in re.findall(r"(?m)^\s{2}([A-Za-z_][A-Za-z0-9_]*)\(", rust_block(host_routes, marker)):
            surfaces.add((host_routes_path, "host-operation", f"personhoodHostRoutes::{name}"))
    host_protocol_path = "product-sdk/packages/origin-sdk-host/src/protocol.ts"
    host_protocol = (root / host_protocol_path).read_text(encoding="utf-8")
    for operation in re.findall(r'^\s*"((?:resources|personhood)[A-Za-z0-9_.-]*)"\s*:', host_protocol, re.MULTILINE):
        surfaces.add((host_protocol_path, "host-operation", operation))

    # Fixture/contract/precompile incumbents are independently scanned even though the
    # current clean P0 census contains no such active file surface.
    fixture_roots = (
        "product-sdk/**/fixtures/**", "product-sdk/**/test/**", "origin-rs/tests/**",
        "origin/**/tests/**",
    )
    for relative in paths:
        lowered = relative.lower()
        if not any(fnmatch.fnmatchcase(relative, pattern) for pattern in fixture_roots):
            continue
        path = root / relative
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        if re.search(r"(?i)\b(?:bulletin|preimage|retention)\b", text):
            kind = "contract-fixture" if path.suffix in {".sol", ".contract"} or "precompile" in lowered else "fixture"
            surfaces.add((relative, kind, f"file:{relative}"))
    return surfaces


# Kept as a compatibility alias for downstream tests; it is now the complete census.
def current_specialized_surfaces(root: Path) -> set[tuple[str, str, str]]:
    return current_surface_census(root)


def deletion_dag(
    manifest: dict[str, Any], root: Path, ledger_path: Path | None
) -> dict[str, Any]:
    items = manifest.get("item", [])
    missing_fields = []
    invalid_items = []
    identifiers = [row.get("id") for row in items]
    duplicate_ids = len(identifiers) - len(set(identifiers))
    by_id = {row.get("id"): row for row in items if row.get("id")}
    orders = [row.get("order") for row in items]
    duplicate_orders = len(orders) - len(set(orders))
    surface_keys = [
        (row.get("path"), row.get("kind"), row.get("symbol")) for row in items
    ]
    duplicate_surfaces = len(surface_keys) - len(set(surface_keys))
    absent_symbols = []
    invalid_edges = []
    for row in items:
        item_id = row.get("id", "<missing>")
        absent = sorted(ITEM_FIELDS - row.keys())
        if absent:
            missing_fields.append({"id": item_id, "fields": absent})
            continue
        if (
            row["kind"] not in ITEM_KINDS
            or row["slice"] not in SLICES
            or not re.fullmatch(r"AC(?:[1-9]|1[0-4])", row["replacement_gate"])
            or not isinstance(row["order"], int)
            or row["order"] < 1
            or row["status"] not in ("pending-delete", "deleted", "replacement-active")
            or any(token in row["path"] for token in ("*", "?", "["))
            or not all(isinstance(value, str) and value for value in (
                row["path"], row["symbol"], row["replacement_consumer"],
                row["replacement_owner"], row["replacement_test"],
            ))
        ):
            invalid_items.append(item_id)
        if (
            not isinstance(row["owns_obsolete"], list)
            or not all(isinstance(value, str) and value for value in row["owns_obsolete"])
        ):
            invalid_items.append(item_id)
        scans = set(row["post_delete_scans"])
        required_scans = {
            f"path:{row['path']}", f"symbol:{row['symbol']}",
            f"consumer:{row['replacement_consumer']}",
        }
        if not required_scans.issubset(scans):
            invalid_items.append(item_id)
        if row["status"] in ("pending-delete", "replacement-active") and not (root / row["path"]).exists():
            invalid_items.append(item_id)
        elif row["status"] in ("pending-delete", "replacement-active") and not exact_surface_exists(row, root):
            absent_symbols.append(item_id)
        elif row["status"] == "deleted" and exact_surface_exists(row, root):
            invalid_items.append(item_id)
        for dependency in row["prerequisites"]:
            target = by_id.get(dependency)
            if target is None or target.get("order", row["order"]) >= row["order"]:
                invalid_edges.append({"item": item_id, "prerequisite": dependency})

    visiting: set[str] = set()
    visited: set[str] = set()
    cycles = []

    def visit(item_id: str, trail: list[str]) -> None:
        if item_id in visiting:
            cycles.append(trail + [item_id])
            return
        if item_id in visited or item_id not in by_id:
            return
        visiting.add(item_id)
        for dependency in by_id[item_id].get("prerequisites", []):
            visit(dependency, trail + [item_id])
        visiting.remove(item_id)
        visited.add(item_id)

    for item_id in sorted(by_id):
        visit(item_id, [])

    declared_surfaces = {
        (row.get("path"), row.get("kind"), row.get("symbol"))
        for row in items
        if row.get("status") in ("pending-delete", "replacement-active")
    }
    observed_surfaces = current_surface_census(root)
    unmapped_surfaces = sorted(observed_surfaces - declared_surfaces)
    declaration_only_surfaces = sorted(declared_surfaces - observed_surfaces)

    decision_rows = {row.get("id"): row for row in manifest.get("decision_reference", [])}
    exclusion_errors = []
    if ledger_path is not None:
        ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
        for row in ledger.get("rows", []):
            if row.get("disposition") != "approved-excluded":
                continue
            reference_id = row.get("approved_adr")
            reference = decision_rows.get(reference_id)
            if not reference:
                exclusion_errors.append(f"{row.get('id')}: missing decision reference {reference_id}")
                continue
            adr_path = root / reference["adr_path"]
            try:
                text = adr_path.read_text(encoding="utf-8")
            except OSError as exception:
                exclusion_errors.append(f"{row.get('id')}: {exception}")
                continue
            status_accepted = re.search(r"(?ms)^## Status\s+Accepted\.", text) is not None
            subdecision_exists = re.search(
                rf"(?m)^### {re.escape(reference['subdecision'])}$", text
            ) is not None
            if not status_accepted or not subdecision_exists:
                exclusion_errors.append(
                    f"{row.get('id')}: decision {reference_id} is not an existing Accepted subdecision"
                )
    surface_inventory = sorted(
        ({"path": path, "kind": kind, "symbol": symbol} for path, kind, symbol in observed_surfaces),
        key=lambda row: (row["path"], row["kind"], row["symbol"]),
    )
    return {
        "deletion_item_count": len(items),
        "deleted_item_count": sum(row.get("status") == "deleted" for row in items),
        "replacement_active_item_count": sum(row.get("status") == "replacement-active" for row in items),
        "pending_delete_item_count": sum(row.get("status") == "pending-delete" for row in items),
        "dag_missing_fields": len(missing_fields),
        "dag_duplicate_ids": duplicate_ids,
        "dag_duplicate_orders": duplicate_orders,
        "dag_invalid_items": len(set(invalid_items)),
        "dag_invalid_edges": len(invalid_edges),
        "dag_cycle_count": len(cycles),
        "absent_symbol_count": len(absent_symbols),
        "duplicate_surface_count": duplicate_surfaces,
        "unmapped_surface_count": len(unmapped_surfaces),
        "declaration_only_surface_count": len(declaration_only_surfaces),
        "observed_surface_count": len(observed_surfaces),
        "surface_inventory_sha256": sha256_bytes(canonical_bytes(surface_inventory)),
        "approved_exclusion_invalid": len(exclusion_errors),
        "dag_details": {
            "missing_fields": missing_fields,
            "invalid_items": sorted(set(invalid_items)),
            "invalid_edges": invalid_edges,
            "cycles": cycles,
            "absent_symbols": sorted(absent_symbols),
            "unmapped_surfaces": [
                {"path": path, "kind": kind, "symbol": symbol}
                for path, kind, symbol in unmapped_surfaces
            ],
            "declaration_only_surfaces": [
                {"path": path, "kind": kind, "symbol": symbol}
                for path, kind, symbol in declaration_only_surfaces
            ],
            "exclusion_errors": exclusion_errors,
        },
    }

def ownership_audit(
    manifest: dict[str, Any], findings: list[dict[str, Any]]
) -> dict[str, Any]:
    items = manifest.get("item", [])
    unmapped = []
    duplicate = []
    assignments = []
    for index, finding in enumerate(findings):
        candidates = [
            row["id"] for row in items
            if finding["item_id"] in row.get("owns_obsolete", [])
            and (
                not finding.get("path")
                or row.get("path") == finding.get("path")
            )
        ]
        if not candidates:
            unmapped.append(index)
        elif len(candidates) > 1:
            duplicate.append({"finding": index, "owners": sorted(candidates)})
        else:
            assignments.append({"finding": index, "owner": candidates[0]})
    return {
        "unmapped_finding_count": len(unmapped),
        "duplicate_owner_count": len(duplicate),
        "finding_assignment_count": len(assignments),
        "finding_assignment_sha256": sha256_bytes(canonical_bytes(assignments)),
        "ownership_details": {
            "unmapped_findings": unmapped,
            "duplicate_owners": duplicate,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--ledger", type=Path)
    parser.add_argument("--p0-audit", action="store_true")
    args = parser.parse_args()

    root = args.root.resolve()
    manifest = tomllib.loads(args.manifest.read_text(encoding="utf-8"))
    paths = tracked_and_untracked(root)
    allowed, allowlist_errors = allowlist(manifest, root)
    dag = deletion_dag(manifest, root, args.ledger)
    findings: list[dict[str, Any]] = []
    packages: set[str] | None = None
    npm: set[str] | None = None

    for item in manifest.get("obsolete", []):
        item_id = item["id"]
        kind = item["kind"]
        value = item["value"]
        roots = item.get("roots", ["**"])
        if kind == "cargo-package":
            packages = packages if packages is not None else cargo_packages(root)
            if value in packages:
                findings.append({"item_id": item_id, "kind": kind, "value": value})
            continue
        if kind == "npm-package":
            npm = npm if npm is not None else npm_packages(root, paths)
            if value in npm:
                findings.append({"item_id": item_id, "kind": kind, "value": value})
            continue
        if kind == "path":
            for path in paths:
                if fnmatch.fnmatchcase(path, value):
                    findings.append({"item_id": item_id, "kind": kind, "path": path})
            continue
        if kind not in ("symbol", "fixture-token", "public-taxonomy"):
            allowlist_errors.append(f"unknown obsolete kind {kind} for {item_id}")
            continue
        expression = re.compile(rf"(?<![A-Za-z0-9_]){re.escape(value)}(?![A-Za-z0-9_])")
        for path in paths:
            if Path(path).suffix not in TEXT_SUFFIXES:
                continue
            if not any(fnmatch.fnmatchcase(path, pattern) for pattern in roots):
                continue
            try:
                lines = (root / path).read_text(encoding="utf-8").splitlines()
            except (OSError, UnicodeDecodeError):
                continue
            for number, line in enumerate(lines, 1):
                if expression.search(line) and (path, number, item_id) not in allowed:
                    findings.append(
                        {
                            "item_id": item_id,
                            "kind": kind,
                            "path": path,
                            "line": number,
                            "line_sha256": __import__("hashlib").sha256(line.encode("utf-8")).hexdigest(),
                        }
                    )

    counts = {kind: 0 for kind in ("symbol", "fixture-token", "public-taxonomy", "path", "cargo-package", "npm-package")}
    for finding in findings:
        counts[finding["kind"]] += 1
    ownership = ownership_audit(manifest, findings)
    structural_failures = sum(
        dag[key] for key in (
            "dag_missing_fields", "dag_duplicate_ids", "dag_duplicate_orders", "dag_invalid_items",
            "dag_invalid_edges", "dag_cycle_count", "absent_symbol_count",
            "duplicate_surface_count", "unmapped_surface_count", "declaration_only_surface_count",
            "approved_exclusion_invalid",
        )
    ) + ownership["unmapped_finding_count"] + ownership["duplicate_owner_count"] + len(allowlist_errors)
    if not args.p0_audit:
        structural_failures += dag["pending_delete_item_count"]
    stale_failures = len(findings)
    passed = structural_failures == 0 and (args.p0_audit or stale_failures == 0)
    result = {
        "schema_version": 1,
        "status": "pass" if passed else "blocked",
        "manifest_sha256": sha256_file(args.manifest),
        "obsolete_symbol_count": counts["symbol"],
        "obsolete_fixture_count": counts["fixture-token"] + counts["path"],
        "public_old_taxonomy_count": counts["public-taxonomy"],
        "deleted_dependency_count": counts["cargo-package"] + counts["npm-package"],
        "allowlist_invalid": len(allowlist_errors),
        "finding_count": len(findings),
        "findings": findings,
        "allowlist_errors": allowlist_errors,
        **ownership,
        **dag,
    }
    atomic_write_json(args.out, result)
    print(f"{result['status'].upper()} deletion proof: {len(findings)} live finding(s)")
    return 0 if args.p0_audit or result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())

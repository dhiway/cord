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

"""Validate the P0 Foundation/Commons SDK platform freeze without third-party modules."""

from __future__ import annotations

import ast
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "docs/sdk/app-platform-baseline.toml"
CAPABILITIES = ROOT / "docs/sdk/app-platform-capability-matrix.toml"
PACKAGES = ROOT / "docs/sdk/package-architecture.toml"


def fail(message: str) -> None:
    raise SystemExit(f"FAIL sdk platform contract: {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def scalar(text: str, key: str) -> str:
    match = re.search(rf'^{re.escape(key)}\s*=\s*"([^"]*)"\s*$', text, re.MULTILINE)
    if not match:
        fail(f"missing string scalar {key}")
    return match.group(1)


def table_blocks(text: str, table: str) -> list[str]:
    return re.findall(
        rf'^\[\[{re.escape(table)}\]\]\n(.*?)(?=^\[\[|^\[[^\[]|\Z)',
        text,
        re.MULTILINE | re.DOTALL,
    )


def array(block: str, key: str) -> list[str]:
    match = re.search(rf'^{re.escape(key)}\s*=\s*(\[[^\n]*\])\s*$', block, re.MULTILINE)
    if not match:
        fail(f"missing array {key}")
    value = ast.literal_eval(match.group(1))
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        fail(f"invalid string array {key}")
    return value


def integer(block: str, key: str) -> int:
    match = re.search(rf'^{re.escape(key)}\s*=\s*([0-9]+)\s*$', block, re.MULTILINE)
    if not match:
        fail(f"missing integer {key}")
    return int(match.group(1))


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


baseline = BASELINE.read_text()
if scalar(baseline, "branch") != "sm-update-sub-0x65":
    fail("baseline branch drift")
if git("branch", "--show-current") != scalar(baseline, "branch"):
    fail("wrong working branch")
freeze = scalar(baseline, "freeze_commit")
if subprocess.run(
    ["git", "merge-base", "--is-ancestor", freeze, "HEAD"], cwd=ROOT, check=False
).returncode:
    fail("freeze commit is not an ancestor of HEAD")
for field, relative in (
    ("cargo_toml_sha256", "Cargo.toml"),
    ("cargo_lock_sha256", "Cargo.lock"),
    ("product_sdk_package_sha256", "product-sdk/package.json"),
    ("product_sdk_lock_sha256", "product-sdk/package-lock.json"),
):
    if scalar(baseline, field) != sha256(ROOT / relative):
        fail(f"{relative} drift")

manifest = (ROOT / "docs/orbis-completion-manifest.toml").read_text()
if not re.search(r'^manifest_version\s*=\s*22$', manifest, re.MULTILINE):
    fail("completion manifest version is not 22")
if scalar(manifest, "implementation_branch") != "sm-update-sub-0x65":
    fail("completion manifest branch drift")

metadata_identity = json.loads((ROOT / "docs/sdk/metadata/commons-v29.json").read_text())
metadata_path = ROOT / metadata_identity["scale_path"]
if metadata_identity.get("runtime_metadata_version") != 14:
    fail("Commons metadata version is not V14")
if metadata_path.stat().st_size != metadata_identity.get("scale_bytes"):
    fail("Commons SCALE metadata size drift")
if sha256(metadata_path) != metadata_identity.get("scale_sha256"):
    fail("Commons SCALE metadata digest drift")
papi = json.loads(
    (ROOT / "product-sdk/packages/descriptors/generated/commons-papi-manifest.json").read_text()
)
if papi.get("schema") != "cord.commons-papi-descriptor.v1":
    fail("Commons PAPI descriptor schema drift")
if papi.get("metadata", {}).get("scale_sha256") != metadata_identity.get("scale_sha256"):
    fail("Commons PAPI metadata binding drift")
descriptor = json.loads(
    (ROOT / "product-sdk/packages/descriptors/generated/orbis-descriptor.json").read_text()
)
if descriptor.get("productionPapiDescriptorGenerated") is not True:
    fail("Commons PAPI descriptor is not marked generated")

routes = json.loads((ROOT / "docs/sdk/native-route-contract.json").read_text())
expected_route_count = 138
if routes.get("route_count") != expected_route_count:
    fail(f"native route count is not {expected_route_count}")
if len(routes.get("routes", [])) != expected_route_count:
    fail("native route inventory length does not match route_count")
if len({route.get("id") for route in routes["routes"]}) != expected_route_count:
    fail("native route IDs are not unique")
for relative in ("product-sdk/README.md", "docs/sdk/README.md", "docs/sdk/native-contract.md"):
    content = (ROOT / relative).read_text()
    if re.search(r'\b132\b', content):
        fail(f"stale 132-route claim in {relative}")

capability_text = CAPABILITIES.read_text()
capability_blocks = table_blocks(capability_text, "capability")
if len(capability_blocks) != 16:
    fail(f"expected 16 capability rows, found {len(capability_blocks)}")
capability_ids = [scalar(block, "id") for block in capability_blocks]
if len(capability_ids) != len(set(capability_ids)):
    fail("duplicate capability ID")
for block in capability_blocks:
    for field in (
        "requirement",
        "journey",
        "runtime_owner",
        "sdk_owner",
        "host_owner",
        "state",
        "contract_policy",
        "evidence",
    ):
        value = scalar(block, field)
        if not value or value.lower() in {"unknown", "tbd", "todo"}:
            fail(f"capability {scalar(block, 'id')} has unresolved {field}")

package_text = PACKAGES.read_text()
package_blocks = table_blocks(package_text, "package")
package_names = [scalar(block, "name") for block in package_blocks]
if len(package_names) != 24 or len(package_names) != len(set(package_names)):
    fail(f"target package inventory drift: {len(package_names)}")
package_set = set(package_names)
graph: dict[str, list[str]] = {}
for block in package_blocks:
    name = scalar(block, "name")
    if not scalar(block, "owner"):
        fail(f"package {name} has no owner")
    dependencies = array(block, "dependencies")
    missing = set(dependencies) - package_set
    if missing:
        fail(f"package {name} has unknown dependencies: {sorted(missing)}")
    graph[name] = dependencies

visiting: set[str] = set()
visited: set[str] = set()


def visit(name: str) -> None:
    if name in visiting:
        fail(f"package dependency cycle at {name}")
    if name in visited:
        return
    visiting.add(name)
    for dependency in graph[name]:
        visit(dependency)
    visiting.remove(name)
    visited.add(name)


for package_name in graph:
    visit(package_name)
if "@cord-network/origin-sdk-contracts" in graph["@cord-network/origin-sdk"]:
    fail("umbrella depends on optional contracts")

current = json.loads((ROOT / "product-sdk/package.json").read_text())
current_table_match = re.search(r'^\[current\]\n(.*?)(?=^\[)', package_text, re.MULTILINE | re.DOTALL)
if not current_table_match:
    fail("missing current package inventory")
current_table = current_table_match.group(1)
expected_exports = array(current_table, "exports")
if list(current.get("exports", {})) != expected_exports or len(expected_exports) != integer(current_table, "export_count"):
    fail("current public export inventory drift")
present_packages = {scalar(block, "name") for block in package_blocks if scalar(block, "state") == "present"}
workspace_packages = {
    json.loads(path.read_text())["name"]
    for path in (ROOT / "product-sdk/packages").glob("origin-sdk*/package.json")
}
if workspace_packages != present_packages or len(workspace_packages) != integer(current_table, "publishable_packages"):
    fail("current publishable package inventory drift")

header = subprocess.run(
    [sys.executable, "scripts/check-source-headers.py"],
    cwd=ROOT,
    text=True,
    capture_output=True,
)
if header.returncode:
    fail(header.stdout.strip() or header.stderr.strip() or "source header audit failed")

print(
    "PASS sdk platform contract: "
    f"capabilities={len(capability_blocks)} packages={len(package_blocks)} "
    f"routes={expected_route_count} branch=sm-update-sub-0x65"
)

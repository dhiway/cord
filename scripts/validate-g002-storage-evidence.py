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

"""Validate materialized G002 storage weight and metadata evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text())


def git_output(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def verify_source(source: dict[str, object]) -> None:
    commit = str(source["commit"])
    tree = str(source["tree"])
    git_output("cat-file", "-e", f"{commit}^{{commit}}")
    assert git_output("rev-parse", f"{commit}^{{tree}}") == tree


def verify_component(root: Path, component: dict[str, object]) -> dict[str, object]:
    receipt_path = root / str(component["receipt_path"])
    assert sha256(receipt_path) == component["receipt_sha256"]
    receipt = read_json(receipt_path)
    assert receipt["status"] == "pass"
    assert receipt["claim_boundary"] == {
        "g002_control_plane": True,
        "g003_provider_byte_plane": False,
        "production_ready": False,
    }
    assert receipt["measurement_source"] == component["measurement_source"]
    verify_source(receipt["measurement_source"])
    benchmark = receipt["benchmark"]
    assert benchmark["pallet"] == component["pallet"]
    assert benchmark["benchmark_count"] == component["benchmark_count"]
    for kind in ["raw_json", "generated_weight"]:
        path = root / str(component[f"{kind}_path"])
        expected = component[f"{kind}_sha256"]
        assert sha256(path) == expected
        assert benchmark[f"{kind}_path"] == component[f"{kind}_path"]
        assert benchmark[f"{kind}_sha256"] == expected
    return {
        "pallet": component["pallet"],
        "measurement_commit": component["measurement_source"]["commit"],
        "raw_json_sha256": component["raw_json_sha256"],
        "generated_weight_sha256": component["generated_weight_sha256"],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--evidence",
        type=Path,
        default=Path("docs/evidence/p1/g002-control-plane-v1/storage-weight-evidence-v1.json"),
    )
    parser.add_argument("--compact-wasm", type=Path)
    args = parser.parse_args()
    root = Path(git_output("rev-parse", "--show-toplevel"))
    evidence_path = (root / args.evidence).resolve()
    evidence = read_json(evidence_path)
    assert evidence["status"] == "pass"
    assert evidence["claim_boundary"] == {
        "g002_control_plane": True,
        "g003_provider_byte_plane": False,
        "production_ready": False,
        "feature_complete": False,
    }
    components = evidence["components"]
    assert [component["pallet"] for component in components] == [
        "pallet_orbis_storage_provider",
        "pallet_orbis_drive",
        "pallet_orbis_s3",
    ]
    verified = [verify_component(root, component) for component in components]
    provider_source = components[0]["measurement_source"]
    drive_source = components[1]["measurement_source"]
    s3_source = components[2]["measurement_source"]
    assert provider_source != drive_source
    assert drive_source == s3_source

    metadata = evidence["metadata"]
    metadata_receipt_path = root / str(metadata["receipt_path"])
    assert sha256(metadata_receipt_path) == metadata["receipt_sha256"]
    metadata_receipt = read_json(metadata_receipt_path)
    assert metadata_receipt["status"] == "pass"
    assert metadata_receipt["source"] == metadata["source"]
    verify_source(metadata_receipt["source"])
    record_path = root / str(metadata["record_path"])
    assert sha256(record_path) == metadata["record_sha256"]
    record = read_json(record_path)
    assert record["metadata_hash"] == metadata["metadata_hash"]
    assert record["compact_wasm_sha256"] == metadata["compact_wasm_sha256"]
    assert metadata_receipt["metadata_hash"] == metadata["metadata_hash"]
    assert metadata_receipt["compact_wasm_sha256"] == metadata["compact_wasm_sha256"]
    manifest_path = root / str(metadata["manifest_path"])
    assert sha256(manifest_path) == metadata["manifest_sha256"]
    assert metadata_receipt["transaction_manifest"] == {
        "path": metadata["manifest_path"],
        "sha256": metadata["manifest_sha256"],
    }
    manifest = read_json(manifest_path)
    manifest_record = next(
        row for row in manifest["files"] if row.get("file") == record_path.name
    )
    assert manifest_record["sha256"] == metadata["record_sha256"]
    assert metadata_receipt["compressed_wasm_sha256"] == metadata["compressed_wasm_sha256"]
    export_gate = metadata["metadata_export_gate"]
    assert metadata_receipt["metadata_export_gate"] == export_gate
    assert export_gate["complete"] is True
    export_path = root / str(export_gate["binding_path"])
    assert sha256(export_path) == export_gate["binding_sha256"]
    export_receipt = read_json(export_path)
    assert export_receipt["complete"] is True
    assert export_receipt["metadata_sha256"] == metadata_receipt["scale_metadata_sha256"]
    assert export_receipt["portable_registry_sha256"] == metadata_receipt["portable_registry_sha256"]
    assert export_receipt["logical_types_spec_sha256"] == export_gate["logical_types_spec_sha256"]
    assert {row["logical_name"] for row in export_receipt["logical_types"]} == {
        "cord.storage.ChunkLocation.v1",
        "cord.storage.Commitment.v1",
        "cord.storage.CommitmentPayload.v2",
        "cord.storage.MmrLeaf.v1",
        "cord.storage.MmrProof.v1",
    }

    registry_declaration = metadata_receipt["portable_registry"]
    registry_path = root / str(registry_declaration["path"])
    assert sha256(registry_path) == registry_declaration["sha256"]
    assert registry_declaration["sha256"] == metadata_receipt["portable_registry_sha256"]
    registry = read_json(registry_path)
    assert registry["metadata_sha256"] == metadata_receipt["scale_metadata_sha256"]
    types = registry["types"]
    promotion_contract = export_gate["promotion"]
    promotion_rows = [
        row for row in types if row.get("path") == promotion_contract["type_path"]
    ]
    assert len(promotion_rows) == 1
    promotion = promotion_rows[0]
    assert promotion["id"] == promotion_contract["portable_id"]
    assert promotion["shape"] == {"composite": promotion_contract["fields"]}
    assert [field["name"] for field in promotion["definition"]["fields"]] == [
        "version", "bucket_id", "snapshot_nonce", "duty_id"
    ]

    call_rows = [
        row for row in types if row.get("path") == promotion_contract["call_type_path"]
    ]
    assert len(call_rows) == 1
    variants = call_rows[0]["definition"]["variants"]
    call = next(
        row for row in variants if row.get("name") == promotion_contract["call_name"]
    )
    assert call["index"] == promotion_contract["call_index"] == 26
    assert call["fields"][0] == {
        "name": "payload", "type_id": promotion_contract["payload_type_id"]
    }
    assert promotion_contract["payload_type_id"] == promotion["id"]
    if args.compact_wasm is not None:
        assert sha256(args.compact_wasm) == metadata["compact_wasm_sha256"]

    result = {
        "status": "pass",
        "evidence_sha256": sha256(evidence_path),
        "components": verified,
        "metadata_hash": metadata["metadata_hash"],
        "compact_wasm_sha256": metadata["compact_wasm_sha256"],
        "claim_boundary": evidence["claim_boundary"],
    }
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

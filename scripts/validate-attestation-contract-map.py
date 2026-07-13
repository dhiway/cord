#!/usr/bin/env python3
"""Focused exactness check for adopted native Attestation contract-map rows."""

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MAP = ROOT / "docs/sdk/contract-to-native-map.json"
GENERIC = "Attestation calls/runtime APIs/events/errors"
DELEGATED_VECTOR_FUNCTIONS = {
	"attestByDelegation",
	"multiAttestByDelegation",
	"multiRevokeByDelegation",
	"revokeByDelegation",
}
EVENT_VECTOR_SYMBOLS = {"Attested", "Revoked"}


def main() -> None:
	document = json.loads(MAP.read_text())
	errors: list[str] = []
	rows = [
		row
		for row in document["entries"]
		if row.get("runtime_owner") == "Attestation"
		and row.get("disposition") in {"adopt-semantic", "intentional-change"}
	]
	for row in rows:
		label = f"{row['source_id']}::{row['source_symbol_kind']}::{row['source_symbol']}"
		for field in ("native_target", "rust_sdk", "typescript_sdk", "implementation_evidence"):
			if not row.get(field):
				errors.append(f"{label}: missing {field}")
		if row.get("native_target") == GENERIC or row.get("rust_sdk") == "attestation" or row.get("typescript_sdk") == "attestation":
			errors.append(f"{label}: generic target or SDK surface")
		if str(row.get("native_target", "")).startswith("not-applicable:") and (
			row.get("rust_sdk") != "none" or row.get("typescript_sdk") != "none"
		):
			errors.append(f"{label}: not-applicable row exposes an SDK surface")
		for field in ("implementation_evidence", "semantic_evidence", "vector_evidence"):
			if field in row and not (ROOT / row[field]).is_file():
				errors.append(f"{label}: missing evidence file {row[field]}")
		for evidence in row.get("vector_test_evidence", []):
			if not (ROOT / evidence).is_file():
				errors.append(f"{label}: missing vector test {evidence}")
		if "vector_evidence" in row:
			allowed = (
				row["source_symbol_kind"] == "function"
				and row["source_symbol"] in DELEGATED_VECTOR_FUNCTIONS
			) or (
				row["source_symbol_kind"] == "event"
				and row["source_symbol"] in EVENT_VECTOR_SYMBOLS
			)
			if not allowed:
				errors.append(f"{label}: unrelated delegated/event vector evidence")
			if not row.get("vector_test_evidence"):
				errors.append(f"{label}: vector is not bound to executable tests")

	if GENERIC in MAP.read_text():
		errors.append("map still contains the generic Attestation target phrase")
	if errors:
		raise SystemExit("\n".join(errors))
	print(f"validated {len(rows)} exact Attestation adopted/intentional-change rows")


if __name__ == "__main__":
	main()

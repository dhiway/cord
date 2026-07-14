#!/usr/bin/env python3
"""Focused exactness check for adopted native Attestation contract-map rows."""

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MAP = ROOT / "docs/sdk/contract-to-native-map.json"
GENERIC = "Attestation calls/runtime APIs/events/errors"
EIP712_SOURCE_SUFFIX = "/EIP712Verifier.sol"
ATTESTATION_VECTOR = ROOT / "docs/sdk/vectors/attestation-v1.json"
DELEGATED_VECTOR_FUNCTIONS = {
	"attestByDelegation",
	"multiAttestByDelegation",
	"multiRevokeByDelegation",
	"revokeByDelegation",
}
EVENT_VECTOR_SYMBOLS = {"Attested", "Revoked"}
REQUIRED_VECTOR_TESTS = {
	"origin/orbis/pallets/attestation/src/tests.rs",
	"origin-rs/src/product_sdk/domains/attestation.rs",
	"product-sdk/tests/attestation/attestation-vectors.test.ts",
	"product-sdk/tests/host/attestation-contracts.test.ts",
	"product-sdk/tests/host/attestation-events.test.ts",
}
FORBIDDEN_GENERIC_FRAGMENTS = {
	".nativeRoutes",
	"AttestationQuery/AttestationCommand",
	"SchemaRecord/AttestationRecord/DelegatedIntent",
	"Attestation atomic derived indexes",
	"pallet_orbis_attestation::Error (source semantic",
	"IssuerSignature.value",
	"IssuerSignature.scheme",
	"Config/storage/call invariant suite",
	"origin-rs/src/product_sdk/domains/attestation.rs::Validate",
	"route input validation",
	"BatchOf<AttestationInput>",
	"DelegatedIssueBatchOf<SignedDelegatedIntent>",
	"DelegatedRevokeBatchOf<SignedDelegatedRevokeIntent>",
	"native delegated-attestation calls and intent verifier",
}
FORBIDDEN_DEFINITION_PATHS = {
	"product-sdk/src/attestation.ts::IssuerSignature",
	"origin/orbis/pallets/attestation/src/lib.rs::AttestationId",
	"origin-rs/src/product_sdk/domains/attestation.rs::AttestationId",
	"product-sdk/src/attestation.ts::AttestationId",
	"origin-rs/src/product_sdk/domains/attestation.rs::delegatedIssueSigningPayload",
}
EXACT_SYMBOLS = {
	("function", "attestationCount"): (
		"AttestationApi::attestation_count",
		"AttestationQuery::AttestationCount",
		"attestation.attestationCount",
	),
	("field", "Attestation.data"): (
		"AttestationRecord::payload_commitment",
		"AttestationView::payload_commitment",
		"AttestationView.payload_commitment",
	),
	("field", "Attestation.id"): (
		"Config::Hash (Attestations storage key)",
		"AttestationView::attestation",
		"AttestationView.attestation",
	),
	("state", "_revocationsOffchain"): (
		"ExternalStatuses",
		"AttestationQuery::ExternalStatus",
		"attestation.externalStatus",
	),
	("error", "AttestationService__AlreadyRevoked"): (
		"Error::AttestationRevoked",
		"NativeError",
		"ProductSdkError",
	),
	("function", "multiAttest"): (
		"Call::issue_batch",
		"AttestationCommand::IssueBatch",
		"attestation.issueBatch",
	),
	("function", "multiRevoke"): (
		"Call::revoke_batch",
		"AttestationCommand::RevokeBatch",
		"attestation.revokeBatch",
	),
	("function", "onAttest"): (
		"issue_inner atomic IssuerAttestations/SubjectSchemaAttestations update",
		"AttestationEvent::AttestationIssued",
		"AttestationEvent event=attestation_issued",
	),
	("function", "onRevoke"): (
		"revoke_inner atomic Attestations update",
		"AttestationEvent::AttestationRevoked",
		"AttestationEvent event=attestation_revoked",
	),
	("field", "Signature.r"): (
		"Config::Signature encoded bytes",
		"Signature::bytes",
		"types.ts::IssuerSignature (opaque encoding; no r component)",
	),
	("function", "_toFixedSignature"): (
		"Config::Signature + Verify::verify",
		"domains/attestation.rs::Signature",
		"types.ts::IssuerSignature",
	),
	("function", "_bindingMessage"): (
		"delegated_signing_payload",
		"domains/attestation.rs::delegated_issue_signing_payload",
		"attestation.ts::delegatedIssueSigningPayload",
	),
	("function", "_wrapBytes"): (
		"delegated_signing_payload SCALE encoding",
		"domains/attestation.rs::delegated_issue_signing_payload",
		"attestation.ts::delegatedIssueSigningPayload",
	),
	("field", "RevocationRequestData.id"): (
		"Config::Hash (Attestations storage key)",
		"domains/common.rs::AttestationId",
		"types.ts::AttestationId",
	),
	("field", "SchemaRecord.id"): (
		"Config::Hash (Schemas storage key)",
		"SchemaView::schema",
		"SchemaView.schema",
	),
	("struct", "RevocationRequestData"): (
		"Config::Hash (Attestations storage key)",
		"domains/common.rs::AttestationId",
		"types.ts::AttestationId",
	),
	("struct", "Signature"): (
		"Config::Signature",
		"domains/attestation.rs::Signature",
		"types.ts::IssuerSignature",
	),
	("function", "_page"): (
		"runtime-api/src/lib.rs::IdPage/MAX_PAGE_SIZE",
		"domains/common.rs::PageRequest",
		"types.ts::PageInput",
	),
	("error", "RecipientAndAttesterIndexResolver__PageSizeTooLarge"): (
		"runtime-api/src/lib.rs::MAX_PAGE_SIZE",
		"NativeErrorCode::InvalidInput",
		"ProductSdkError(code=invalid_input)",
	),
	("state", "MAX_PAGE_SIZE"): (
		"runtime-api/src/lib.rs::MAX_PAGE_SIZE",
		"domains/common.rs::PageRequest::limit",
		"types.ts::PageInput.limit",
	),
	("state", "MESSAGE_PREFIX"): (
		"DELEGATED_INTENT_DOMAIN/DELEGATED_REVOKE_DOMAIN",
		"delegated_issue_signing_payload/delegated_revoke_signing_payload",
		"delegatedIssueSigningPayload/delegatedRevokeSigningPayload",
	),
	("field", "MultiAttestationRequest.data"): (
		"BatchOf<T> (bounded AttestationInputOf<T>)",
		"AttestationCommand::IssueBatch::inputs",
		"attestation.issueBatch items",
	),
	("struct", "MultiAttestationRequest"): (
		"BatchOf<T> (bounded AttestationInputOf<T>)",
		"Vec<AttestationInput>",
		"readonly AttestationInput[]",
	),
	("struct", "MultiDelegatedAttestationRequest"): (
		"DelegatedIssueBatchOf<T> (bounded SignedDelegatedIntent<T>)",
		"Vec<SignedDelegatedIssue>",
		"readonly SignedDelegatedIssue[]",
	),
	("struct", "MultiDelegatedRevocationRequest"): (
		"DelegatedRevokeBatchOf<T> (bounded SignedDelegatedRevokeIntent<T>)",
		"Vec<SignedDelegatedRevoke>",
		"readonly SignedDelegatedRevoke[]",
	),
}

EXACT_INVARIANT_TARGETS = (
	"Schemas/Attestations/IssuerAttestations/SubjectSchemaAttestations/KnownSubjects",
	"impl Validate for AttestationInput/DelegatedIntent/DelegatedRevokeIntent/Signature/AttestationQuery, AttestationCommand",
	"attestation.createSchema/issue/issueDelegated/issueBatch/revoke/revokeDelegated/revokeBatch",
)

EIP712_EXACT_SYMBOLS = {
	("error", "EIP712Verifier__DeadlineExpired"): (
		"Error::IntentExpired", "AttestationCommand::validate_at", "DelegatedIssueIntent.deadline/DelegatedRevokeIntent.deadline",
	),
	("error", "EIP712Verifier__InvalidNonce"): (
		"Error::InvalidNonce", "AttestationCommand::IssueDelegated/RevokeDelegated", "attestation.issueDelegated/revokeDelegated",
	),
	("error", "EIP712Verifier__InvalidSignature"): (
		"Error::InvalidSignature", "Signature/AttestationCommand::IssueDelegated/RevokeDelegated", "types.ts::IssuerSignature",
	),
	("event", "NonceIncreased"): (
		"Event::DelegatedIntentConsumed/DelegatedRevocationConsumed",
		"AttestationEvent::DelegatedIntentConsumed/DelegatedRevocationConsumed",
		"AttestationEvent delegated_intent_consumed/delegated_revocation_consumed",
	),
	("function", "_time"): (
		"ensure_delegated_context -> frame_system::Pallet::block_number <= deadline",
		"DelegatedIntent::validate_at/DelegatedRevokeIntent::validate_at",
		"DelegatedIssueIntent.deadline/DelegatedRevokeIntent.deadline",
	),
	("function", "_verifyAttest"): (
		"validate_delegated_issue + delegated_signing_payload",
		"delegated_issue_signing_payload + AttestationCommand::IssueDelegated",
		"delegatedIssueSigningPayload + attestation.issueDelegated",
	),
	("function", "_verifyRevoke"): (
		"validate_delegated_revoke + delegated_revoke_signing_payload",
		"delegated_revoke_signing_payload + AttestationCommand::RevokeDelegated",
		"delegatedRevokeSigningPayload + attestation.revokeDelegated",
	),
	("function", "constructor"): (
		"Config + DELEGATED_INTENT_DOMAIN/DELEGATED_REVOKE_DOMAIN (static composition; no constructor)",
		"delegated_issue_signing_payload/delegated_revoke_signing_payload (no constructor)",
		"delegatedIssueSigningPayload/delegatedRevokeSigningPayload (no constructor)",
	),
	("function", "getAttestTypeHash"): (
		"DELEGATED_INTENT_DOMAIN + delegated_signing_payload canonical SCALE tuple",
		"delegated_issue_signing_payload",
		"delegatedIssueSigningPayload",
	),
	("function", "getDomainSeparator"): (
		"DELEGATED_INTENT_DOMAIN/DELEGATED_REVOKE_DOMAIN + ensure_delegated_context(genesis_hash,spec_version)",
		"DelegatedIntent::genesis_hash/spec_version + DelegatedRevokeIntent::genesis_hash/spec_version",
		"DelegatedIssueIntent.genesis_hash/spec_version + DelegatedRevokeIntent.genesis_hash/spec_version",
	),
	("function", "getName"): (
		"DELEGATED_INTENT_DOMAIN/DELEGATED_REVOKE_DOMAIN (no EIP-712 name)",
		"delegated_issue_signing_payload/delegated_revoke_signing_payload",
		"delegatedIssueSigningPayload/delegatedRevokeSigningPayload",
	),
	("function", "getNonce"): (
		"AttestationApi::next_delegated_nonce", "AttestationQuery::NextDelegatedNonce", "attestation.nextDelegatedNonce",
	),
	("function", "getRevokeTypeHash"): (
		"DELEGATED_REVOKE_DOMAIN + delegated_revoke_signing_payload canonical SCALE tuple",
		"delegated_revoke_signing_payload",
		"delegatedRevokeSigningPayload",
	),
	("function", "increaseNonce"): (
		"consume_delegated_issue/consume_delegated_revoke -> NextDelegatedNonce",
		"AttestationEvent::DelegatedIntentConsumed/DelegatedRevocationConsumed",
		"AttestationEvent delegated_intent_consumed/delegated_revocation_consumed",
	),
	("state", "ATTEST_TYPEHASH"): (
		"DELEGATED_INTENT_DOMAIN + delegated_signing_payload canonical SCALE tuple",
		"delegated_issue_signing_payload", "delegatedIssueSigningPayload",
	),
	("state", "REVOKE_TYPEHASH"): (
		"DELEGATED_REVOKE_DOMAIN + delegated_revoke_signing_payload canonical SCALE tuple",
		"delegated_revoke_signing_payload", "delegatedRevokeSigningPayload",
	),
	("state", "_nonces"): (
		"NextDelegatedNonce", "AttestationQuery::NextDelegatedNonce", "attestation.nextDelegatedNonce",
	),
	("invariant-bundle", "roles-storage-economic-signature-lifecycle"): (
		"DELEGATED_INTENT_DOMAIN/DELEGATED_REVOKE_DOMAIN + NextDelegatedNonce + ensure_delegated_context",
		"impl Validate for DelegatedIntent/DelegatedRevokeIntent/Signature + DelegatedIntent::validate_at/DelegatedRevokeIntent::validate_at",
		"types.ts::IssuerSignature",
	),
}
EIP712_VECTOR_SYMBOLS = {f"EIP712Verifier.{name}" for _, name in EIP712_EXACT_SYMBOLS}
EIP712_VECTOR_EVENTS = {"delegated_intent_consumed", "delegated_revocation_consumed"}


def main() -> None:
	document = json.loads(MAP.read_text())
	errors: list[str] = []
	rows = [
		row
		for row in document["entries"]
		if (
			row.get("runtime_owner") == "Attestation"
			or str(row.get("source_id", "")).endswith(EIP712_SOURCE_SUFFIX)
		)
		and row.get("disposition") in {"adopt-semantic", "intentional-change"}
	]
	for row in rows:
		label = f"{row['source_id']}::{row['source_symbol_kind']}::{row['source_symbol']}"
		for field in ("native_target", "rust_sdk", "typescript_sdk", "implementation_evidence"):
			if not row.get(field):
				errors.append(f"{label}: missing {field}")
		if row.get("native_target") == GENERIC or row.get("rust_sdk") == "attestation" or row.get("typescript_sdk") == "attestation":
			errors.append(f"{label}: generic target or SDK surface")
		serialized = json.dumps(row, sort_keys=True)
		for fragment in FORBIDDEN_GENERIC_FRAGMENTS:
			if fragment in serialized:
				errors.append(f"{label}: forbidden generic mapping fragment {fragment}")
		for field in ("native_target", "rust_sdk", "typescript_sdk"):
			target = str(row.get(field, ""))
			for wrong_path in FORBIDDEN_DEFINITION_PATHS:
				if target == wrong_path or target.startswith((wrong_path + " ", wrong_path + "/")):
					errors.append(f"{label}: {field} uses imported or nonexistent definition path {wrong_path}")
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
		if set(row.get("vector_test_evidence", [])) != REQUIRED_VECTOR_TESTS:
			errors.append(f"{label}: incomplete executable pallet/Rust/TypeScript evidence")
		if row["source_symbol_kind"] == "field":
			rust_surface = row.get("rust_sdk", "").removeprefix(
				"origin-rs/src/product_sdk/domains/attestation.rs::"
			)
			typescript_surface = row.get("typescript_sdk", "").removeprefix(
				"product-sdk/src/attestation.ts::"
			)
			if (
				("::" not in rust_surface and rust_surface[:1].islower())
				or ("." not in typescript_surface and typescript_surface[:1].islower())
			):
				errors.append(f"{label}: SDK field target is a bare module-level field")
		if "vector_evidence" in row:
			allowed = (
				row["source_id"].endswith(EIP712_SOURCE_SUFFIX)
			) or (
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

	for key, exact in EXACT_SYMBOLS.items():
		matching = [row for row in rows if (row["source_symbol_kind"], row["source_symbol"]) == key]
		if not matching:
			errors.append(f"missing exact Attestation semantic {key[0]}::{key[1]}")
		for row in matching:
			for field, symbol in zip(("native_target", "rust_sdk", "typescript_sdk"), exact):
				if symbol not in row[field]:
					errors.append(f"{row['source_id']}::{key[1]}: {field} lacks {symbol}")

	invariant_rows = [
		row for row in rows
		if row["source_symbol_kind"] == "invariant-bundle"
		and not row["source_id"].endswith(EIP712_SOURCE_SUFFIX)
	]
	if len(invariant_rows) != 9:
		errors.append(f"expected 9 active Attestation invariant bundles, found {len(invariant_rows)}")
	for row in invariant_rows:
		label = f"{row['source_id']}::invariant-bundle::{row['source_symbol']}"
		for field, symbol in zip(("native_target", "rust_sdk", "typescript_sdk"), EXACT_INVARIANT_TARGETS):
			if symbol not in row[field]:
				errors.append(f"{label}: {field} lacks concrete invariant target list {symbol}")

	eip_rows = [row for row in rows if row["source_id"].endswith(EIP712_SOURCE_SUFFIX)]
	if len(eip_rows) != 18:
		errors.append(f"expected 18 active EIP712Verifier semantics, found {len(eip_rows)}")
	if len({(row["source_symbol_kind"], row["source_symbol"]) for row in eip_rows}) != 18:
		errors.append("EIP712Verifier semantics are missing or duplicated")
	for key, exact in EIP712_EXACT_SYMBOLS.items():
		matching = [row for row in eip_rows if (row["source_symbol_kind"], row["source_symbol"]) == key]
		if len(matching) != 1:
			errors.append(f"expected one exact EIP712Verifier semantic {key[0]}::{key[1]}")
			continue
		row = matching[0]
		if row.get("runtime_owner") != "Attestation":
			errors.append(f"EIP712Verifier {key[0]}::{key[1]} has non-canonical owner {row.get('runtime_owner')}")
		for field, symbol in zip(("native_target", "rust_sdk", "typescript_sdk"), exact):
			if symbol not in row[field]:
				errors.append(f"EIP712Verifier::{key[1]}: {field} lacks {symbol}")
	vector = json.loads(ATTESTATION_VECTOR.read_text())
	vector_symbols = set(vector.get("source_symbols", []))
	missing_vector_symbols = sorted(EIP712_VECTOR_SYMBOLS - vector_symbols)
	if missing_vector_symbols:
		errors.append(f"attestation vector lacks EIP712Verifier source symbols: {missing_vector_symbols}")
	vector_events = {
		entry.get("runtime_event", {}).get("event") for entry in vector.get("events", [])
	}
	missing_vector_events = sorted(EIP712_VECTOR_EVENTS - vector_events)
	if missing_vector_events:
		errors.append(f"attestation vector lacks delegated-consumption events: {missing_vector_events}")

	for row in document["entries"]:
		path = row.get("source_id", "").lower()
		name = row.get("source_symbol", "").lower()
		attestation_source = path.startswith("attestation-protocol:") or (
			path.startswith("browse:") and any(part in path for part in (
				"iattestationservice.sol", "iattestationresolver.sol", "ischemaregistry.sol",
				"recipientandattesterindexresolver.sol", "trustedattesterindexresolver.sol",
			))
		)
		resolver_service = (
			attestation_source and (
			"interfaces/iattestationresolver.sol" in path
			or ("resolver" in path and (
				name in {"constructor", "getservice", "_service", "onlyservice"}
				or name.endswith("__invalidservice")
				or name.endswith("__accessdenied")
			))
			)
		)
		if resolver_service and row.get("disposition") != "retired":
			errors.append(
				f"{row.get('source_id')}::{row.get('source_symbol')}: resolver callback/service surface is not retired"
			)

	if GENERIC in MAP.read_text():
		errors.append("map still contains the generic Attestation target phrase")
	if errors:
		raise SystemExit("\n".join(errors))
	print(f"validated {len(rows)} exact Attestation adopted/intentional-change rows")


if __name__ == "__main__":
	main()

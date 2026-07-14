#!/usr/bin/env python3
"""Fail closed on P0 M2/M3/M9 census and clean-cutover invariants."""
import csv, hashlib, json, re, subprocess, sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
workspace = root.parent
csv_path = root / "docs/architecture/contract-to-native-migration.csv"
ev = root / "docs/evidence/p0-contract-native"
errors = []
approval_field_names = {"approval_state", "approval_manifest_schema_version", "approval_manifest_path", "approval_manifest_sha256"}
def canonical_hash(value): return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
with csv_path.open(newline="") as f:
    rows = list(csv.DictReader(f))
ids = [r["source_id"] for r in rows]
if not rows: errors.append("empty census")
if len(ids) != len(set(ids)): errors.append("duplicate source_id")
expected_sources = set()
for path in (workspace / ".omx/cache").rglob("*.sol"):
    rel = path.relative_to(workspace / ".omx/cache")
    if any(part in {"lib", "node_modules"} for part in rel.parts): continue
    expected_sources.add(f"{rel.parts[0]}:{Path(*rel.parts[1:]).as_posix()}")
if set(ids) != expected_sources:
    errors.append(f"cache census mismatch missing={len(expected_sources-set(ids))} extra={len(set(ids)-expected_sources)}")
for r in rows:
    if not re.fullmatch(r"[0-9a-f]{40}", r["commit"]): errors.append(f"bad commit:{r['source_id']}")
    if not re.fullmatch(r"[0-9a-f]{64}", r["blob_sha256"]): errors.append(f"bad blob hash:{r['source_id']}")
    if r["disposition"] not in {"adopt-semantic", "intentional-change", "retired", "not-applicable"}: errors.append(f"unknown:{r['source_id']}")
    if r["deployment_state_input"] != "none": errors.append(f"deployment input:{r['source_id']}")
    if r["compatibility_or_data_migration"] != "forbidden-clean-genesis": errors.append(f"legacy path:{r['source_id']}")
    if r["disposition"] in {"adopt-semantic", "intentional-change"} and "none" in (r["target_pallet"], r["rust_sdk_surface"], r["typescript_sdk_surface"]): errors.append(f"incomplete mapping:{r['source_id']}")
approval_path = ev / "architect-semantic-disposition-approval.json"
approval_hash = hashlib.sha256(approval_path.read_bytes()).hexdigest()
approval = json.loads(approval_path.read_text())
approval_rel = approval_path.relative_to(root).as_posix()
if approval.get("schema_version") != 2 or approval.get("required_reviewer_role") != "architect": errors.append("approval schema/required role mismatch")
if approval.get("branch") != "sm-update-sub-0x63": errors.append("approval branch mismatch")
current_head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
source_base_head = approval.get("source_base_head")
if not isinstance(source_base_head, str) or not re.fullmatch(r"[0-9a-f]{40}", source_base_head):
    errors.append("approval source base HEAD invalid")
elif subprocess.run(["git", "-C", str(root), "merge-base", "--is-ancestor", source_base_head, current_head], check=False).returncode != 0:
    errors.append("approval source base HEAD is not an ancestor of current HEAD")
for r in rows:
    if r.get("approval_manifest_schema_version") != "2" or r.get("approval_manifest_path") != approval_rel or r.get("approval_manifest_sha256") != approval_hash:
        errors.append(f"row approval reference mismatch:{r['source_id']}")
report = json.loads((ev / "contract-census.report.json").read_text())
if report["status"] not in {"blocked", "pass"} or report["source_components"] != len(rows): errors.append("census report mismatch")
if report["unapproved_source_components"] and report["status"] == "pass": errors.append("false pass with unapproved semantics")
if report["unapproved_source_components"] and report["approved_semantic_design_coverage_percent"] == 100: errors.append("false 100% approval coverage")
if report.get("source_summary_audit", {}).get("status") != "pass" or report.get("source_summary_audit", {}).get("rows") != len(rows): errors.append("source summary audit report mismatch")
readiness = json.loads((ev / "semantic-review-readiness.json").read_text())
expected_readiness = "approved" if approval["verdict"] == "APPROVED" else "ready-for-architect-re-review"
if readiness["status"] != expected_readiness or readiness["approval_status"] != approval["verdict"]: errors.append("semantic review readiness/approval state invalid")
if readiness["census_sha256"] != hashlib.sha256(csv_path.read_bytes()).hexdigest(): errors.append("readiness census hash mismatch")
if readiness["design_map_sha256"] != hashlib.sha256((root / "docs/sdk/contract-to-native-map.json").read_bytes()).hexdigest(): errors.append("readiness design hash mismatch")
design = json.loads((root / "docs/sdk/contract-to-native-map.json").read_text())
if not design.get("clean_genesis") or not design.get("entries"): errors.append("missing semantic design map")
for item in design.get("entries", []):
    if item["compatibility_facade"] or item["data_migration_input"]: errors.append(f"legacy design mapping:{item['source_id']}")
    if not item["native_target"] or not item["disposition"] in {"adopt-semantic", "intentional-change", "retired", "not-applicable"}: errors.append(f"bad semantic entry:{item['source_id']}")
    if not item.get("target_kind") or not item.get("bounded_semantic_disposition"): errors.append(f"generic semantic entry:{item['source_id']}")
    if item.get("approval_manifest_schema_version") != 2 or item.get("approval_manifest_path") != approval_rel or item.get("approval_manifest_sha256") != approval_hash:
        errors.append(f"map approval reference mismatch:{item['source_id']}:{item['source_symbol']}")
entries = design.get("entries", [])
if design.get("approval_manifest", {}).get("sha256") != approval_hash: errors.append("design top-level approval reference mismatch")
if approval.get("census_payload_sha256") != canonical_hash([{k:v for k,v in r.items() if k not in approval_field_names} for r in rows]): errors.append("approval census payload hash drift")
if approval.get("design_payload_sha256") != canonical_hash([{k:v for k,v in x.items() if k not in approval_field_names} for x in entries]): errors.append("approval design payload hash drift")
entry_keys = [(x["source_id"], x["source_symbol_kind"], x["source_symbol"]) for x in entries]
require_unique_error = len(entry_keys) != len(set(entry_keys))
if require_unique_error: errors.append("duplicate symbol-level design entry")
def selected(fragment): return [item for item in entries if fragment.lower() in item["source_id"].lower()]
def require(condition, message):
    if not condition: errors.append(message)

root_gateway = selected("RootGatewayDispatcher.sol")
require(root_gateway and all(x["disposition"] == "retired" for x in root_gateway), "RootGatewayDispatcher not fully retired")
protocol_registry = selected("DotnsProtocolRegistry.sol")
require(protocol_registry and all(x["disposition"] == "retired" for x in protocol_registry), "protocol-address registry not fully retired")
store_factory = selected("StoreFactory.sol")
require({"retired", "intentional-change"} <= {x["disposition"] for x in store_factory}, "StoreFactory record/proxy semantics not split")
publisher = [x for x in selected("Publisher.sol") if x["source_id"].startswith("browse:")]
require(publisher and all(x["disposition"] == "not-applicable" and "Attestation" not in x["native_target"] for x in publisher), "Browse Publisher invented/misclassified native authority")
passport = selected("ZKPassportRegistry.sol")
require(passport and all(x["disposition"] == "retired" for x in passport), "permissionless ZKPassport trust not retired")
individuality = selected("individuality-community:precompiles/personhood/sol/IPersonhood.sol")
require(any("unlinkable" in x["bounded_semantic_disposition"] for x in individuality), "Individuality unlinkability missing")
require(any("replay" in x["bounded_semantic_disposition"] and "caller" in x["bounded_semantic_disposition"] for x in individuality), "Individuality replay/caller binding missing")
attestation = selected("attestation-protocol:")
require(any("timestamp" in x["source_symbol"].lower() and x["disposition"] == "retired" for x in attestation), "timestamp disposition missing")
require(any("offchain" in x["source_symbol"].lower() and "off-chain" in x["bounded_semantic_disposition"] for x in attestation), "off-chain revocation/privacy disposition missing")
require(any(x["source_symbol"].lower().startswith("multi") and "all-or-nothing" in x["bounded_semantic_disposition"] and "MaxBatchItems" in x["bounded_semantic_disposition"] for x in attestation), "bounded atomic batch disposition missing")
require(any("Blake2 AttestationId" in x["bounded_semantic_disposition"] for x in attestation), "AttestationId derivation missing")
require(any("resolver" in x["source_symbol"].lower() and "callback" in x["bounded_semantic_disposition"] for x in attestation), "resolver callback disposition missing")
require(any(x["source_symbol"].lower().endswith(".resolver") and "IndexPolicy" in x["bounded_semantic_disposition"] for x in attestation), "schema resolver-address replacement missing")
required_vectors = ("fuzz", "invariant", "stress", "reentrant", "refund", "revert", "lifecycle", "delegation", "deployment", "role")
vector_categories = [x.get("vector_category", "") for x in entries]
for marker in required_vectors: require(any(marker in category for category in vector_categories), f"fixture vector marker missing:{marker}")

# Exact state sets prove the conservative parser did not promote mapping key/value
# names or function locals to storage symbols.
expected_state = {
 "dotns:contracts/registry/DotnsProtocolRegistry.sol": {"__gap", "_addresses", "_registeredRefcount"},
 "dotns:contracts/store/StoreFactory.sol": {"labelStoreBeacon", "userStoreBeacon", "protocolRegistry", "_labelStores", "_userStores", "_labelStoreList", "_userStoreList"},
 "browse:evm/src/Publisher.sol": {"PERSONHOOD", "DOT_NODE", "PERSONHOOD_CONTEXT", "RATE_WINDOW", "LITE_DAILY_LIMIT", "FULL_DAILY_LIMIT", "registrar", "_published", "_publications", "_windows"},
 "localdot-community:packages/contracts/contracts/ZKPassportRegistry.sol": {"VERSION", "attestations", "uniqueIdToWallet"},
 "attestation-protocol:evm/contracts/AttestationService.sol": {"_schemaRegistry", "attestationCount", "_attestations", "_timestamps", "_revocationsOffchain"},
}
row_by_id = {r["source_id"]: r for r in rows}
dispositions_by_source = {}
for item in entries: dispositions_by_source.setdefault(item["source_id"], set()).add(item["disposition"])
for source_id, symbol_dispositions in dispositions_by_source.items():
    expected_summary = (f"all:{next(iter(symbol_dispositions))}" if len(symbol_dispositions) == 1 else "mixed:" + ",".join(sorted(symbol_dispositions)))
    require(row_by_id[source_id]["symbol_disposition_state"] == expected_summary, f"file symbol-disposition summary mismatch:{source_id}")
    if len(symbol_dispositions) == 1:
        require(row_by_id[source_id]["disposition"] == next(iter(symbol_dispositions)), f"false file disposition:{source_id}")
    else:
        require(row_by_id[source_id]["disposition"] == "intentional-change", f"mixed file not normalized intentional-change:{source_id}")
for source_id, expected in expected_state.items():
    actual = {s.split(":", 1)[1] for s in row_by_id[source_id]["semantic_symbols"].split(";") if s.startswith("state:")}
    require(actual == expected, f"conservative state parse mismatch:{source_id}:actual={sorted(actual)}")
summary_requirements = {
 "attestation-protocol:evm/contracts/AttestationService.sol": ("non-unique IDs use ++attestationCount", "unique-schema IDs use keccak256(attester, recipient, schema)", "revoke mutates revocationTime", "overwrite resets time/expiration/revocation/data"),
 "attestation-protocol:evm/contracts/SchemaRegistry.sol": ("mutable _count and _schemas", "id = ++_count"),
 "browse:evm/src/Semver.sol": ("constructor-fixed immutable _major, _minor and _patch", "no mutation or upgrade function"),
 "browse:evm/src/TrustedAttesterIndexResolver.sol": ("sole immutable _trustedAttester", "only that fixed attester", "pagination limit constant is 100"),
 "dotns:contracts/deploy/Create3Factory.sol": ("permissionless payable deploy", "forwards msg.value", "collision behavior comes from CREATE3"),
 "dotns:contracts/external/revive/ISystem.sol": ("only callerIsRoot() view", "reverts for signed/non-Root origin"),
 "dotns:contracts/utils/Multicall3.sol": ("aggregate/try/aggregate3/value", "arbitrary target calls", "msg.value equal accumulated call values"),
 "dotns:contracts/registrars/RootGatewayDispatcher.sol": ("immutable TARGET", "fallback calls ISystem.callerIsRoot", "TARGET.call(msg.data)"),
 "dotns:contracts/registry/DotnsProtocolRegistry.sol": ("bytes32-to-address mapping", "address refcounts", "onlyOwner"),
 "browse:evm/src/Publisher.sol": ("personhood tier and rolling timestamp window", "swap-remove", "no Attestation call is declared"),
 "localdot-community:packages/contracts/contracts/ZKPassportRegistry.sol": ("permissionless submitAttestation", "no issuer/proof verifier", "permits identifier reuse"),
 "dotns:contracts/external/personhood/IPersonhood.sol": ("different per application context", "no mutation or proof verification function"),
}
for source_id, needles in summary_requirements.items():
    summary = row_by_id[source_id]["roles_storage_invariants"] + " " + row_by_id[source_id]["economic_signature_lifecycle"]
    for needle in needles: require(needle in summary, f"stale/false file summary:{source_id}:missing={needle}")
for row in rows:
    summary = row["roles_storage_invariants"] + " " + row["economic_signature_lifecycle"]
    require(bool(summary.strip()), f"empty source summary:{row['source_id']}")
    for stale in ("source-reviewed", "canonical native", "no speculative token economics", "native signed mutation",
                  "contract address/value/proxy semantics retired", "no independent authority", "bounded native storage"):
        require(stale not in summary, f"generic/design claim leaked into source summary:{row['source_id']}:{stale}")
cleanup = json.loads((ev / "native-cutover-cleanup.report.json").read_text())
if cleanup["status"] not in {"baseline-recorded", "pass"}: errors.append("invalid cleanup status")
if cleanup["migrated_domain_callable_contracts"] and not cleanup.get("known_survivors"):
    errors.append("uncatalogued migrated-domain survivor")
if "--final" in sys.argv:
    for field in ("unowned_survivors", "migrated_domain_callable_contracts", "deprecated_facades", "dead_product_paths"):
        if cleanup[field] != 0: errors.append(f"final cleanup {field} != 0")
allow = json.loads((ev / "native-cutover-allowlist.json").read_text())
allowlist_hash = hashlib.sha256((ev / "native-cutover-allowlist.json").read_bytes()).hexdigest()
if cleanup.get("allowlist_sha256") != allowlist_hash:
    errors.append("cleanup allowlist hash mismatch")
for entry in allow["entries"]:
    for field in ("owner", "unrelated_live_use", "dependency_path", "test_evidence", "exclusions"):
        if not entry.get(field): errors.append(f"allowlist missing {field}")
for artifact_name, key in (("contract-census.report.json", "approval_manifest"), ("semantic-review-readiness.json", "approval_manifest"),
                           ("native-cutover-allowlist.json", "approval_manifest"), ("native-cutover-cleanup.report.json", "approval_manifest")):
    artifact = json.loads((ev / artifact_name).read_text())
    if artifact.get(key, {}).get("sha256") != approval_hash: errors.append(f"report approval reference mismatch:{artifact_name}")
index = json.loads((ev / "evidence-index.json").read_text())
if index.get("semantic_approval_manifest", {}).get("sha256") != approval_hash: errors.append("index approval reference mismatch")

# Markdown is a human review record, but cannot grant approval independently.
approval_doc = (ev / "architect-semantic-disposition-approval.md").read_text()
front = dict(re.findall(r"^([A-Za-z0-9-]+):\s*(.+)$", approval_doc, re.M))
expected_front = {"Verdict": approval["verdict"], "Reviewer-Role": approval["reviewer_role"],
                  "Review-Thread-ID": approval["review_thread_id"], "Approval-Manifest-SHA256": approval_hash,
                  "Census-Payload-SHA256": approval["census_payload_sha256"], "Design-Payload-SHA256": approval["design_payload_sha256"],
                  "Branch": approval["branch"], "Source-Base-HEAD": approval["source_base_head"]}
for key, value in expected_front.items():
    if front.get(key) != str(value): errors.append(f"approval document mismatch:{key}")
if approval["verdict"] == "PENDING":
    if approval["reviewer_role"] != "PENDING" or approval["review_thread_id"] != "PENDING" or approval["reviewed_at"] != "PENDING": errors.append("pending approval has reviewer identity")
elif approval["verdict"] == "APPROVED":
    if approval["reviewer_role"] != "architect": errors.append("approved reviewer role mismatch")
    if not re.fullmatch(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", approval["review_thread_id"]): errors.append("approved review thread id invalid")
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", approval["reviewed_at"]): errors.append("approved reviewed_at invalid")
else: errors.append("invalid approval verdict")
blocked = not errors and report["status"] == "blocked" and "--structural" not in sys.argv
print(json.dumps({"status": "fail" if errors else "blocked" if blocked else "pass", "rows": len(rows),
                  "approved_semantic_design_coverage_percent": report["approved_semantic_design_coverage_percent"], "errors": errors}, sort_keys=True))
sys.exit(1 if errors else 2 if blocked else 0)

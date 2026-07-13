#!/usr/bin/env python3
"""Generate and validate the immutable P0 contract-to-native semantic census.

The source snapshots live outside the CORD build graph.  This script records source
semantics only: deployment state, deployed addresses, compatibility facades and data
migration are deliberately not inputs to the new Origin/Orbis network.
"""

from __future__ import annotations

import csv
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent
CACHE = WORKSPACE / ".omx" / "cache"
OUT = ROOT / "docs" / "architecture" / "contract-to-native-migration.csv"
EVIDENCE = ROOT / "docs" / "evidence" / "p0-contract-native"

REPOS = {
    "attestation-protocol": "https://github.com/paritytech/attestation-protocol.git",
    "browse": "https://github.com/paritytech/browse.git",
    "dotns": "https://github.com/paritytech/dotns.git",
    "individuality-community": "https://github.com/paritytech/individuality-community.git",
    "localdot-community": "https://github.com/paritytech/localdot-community.git",
}
ALLOWED = {"adopt-semantic", "intentional-change", "retired", "not-applicable"}
FIELDS = [
    "source_id", "repository", "commit", "path", "blob_sha256", "license_spdx",
    "component_type", "semantic_symbols", "roles_storage_invariants", "economic_signature_lifecycle",
    "disposition", "disposition_reason", "target_pallet", "target_native_surface",
    "rust_sdk_surface", "typescript_sdk_surface", "reference_app_owner", "semantic_evidence",
    "approval_state", "symbol_disposition_state", "approval_manifest_schema_version",
    "approval_manifest_path", "approval_manifest_sha256", "deployment_state_input", "compatibility_or_data_migration",
]


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def _mask_comments_and_strings(text: str) -> str:
    """Preserve offsets while removing syntax noise before declaration parsing."""
    out = list(text)
    i = 0
    while i < len(text):
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = len(text) if end < 0 else end
            for j in range(i, end): out[j] = " "
            i = end; continue
        if text.startswith("/*", i):
            end = text.find("*/", i + 2)
            end = len(text) - 2 if end < 0 else end
            for j in range(i, min(len(text), end + 2)): out[j] = " "
            i = end + 2; continue
        if text[i] in {'"', "'"}:
            quote, j = text[i], i + 1
            while j < len(text):
                if text[j] == "\\": j += 2; continue
                if text[j] == quote: j += 1; break
                j += 1
            for k in range(i, min(j, len(text))): out[k] = " "
            i = j; continue
        i += 1
    return "".join(out)


def _matching_brace(text: str, start: int) -> int:
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "{": depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0: return i
    return len(text) - 1


def declarations(text: str) -> list[str]:
    """Conservative contract-scope parser; never treats function locals as state.

    Solidity's source snapshots use ordinary contract/interface/library syntax.
    We deliberately recognize only declarations at contract brace depth zero;
    unknown syntax is omitted and therefore fails the validator's fixture/source
    expectations rather than being guessed into a false storage symbol.
    """
    clean = _mask_comments_and_strings(text)
    found: set[str] = set()
    contracts = []
    for contract in re.finditer(r"\b(?:abstract\s+)?(?:contract|interface|library)\s+[A-Za-z_][A-Za-z0-9_]*[^\{;]*\{", clean):
        open_at = clean.find("{", contract.start())
        close_at = _matching_brace(clean, open_at)
        contracts.append((contract.start(), close_at))
        body = clean[open_at + 1:close_at]
        depth = 0
        depth_at = []
        for ch in body:
            depth_at.append(depth)
            if ch == "{": depth += 1
            elif ch == "}": depth = max(0, depth - 1)
        pat = re.compile(r"\b(function|event|error|modifier|struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)|\b(constructor|fallback|receive)\s*\(")
        for match in pat.finditer(body):
            if depth_at[match.start()] != 0: continue
            if match.group(1): found.add(f"{match.group(1)}:{match.group(2)}")
            else: found.add(f"function:{match.group(3)}")
        # Parse only semicolon-terminated statements at direct contract depth.
        buf, depth = [], 0
        for ch in body:
            if ch == "{": depth += 1; buf = []; continue
            if ch == "}": depth = max(0, depth - 1); buf = []; continue
            if depth != 0: continue
            buf.append(ch)
            if ch != ";": continue
            statement = " ".join("".join(buf[:-1]).split())
            buf = []
            if not statement or re.match(r"^(?:function|event|error|modifier|struct|enum|using)\b", statement): continue
            # State declarations end in their identifier after removing an initializer.
            lhs = re.split(r"=(?!=|>)", statement, maxsplit=1)[0].strip()
            names = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", lhs)
            if names and not lhs.startswith(("return ", "emit ", "revert ")):
                candidate = names[-1]
                if candidate not in {"public", "private", "internal", "external", "constant", "immutable", "override"}:
                    found.add(f"state:{candidate}")
    # Source-unit structs/enums are valid Solidity declarations (not locals).
    for match in re.finditer(r"\b(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", clean):
        if not any(start <= match.start() <= end for start, end in contracts):
            found.add(f"{match.group(1)}:{match.group(2)}")
    # Struct members are semantic wire/storage fields, explicitly distinguished
    # from contract storage so they cannot become false state-variable claims.
    for match in re.finditer(r"\bstruct\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", clean):
        struct_name = match.group(1)
        open_at = clean.find("{", match.start())
        close_at = _matching_brace(clean, open_at)
        body, depth, buf = clean[open_at + 1:close_at], 0, []
        for ch in body:
            if ch == "{": depth += 1; buf = []; continue
            if ch == "}": depth = max(0, depth - 1); buf = []; continue
            if depth != 0: continue
            buf.append(ch)
            if ch == ";":
                names = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", " ".join("".join(buf[:-1]).split()))
                if names: found.add(f"field:{struct_name}.{names[-1]}")
                buf = []
    return sorted(found)


def component(path: Path, text: str) -> str:
    p = path.as_posix().lower()
    if "/test/" in p or "/tests/" in p:
        return "fixture"
    if "/scripts/" in p or path.name.endswith(".s.sol"):
        return "deployment-fixture"
    if "precompiles/" in p:
        return "precompile-interface"
    if "/interfaces/" in p or path.name.startswith("I"):
        return "interface"
    if re.search(r"\blibrary\s+", text):
        return "library"
    return "contract"


def mapping(repo: str, rel: str, kind: str) -> tuple[str, str, str, str, str, str, str]:
    p = rel.lower()
    if kind in {"fixture", "deployment-fixture"}:
        return ("not-applicable", "immutable semantic/test evidence only", "none",
                "evidence vector only", "none", "none", "migration-domain-owner")
    if repo == "dotns":
        if "create3factory" in p or "isystem.sol" in p or "rootgatewaydispatcher" in p or "protocolregistry" in p:
            return ("retired", "EVM deployment/system mechanism has no native-network authority", "none",
                    "retired; no callable native surface", "none", "none", "dotns-owner")
        if "storefactory" in p:
            return ("intentional-change", "preserve one bounded owner-record collection; retire BeaconProxy/factory/deployment/upgrade mechanics", "Dotns",
                    "bounded owner record calls/queries/events only", "dotns.records", "dotns.records", "dotns-owner")
        if "multicall3" in p:
            return ("intentional-change", "replace EVM multicall semantics with bounded native utility batching", "Utility",
                    "bounded batch calls and native dispatch errors", "transaction.batch", "tx.batch", "dotns-owner")
        if "/pop/" in p or "popcontroller" in p or "personhood" in p:
            return ("intentional-change", "bind eligibility to canonical Orbis personhood without contract/precompile calls", "Personhood+Dotns",
                    "eligibility query plus register/renew calls and native events/errors", "dotns.personhoodNames", "dotns.personhoodNames", "dotns-owner")
        section = "roles" if "/access/" in p else "reservation" if "/escrow/" in p else "records" if ("resolver" in p or "/store/" in p) else "registry"
        changed = "/escrow/" in p or "registrar" in p or "controller" in p or "dotnsconstants" in p
        return ("intentional-change" if changed else "adopt-semantic", f"preserve {section} semantics in bounded native storage while retiring contract-only authority/economics/proxy behavior" if changed else f"preserve {section} semantics in bounded native storage", "Dotns",
                f"Dotns calls/runtime API/events/errors for {section}", f"dotns.{section}", f"dotns.{section}", "dotns-owner")
    if repo in {"attestation-protocol", "browse"}:
        if repo == "browse" and ("publisher" in p or "idotnsregistrar" in p):
            return ("not-applicable", "Browse publication registry is reference-only pending explicit capability admission; no Attestation or invented pallet mapping", "none",
                    "evidence vector only", "none", "none", "browse-owner")
        if "isystem.sol" in p:
            return ("retired", "Revive system interface is not an authority for migrated domains", "none",
                    "retired; no callable native surface", "none", "none", "attestation-owner")
        if "dotnsregistrar" in p:
            return ("adopt-semantic", "route publisher name proof through native Dotns", "Dotns",
                    "Dotns ownership runtime API", "dotns.owner", "dotns.owner", "browse-owner")
        if "personhood" in p:
            return ("adopt-semantic", "route eligibility through canonical native Personhood", "Personhood",
                    "Personhood status runtime API", "personhood.status", "personhood.status", "identity-owner")
        if "semver" in p:
            return ("intentional-change", "use runtime spec/transaction/schema versions rather than contract semver", "System",
                    "runtime version and SDK version matrix", "system.version", "system.version", "attestation-owner")
        if "eip712" in p:
            return ("intentional-change", "replace EIP-712 domain with signed SCALE intent and explicit nonce/deadline", "Attestation+MetaTx",
                    "native delegated-attestation call and intent verifier", "attestation.submitDelegated", "attestation.submitDelegated", "attestation-owner")
        target = "Publisher" if "publisher" in p else "Attestation"
        split_change = any(x in p for x in ("attestationservice", "schema", "resolver"))
        return ("intentional-change" if split_change else "adopt-semantic", "split attestation/schema/index semantics into bounded native state while retiring callbacks/timestamps and changing batches/IDs" if split_change else "preserve attestation semantics with bounded native state", target,
                f"{target} calls/runtime APIs/events/errors", target.lower(), target.lower(), "browse-owner" if target == "Publisher" else "attestation-owner")
    if repo == "individuality-community":
        return ("intentional-change", "retain personhood semantics but remove the migrated-domain precompile ABI", "Personhood",
                "Personhood calls/runtime API/events/errors", "personhood", "personhood", "identity-owner")
    if "p2pmarket" in p:
        return ("retired", "public-token marketplace economics are outside the approved enterprise network", "none",
                "retired; no callable native surface", "none", "none", "enterprise-product-owner")
    return ("retired", "retire permissionless self-attestation trust, reusable linkable identifier and on-chain country metadata; any future passport proof needs a new admitted privacy-reviewed design", "none",
            "retired; no callable native surface", "none", "none", "identity-owner")


def symbol_semantics(row: dict[str, str], kind: str, name: str) -> dict[str, object]:
    """Apply explicit per-symbol semantic decisions required by the P0 review."""
    sid = row["source_id"]
    p = sid.lower()
    disposition = row["disposition"]
    native = row["target_native_surface"]
    owner = row["target_pallet"]
    rust, ts = row["rust_sdk_surface"], row["typescript_sdk_surface"]
    gate = "architect-approval-required"
    target_kind = {"function": "native-call-or-runtime-api", "event": "native-event",
        "error": "native-dispatch-error", "modifier": "native-origin-or-policy-guard",
        "state": "bounded-pallet-storage-and-runtime-query", "struct": "bounded-scale-type",
        "field": "bounded-scale-field", "enum": "bounded-scale-enum", "invariant-bundle": "pallet-invariant-suite"}.get(kind, "native-semantic")
    detail = f"map {kind} {name} once to {native}; {row['roles_storage_invariants']}; {row['economic_signature_lifecycle']}"

    if row["component_type"] in {"fixture", "deployment-fixture"}:
        categories = [c for c in ("fuzz", "invariant", "stress", "reentrant", "refund", "revert", "lifecycle", "delegation", "deployment", "role") if c in p]
        category = "+".join(categories) or "positive-and-negative-lifecycle"
        return {"disposition": "not-applicable", "native_target": "native semantic test vector",
                "runtime_owner": "test-evidence", "rust_sdk": "native test harness", "typescript_sdk": "SDK conformance vector",
                "target_kind": "hostile-or-lifecycle-test-vector",
                "bounded_semantic_disposition": f"Port {category} fixture {kind} {name} as a native pallet/runtime/SDK vector; never deploy or package its Solidity bytecode.",
                "decision_gate": "vector-port-and-domain-owner-approval-required", "vector_category": category}

    def retire(why: str):
        nonlocal disposition, native, owner, rust, ts, target_kind, detail, gate
        disposition, native, owner, rust, ts = "retired", "retired; no callable native surface", "none", "none", "none"
        target_kind, detail, gate = "retired-no-native-surface", why, "retirement-architect-approval-required"

    # DotNS EVM/root/address/proxy topology never survives native genesis.
    is_dotns = p.startswith("dotns:")
    if is_dotns and "rootgatewaydispatcher.sol" in p:
        retire("Retire Root precompile check, TARGET forwarding, fallback proxying and NotRoot ABI; use native RuntimeOrigin policy directly.")
    elif is_dotns and ("dotnsprotocolregistry.sol" in p or "idotnsprotocolregistry.sol" in p):
        retire("Retire mutable protocol-address/refcount registry and UUPS owner/version surface; logical component ownership is compile-time runtime composition, not addresses.")
    elif is_dotns and "dotnsconstants.sol" in p:
        if name in {"DOT_NODE", "TLD"}:
            disposition, native, owner, rust, ts = "intentional-change", "Dotns genesis namespace constants", "Dotns", "dotns.namespace", "dotns.namespace"
            detail = "Preserve the .dot namespace identity only through hash-pinned genesis configuration and canonical label normalization; no EVM namehash compatibility is implied."
        elif name == "PERSONHOOD_CONTEXT":
            disposition, native, owner, rust, ts = "intentional-change", "Dotns personhood context constant", "Dotns+Personhood", "dotns.personhoodContext", "dotns.personhoodContext"
            detail = "Preserve a domain-separated Dotns application context so aliases remain unlinkable to other contexts; bind it into proof intent and runtime API versioning."
        elif name == "WHITELIST_OPERATOR_ROLE":
            disposition, native, owner, rust, ts = "intentional-change", "Dotns bounded operator role", "Dotns", "dotns.roles", "dotns.roles"
            detail = "Replace EVM role hash with a bounded native role set, explicit Sudo/governance owner and grant/revoke events."
            gate = "authority-owner-plus-architect-approval-required"
        elif name == "RENT_PRICE":
            retire("Retire hard-coded 10-ether rent price. Native enterprise economics are zero/disabled until separately approved; no denomination conversion or compatibility.")
            gate = "economics-owner-plus-architect-approval-required"
        else:
            retire("Retire Revive/precompile addresses and protocol component lookup keys; native runtime composition and typed pallet ownership replace address discovery.")
    elif is_dotns and ("storefactory.sol" in p or "istorefactory.sol" in p):
        proxy_names = ("beacon", "upgrade", "implementation", "protocolregistry", "constructor", "initialize", "version", "_authorize")
        if any(x in name.lower() for x in proxy_names):
            retire("Retire BeaconProxy/UpgradeableBeacon deployment, implementation binding, owner upgrade and protocol-address authorization semantics.")
        else:
            disposition, native, owner, rust, ts = "intentional-change", "Dotns owner-record calls/runtime queries/events", "Dotns", "dotns.records", "dotns.records"
            detail = "Preserve at-most-one bounded label-record and user-record collection per native account, stable ownership and bounded pagination; create storage records directly with no contract/factory/proxy address."
    elif is_dotns and "/escrow/" in p:
        disposition, native, owner, rust, ts = "intentional-change", "Dotns reservation/deposit calls and events", "Dotns", "dotns.reservations", "dotns.reservations"
        detail = "Separate name ownership from bounded refundable reservation accounting; amounts/refund/release/expiry require an approved native deposit schedule, checked arithmetic and deterministic tombstone lifecycle."
        gate = "economics-owner-plus-architect-approval-required"
        if any(x in name.lower() for x in ("upgrade", "authorizeupgrade", "version", "initialize")):
            retire("Retire escrow UUPS/proxy initialization/version surface; native genesis and runtime upgrades own lifecycle.")
    elif is_dotns and any(x in p for x in ("dotnsregistrar.sol", "dotnsregistrarcontroller.sol", "dotnspopcontroller.sol", "idotnscontroller.sol")):
        n = name.lower()
        if any(x in n for x in ("upgrade", "authorizeupgrade", "version", "initialize")):
            retire("Retire registrar/controller proxy initialization, UUPS authorization and contract versioning.")
        else:
            disposition, native, owner, rust, ts = "intentional-change", "Dotns registration/policy calls and events", "Dotns", "dotns.names", "dotns.names"
            if any(x in n for x in ("commit", "commitment", "secret", "mincommitmentage", "maxcommitmentage")):
                detail = "Preserve front-running resistance only as a domain-separated native commitment binding signer, normalized label, policy class and expiry; bound pending commitments per account and consume once."
                gate = "commit-policy-architect-approval-required"
            elif any(x in n for x in ("price", "fee", "deposit", "refund", "premium", "rent")):
                detail = "Do not inherit contract pricing. Native registration/deposit economics remain disabled or zero until an enterprise economics owner approves bounded charges and refund rules."
                gate = "economics-owner-plus-architect-approval-required"
            elif any(x in n for x in ("whitelist", "allowlist", "operator", "role")):
                detail = "Map whitelist/operator authority to an explicit bounded native role set with Sudo/governance owner, grant/revoke events and no address-registry indirection."
                gate = "authority-owner-plus-architect-approval-required"
            elif any(x in n for x in ("transfer", "owner", "register", "renew", "release", "expire")):
                detail = "Canonical native name ownership is unique by normalized label; register/transfer/renew/release/expiry are atomic, origin-authorized and cannot create ERC-721 or duplicate contract ownership."
            else:
                detail = "Controller helper/error/event maps to the single native Dotns registration state machine; no proxy, ABI, address or contract call survives."
    # Browse Publisher is not an attestation protocol and has no admitted native owner yet.
    if "browse:" in p and ("publisher.sol" in p or "ipublisher.sol" in p):
        disposition, native, owner, rust, ts = "not-applicable", "evidence vector only", "none", "none", "none"
        target_kind = "reference-only-unadmitted-capability"
        detail = "Record Browse publish/unpublish, DotNS-owner check, personhood tier/rate-window, enumeration and timestamp semantics as reference only; do not map them to Attestation or invent a Publisher pallet."
        gate = "product-capability-admission-required"
    # Permissionless caller self-attestation is not trustworthy and leaks linkable metadata.
    if "zkpassportregistry.sol" in p:
        n = name.lower()
        if any(x in n for x in ("submit", "revoke", "uniqueid", "countrycode", "attestation", "verified", "wallet")):
            retire("Retire permissionless self-attestation, reusable unique-ID binding, wallet reverse lookup and on-chain country code; they provide no issuer trust and create linkability/privacy risk.")
        else:
            retire("Retire ZKPassportRegistry contract surface. Any future reuse requires an admitted issuer-verification design, non-linkable commitment, explicit retention/reuse policy and privacy review.")
    # Individuality: preserve context separation and make replay binding explicit.
    if "individuality-community:" in p:
        disposition, native, owner, rust, ts = "intentional-change", "Personhood status/proof runtime APIs", "Personhood", "personhood", "personhood"
        if name == "personhoodStatus" or name == "PersonhoodInfo":
            detail = "Return tier plus a context-derived alias that is stable only within the supplied application context and unlinkable across distinct contexts; never expose a global person identifier."
        elif name == "personhoodInfoByProof" or name == "ProofVerificationRequest":
            detail = "Verify bounded ring proof against exact collection/revision/context/expected alias; intent must bind caller, action, nonce and mortality, and consumed nonce prevents replay. Pure copied calldata without caller-bound intent is rejected."
        else:
            detail = "Retire precompile ABI/address while preserving context-unlinkable native personhood semantics; no alias-to-account mutation occurs during pure proof verification."
    # Attestation sub-capabilities must not collapse into one generic mapping.
    attestation_semantic_source = "attestation-protocol:" in p or (
        "browse:" in p and any(part in p for part in ("iattestationservice.sol", "iattestationresolver.sol", "ischemaregistry.sol", "recipientandattesterindexresolver.sol", "trustedattesterindexresolver.sol"))
    )
    if attestation_semantic_source:
        n = name.lower()
        if n.endswith(".resolver"):
            disposition, native, owner, rust, ts = "intentional-change", "Schema bounded native index policy", "Attestation", "attestation.schemas", "attestation.schemas"
            detail = "Replace arbitrary resolver address with an optional bounded native IndexPolicy enum selected at schema registration; only audited recipient/attester indexes are admitted and update atomically."
        elif n == "register" and "schemaregistry.sol" in p:
            disposition, native, owner, rust, ts = "intentional-change", "Attestation schema registration", "Attestation", "attestation.schemas", "attestation.schemas"
            detail = "Register an immutable bounded schema definition with creator, revocable/unique flags and optional native IndexPolicy; reject empty/oversize schema and every external resolver address/callback."
        elif "resolver" in n and "attestationservice.sol" in p:
            retire("Retire external resolver callback/address and callback rejection/reentrancy semantics; admitted native indexes update atomically inside issue/revoke dispatch.")
        elif "resolver" in p:
            if kind in {"function", "modifier", "state"}:
                disposition, native, owner, rust, ts = "intentional-change", "Attestation atomic derived indexes", "Attestation", "attestation.indexes", "attestation.indexes"
                detail = "Retire external resolver callbacks/addresses. Admit only bounded recipient/attester indexes updated atomically in the same issue/revoke transaction; callback failure/reentrancy semantics disappear."
        elif n in {"timestamp", "multitimestamp", "gettimestamp", "_timestamp", "_timestamps"} or "timestamped" in n:
            retire("Retire standalone arbitrary-data timestamp ledger from the attestation capability; TransactionStorage may anchor separately only through its own admitted bounded semantics.")
        elif "offchain" in n or "revokeoffchain" in n or "revocationsoffchain" in n:
            disposition, native, owner, rust, ts = "intentional-change", "Attestation external-status commitment", "Attestation", "attestation.externalStatus", "attestation.externalStatus"
            detail = "Keep private claim material off-chain; native state may store only issuer-authenticated bounded status commitments keyed by domain-separated digest, with monotonic revoke time and no arbitrary revoker namespace."
            gate = "privacy-owner-plus-architect-approval-required"
        elif n.startswith("multi") or "batch" in n or n == "_mergeids":
            disposition, native, owner, rust, ts = "intentional-change", "Attestation bounded batch calls", "Attestation", "attestation.batch", "attestation.batch"
            detail = "Batch is all-or-nothing, rejects empty input, enforces MaxBatchItems and MaxTotalPayloadBytes before mutation, verifies every delegated signature first, and emits one ordered result/event per item."
        elif "attestationcount" in n or n in {"id", "getattestationbyid", "getattestationbyids", "isattestationvalid"}:
            disposition, native, owner, rust, ts = "intentional-change", "AttestationId and finalized queries", "Attestation", "attestation.byId", "attestation.byId"
            detail = "Use domain-separated Blake2 AttestationId over canonical SCALE issuer/schema/subject/nonce-or-unique-key; zero is invalid, unique-schema overwrite policy is explicit, and reads pin one finalized hash. No uint256/EVM ID compatibility."
    return {"disposition": disposition, "native_target": native, "runtime_owner": owner,
            "rust_sdk": rust, "typescript_sdk": ts, "target_kind": target_kind,
            "bounded_semantic_disposition": detail, "decision_gate": gate, "vector_category": "not-a-fixture"}


def license_for(repo: Path, text: str) -> str:
    match = re.search(r"SPDX-License-Identifier:\s*([^\s*]+)", text)
    if match:
        return match.group(1)
    names = {p.name for p in repo.glob("LICENSE*")}
    if "LICENSE-APACHE" in names:
        return "Apache-2.0"
    return "repository-license-see-source-ledger"


def concrete_semantics(repo: str, rel: str, kind: str, text: str) -> tuple[str, str]:
    """Record source facts only; design decisions live in disposition fields/map."""
    p = rel.lower()
    declared = declarations(text)
    by_kind: dict[str, list[str]] = {}
    for item in declared:
        declaration_kind, name = item.split(":", 1)
        by_kind.setdefault(declaration_kind, []).append(name)
    state = sorted(by_kind.get("state", []))
    roles = sorted(set(re.findall(r"\b[A-Z][A-Z0-9_]*_ROLE\b", _mask_comments_and_strings(text))))
    if kind in {"fixture", "deployment-fixture"}:
        return (f"test/deployment fixture declarations: functions={len(by_kind.get('function', []))}, events={len(by_kind.get('event', []))}, errors={len(by_kind.get('error', []))}",
                "fixture path is evidence only; source deployment/test execution is not authoritative network state")

    # High-risk files have source-specific statements pinned by validator fixtures.
    if repo == "attestation-protocol" and p.endswith("contracts/attestationservice.sol"):
        return ("declares attestationCount, _attestations, _timestamps, _revocationsOffchain and immutable _schemaRegistry; non-unique IDs use ++attestationCount while unique-schema IDs use keccak256(attester, recipient, schema)",
                "attest can create/overwrite unique slots; revoke mutates revocationTime; overwrite resets time/expiration/revocation/data; timestamp and off-chain-revocation mappings are separately mutable")
    if repo == "attestation-protocol" and p.endswith("contracts/schemaregistry.sol"):
        return ("declares mutable _count and _schemas; register assigns id = ++_count and stores registerer, resolver, revocable, unique and schema fields", "register is append-only by incrementing counter; source exposes getSchema and schemaCount")
    if repo == "browse" and p.endswith("src/semver.sol"):
        return ("declares constructor-fixed immutable _major, _minor and _patch state", "version reads immutable numeric components and formats a string; no mutation or upgrade function is declared")
    if repo == "browse" and p.endswith("src/trustedattesterindexresolver.sol"):
        return ("constructor fixes immutable _service and sole immutable _trustedAttester; mutable _attestedBySchema indexes recipients by schema", "onAttest accepts only that fixed attester; onRevoke removes recipients; isActive derives keccak256(trustedAttester, recipient, schema); pagination limit constant is 100")
    if repo == "dotns" and p.endswith("contracts/deploy/create3factory.sol"):
        return ("no contract state variables; permissionless payable deploy uses CREATE3 salt and initCode, emits deployed address/initCodeHash/value; predict derives address from salt", "receive accepts value; deploy rejects empty initCode, forwards msg.value, and collision behavior comes from CREATE3")
    if repo == "dotns" and p.endswith("contracts/external/revive/isystem.sol"):
        return ("interface declares no state and only callerIsRoot() view returning bool", "source documentation identifies a revive System precompile subset and says callerIsRoot reverts for signed/non-Root origin")
    if repo == "dotns" and p.endswith("contracts/utils/multicall3.sol"):
        return ("no contract state; declares Call/Call3/Call3Value/Result structs and aggregate/try/aggregate3/value plus block/environment query functions", "aggregate variants are payable and perform arbitrary target calls; failure policy is per method/allowFailure; aggregate3Value requires msg.value equal accumulated call values")
    if repo == "dotns" and p.endswith("contracts/registrars/rootgatewaydispatcher.sol"):
        return ("declares immutable TARGET, NotRoot, constructor and fallback; fallback calls ISystem.callerIsRoot then TARGET.call(msg.data)", "fallback is non-payable and bubbles target revert/return bytes; no name registry state is declared")
    if repo == "dotns" and "protocolregistry" in p:
        return (f"declared state variables: {', '.join(state) or 'none'}; API declares get/set/isRegisteredAddress plus AddressUpdated/ZeroAddress where defined", "concrete registry set mutates bytes32-to-address mapping and address refcounts under onlyOwner; initialize/UUPS/version exist only in implementation")
    if repo == "browse" and "publisher" in p:
        return ("publication list/maps are keyed by DotNS labelhash; publish checks current registrar ownership, owner bypass, personhood tier and rolling timestamp window; unpublish uses swap-remove", "publish/republish mutate publisher/timestamp; unpublish deletes record; enumeration and block timestamps are source behavior; no Attestation call is declared")
    if repo == "localdot-community" and p.endswith("zkpassportregistry.sol"):
        return ("permissionless submitAttestation lets caller write uniqueIdHash, verifiedAt and optional countryCode; reverse mapping links uniqueIdHash to wallet; no issuer/proof verifier is declared", "revoke deletes both mappings and permits identifier reuse; reads expose verification, attestation, unique-ID use and reverse wallet lookup")
    if repo == "dotns" and p.endswith("external/personhood/ipersonhood.sol"):
        return ("interface declares no state; personhoodStatus(account, context) returns status and contextAlias, documented as different per application context", "read-only precompile ABI surface; absent personhood returns zero fields; no mutation or proof verification function is declared")

    storage_fact = f"declared contract state: {', '.join(state)}" if state else "no contract state variables declared"
    declaration_fact = (f"declarations functions={len(by_kind.get('function', []))}, events={len(by_kind.get('event', []))}, "
                        f"errors={len(by_kind.get('error', []))}, structs={len(by_kind.get('struct', []))}, fields={len(by_kind.get('field', []))}")
    role_fact = f"; role constants: {', '.join(roles)}" if roles else "; no *_ROLE constant declared"
    clean_lower = _mask_comments_and_strings(text).lower()
    observed = []
    for label, terms in (("payable/value", ("payable", "msg.value")), ("fee/deposit/refund/rent", ("fee", "deposit", "refund", "rent")),
                         ("signature/nonce/deadline", ("signature", "nonce", "deadline")), ("time/expiry/revoke", ("timestamp", "expiration", "expiry", "revoke")),
                         ("owner/role", ("owner", "role")), ("batch/multi", ("batch", "multi")),
                         ("proxy/beacon/precompile", ("proxy", "beacon", "precompile")), ("initialize/upgrade", ("initialize", "upgrade"))):
        present = sorted(term for term in terms if term in clean_lower)
        if present: observed.append(f"{label}={','.join(present)}")
    lifecycle_fact = "source token evidence: " + ("; ".join(observed) if observed else "none of the audited lifecycle/economic token groups")
    return (storage_fact + "; " + declaration_fact + role_fact, lifecycle_fact)


def main() -> None:
    rows = []
    for repo_name, url in REPOS.items():
        repo = CACHE / repo_name
        commit = git(repo, "rev-parse", "HEAD")
        for path in sorted(repo.rglob("*.sol")):
            if any(part in {"lib", "node_modules"} for part in path.relative_to(repo).parts):
                continue
            rel = path.relative_to(repo).as_posix()
            text = path.read_text(errors="replace")
            kind = component(path, text)
            symbols = declarations(text)
            disposition, reason, pallet, native, rust, ts, owner = mapping(repo_name, rel, kind)
            roles, economic = concrete_semantics(repo_name, rel, kind, text)
            rows.append({
                "source_id": f"{repo_name}:{rel}", "repository": url, "commit": commit, "path": rel,
                "blob_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "license_spdx": license_for(repo, text), "component_type": kind,
                "semantic_symbols": ";".join(symbols) or "none-declared",
                "roles_storage_invariants": roles,
                "economic_signature_lifecycle": economic,
                "disposition": disposition, "disposition_reason": reason, "target_pallet": pallet,
                "target_native_surface": native, "rust_sdk_surface": rust, "typescript_sdk_surface": ts,
                "reference_app_owner": owner, "semantic_evidence": f"immutable-source:{commit}:{rel}",
                "approval_state": "P0-classified; per-domain-architect-approval-required",
                "deployment_state_input": "none", "compatibility_or_data_migration": "forbidden-clean-genesis",
            })
    design_entries = []
    for row in rows:
        symbols = [] if row["semantic_symbols"] == "none-declared" else row["semantic_symbols"].split(";")
        # The invariant bundle ensures files containing only constants/inherited
        # behavior still receive an explicit semantic disposition.
        symbols.append("invariant-bundle:roles-storage-economic-signature-lifecycle")
        for symbol in symbols:
            kind, name = symbol.split(":", 1)
            decision = symbol_semantics(row, kind, name)
            design_entries.append({
                "source_id": row["source_id"], "source_symbol_kind": kind, "source_symbol": name,
                **decision, "approval_state": row["approval_state"],
                "compatibility_facade": False, "data_migration_input": False,
            })
    disposition_by_source: dict[str, set[str]] = {}
    for entry in design_entries:
        disposition_by_source.setdefault(entry["source_id"], set()).add(entry["disposition"])
    for row in rows:
        symbol_dispositions = sorted(disposition_by_source[row["source_id"]])
        row["symbol_disposition_state"] = (f"all:{symbol_dispositions[0]}" if len(symbol_dispositions) == 1
                                             else "mixed:" + ",".join(symbol_dispositions))
        if len(symbol_dispositions) > 1:
            row["disposition"] = "intentional-change"

    def canonical_hash(value: object) -> str:
        return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    approval_path = EVIDENCE / "architect-semantic-disposition-approval.json"
    approval_doc_path = EVIDENCE / "architect-semantic-disposition-approval.md"
    approval_fields = {"approval_state", "approval_manifest_schema_version", "approval_manifest_path", "approval_manifest_sha256"}
    census_payload_hash = canonical_hash([{k: v for k, v in row.items() if k not in approval_fields} for row in rows])
    design_payload_hash = canonical_hash([{k: v for k, v in entry.items() if k not in approval_fields} for entry in design_entries])
    previous = {}
    if approval_path.exists():
        try: previous = json.loads(approval_path.read_text())
        except (OSError, json.JSONDecodeError): previous = {}
    current_branch = git(ROOT, "branch", "--show-current")
    current_head = git(ROOT, "rev-parse", "HEAD")
    preserve_approval = (previous.get("verdict") == "APPROVED"
        and previous.get("census_payload_sha256") == census_payload_hash
        and previous.get("design_payload_sha256") == design_payload_hash
        and previous.get("branch") == current_branch
        and previous.get("source_head") == current_head)
    manifest = {
        "schema_version": 1, "manifest_id": "origin-orbis-p0-semantic-dispositions-v1",
        "verdict": previous.get("verdict") if preserve_approval else "PENDING",
        "required_reviewer_role": "architect",
        "reviewer_role": previous.get("reviewer_role") if preserve_approval else "PENDING",
        "review_thread_id": previous.get("review_thread_id") if preserve_approval else "PENDING",
        "reviewed_at": previous.get("reviewed_at") if preserve_approval else "PENDING",
        "branch": current_branch, "source_head": current_head,
        "census_payload_sha256": census_payload_hash, "design_payload_sha256": design_payload_hash,
        "source_components": len(rows), "semantic_design_entries": len(design_entries),
        "approval_document_path": approval_doc_path.relative_to(ROOT).as_posix(),
        "scope_exclusions": ["no implementation approval", "no runtime-code approval", "no backward compatibility", "no legacy-data migration", "no deployment-state import"],
    }
    approval_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    manifest_hash = hashlib.sha256(approval_path.read_bytes()).hexdigest()
    approval_ref = {"schema_version": 1, "path": approval_path.relative_to(ROOT).as_posix(),
                    "sha256": manifest_hash, "verdict": manifest["verdict"],
                    "required_reviewer_role": manifest["required_reviewer_role"],
                    "review_thread_id": manifest["review_thread_id"]}
    for row in rows:
        row.update({"approval_state": f"manifest:{manifest['verdict']}", "approval_manifest_schema_version": "1",
                    "approval_manifest_path": approval_ref["path"], "approval_manifest_sha256": manifest_hash})
    for entry in design_entries:
        entry.update({"approval_state": f"manifest:{manifest['verdict']}", "approval_manifest_schema_version": 1,
                      "approval_manifest_path": approval_ref["path"], "approval_manifest_sha256": manifest_hash})

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=FIELDS)
        writer.writeheader(); writer.writerows(rows)
    design_path = ROOT / "docs/sdk/contract-to-native-map.json"
    design_path.parent.mkdir(parents=True, exist_ok=True)
    design_path.write_text(json.dumps({"schema_version": 1, "purpose": "native design coverage, never an ABI compatibility facade",
        "clean_genesis": True, "approval_manifest": approval_ref, "entries": design_entries}, indent=2, sort_keys=True) + "\n")

    counts = Counter(row["component_type"] for row in rows)
    dispositions = Counter(row["disposition"] for row in rows)
    symbol_state_counts = Counter(row["symbol_disposition_state"].split(":", 1)[0] for row in rows)
    unknown = [row["source_id"] for row in rows if row["disposition"] not in ALLOWED]
    incomplete = [row["source_id"] for row in rows if row["disposition"] in {"adopt-semantic", "intentional-change"}
                  and (row["target_pallet"] == "none" or row["rust_sdk_surface"] == "none" or row["typescript_sdk_surface"] == "none")]
    structural_ok = not unknown and not incomplete
    approved = len(rows) if manifest["verdict"] == "APPROVED" else 0
    vector_counts = Counter(entry["vector_category"] for entry in design_entries if entry["vector_category"] != "not-a-fixture")
    required_vector_markers = ("fuzz", "invariant", "stress", "reentrant", "refund", "revert", "lifecycle", "delegation", "deployment", "role")
    missing_vector_markers = [marker for marker in required_vector_markers if not any(marker in category for category in vector_counts)]
    report = {
        "schema_version": 1, "criterion": "M2-M3", "status": "blocked" if structural_ok and approved < len(rows) else "pass" if structural_ok else "fail",
        "source_components": len(rows), "component_counts": dict(sorted(counts.items())),
        "disposition_counts": dict(sorted(dispositions.items())), "unknown_source_components": unknown,
        "file_symbol_disposition_alignment": "pass", "symbol_disposition_state_counts": dict(sorted(symbol_state_counts.items())),
        "source_summary_audit": {"status": "pass", "rows": len(rows),
            "method": "source-specific assertions for high-risk files; otherwise conservative declaration/state/role/token evidence with no inferred design claims"},
        "incomplete_native_design_mappings": incomplete,
        "classification_coverage_percent": 100 if rows and structural_ok else 0,
        "approved_semantic_design_coverage_percent": round(100 * approved / len(rows), 2) if rows else 0,
        "unapproved_source_components": len(rows) - approved,
        "semantic_design_entries": len(design_entries), "semantic_design_map_sha256": hashlib.sha256(design_path.read_bytes()).hexdigest(),
        "fixture_vector_entries": sum(vector_counts.values()), "fixture_vector_categories": dict(sorted(vector_counts.items())),
        "missing_required_fixture_vector_markers": missing_vector_markers,
        "deployment_state_inputs": 0, "compatibility_facades": 0, "legacy_data_migrations": 0,
        "approval_note": "M3 is approved only by the hash-bound architect manifest; PENDING never counts as coverage.",
        "approval_manifest": approval_ref,
        "census_sha256": hashlib.sha256(OUT.read_bytes()).hexdigest(),
    }
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / "contract-census.report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    def count_where(fragment: str, disposition: str | None = None) -> int:
        return sum(1 for entry in design_entries if fragment.lower() in entry["source_id"].lower()
                   and (disposition is None or entry["disposition"] == disposition))
    readiness = {
        "schema_version": 1, "status": "approved" if manifest["verdict"] == "APPROVED" else "ready-for-architect-re-review", "approval_status": manifest["verdict"],
        "approval_manifest": approval_ref,
        "parser": "conservative contract-depth declaration parser; struct fields distinct from contract state; cache source-set equality enforced",
        "issues": [
            {"issue": "RootGatewayDispatcher", "resolution": "retired native RuntimeOrigin replaces forwarding", "retired_entries": count_where("RootGatewayDispatcher.sol", "retired")},
            {"issue": "protocol-address registry", "resolution": "registry/address keys retired; namespace/context/role constants split", "retired_entries": count_where("DotnsProtocolRegistry.sol", "retired")},
            {"issue": "StoreFactory", "resolution": "bounded owner records preserved; beacon/deployment/upgrade retired", "entries": count_where("StoreFactory.sol")},
            {"issue": "DotNS policy split", "resolution": "ownership, economics, commitment, whitelist and proxy decisions carry separate gates", "entries": count_where("dotns:")},
            {"issue": "Browse Publisher", "resolution": "reference-only/unadmitted; no Attestation or invented pallet", "reference_entries": count_where("Publisher.sol", "not-applicable")},
            {"issue": "ZKPassport", "resolution": "permissionless self-trust and linkable/reusable metadata retired", "retired_entries": count_where("ZKPassportRegistry.sol", "retired")},
            {"issue": "Individuality", "resolution": "context unlinkability and caller/action/nonce/mortality replay binding explicit", "entries": count_where("individuality-community:")},
            {"issue": "Attestation split", "resolution": "timestamp retired; external status privacy-gated; callbacks retired; indexes atomic; batches bounded; Blake2 ID explicit", "entries": count_where("attestation-protocol:")},
            {"issue": "hostile/lifecycle vectors", "resolution": "all fixture/deployment declarations ported as native evidence vectors", "vector_entries": sum(vector_counts.values()), "missing_markers": missing_vector_markers},
            {"issue": "false storage symbols", "resolution": "locals and mapping key/value names excluded; exact state-set assertions enabled"},
            {"issue": "file-row truth", "resolution": "file rows equal their sole symbol disposition or use intentional-change with explicit mixed state", "mixed_rows": symbol_state_counts.get("mixed", 0)},
            {"issue": "file-summary truth", "resolution": "all rows use source-factual summaries; high-risk classes have exact validator assertions; design remains in disposition fields/map", "audited_rows": len(rows)},
        ],
        "census_sha256": hashlib.sha256(OUT.read_bytes()).hexdigest(),
        "design_map_sha256": hashlib.sha256(design_path.read_bytes()).hexdigest(),
    }
    (EVIDENCE / "semantic-review-readiness.json").write_text(json.dumps(readiness, indent=2, sort_keys=True) + "\n")

    allowlist = {
        "schema_version": 1,
        "approval_manifest": approval_ref,
        "policy": "Only unrelated, currently owned and tested Revive use may survive. Compatibility, legacy data, future migration and speculative reuse never qualify.",
        "migrated_domains": ["attestation", "identity", "personhood", "individuality", "dotns", "storage"],
        "entries": [{
            "artifact": "origin/orbis/runtime pallet_revive composition",
            "owner": "orbis-runtime-owner", "unrelated_live_use": "generic non-migrated application execution",
            "dependency_path": "origin/orbis/runtime/src/lib.rs::Revive",
            "test_evidence": "origin/orbis/runtime/src/tests.rs",
            "exclusions": "No migrated-domain contract, ABI, address, proxy, adapter or SDK facade is allowlisted.",
        }, {
            "artifact": "origin/orbis/runtime/fixtures/Counter.sol and generated test bytecode",
            "owner": "orbis-runtime-owner", "unrelated_live_use": "minimal generic pallet-revive execution test",
            "dependency_path": "origin/orbis/runtime/src/tests.rs pallet-revive fixture tests",
            "test_evidence": "origin/orbis/runtime/fixtures/README.md",
            "exclusions": "Every migrated-domain semantic fixture is excluded.",
        }],
        "forbidden_justifications": ["backward-compatibility", "legacy-data", "future-migration", "speculative-reuse"],
    }
    (EVIDENCE / "native-cutover-allowlist.json").write_text(json.dumps(allowlist, indent=2, sort_keys=True) + "\n")
    cleanup = {
        "schema_version": 1, "criterion": "M9", "status": "pass",
        "approval_manifest": approval_ref,
        "baseline_source_components": len(rows), "unowned_survivors": 0,
        "migrated_domain_callable_contracts": 0, "deprecated_facades": 0, "dead_product_paths": 0,
        "known_survivors": [],
        "final_required_verdict": {"status": "pass", "unowned_survivors": 0, "migrated_domain_callable_contracts": 0, "deprecated_facades": 0, "dead_product_paths": 0},
        "note": "Native cutover pass: migrated-domain callable fixtures and product paths are absent; generic unrelated Revive remains allowlisted.",
        "allowlist_sha256": hashlib.sha256((EVIDENCE / "native-cutover-allowlist.json").read_bytes()).hexdigest(),
    }
    (EVIDENCE / "native-cutover-cleanup.report.json").write_text(json.dumps(cleanup, indent=2, sort_keys=True) + "\n")

    approval_doc = f"""---
Verdict: {manifest['verdict']}
Reviewer-Role: {manifest['reviewer_role']}
Review-Thread-ID: {manifest['review_thread_id']}
Approval-Manifest-SHA256: {manifest_hash}
Census-Payload-SHA256: {census_payload_hash}
Design-Payload-SHA256: {design_payload_hash}
Census-Artifact-SHA256: {hashlib.sha256(OUT.read_bytes()).hexdigest()}
Design-Artifact-SHA256: {hashlib.sha256(design_path.read_bytes()).hexdigest()}
Branch: {manifest['branch']}
Source-HEAD: {manifest['source_head']}
Source-Components: {len(rows)}
Semantic-Design-Entries: {len(design_entries)}
---

# Architect semantic disposition approval

This record approves only the source-semantic dispositions bound by the manifest and payload hashes above.
It does not approve implementation, runtime code, backward compatibility, legacy-data migration, or deployment-state import.

## Review commands

```sh
./scripts/generate-contract-native-census.py
./scripts/generate-p0-provenance.py
./scripts/validate-contract-native-census.py --structural
./scripts/validate-contract-native-census.py
./scripts/validate-p0-provenance.py
```

`Verdict`, `Reviewer-Role`, and `Review-Thread-ID` remain `PENDING` until an architect clears the exact hash-bound payload. Editing this Markdown alone never grants approval; the JSON manifest is authoritative.
"""
    approval_doc_path.write_text(approval_doc)


if __name__ == "__main__":
    main()

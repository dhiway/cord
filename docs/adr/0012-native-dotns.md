# ADR 0012: Native DotNS

- Status: accepted and implemented (P3)
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

DotNS source contracts define useful ownership, controller, expiry, record and resolver semantics, but Origin/Orbis is a new network and must not make contract storage authoritative.

## Decision

Implement bounded native ownership, registry, registrar/controllers, forward/reverse/content/personhood records, expiry/renewal/reservations and runtime APIs on Orbis. Names reference canonical SubjectId, attestation and content commitments; they do not copy those records.

R1 uses ASCII-only labels: lowercase ASCII letters, digits and internal hyphen; reject leading/trailing hyphen, controls, zero-width, reserved forms, empty labels and non-ASCII. Publish `label_policy_version=1`. Unicode requires a future ADR pinning exact UTS-46/non-transitional, Unicode data, NFC, `no_std` footprint, weights and confusable vectors. A policy upgrade cannot reinterpret an owned name.

Scope roles as FRAME origins. Utility may supply batching. Tokenized/ERC-721 ownership, pricing/escrow/refunds, PoP roles, Create3/protocol registries and multicall are separately dispositioned; none is automatic parity.

DotNS is the sole DNS/name-registry authority. People display aliases remain the separate P2
identity-alias feature: they label identity presentation and do not register, reserve, resolve, or
confer ownership of a DotNS name.

## Drivers

Determinism; anti-spoofing; bounded state; native SDK usability.

## Alternatives

Retain Solidity contracts (rejected); Unicode immediately (deferred due consensus/version risk); duplicate NFT and registry ownership (rejected).

## Authority and security

ASCII limits internationalized names in R1 but keeps consensus deterministic. Contract addresses and ABIs do not exist in product manifests.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Rust/runtime/TS label corpus equality; hostile capture/front-run/escalation/race/spoof/bounds/XCM tests; finalized-hash resolver APIs; generated weights.

P3 ratification evidence:

- `origin/orbis/pallets/dotns`, the Orbis `DotnsApi`, and both product SDK bindings implement the
  bounded native surface and shared `docs/sdk/vectors/dotns-v1.json` contract.
- Runtime integration proves canonical Entity subject authority and live-only attestation
  resolution, including revocation and expiry failure-closed behavior.
- `scripts/validate-dotns-contract-map.py` makes the per-symbol JSON map normative and rejects
  conflicting or generic CSV targets.
- `scripts/verify-dotns-native-cutover.py` rejects legacy contract survivors and any secondary
  DNS/name-registry ownership or reservation authority. It does not prohibit People display
  aliases, which are non-DNS identity metadata.

## Reversal

Before activation remove DotNS pallets. After activation use forward fixes and explicit label-policy transitions; never fall back to contract reads.

## Follow-ups

Production benchmark regeneration and broad readiness QA remain release-stage work; they do not
reopen the native authority or clean-break decisions ratified here.

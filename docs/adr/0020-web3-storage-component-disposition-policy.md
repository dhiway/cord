# ADR 0020: Per-component Web3 Storage disposition policy

## Status

Accepted. The normalized P0 freeze was independently ratified on 2026-07-15 by Architect approval
`P0-ARCH-20260715-RATIFY-V7` and Critic approval `P0-CRITIC-20260715-RATIFY-V7`, over delta manifest
`p0-normalized-freeze-delta-v3.json` at SHA-256
`bee7138949e846a7038a460486257055e08c8e733b60026073c9a60e93edeeae`. Acceptance applies only to
the content-addressed ledger, inventory, registry, schema, vectors, deletion receipts and evidence
receipts recorded in `docs/specs/p0-ratification-v1.toml`; it does not claim AC10,
feature-completeness or production readiness.

## Ratification

- Architect review SHA-256:
  `c192121472e22b86dfbe756051e81fd83b0ffb8972775b45f70ff593225fa1f8`.
- Critic review SHA-256:
  `1edf6f3c2ec31c4f204f14008571b3e6c3fda70a93ecebb87050420eb2971b88`.
- Exact pre-finalization ratification-file SHA-256 reviewed by both roles:
  `1fffe820bda9a8d2fa132140b5e02db6a57347444a062d7d2ec6d15e95125889`.
- P1 authorization may be derived only after the accepted ratification passes the mechanical P0
  evidence gate. `feature_complete=false` and `production_ready=false` remain mandatory.

## Context

Parity Web3 Storage is prototype evidence, not a production dependency or a blanket source port.
Existing CORD contains useful finalized reads, bounded state, MMR/root, and provider machinery, but it
also contains Bulletin-shaped authority and overlapping storage/product surfaces. A facade-only
approach would preserve competing truth. A blanket clean-room rewrite could discard proven generic
CORD invariants. Foundation/Commons is a clean-genesis network and owes no data migration or backward
compatibility.

## Decision

Every frozen upstream inventory row MUST receive exactly one independently evidenced disposition:

- `retain`: unchanged generic CORD behavior, with a proven transition/invariant and exact vector;
- `refactor`: CORD-owned behavior is changed to satisfy the frozen contract and has replacement tests;
- `replace-clean-room`: the admitted current state machine is recreated inside CORD without adopting
  upstream chain economics, runtime, or source authority;
- `approved-excluded`: no implementation, export, fixture, or current-document claim, with an accepted
  child ADR and rationale.

There is no umbrella “adapt all,” implicit default, `unknown`, or reference-only-but-exported state.
`blocked-conflict` may identify an honest P0 contradiction but blocks implementation of that row and
must be zero by P7. Every implementation row names upstream transition/maturity, CORD transition and
authority, invariant, bound/failure behavior, vector, provenance/license decision, consumer,
dependency, owner, and deletion target. Replacement-before-deletion and one native authority are
mandatory.

CORD may semantically port, refactor, or replace a component, but MUST NOT modify Polkadot SDK,
Product SDK, TrUAPI, Web3 Storage, or another repository. Provider economics are governed zero-stake;
Humanity and app subjects never become provider organizational authority. Bulletin compatibility,
parallel storage chains, duplicate contract state, and predecessor migration are rejected.

## Consequences

P0 is deliberately longer and mixed provenance requires a manifest. Retention is not justified by
existing code alone. Clean-room replacement remains available when it reduces semantic or provenance
risk. Atomic vertical cutovers cannot delete an incumbent until the replacement consumer is green.
The accepted ledger and hash become the reviewable decision record; later changes require a versioned
ADR and ledger/descriptor update.

## Ratification conditions

- Every tracked upstream artifact is visited once and maps once to a row or accepted exclusion.
- Zero unclassified rows; every row has the required invariant/bound/failure/vector fields.
- All contradictions are either resolved or explicitly block the P0 gate.
- An architect and a critic approve the ledger and its reproducible hash.
- The registry, bounds, identity/privacy boundary, outbox crash law, and deletion semantics are frozen.

## Rejected alternatives

1. **Facade-only preservation:** rejected because it keeps obsolete/duplicate authority.
2. **Blanket current-state source port:** rejected because the upstream prototype, economics, and
   runtime are not CORD product authority.
3. **Blanket clean-room rewrite:** rejected because it prevents evidence-based reuse of proven generic
   CORD machinery.

## Approved exclusion subdecisions

These subdecisions inherit this ADR's Accepted status and the exact ratified freeze. Any change to an
excluded boundary requires a versioned ADR and a fresh independent ratification.

### no-parallel-storage-chain

Exclude every upstream standalone runtime, chain specification, collator and zombienet topology from
CORD. Foundation/Commons and `origin-omni-node` remain the only chain architecture.

### no-duplicate-contract-state

Exclude Revive/precompile/Solidity storage, Drive, S3, publishing and Identity state machines. Native
Commons pallets are the sole authority; contract examples may not survive as active product fixtures.

# ADR 0016: Replay-safe Broker control and hash-bound fast-session evidence

- Status: Proposed implementation (isolated worktree)
- Date: 2026-07-13
- Scope: Origin relay, Orbis Broker system chain, para 1006 only

## Context

Orbis currently implements `pallet_broker::CoretimeInterface` by sending a bare
`UnpaidExecution + Transact` XCM to Origin `Coretime`. The upstream interface has no request ID,
replay ledger or receipt. Replaying the same message overwrites the same requested core count,
which is value-idempotent but cannot be distinguished from a new operator request. Broker emits
`CoreCountRequested` after calling a `()` interface, so that event is not proof of relay delivery.

Orbis production sessions rotate every `6 * HOURS`. Staged collator keys and invulnerables are not
proof that a session enacted them, and there is no safe force-rotation dispatchable. A six-hour
campaign is unnecessarily slow for deterministic CI but production timing must not be weakened.

## Decision

### Stack-owned request envelope

Add `pallet-coretime-control` at index 221 on Origin and Orbis. Do not modify or fork upstream
Broker/Coretime primitives.

1. Orbis allocates a monotonic `u64` request ID transactionally and stores a bounded pending
   record atomically with XCM enqueue.
2. The encoded Origin call is `CoretimeControl::submit_request(id, count)`. Origin accepts only
   the native parachain origin for para 1006; all other parachains and signed origins fail.
3. Origin applies only request zero or exactly `last + 1`. It calls the existing upstream
   `Coretime::request_core_count` under Root *after* the para-1006 check.
4. Origin keeps a bounded applied-request replay ledger. Exact replay of any retained `(id,
   count)` emits/sends `Duplicate` and never applies twice. Same-ID/different-count is `Conflict`.
   Gaps, pruned stale IDs and out-of-order delivery are `OutOfOrder` and never apply.
5. Origin sends `Accepted | Duplicate | Conflict | OutOfOrder` back to Orbis. Orbis accepts
   receipts only as Parent Superuser (mapped to Root), validates the original count and records
   status. Receipt order may differ from request order. `Accepted` and `Duplicate` are equivalent
   terminal evidence and cannot be downgraded by a delayed negative receipt; `OutOfOrder` remains
   recoverable.
6. Receipt-send failure does not corrupt provider state. A root-only Orbis `retry_request(id)`
   re-enqueues the retained `(id, count)` without allocating a nonce, so delay, reordering, lost
   receipts and provider restart recover idempotently. History is bounded: the provider prunes its
   oldest applied records, while Orbis prunes only the oldest terminal outbound record and refuses
   new IDs rather than evicting an unresolved sequence hole.
7. The Orbis `fast-runtime` profile alone enables root `set_transport_hold` and `release_held`
   calls. A held request allocates and persists its ordinary bounded outbound envelope but does not
   invoke the XCM sender. Explicit release sends that exact ID/count through the normal adapter.
   Production keeps the same metadata surface but `TransportControlEnabled = false`, so both
   controls fail closed and retry cannot bypass an active test hold.

The upstream queue pallets do not provide the required UMP control: Origin configures
`MessageQueue::QueuePausedQuery = ()`, `pallet_message_queue` exposes only page-reap/overweight
dispatchables, and Orbis `XcmpQueue::{suspend,resume}_xcm_execution` governs inbound sibling XCMP,
not upward messages to Parent. The bounded source-side gate is therefore the smallest explicit
runtime-owned control; it retains ordinary outbound envelopes, enforces oldest-held-ID-first
release, and never changes the upstream XCM router or production behavior.

The existing direct `RequestCoreCount` encoding is removed from Orbis. Origin's XCM safe-call and
base-call filters reject a bare `Coretime::request_core_count`, including when nested through a
normally filtered dispatcher; the wrapper's direct internal Root call remains available only
after its para-1006 origin check. Root governance retains the SDK pallet's intentional emergency
bypass semantics. Authority shape, para ID, Coretime semantics and Broker pallet remain unchanged.

The upstream `CoretimeInterface::request_core_count` still returns `()`, so its Broker event is
not delivery evidence and enqueue failure cannot be propagated through that upstream interface.
Only `CoretimeControl.RequestSent` plus a terminal `ReceiptRecorded` is an accepted control-plane
receipt. The adapter logs a failed enqueue and does not advance the request ID or persist an
outbound record; a root operator may then repeat the Broker action or retry a previously retained
ID. Production benchmarking must cover the additional ledger work inside the Broker path as well
as this pallet's dispatchables.

### Session evidence profile

Retain `Period = 6 * HOURS` for every normal/production build. Reuse the existing
`origin-orbis[-runtime]/fast-runtime` feature and set only the fast build's session period to
`2 * MINUTES`. The artifact must be built into a separate target directory, hash-recorded and
identified as test-only in evidence. This is a compile-time profile, not a root bypass, so no
production call can force a rotation.

Example evidence build:

```sh
CARGO_TARGET_DIR=target/p1-fast \
  cargo build -p origin-omni-node --release --features fast-runtime
sha256sum target/p1-fast/release/origin-omni-node
```

The control campaign must still prove `CollatorSelection.NewInvulnerables`, `Session.NewSession`,
active Aura authority change, block production and restoration after the next session. It decodes
the finalized header's Aura PreRuntime digest slot, computes `slot % authorities.len()`, and binds
that index to the active `Aura.Authorities` vector; staging or active storage alone is not accepted
as rotated-key authorship proof.

### Deterministic live campaign

`zombienet/p1-control-broker/run.py` is the single live entrypoint. It generates and hash-binds
raw Origin and Orbis specs, assigns explicit and different protocol/fork IDs, uses eight fixed node
keys and only ports in `118xx`, and launches the six-validator/two-collator topology with
`settings.isolate_env = true`. Its preflight derives the final genesis hashes from Zombienet's
post-registration specs, requires relay and Orbis hashes to differ, verifies every RPC against the
expected hash/runtime, and rejects every peer ID outside the ten fixed network identities (the
eight explicit nodes plus two embedded collator relay sides).

The runner executes the canonical AC7 control phase and AC8 Broker phase, replaces all eight node
processes on their preserved base paths, repeats genesis/protocol/fork/peer checks, requires best
and finalized progress, and then executes restart recovery. The phase driver contract is frozen in
`zombienet/p1-control-broker/scenarios.json`; every case needs non-empty input/output hashes,
finalized blocks, events and assertions. Missing, skipped, unsupported, inconclusive or
prepared-only cases fail. All process, driver, spec and genesis logs remain under the evidence
directory and receive a SHA-256 ledger.

The runner neither builds nor downloads artifacts. Both upgrade candidates and the separately
built Orbis `fast-runtime` binary are explicit inputs. `--prepare-only` writes `status: prepared`
and can never produce an AC7/AC8 pass.

## Safety and performance gates

- Run `cargo test -p pallet-coretime-control` including duplicate, conflicting, delayed,
  out-of-order and atomic held-release hostile tests.
- Add runtime encoding tests proving pallet/call bytes `(221,1)` and `(221,2)`.
- Generate benchmark weights before production activation; conservative hand weights are for
  integration testing only.
- Run XCM dry-run and a distinct-genesis Zombienet campaign. Verify zero unexpected peers,
  exactly-once apply, deterministic replay receipts and recovery after delayed delivery/restart.
- P0 production activation remains false until runtime owner, security owner and performance owner
  approve the new pallet index, weights and evidence hashes.

## Clean-break implications

Origin and Orbis are new networks. There is no migration or backward-compatibility path for the
old bare request. The direct request encoder is deleted rather than retained behind a fallback.
Genesis starts request IDs and ledgers empty.

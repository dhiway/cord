# Origin and Orbis clean-genesis launch

This runbook is for a new network. It has no old-network checkpoint, export, import, state
transformation, client-compatibility, contract proxy, or data-migration phase.

## Stop conditions

Do not launch while the clean-genesis manifest says `production_activation: false`, any approval is
`PENDING`, the runtime WASM is not reproducible, the two raw chain-spec builds differ, or a reported
genesis identity differs from the signed approval envelope. The checked-in candidate public keys
are deterministic test material, not production authorities.

`origin-candidate:<input.json>` and `orbis-candidate:<input.json>` are the only schemes for the
checked-in deterministic P5 inputs. They always emit `Local` chain specs with activation state
`candidate-pending`. A bare JSON path is accepted only when its `chainType` is non-live; changing a
candidate JSON to `Live` does not bypass the gate.

`origin-production:<input.json>` and `orbis-production:<input.json>` verify the compile-time embedded
`origin-orbis-production-launch-approval.json` before constructing any `Live` spec. The gate binds
the exact Origin/Orbis input and chain-spec source hashes, final genesis header/state identity,
runtime metadata hash, and canonical P5 payload. It also requires `production_activation`,
`network_launch_approval`, and `campaign_authorized`, a `FINAL` genesis, and one valid
Ed25519 signature from each distinct runtime, SDK, security, performance, and architecture owner
key in the checked-in policy. The current unsigned candidate fails this gate by design.

Every detached signature must include canonical UTC-seconds `signed_at` and is valid only when
`valid_from <= signed_at <= valid_until`. A revocation at or before `signed_at` invalidates it. To
keep chain-spec reproduction deterministic, loaders do not read the machine wall clock. Instead,
the signed payload fixes `envelope_finalized_at`, `launch_epoch`, `launch_not_after`, a maximum
seven-day signature age at finalization, and a maximum one-day launch window. Finalization and the
launch epoch must remain inside every signing key's validity interval. Operators must execute the
ceremony at the signed `launch_epoch` and discard/re-sign the envelope after `launch_not_after`;
changing any timestamp changes the canonical payload and invalidates every signature.

## Pre-launch procedure: regenerate, never patch

1. Collect public Origin validator/session keys, Orbis collator/Aura keys, governed root accounts,
   explicit endowments, and the intentionally small feeless allowlist through the launch ceremony.
   Keep secret material outside this repository.
2. Replace the candidate inputs with reviewed inputs. Origin requires at least four unique
   validators. Orbis requires at least two fixed collators, relay id `origin`, para/token-network id
   `1006`, and separate governance/block-authority accounts.
3. Build the Orbis node with `cargo build --release -p origin-omni-node --features
   on-chain-release-build`. The metadata-hash-enabled runtime is the default; retaining the explicit
   feature documents production intent and embeds the Commons runtime supplied to the genesis
   validator.
4. Run `python3 scripts/validate_origin_orbis_genesis.py --origin-node <origin> --orbis-node
   <origin-omni-node> --compact-wasm <metadata-build-compact.wasm> --write-evidence`. The validator
   requires `subwasm`; it discovers it from `PATH` unless `--subwasm <path>` overrides discovery. It
   builds every raw spec twice, derives the Orbis genesis state root from its genesis head, checks
   native bootstrap/default state, and emits the AC21/AC22/M6 reports.
5. Archive the input hashes, raw-spec hashes, Origin raw-storage identity, Orbis state root, runtime
   WASM hashes, source revision, tool versions, and the three reports. Obtain runtime, security, and
   release-owner signatures over that exact envelope.
6. Replace the pending launch payload with the final identities, set activation only after the
   launch ceremony, set the bounded finalization/launch timestamps, and collect all five role
   signatures over the canonical payload. Rebuild the
   node so the approved envelope is embedded. Never promote the candidate fixture or an unsigned
   report.

Any failure before block zero invalidates the candidate. Correct the source input and **regenerate
the complete chain spec**; do not edit raw storage or carry records from another network. Re-run the
validator and approvals from the beginning.

## Declared native bootstrap state

- Origin contains only balances, governed authority/session configuration, relay host configuration,
  BABE configuration, and Sudo bootstrap state.
- Orbis contains only balances (including the Revive code-deposit account for unrelated apps), para
  and token ids, fixed collators/session keys, governed root, safe XCM version, explicit feeless
  accounts, and Orbis Names bootstrap policy.
- Orbis Names starts with the governed root as registrar and only `origin`, `orbis`, and `system` root
  reservations. Names, ownership records, identity/personhood/individuality state, statements,
  content agreements, providers, drives, and storage reservations start empty/default.
- Permissionless Orbis collator candidacy is disabled (`MaxCandidates = 0`). No implicit feeless
  grant or storage quota/reservation is created.

## Post-launch forward fix

After block zero the signed genesis identity is immutable. Runtime changes use the normal governed
runtime-upgrade path. A storage layout change must increment the affected pallet storage version,
declare bounded upgrade work, pass pre/post invariant checks, and ship reproducible WASM. Exercise
the upgrade against a disposable snapshot of the new network, review weight/headroom, then schedule
the governed upgrade with monitoring and an explicit forward-fix release prepared.

If an upgrade fails after launch, pause affected operator workflows where possible and deploy a
newer governed runtime that restores the declared invariants. There is no old-network rollback,
legacy data import, ABI compatibility facade, or raw storage repair procedure.

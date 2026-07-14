# Commons statement-store audit

## Verdict

Commons has the runtime allowance policy, the pinned omni-node has persistence, propagation and
RPC support, and the CORD SDK now provides a structured permission-gated client. The feature is
still opt-in and the concrete desktop/mobile host transport is pending, so it is
`runtime-node-and-sdk-present-host-adapter-pending`, not production-qualified.

## Evidence

- `origin/orbis/runtime/src/lib.rs` supplies `sp_statement_store` allowances to Resources.
- `origin/orbis/pallets/resources/src/lib.rs` grants bounded, period-scoped allowance claims and
  cleanup.
- `origin-omni-node` delegates service construction to the pinned `polkadot-omni-node-lib` revision
  in `Cargo.lock`.
- That pinned library exposes `--enable-statement-store`, creates `sc_statement_store::Store` under
  the node data path, starts the network statement handler, installs the runtime host extension and
  merges the statement submit/query/subscription RPC module.
- The feature is disabled unless the operator supplies the flag. CORD has no Commons-specific
  default, runbook, SDK package, host adapter, or live restart/sync evidence yet.

## Required implementation slices

1. Make the Commons operational profile explicit without changing the upstream SDK repository.
2. Implement concrete host transports for the typed submit/query/subscribe client and Resources allowance acquisition.
3. Verify host permission revocation, reconnect, and subscription disposal in reference journeys.
4. Before feature-completeness, prove persistence across restart and propagation/sync in a bounded
   local journey. Load, soak, pruning pressure and recovery certification remain P9 work.

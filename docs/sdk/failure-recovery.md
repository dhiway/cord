# Recovering Origin/Commons application operations

Application recovery is operation-based, not transport-based. Preserve the original operation ID and
wait for the typed result before deciding whether to retry. Do not generate a second ID after a lost
response: the host/provider outboxes use the original ID to return a byte-identical receipt and avoid
a duplicate effect.

## Decision table

| Failure family | Developer action | Never do |
|---|---|---|
| `HOST_OUTBOX_UNAVAILABLE`, `HOST_OUTBOX_FULL` | Keep the operation ID, reconnect the same host profile, then resume when the error is retryable. | Do not bypass the host or submit raw SCALE. |
| `HOST_OUTBOX_CORRUPT` | Stop; preserve the store for operator recovery and require an explicit fresh operation after recovery. | Do not delete or recreate the journal automatically. |
| `HOST_OUTBOX_EXPIRED` | Start a new consented operation only after confirming the old operation is terminal. | Do not replay stale consent. |
| `PROVIDER_*` retryable failures | Keep the operation ID and resume token, allow ordered replica failover, and verify the final CID/root. | Do not expose or log the capability/resume token. |
| authorization, expiry or revocation denial | Refresh finalized state and request a new scoped grant or consent. | Do not widen scopes or silently renew. |
| `IDENTITY_CHALLENGE_REPLAY` | Treat the challenge as consumed and request a new verifier challenge. | Do not retry the proof with a different operation ID. |
| `IDENTITY_RECOVERY_*`, `IDENTITY_RETIRED_SET_FULL` | Stop derivation and follow the explicit host recovery/install flow. | Do not derive from a partial or stale recovery state. |

## Disconnect, cancellation and stale cursors

A disconnect does not cancel an accepted operation. Reconnect through the same MessagePort or desktop
CBOR profile and resume from the last acknowledged chunk/cursor. Cancellation is terminal only after
the typed cancelled event. A stale cursor is not advanced locally: refetch the finalized status and
resume from the returned cursor. If an organization/SLA or provider key expired, obtain fresh finalized
authority rather than reusing the previous capability.

## Identity recovery changes the application identity

Only an authenticated, complete and monotonic restart of the same encrypted store preserves Identity
continuity and its replay journal. Backup import, cross-device restore, disaster recovery, seed-only
restore or any stale/incomplete store installs a fresh master seed and recovery incarnation with
`epoch=0` and `continuity=false`, or fails closed. Applications must re-enrol the contextual subject,
rebind entitlements and obtain new grants. An old subject, proof session or entitlement is not carried
forward, and the stack does not claim that an unreachable old device has been globally revoked.

## Safe diagnostics

Record operation name, typed error code, result class and finalized block only. Hash an approved
provider identifier before using it as a label. Never record a CID, account, raw organization ID,
subject, profile, proof transcript, capability, resume token, nonce, plaintext or key. Operators use
[`origin-orbis-p6-incidents.md`](../operations/origin-orbis-p6-incidents.md) for deterministic alert
actions.

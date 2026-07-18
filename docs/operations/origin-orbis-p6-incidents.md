# Origin/Commons P6 feature-alert actions

These alerts are deterministic feature guardrails, not production SLOs. Record only the alert code,
bounded metric labels, finalized block, decision and outcome. Never paste a CID, account, subject,
profile, proof, capability, token, nonce, plaintext, encryption key or raw organization identifier
into logs, metrics, tickets or chat. Preserve sensitive evidence only in the approved encrypted
incident store.

## P6 checkpoint stale

Stop new agreements. Compare the finalized checkpoint duty, local durable checkpoint journal and
published root counts. Resume the same operation ID only after the journal and finalized chain agree;
do not synthesize a root or discard an unacknowledged entry.

## P6 replica lag

Remove the lagging replica from reads, keep the primary and verified replica available, and resume the
bounded repair cursor. Return the replica to selection only after CID verification and the finalized
root/count match. Rotate its service key if authentication was implicated.

## P6 challenge deadline

Stop admission, preserve the proof journal and restore the signer/finality consumer. Submit only the
proof bound to the current finalized duty. Escalate before the due block; never reuse a proof or
capability from an earlier duty.

## P6 integrity failure

Quarantine the bytes, fail closed, retain the redacted integrity count and fetch from a different
approved replica. Verify the complete CID and committed root before repair. Do not log or return the
corrupt content, CID or capability.

## P6 auth rejection spike

Rate limit the caller. At one finalized hash, verify provider status, organization/SLA expiry, grant
audience, service-key generation and revocation. Do not broaden or silently renew authority. The
application must obtain a fresh scoped grant when the rejection is legitimate.

## P6 finality lag

Pause non-idempotent submission. Compare best/finalized heads across Origin and Commons, preserve
outboxes and retry only byte-identical operation IDs after finality resumes. Do not reset nonces,
truncate journals or switch to a contract/legacy path.

## Typed provider failure line

`provider_failure code=<bounded-code> action=<bounded-action>` is intentionally the complete public
log line. Use its action to select the corresponding subsystem check. Raw errors are local sensitive
material and are not appended to the line. Close an alert only after two clear evaluation windows,
all durable operations are accounted for and a named owner records the follow-up.

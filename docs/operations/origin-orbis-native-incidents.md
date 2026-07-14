# Origin/Orbis native incident actions

All incident actions are native and auditable. The incident commander records alert time, exact
finalized heads, runtime/metadata hashes, affected domain, decision, approvers, actions and exit
checks. Do not enable a contract fallback, dual write, legacy import or compatibility facade.

## Relay finality lag

Freeze admission and upgrades, compare best/finalized heads across distinct Origin validators,
inspect GRANDPA peers/session keys and follow the approved quorum recovery ceremony. Resume only
after finalized progress is stable and Orbis inclusion catches up.

## Orbis finality lag

Pause application admission, compare collator views, relay inclusion and claim queues, then isolate
the faulty collator or capacity assignment. Do not resubmit non-idempotent application intents.

## Provider unhealthy

Remove the provider endpoint from application selection, preserve data and journals, verify the
registered account/service key at one finalized hash, then restart from the same durable directory.
Suspend the native provider if identity or integrity cannot be proven.

## Provider proof deadline

Stop new agreements, inspect checkpoint duty, local historical root and outbox durability, and
restore the signer/consumer. Escalate before the due block; never fabricate a root or proof.

## Provider finality backlog

Stop producer admission, compare ordered provider-submissions-v3 and receipts-v3 journals, Orbis
finality and signer nonce. Resume only after every durable entry has one exact finalized receipt.

## Provider capacity

Stop new allocation before the hard cap, confirm on-chain allocated/pending bytes match `/stats`,
and register additional approved capacity/provider through the native storage authority.

## CID integrity

Quarantine the endpoint, retain corrupt bytes and CID evidence, retry another approved source, and
alert the provider owner. Never return unverified content or rewrite the on-chain commitment.

## Host permission denials

Confirm application identity, exact requested scope, consent expiry and nonce. Do not broaden a
grant automatically. Re-consent through the host only after the application manifest is approved.

## Runtime binding drift

The host remains fail closed. Stop signing/submission, compare genesis/spec/transaction/metadata,
descriptor and chain-spec hashes, regenerate the CORD-owned descriptor, and repeat ratification.

## Native revocation failure

Stop dependent reads/writes, verify issuer/registrar authority and the exact finalized status,
retry only with a fresh intent/nonce when safe, then use the native emergency pause if status
cannot be made authoritative.

## Exit checklist

Close only after finalized state and service observations agree, alerts clear for two evaluation
windows, queued operations are accounted for, permissions/keys are rotated when implicated, and a
postmortem owner/deadline is recorded.

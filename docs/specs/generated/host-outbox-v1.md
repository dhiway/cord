# Host outbox v1 state diagram (non-authoritative)

Generated from `../host-outbox-v1.state-machine.json` SHA-256
`bf9ba9ee5ab22dd537600bc0677950a655a4f0624e2f459715d9e4c06a54e8ef`. Regeneration MUST fail on
input-hash or transition drift.

```mermaid
stateDiagram-v2
  [*] --> Prepared: durable commit
  Prepared --> SentAdvisory: network_send
  SentAdvisory --> Prepared: transport_loss / byte-identical retry
  Prepared --> ResponseInstalled: authenticated_response
  SentAdvisory --> ResponseInstalled: authenticated_response
  ResponseInstalled --> AckConfirmed: provider_ack_confirmed
  AckConfirmed --> GC: durable successor/terminal + TTL
  Prepared --> Expired: recovery_window_closed
  ResponseInstalled --> Expired: recovery_window_closed
  Prepared --> Quarantined: integrity_failure
  ResponseInstalled --> Quarantined: integrity_failure
```

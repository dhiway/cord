# `@cord-network/origin-sdk-statement-store`

Structured, permission-gated submit, query, and reconnecting subscription access to the Commons statement store. Applications work with typed drafts and records; the host owns signing, encoding, endpoint selection, node feature detection, reconnect, and disposal.

Allowance reads compose `@cord-network/origin-sdk-resources`. Commons operators must enable the statement store explicitly. Whole-store inspection and destructive node-local administration are excluded.

`@cord-network/origin-sdk-statement-store/testing` exports an in-memory transport with submit,
query, streaming updates, injection, inspection, reset, and active-subscription accounting.

# `@cord-network/origin-sdk-host`

Host-owned product identity, permissions, signing, chain selection, namespaced storage, preimages, resource allocation, and statement transport.

Application code sends typed inner requests. `createTruApiBridge` alone constructs and validates the versioned `cord.origin-host` envelopes. Every sensitive operation revalidates its grant, and signing always requires a separate per-call host approval. Endpoint selection remains host-owned; `createHostChainProvider` supplies a structurally compatible Commons provider without accepting an application-selected endpoint.

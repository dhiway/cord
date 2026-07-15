# `@cord-network/origin-sdk-local-storage`

Typed, application-scoped local storage over the Origin host. The package has no direct `localStorage`, IndexedDB, filesystem, or endpoint fallback. Use `bytesCodec`, `utf8Codec`, or a validator-backed `jsonCodec`; `/testing` provides an in-memory fake.

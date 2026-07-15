# Foundation/Commons normative contracts

This directory freezes the P0 contracts for the clean-genesis Foundation/Commons application
platform. The words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, and **MAY** are normative as in
RFC 2119. There is no version-1, Bulletin, legacy-CORD, Solidity, or migration compatibility surface.

## Authority and projections

1. `origin-host-registry-v2.cddl` and the machine-readable registries/manifests in this directory are
   normative.
2. The Markdown contracts explain invariants and lifecycle; they do not loosen a machine bound.
3. `origin-host-registry-v2.schema.json` is a non-authoritative mobile-JSON projection. JSON uses decimal-string object keys that map to unsigned CBOR integer keys; bytes use
   unpadded base64url, `u64` values use decimal strings, and enums use their stable registry names; implementations MUST canonicalize to the
   CDDL-defined deterministic CBOR before authorization, hashing, or signing.
4. Files under `generated/` are explanatory projections. They MUST be regenerated from their named
   state-machine JSON source and MUST NOT be edited as an independent authority.

A release MUST fail when the SHA-256 of any normative input differs from the input hash recorded in a
generated artifact, when an operation/error/vector lacks one-to-one registry coverage, or when the
JSON projection accepts a value forbidden by CDDL. The registry descriptor hash is SHA-256 of the
exact `origin-host-registry-v2.cddl` bytes. Runtime metadata later binds SCALE portable type IDs to the
stable logical type names frozen here; absence or drift of that binding is a release blocker.

## Contract index

- `storage-content-v1.md`: content addressing, chunking, ranges, idempotency and lifecycle.
- `storage-checkpoints-v2.md`: MMR checkpoints, proof finality, equivocation and replica failover.
- `storage-control-v1.md`: buckets, agreements, capacity and honest deletion semantics.
- `drive-filesystem-v1.md` and `s3-v1.md`: developer object models.
- `storage-encryption-v1.md`: v1 envelope, custody, maturity boundary and vectors.
- `provider-organization-sla-v1.md`: enterprise provider authority, separate from Humanity.
- `identity-v2.md`: non-joined Identity operations, grants, contextual subjects and recovery.
- `host-provider-protocol-v2.md`: negotiation, deterministic CBOR, capabilities, durable outbox,
  resume and crash law.
- `origin-host-registry-v2.*`: protocol CDDL, errors, operations, bounds, schema and vectors.

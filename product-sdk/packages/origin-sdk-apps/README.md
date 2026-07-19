# `@cord-network/origin-sdk-apps`

Commons-native static application manifests and resolution. `OriginAppManifestV1` is stored as
content; the native Names pallet stores only its retained content commitment. Resolution pins name
status, ownership, content, and live attestation records to one finalized Commons block before
returning a sandbox instruction.

The package contains no registry contract, deployment contract, ABI, external chain address, or
second ownership authority. Content retention must finalize before its Names binding is submitted.

`createOriginStaticPackager` emits a deterministic content-addressed root index plus one raw block
per sorted file. `createOriginAppDeployer` skips blocks already present in the host content store,
prepares native TransactionStorage writes for new blocks and the manifest, and exposes the Names
binding only as a follow-up step after retention finality. An audited CAR/UnixFS packager can be
injected through the same `OriginBundlePackager` interface.

# `@cord-network/origin-sdk-apps`

Commons-native static application manifests and resolution. `OriginAppManifestV1` is stored as
content; the native Names pallet stores only its retained content commitment. Resolution pins name
status, ownership, content, and live attestation records to one finalized Commons block before
returning a sandbox instruction.

The package contains no registry contract, deployment contract, ABI, external chain address, or
second ownership authority. Object metadata publication must finalize before its Names binding is submitted when an S3 bucket target is supplied.

`createOriginStaticPackager` emits a deterministic content-addressed root index plus one raw block
per sorted file. `createOriginAppDeployer` skips blocks already present in the explicit authenticated native-provider content store,
prepares native S3 object metadata writes for bundle blocks and the manifest when an
`objectPublication` bucket is supplied, and exposes the Names binding only as a follow-up step after object metadata finality. An audited CAR/UnixFS packager can be
injected through the same `OriginBundlePackager` interface.

`createApp` accepts explicit `content` and `blocks` adapters supplied by the authenticated native
provider integration. Host permissions never carry application bytes. Launch manifests accept only
raw CIDs with a Blake2b-256 multihash; DAG-PB and SHA-256 forms are rejected.

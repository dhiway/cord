# CORD Product SDK — native SDK v1

## Publishable Origin SDK packages

The Tier A package floor is available as side-effect-free ESM with declarations:
`origin-sdk-result`, `origin-sdk-errors`, `origin-sdk-descriptors`, `origin-sdk-chain-client`,
`origin-sdk-host`, `origin-sdk-signer`, and `origin-sdk-tx` under the `@cord-network` scope.
`npm run test:packed` builds and installs only their tarballs into a clean consumer, then proves a
runtime-bound finalized read and a host-approved finalized transaction. The private root exports
remain temporary inputs for Tier B extraction and are not publication targets.

This workspace is the first supported transport-neutral TypeScript SDK for the
clean-break Foundation/Commons stack. It freezes the Foundation `9901/2` and Commons
`29/8` version matrix, current Commons metadata, typed descriptor contract, and
cross-language native semantic vectors.

It is deliberately **not** a production mobile rewrite, a performance result,
or a legacy contract compatibility layer. The host remains the authority for
permissions and consent. Product calls never accept raw SCALE, pallet/call
indices, or migrated-domain contract ABIs.

```sh
npm --prefix product-sdk ci
npm --prefix product-sdk run generate:descriptors
npm --prefix product-sdk run update:sdk-freeze
npm --prefix product-sdk run validate:sdk-freeze
```

## Typed network host

`@cord-network/product-sdk-workspace/network-host` provides the CORD-owned
network adapter behind the permission/consent host. Applications inject a
typed metadata-capable client, chain signer, and exact typed routes:

```ts
import { FakeHost } from "./packages/host/src/fake-host.ts";
import {
  createTypedNetworkHostRoutes,
  ORBIS_NETWORK_BINDING,
  type TypedNetworkRoutes,
} from "@cord-network/product-sdk-workspace/network-host";

const routes = {
  "attestation:schema_by_id": {
    finality: "finalized",
    query: (payload, { client, at, signal }) =>
      client.apis.AttestationApi.schemaById(payload.schema, { at, signal }),
  },
  "attestation:revoke": {
    finality: "submit-and-finalize",
    transaction: (payload, { client }) =>
      client.tx.Attestation.revoke({ attestation: payload.attestation }),
  },
} satisfies TypedNetworkRoutes<typeof client, typeof signer>;

const host = new FakeHost({
  ...createTypedNetworkHostRoutes({
    client,
    signer,
    binding: ORBIS_NETWORK_BINDING,
    routes,
  }),
});
```

Every read captures one finalized block and passes its exact hash to the typed
query. Before execution, the adapter compares genesis, spec version,
transaction version, metadata hash, descriptor digest, and chain-spec digest.
Typed submissions are constructed at a verified finalized runtime and resolve
only after a finalized status carrying both block and extrinsic hashes. Abort
signals close in-flight status streams, and transport/dispatch failures map to
the stable product error vocabulary.

`ORBIS_NETWORK_BINDING` is generated together with the descriptor and host
schema. `generate:descriptors --check` fails when any of those artifacts drift;
consumers must not copy its digest into application code. The checked-in network is
`candidate-pending`, so development request contexts must explicitly use
`ORBIS_CANDIDATE_NETWORK_BINDING`. A request using production access is rejected until a signed
activation envelope derives `production-approved`.

`packages/descriptors/generated/orbis-descriptor.json` is a deterministic
native host contract manifest bound to the checked-in runtime metadata-hash
record and SDK manifests. The checked-in Commons V14 SCALE metadata generates a byte-reproducible
PAPI descriptor with `polkadot-api@2.1.6`; `check:papi` rejects metadata or generated-output drift.
The authoritative 138-method inventory is generated from `docs/sdk/native-route-contract.json`;
Rust and TypeScript execute every canonical route sample. The network-bound descriptor and host
schema are regenerated only from a reconciled live metadata identity. The descriptor binds the canonical P5 signing-payload hash while
the signing payload binds a canonical descriptor-contract digest that excludes
only that mutable binding field, avoiding a circular/full-envelope hash claim.
It is the supported closed host-route policy inventory rather than a substitute for the generated
PAPI package. The Subxt runtime adapter resolves calls from verified metadata and the freeze
validator rejects version, metadata, descriptor, route, or schema drift.

## Verified content retrieval

`@cord-network/origin-sdk-cloud-storage` provides the CORD-owned, transport-neutral content client. Applications
inject an ordered set of gateway and/or Bitswap block providers; the SDK applies
that order deterministically, bounds block/file sizes and DAG block counts, and
verifies CIDv0/v1 SHA2-256 or Blake2b-256 multihashes before returning bytes.
Cancellation uses `AbortSignal`. Terminal retrieval failures use the typed
`content_unavailable` and `content_integrity` codes.

Raw CIDs are returned directly. DAG-PB/UnixFS is intentionally not decoded by
the core client: callers must inject an audited decoder, and its `loadBlock`
callback exposes only CID-verified root/linked blocks. This avoids representing
an unsupported DAG-PB block as reconstructed UnixFS content.

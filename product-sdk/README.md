# CORD Product SDK — P0 executable foundation

This workspace is the transport-neutral TypeScript foundation for Orbis. It
freezes the spec-29/transaction-8 descriptor contract, checks current runtime
vectors, executes the hostile fake-host suite, and validates E/Q/C inputs.

It is deliberately **not** a production mobile rewrite, a performance result,
or a legacy contract compatibility layer. The host remains the authority for
permissions and consent. Product calls never accept raw SCALE, pallet/call
indices, or migrated-domain contract ABIs.

```sh
npm --prefix product-sdk ci
npm --prefix product-sdk run generate:descriptors
npm --prefix product-sdk test
```

## Typed network host

`@cord-network/product-sdk-workspace/network-host` provides the CORD-owned
network adapter behind the permission/consent host. Applications inject a
descriptor-generated PAPI-like client, chain signer, and exact typed routes:

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
consumers must not copy its digest into application code.

`packages/descriptors/generated/orbis-descriptor.json` is a deterministic
bootstrap descriptor contract derived from the checked-in runtime metadata-hash
record and SDK manifests. It binds the canonical P0 signing-payload hash while
the signing payload binds a canonical descriptor-contract digest that excludes
only that mutable binding field, avoiding a circular/full-envelope hash claim.
A full generated PAPI descriptor and concrete route registration remain a
delivery requirement for each deployed product domain; the checked-in bootstrap
descriptor must not be represented as a production-generated PAPI descriptor.

## Verified content retrieval

`src/content.ts` is the CORD-owned, transport-neutral content client. Applications
inject an ordered set of gateway and/or Bitswap block providers; the SDK applies
that order deterministically, bounds block/file sizes and DAG block counts, and
verifies CIDv0/v1 SHA2-256 or Blake2b-256 multihashes before returning bytes.
Cancellation uses `AbortSignal`. Terminal retrieval failures use the typed
`content_unavailable` and `content_integrity` codes.

Raw CIDs are returned directly. DAG-PB/UnixFS is intentionally not decoded by
the core client: callers must inject an audited decoder, and its `loadBlock`
callback exposes only CID-verified root/linked blocks. This avoids representing
an unsupported DAG-PB block as reconstructed UnixFS content.

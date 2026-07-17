# `@cord-network/origin-sdk`

Hosted Commons application bootstrap. `createApp` verifies the exact Commons genesis/runtime/metadata, selects a host account, and wires scoped storage plus all native application domains. It never accepts an RPC endpoint, contract address, ABI, or raw SCALE input.

```ts
const result = await createApp({
  product: { id: "festival.app", name: "Festival" },
  bridge: originHostBridge,
  runtime: commonsRuntimeExecutor,
});
if (!result.success) throw result.error;
const app = result.value;
await app.storage.set("theme", "dark", utf8Codec);
const accounts = await app.signer.accounts();
await app.close();
```

Application tests import `createFakeApp` from `@cord-network/origin-sdk/testing`. Host storage,
signing, and prepared native writes are observable in memory. Native reads
fail with `unconfigured_chain_read` unless the test supplies a domain override or runtime adapter;
the fake never invents an RPC endpoint.

`app.apps` stores and resolves `OriginAppManifestV1` through the host content transport and native
Commons Names. It does not load a contract facade or a second application registry.

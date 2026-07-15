# `@cord-network/origin-sdk`

Hosted Commons application bootstrap. `createApp` verifies the exact Commons genesis/runtime/metadata, selects a host account, and wires scoped storage plus all native application domains. It never accepts an RPC endpoint, contract address, ABI, or raw SCALE input.

```ts
const result = await createApp({
  product: { id: "festival.app", name: "Festival" },
  bridge: originHostBridge,
  runtime: commonsRuntimeAdapters,
});
if (!result.success) throw result.error;
const app = result.value;
await app.storage.set("theme", "dark", utf8Codec);
const accounts = await app.signer.accounts();
await app.close();
```

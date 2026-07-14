# @cord-network/origin-sdk-identity

Typed, finalized-hash-pinned identity reads and People pallet transaction preparation for Commons.
Applications inject the generated runtime adapter and use only typed Commons-native parameters.

```ts
const identity = createIdentityClient(chain, adapter);
const status = await identity.status(accountId("5..."));
const prepared = await identity.prepareSetIdentity(info);
```

Registrar administration, username authority management, forced identity mutation, and maintenance
calls are deliberately excluded from the application surface.

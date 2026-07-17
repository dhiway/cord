# @cord-network/origin-sdk-identity

The canonical Commons Identity developer surface. Seven Host-v2 operations are independently
granted and return closed, operation-specific responses; transaction signing has its own operation
and grant.

```ts
const identity = createIdentityV2Client("festival.app", hostBridge);
const result = await identity.humanityStatus(grant, request, finalizedInvocation);
```

The package also owns the shared typed account and hash constructors used by native domain packages.
It does not expose a second direct-runtime Identity client or a composite identity record.

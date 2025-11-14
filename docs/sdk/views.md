# Runtime Views & Authorizations

Most read-only data comes from runtime view functions secured with the `AuthorizationRequest` type. The SDK includes builders and query facades for the most common pallets.

## Signing View Payloads

```rust
use oc::demo::util::fresh_authorization_with_client; // thin wrapper

let auth = fresh_authorization_with_client(&client, &signer).await?; // valid for ~30 blocks
```

Under the hood this calls `AuthorizationBuilder::generate_view_authorization(&keypair, &AuthorizationBuilder::default_context(), reference_block, None)`, detects the signature scheme, and returns an `AuthorizationRequest` ready for the view functions.

## Entity Views

```rust
let entity = client
    .query()
    .entity()
    .details(&auth, &token_identifier)
    .await?;

let timeline = client
    .query()
    .token()
    .timeline(&TokenTimelineRequest {
        auth: fresh_authorization_with_client(&client, &signer).await?,
        token: token_identifier.clone(),
        start: None,
        limit: Some(10),
    })
    .await?;
```

`entity()` exposes helpers for `details`, `account_token`, `entity_nym`, attribute history, etc. See `src/query/entity.rs` for the full list.

## Register Views

```rust
let req = RegisterDetailsRequest {
    auth: fresh_authorization_with_client(&client, &signer).await?,
    registry: registry_id.clone(),
};
let details = client.query().register().details(&req).await?;
let lookup_specs = client.query().register().lookup_specs(&RegisterLookupSpecsRequest {
    auth: fresh_authorization_with_client(&client, &signer).await?,
    registry: registry_id.clone(),
}).await?;

let packet = client.query().register().packet_snapshot(&RegisterPacketSnapshotRequest {
    auth: fresh_authorization_with_client(&client, &signer).await?,
    registry: registry_id.clone(),
    packet: packet_id.clone(),
    version: None,
}).await?;
```

## Token Views

Use `client.query().token()` for timeline/state helpers. Typical usage is shown in the packet demo when printing a packet’s recent activity.

## Error Handling

All query methods return `Result<T>` (SDK error). For view-specific failures (missing resources, bad auth, oversized responses) the helpers map the runtime’s `AuthorizationError` into typed variants (`Error::NotFound`, `Error::Params`, etc.). Bubble those errors up to your UI or log them for diagnosis.

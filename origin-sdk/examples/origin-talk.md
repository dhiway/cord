# Origin

Origin is the data layer that glues CORD runtimes together. The building blocks below show how
elements flow through pallets, primitives, and runtimes so you can model real-world credentials,
logs, and assets with predictable version history.

## How the pillars fit together
- **Elements** give each field flexible, typed storage without schema rewrites.
- **Packets** version those fields so history is provable and reversible.
- **Registries** decide who may write and what shape a packet should have.
- **Tokens** wrap every entity/registry/packet in a self-describing ID that any runtime can
  resolve. Together they deliver interoperability, auditability, and evolution without migrations.

## Core Concepts (why they exist)

### Elements (a.k.a. Elum)
**Why:** On-chain records need typed, bounded, and cheap-to-verify values. A single enum keeps the
codec stable and avoids bespoke schemas per sector.

**What:** Leaf values (`Raw`, `Bool`, `U64`, `U128`, `Hash`, `Token`, `CID`) with built-in
validation (`Elum::validate`). Unlike a fixed struct field (e.g. `info: Vec<u8>`), an `Element`
lets the same key accept multiple shapes over time: start with a human-readable string, later
upgrade to a token or a hash without a storage migration.

**Where it helps in real life:**
- Credentials: `email` stays `Raw` text, but `student_id` can move from `U64` to a `Token` when the
  campus issues smartcards—no schema rewrite.
- Supply chain: `lot_hash` as `Hash`; `owner` as `Token`; `qc_passed` as `Bool`—all share one
  attribute container.
- Media/provenance: `content` as `CID`, then later a `Hash` of the rendered artifact for notarized
  exports.

```rust
use origin_primitives::element::{Elum, ElementType};
use frame_support::traits::ConstU32;

type Small = ConstU32<128>;

let email: Elum<Small> = Elum::Raw(b"alice@example.com".to_vec().try_into().unwrap());
assert_eq!(ElementType::from(&email), ElementType::Raw);
```

### Packets
**Why:** Real records change. Packets capture every version with a digest and status so auditors can
replay history.

**What:** Versioned attribute bundles stored under a packet token; status = `Active/Revoked/Deleted`.

**Real-world fit:**
- Diploma corrections: v1 (issued), v2 (name spelling fix), v3 (revoked for fraud) — all tracked.
- Shipping: each checkpoint appends a new packet version with GPS hash + signer.
- Compliance logs: one packet tracks audit evidence; attributes mix typed `Bool` flags and `CID`
  links to encrypted blobs.

```rust
// Snippet from examples/packet_issue.rs
use origin_sdk::{OriginClient, extrinsic::builder::DynamicCallBuilder, client::signer::MultiKeySigner};
use scale_value::Value;

let signer = MultiKeySigner::from_seed("//Alice")?;
let client = OriginClient::connect("ws://localhost:9944").await?;

let call = DynamicCallBuilder::new().call(
    "Packet",
    "issue",
    vec![Value::from_bytes(b"demo-registry"), Value::from_bytes(b"demo-packet-body")],
);
let outcome = client
    .tx()
    .using(&signer)
    .entity()
    .submit_set_info_from_nested(&nested)
    .await?;
println!("Packet issued: {:?}", outcome.hash);
```

### Registries
**Why:** Packets need context: who can write, what shape the data has, and how lookups work. A
registry is that context.

**What:** Collections of packets with governance. Two flavors:
- **Schemaless** (`pallet_entity`): flexible identities or profiles with ad-hoc attributes.
- **Schema-driven** (`pallet_register`): enforce `RegistryAttributeSpec` types and optionality.

**Real-world fit:**
- University transcript registry with enforced schema (`name`, `course`, `grade`).
- Open data catalog where NGOs can add fields as they learn (schemaless).

```rust
// Create a schema-driven registry (examples/registry_create.rs)
use origin_sdk::{OriginClient, client::signer::MultiKeySigner, extrinsic::builder::DynamicCallBuilder};
use scale_value::Value;

let signer = MultiKeySigner::from_seed("//Alice")?;
let client = OriginClient::connect("ws://localhost:9944").await?;

let call = DynamicCallBuilder::new().call(
    "Register",
    "create",
    vec![Value::from_bytes(b"transcript-registry"), Value::from_bytes(b"v1 schema bytes")],
);
client
    .tx()
    .using(&signer)
    .entity()
    .submit_set_info_from_nested(&nested)
    .await?;
```

#### Sample registry schemas

**Transcript (schema-driven)**

```json
{
  "name": { "kind": "Raw", "optional": false },
  "course_code": { "kind": "Raw", "optional": false },
  "grade": { "kind": "Raw", "optional": false },
  "issued_at": { "kind": "U64", "optional": false },
  "issuer": { "kind": "Token", "optional": false }
}
```

Rust builder you can feed into `Register::create` (each spec is SCALE-encoded):

```rust
use origin_primitives::registry::RegistryAttributeSpec;
use origin_primitives::element::ElementType;

let specs = vec![
    RegistryAttributeSpec { key: b"name".to_vec(), kind: ElementType::Raw, optional: false },
    RegistryAttributeSpec { key: b"course_code".to_vec(), kind: ElementType::Raw, optional: false },
    RegistryAttributeSpec { key: b"grade".to_vec(), kind: ElementType::Raw, optional: false },
    RegistryAttributeSpec { key: b"issued_at".to_vec(), kind: ElementType::U64, optional: false },
    RegistryAttributeSpec { key: b"issuer".to_vec(), kind: ElementType::Token, optional: false },
];
```

**Supply chain (schemaless)**

Start empty under `pallet_entity`; producers add ad-hoc keys like `qc_passed`, `temperature_c`,
`custody.owner`, `custody.hand_off_hash` as they operate. No schema migration needed.

#### Example packet payloads

**Transcript packet (version 1)**

```json
{
  "name": "Ada Lovelace",
  "course_code": "CS101",
  "grade": "A",
  "issued_at": 1722470400,
  "issuer": "5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW"
}
```

**Supply chain checkpoint (version 3)**

```jsonc
{
  "lot_hash": "0x8f4c...d1",        // ElementType::Hash
  "qc_passed": true,                 // ElementType::Bool
  "temperature_c": 4,                // ElementType::U64
  "custody.owner": "5EU...",        // ElementType::Token
  "custody.hand_off_hash": "0x91.." // ElementType::Hash
}
```

To submit through the SDK you can map these into `scale_value::Value` list matching the registry
order or into `Attributes` if you build packets programmatically inside a runtime.

```rust
use scale_value::Value;

let payload = Value::map(vec![
    ("lot_hash".into(), Value::from_bytes(hex::decode("8f4c...d1").unwrap())),
    ("qc_passed".into(), Value::bool(true)),
    ("temperature_c".into(), Value::u64(4)),
    ("custody.owner".into(), Value::from_bytes(ss58_owner.as_ref())),
    ("custody.hand_off_hash".into(), Value::from_bytes(hex::decode("91..").unwrap())),
]);
```

### Tokenization
**Why:** Cross-runtime portability. A token encodes which pallet, which network, and the digest so
any node can resolve it without extra metadata.

**What:** `Ss58Identifier` with embedded network id (Origin vs solo), pallet id, origin flag, digest
checksum. `pallet_token` can resolve and fetch timelines.

**Real-world fit:**
- Scan a QR on a product; the app resolves if it’s a registry (catalog) or packet (cert) and shows
  custody history.
- Move identifiers between relay (braid) and sidechain (loom) without reissuance.

```rust
// Token resolver (examples/token_demo.rs)
use origin_primitives::Ss58Identifier;
use origin_sdk::{OriginClient, query::Query, client::signer::MultiKeySigner};

let token_id = Ss58Identifier::try_from(args.token.clone())?;
let client = OriginClient::connect(&args.endpoint).await?;
let domain = Query::new(&client);

if let Ok(entity) = domain.entity().overview(token_id.clone()).await {
    println!("Entity owner: {:?}", entity.linked_accounts.first());
}
```

## Pallets (what ships in the Origin runtimes)

- `pallet_entity` (`origin/pallets/entity`)
  - Schemaless identities with tokens, linked accounts, attribute history, and view authorizations
  (TTL-gated signatures). Great for business profiles, IoT nodes, or SSI identifiers.

- `pallet_register` (`origin/pallets/register`)
  - Schema-driven registries, permissions (`RegistryPermissions`), status (`Active/Revoked/Deleted`)
    and governance hooks. Use it to define credential schemas or sector-specific catalogs.

- `pallet_token` (`origin/pallets/token`)
  - Builds and resolves SS58 tokens, records state events (who changed what, when), and exposes a
    timeline API used by the SDK demo. Every other pallet leans on it for cross-runtime IDs.

- `pallet_authorities` (`origin/pallets/authorities`)
  - Tracks authority sets for Origin hub deployments; provides membership proofs for registries or
    relay routing.

- `pallet_feeless` (`origin/pallets/feeless`)
  - Marks accounts that can submit feeless extrinsics (used by relayers/meta-tx flows).

## Primitives (types you touch in code)

- `Ss58Identifier` and `DecodedIdentifier`
  - Encode/decode tokens with embedded network + pallet bits; `DEV_IDENT` (29) vs `ORIGIN_IDENT`
    (0) distinguish solo vs origin mode.

- Elements & Attributes
  - `Elum` carries typed payloads; `Attribute` pairs a key with an element; `AttributeValueView`
    and `ElementView` provide readable forms for SDK output.

- Registries
  - `RegistryKind` (`Raw`, `Token`, `Hash`), `RegistryAttributeSpec` (key, expected
    `ElementType`, optional flag), `RegistryStatus`.

- Packets
  - `PacketState`, `PacketStatus`, `PacketMetadata`, and `PacketSnapshot` capture per-version
    state plus digests so timelines are deterministic.

- Authorization
  - Reusable structures in `origin_primitives::authorization` enforce TTL on viewer-signed
    payloads; used by entity, token, and registry views.

## Runtimes

- **Braid** (`runtimes/braid`): default relay/hub runtime (`spec_name = "braid"`). Includes the
  Origin pallets above plus staking, governance, and fee tuning. Use `--chain braid-dev` to run a
  local node.

- **Loom** (`runtimes/loom`): companion runtime (`spec_name = "loom"`) with the same Origin
  primitives, tuned for parachain/sidechain style deployments. Ideal when you need lightweight
  execution but Origin-compatible tokens.

- **Weave** (`runtimes/weave`): test/benchmark-focused runtime that keeps the Origin pallets
  intact while layering extra pallets for economics and E2E testing.

Run any of them from the repo root after building `./target/release/cord`:

```
./target/release/cord --dev                      # solo dev
./target/release/cord --chain braid-dev          # braid
./target/release/cord --chain loom-dev           # loom
```

## Putting it together (quick flows)

1) **Register an issuer (entity)**: mint an entity token with display/web/email using
   `pallet_entity`. This establishes who signs later records and how long reader authorizations last.

2) **Define a schema (registry)**: call `Register::create` with schema bytes (JSON/Avro/Protobuf in
   `Element::Raw`). Set `RegistryPermissions` so only the issuer (or delegates) can add packets.

3) **Issue packets (records)**: issuer calls `Packet::issue` to write versioned entries (grades,
   custody hops). Each version is digest-hashed and tied to the registry rules.

4) **Tokenize and share**: every entity/registry/packet gets an `Ss58Identifier` that any runtime
   can decode. Apps embed this in QR/NFC/URLs.

5) **Resolve & audit**: consumers use the SDK `token` view to resolve a token, fetch the timeline,
   and verify signatures plus block heights. Works offline-first once headers are synced.

Use the ready-made SDK examples to exercise the path end-to-end:
- `cargo run -p origin-sdk --example entity_view`
- `cargo run -p origin-sdk --example registry_create`
- `cargo run -p origin-sdk --example packet_issue`
- `cargo run -p origin-sdk --example token_demo -- --token <ss58>`

These snippets keep everything deterministic and offline-friendly so you can map your real-world
records to Origin primitives before wiring production keys or networks.

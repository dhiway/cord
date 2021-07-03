<!-- [![Build and Test](https://github.com/dhiway/cord-node/workflows/Build%20and%20Test/badge.svg)](https://github.com/dhiway/cord-node/actions) -->

# CORD

The CORD node implementation uses Parity Substrate and KILT modules as the underlying
technology stack with DID, #MARK Type, #MARKS and hierarchical Trust Modules.

- [CORD](#cord)
  - [Accounts](#accounts)
  - [Build & Run](#build--run)
    - [Building in dev mode](#building-in-dev-mode)
    - [Building in performant release mode](#building-in-performant-release-mode)
    - [Start the Node, by running:](#start-the-node-by-running)
    - [Debug Mode](#debug-mode)
    - [Release Mode](#release-mode)
    - [Setup the local node with session keys, by running](#setup-the-local-node-with-session-keys-by-running)
  - [CORD User Accounts](#cord-user-accounts)
    - [Polkadot{.js} Browser Plugin](#polkadotjs-browser-plugin)
    - [Polkadot Web UI](#polkadot-web-ui)
  - [Building docker image](#building-docker-image)
  - [Node Modules functionalities](#node-modules-functionalities)
    - [DID Module](#did-module)
      - [Add](#add)
      - [CRUD](#crud)
    - [MTYPE (Schema) Module](#mtype-schema-module)
    - [Mark Module](#mark-module)
      - [Add](#add-1)
      - [Revoke](#revoke)
      - [Lookup](#lookup)
    - [Hierarchy of Trust Module](#hierarchy-of-trust-module)
      - [Create root](#create-root)
      - [Add delegation](#add-delegation)
      - [Revoke](#revoke-1)
    - [Substrate Documentation](#substrate-documentation)
      - [Substrate Tutorials](#substrate-tutorials)
      - [Substrate JSON-RPC API](#substrate-json-rpc-api)
      - [Substrate Reference Rust Docs](#substrate-reference-rust-docs)

### Accounts

[Documentation](./docs/accounts.md)

### Build & Run

You need to have [rust and cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed. Clone this repo and navigate into it.

You can build it by executing these commands:

```
./scripts/init.sh
```

#### Building in dev mode

```
cargo build
```

#### Building in performant release mode

```
cargo build --release
```

#### Start the Node, by running:

```
./target/<debug \ release> /cord [FLAGS] [OPTIONS]
```

For CORD CLI options see the section about [Commands](#cord-cli-options).

#### Debug Mode

```
./target/debug/cord --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

#### Release Mode

```
./target/release/cord --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

#### Setup the local node with session keys, by running

```
bash ./scripts/setup-dev-chain.sh
```

### CORD User Accounts

A valid account only requires a private key that can sign on one of the supported curves and signature schemes. CORD uses Ed25519 as the signature scheme for all inhjected accounts.

#### Polkadot{.js} Browser Plugin

The browser plugin is available for both [Google Chrome](https://chrome.google.com/webstore/detail/polkadot%7Bjs%7D-extension/mopnmbcafieddcagagdcbnhejhlodfdd?hl=en) (and Chromium based browsers like Brave) and [FireFox](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension).

Open the Polkadot{.js} browser extension by clicking the orange and white Polkadot{.js} logo on the top bar of your browser. You will see a browser popup. Click the small plus icon in the top right and choose the option to `Restore account form backup JSON file`. Follow the instructions to import default [user](./scripts/accounts/users) and [stash](./scripts/accounts/stash) accounts.

#### Polkadot Web UI

Add the following code to `Developer` -> `Settings`

```json
{
  "Address": "AccountId",
  "BlockNumber": "u32",
  "DelegationNodeId": "Hash",
  "ErrorCode": "u16",
  "Index": "u32",
  "LookupSource": "AccountId",
  "Permissions": "u32",
  "PublicBoxKey": "Hash",
  "PublicSigningKey": "Hash",
  "RefCount": "u32",
  "Signature": "MultiSignature"
}
```

### Building docker image

Clone this repo and navigate into it.

Build docker image:

```
docker build -t local/cord:develop .
```

start, by running:

```
docker run -p 9944:9944 -p 9933:9933 local/cord:develop --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

with persistent mount that will keep the chain data locally:

```
docker run -p 9944:9944 -p 9933:9933 -v /my/local/folder:/cord local/cord:develop  --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

## Node Modules functionalities

The CORD node provides an immutable transaction ledger for various workflows supported by the network.

**Remote Procedure Calls**

The [Polkadot API](https://polkadot.js.org/api/) helps with communicating with the JSON-RPC endpoint, and the clients and services never have to talk directly with the endpoint.

**Blocktime**

The blocktime is currently set to 4 seconds, but this setting is subject to change based on further research.

**Extrinsics and Block Storage**

In Substrate, the blockchain transactions are abstracted away and are generalised as [extrinsics](https://docs.substrate.dev/docs/extrinsics) in the system. They are called extrinsics since they can represent any piece of information that is regarded as input from “the outside world” (i.e. from users of the network) to the blockchain logic. The blockchain transactions are implemented through these general extrinsics, that are signed by the originator of the transaction. We use this framework to write the protocol specific data entries on the blockchain: [DID], [MTYPE], [Mark] and [Delegation]. The processing of each of these entry types is handled by our custom runtime modules.

Under the current consensus algorithm, authority validator nodes (whose addresses are listed
in the genesis block) can create new blocks. These nodes [validate](https://substrate.dev/docs/en/knowledgebase/learn-substrate/tx-pool#transaction-lifecycle) incoming transactions, put them into the pool, and include them in a new block. While creating the block, the node executes the transactions and stores the resulting state changes in its local storage. Note that the size of the entry depends on the number of arguments the transaction, (i.e., the respective extrinsic method) has. The size of the block is hence dynamic and will depend on the number and type of transactions included in the new block. The valid new blocks are propagated through the network and other nodes execute these blocks to update their local state (storage).

**Authoring & Consensus Algorithm**

We use [Aura](https://wiki.parity.io/Aura) as the authoring algorithm, since we are a public, permissioned blockchain mode.

For consensus we use [GRANDPA](https://github.com/paritytech/substrate#2-description).

**Governance**

- [TBD]

### DID Module

The node runtime defines an DID module exposing:

#### Add

```rust
add(origin, sign_key: T::PublicSigningKey, box_key: T::PublicBoxKey, doc_ref: Option<Vec<u8>>) -> DispatchResult
```

This function takes the following parameters:

- origin: public [ss58](<https://wiki.parity.io/External-Address-Format-(SS58)>) address address of the sender account
- sign_key: the [ed25519](http://ed25519.cr.yp.to/) public signing key of the owner
- box_key: the [x25519-xsalsa20-poly1305](http://nacl.cr.yp.to/valid.html) public encryption key of the owner
- doc_ref: Optional u8 byte vector representing the reference (URL) to the DID document

The node verifies the transaction signature corresponding to the owner and inserts it to the blockchain storage by using a map:

```rust
T::AccountId => Option<(T::PublicSigningKey, T::PublicBoxKey, Option<Vec<u8>>)>
```

#### CRUD

As DID supports CRUD (Create, Read, Update, Delete) operations, a `get(dids)` method reads a DID for an account address, the anchor function may also be used to update a DID and a `remove(origin) -> Result` function that takes the owner as a single parameter removes the DID from the map, so any later read operation call does not return the data of a removed DID.

### MTYPE (Schema) Module

The node runtime defines a MARK TYPE (Schema) module exposing

```rust
anchor(origin, schema_hash: T::Hash) -> DispatchResult
```

This function takes following parameters:

- origin: public [ss58](https://substrate.dev/docs/en/knowledgebase/advanced/ss58-address-format) address of the sender account
- schema_hash: Hash of the schema as a [blake2b](https://blake2.net/) string

The node verifies the transaction signature corresponding to the creator and inserts it to the blockchain storage by using a map:

```rust
T::Hash => Option<T::AccountId>
```

### Mark Module

The node runtime defines an Mark module exposing functions to

- add a #MARK (`anchor`)
- revoke a #MARK (`revoke`)
- lookup a #MARK (`lookup`)
- lookup #MARKs for a delegation (used later in Complex Trust Structures) on chain.

#### Add

```rust
anchor(origin, stream_hash: T::Hash, mtype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> DispatchResult
```

The `anchor` function takes following parameters:

- origin: The caller of the method, i.e., public address ([ss58](https://substrate.dev/docs/en/knowledgebase/advanced/ss58-address-format)) of the Marker
- stream_hash: The content hash as [blake2b](https://blake2.net/) string used as the key of the entry
- mtype_hash: The [blake2b](https://blake2.net/) hash of MTYPE used
- delegate_id: Optional reference to a delegation which this mark is based on

The node verifies the transaction signature and insert it to the state, if the provided issuer didn’t already anchor the provided stream_hash. The mark is stored by using a map:

```rust
T::Hash => Option<Mark<T>>
```

Delegated Marks are stored in an additional map:

```rust
T::DelegationNodeId => Vec<T::Hash>
```

#### Revoke

```rust
revoke(origin, stream_hash: T::Hash, max_depth: u64) -> DispatchResult
```

The `revoke` function takes the stream_hash (which is the key to lookup a mark) as argument. After looking up the mark and checking invoker permissions, the revoked flag is set to true and the updated mark is stored on chain.

#### Lookup

The mark lookup is performed with the `stream_hash`, serving as the key to the mark store. The function `get_marks(stream_hash)` is exposed to the outside clients and services on the blockchain for this purpose.

Similarly, as with the simple lookup, to query all marks created by a certain delegate, the runtime defines the function `get_delegated_marks(DelegateId)` that is exposed to the outside.

### Hierarchy of Trust Module

The node runtime defines a Delegation module exposing functions to

- create a root `create_root`
- add a delegation `add_delegation`
- revoke a delegation `revoke_delegation`
- revoke a whole hierarchy `revoke_root`
- lookup a root `get(root)`
- lookup a delegation `get(delegation)`
- lookup children of a delegation `get(children)`
  on chain.

#### Create root

```rust
create_root(origin, root_id: T::DelegationNodeId, mtype_hash: T::Hash) -> DispatchResult
```

The `create_root` function takes the following parameters:

- origin: The caller of the method, i.e., public address (ss58) of the owner of the
  trust hierarchy
- root_id: A V4 UUID identifying the trust hierarchy
- mtype_hash: The blake2b hash of the MTYPE the trust hierarchy is associated with

The node verifies the transaction signature and insert it to the state. The root is stored by using
a map:

```rust
T::DelegationNodeId => Option<DelegationRoot<T>>
```

#### Add delegation

```rust
add_delegation(origin, delegation_id: T::DelegationNodeId, root_id: T::DelegationNodeId, parent_id: Option<T::DelegationNodeId>, delegate: T::AccountId, permissions: Permissions, delegate_signature: T::Signature) -> DispatchResult
```

The `add_delegation` function takes the following parameters:

/// Adds a delegation node on chain, where
/// origin - the origin of the transaction
/// delegation_id - unique identifier of the delegation node to be added
/// root_id - id of the hierarchy root node
/// parent_id - optional identifier of a parent node this delegation node is created under
/// delegate - the delegate account
/// permission - the permissions delegated
/// delegate_signature - the signature of the delegate to ensure it's done under his permission

- origin: The caller of the method, i.e., public address (ss58) of the delegator
- delegation_id: unique identifier of the delegation node to be added
- root_id: id of the hierarchy root node
- parent_id: Optional, id of the parent node this delegation is created under
- delegate: The public address (ss58) of the delegate (ID receiving the delegation)
- permissions: The permission bit set (having 0001 for anchoring permission and 0010 for delegation permission)
- delegate_signature: ed25519 based signature of the delegate to ensure it's done under his permission

The node verifies the transaction signature and the delegate signature as well as all other data
to be valid and the delegator to be permitted and then inserts it to the state. The delegation is
stored by using a map:

```rust
T::DelegationNodeId => Option<DelegationNode<T>>
```

Additionally, if the delegation has a parent delegation, the information about the children of its
parent is updated in the following map that relates parents to their children:

```rust
T::DelegationNodeId => Vec<T::DelegationNodeId>
```

#### Revoke

```rust
revoke(delegation: &T::DelegationNodeId, sender: &T::AccountId) -> DispatchResult
```

and

```rust
revoke_children(delegation: &T::DelegationNodeId, sender: &T::AccountId) -> DispatchResult
```

## Substrate Benchmarking -

To know more about generating weights for the specific pallets -
Go to - docs/benchmarking.md

or check out the weights file in the respective pallets.

### Substrate Documentation

#### [Substrate Tutorials](https://substrate.dev/en/tutorials)

#### [Substrate JSON-RPC API](https://polkadot.js.org/docs/substrate/rpc)

#### [Substrate Reference Rust Docs](https://substrate.dev/rustdocs/v2.0.0/sc_service/index.html)

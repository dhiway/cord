<!-- [![Build and Test](https://github.com/dhiway/cord-node/workflows/Build%20and%20Test/badge.svg)](https://github.com/dhiway/cord-node/actions) -->

# CORD

The CORD node implementation uses Parity Substrate and KILT modules as the underlying 
technology stack with DID, #MARK Type, #MARKS and hierarchical Trust Modules.

- [CORD](#cord)
    - [Key Management](#key-management)
    - [Building docker image](#building-docker-image)
    - [Build code without docker](#build-code-without-docker)
      - [Building in dev mode](#building-in-dev-mode)
      - [Building in performant release mode](#building-in-performant-release-mode)
    - [Commands](#commands)
      - [Node binary](#node-binary)
      - [Polkadot Web UI](#polkadot-web-ui)
  - [Node Modules functionalities](#node-modules-functionalities)
    - [DID Module](#did-module)
      - [Add](#add)
      - [CRUD](#crud)
    - [#MARK Type Module](#mark-type-module)
    - [Attestation Module](#attestation-module)
      - [Add](#add-1)
      - [Revoke](#revoke)
      - [Lookup](#lookup)
    - [Hierarchy of Trust Module](#hierarchy-of-trust-module)
      - [Create root](#create-root)
      - [Add delegation](#add-delegation)
      - [Revoke](#revoke-1)
  - [bs58-cli Install](#bs58-cli-install)
  - [Substrate Documentation:](#substrate-documentation)
    - [Substrate Tutorials](#substrate-tutorials)
    - [Substrate JSON-RPC API](#substrate-json-rpc-api)
    - [Substrate Reference Rust Docs](#substrate-reference-rust-docs)


### Key Management
[Documentation](node/docs/keys.md)

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

with persistent mount that will keep the kchain data locally:

```
docker run -p 9944:9944 -p 9933:9933 -v /my/local/folder:/cord local/cord:develop  --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

To setup the local node with proper aura and granda keys, execute below:

```
bash ./scripts/setup-dev-chain.sh
```

For execution see the section about [Commands](#commands).

### Build code without docker

You need to have [rust and cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed. Clone this repo and navigate into it.

You can build it by executing these commands:

```
./scripts/init.sh
```

#### Building in dev mode

```
cargo build
```

start, by running:

```
./target/debug/cord [node command]
```

#### Building in performant release mode

```
cargo build --release
```

start, by running:

```
./target/release/cord [node command]
```

For execution see the section about commands.

### Commands

To start the node you have following options:

- start-node.sh helper script
- executing the node binary directly

#### Node binary

After finished building the node binary can be found in the `./target` directory:
for dev mode build:

```
./target/debug/cord --dev --ws-port 9944 --ws-external --rpc-external
```

for release mode build:

```
./target/release/cord --dev --ws-port 9944 --ws-external --rpc-external
```
#### Polkadot Web UI
Add the following code to `Developer` -> `Settings`
``` json
{
    "Address": "AccountId",
    "BlockNumber": "u64",
    "DelegationNodeId": "Hash",
    "ErrorCode": "u16",
    "Index": "u64",
    "LookupSource": "AccountId",
    "Permissions": "u32",
    "PublicBoxKey": "Hash",
    "PublicSigningKey": "Hash",
    "RefCount": "u8",
    "Signature": "MultiSignature"
}
```
## Node Modules functionalities

The CORD node provides an immutable transaction ledger for various workflows supported by the network.

Building our blockchain on Substrate has multiple advantages. Substrate has a very
good fundamental [architecture](https://substrate.dev/docs/en/knowledgebase/runtime/) and [codebase](https://github.com/paritytech/substrate) created by blockchain experts. Substrate
framework is developed in Rust, a memory efficient and fast compiled system programming
language, which provides a secure environment with virtually no runtime errors. Moreover, the
node runtime is also compiled to WebAssembly, so older version native nodes can always run
the latest version node runtime in a WebAssembly virtual machine to bypass the problem of a
blockchain fork. Importantly, there is a vibrant developer community and rich [documentation](https://substrate.dev/).

Our implementation is based on the [substrate-node-template](https://github.com/substrate-developer-hub/substrate-node-template) library (skeleton template for
quickly building a substrate based blockchain), which is linked to the main Substrate
codebase.

**Remote Procedure Calls**

The [Polkadot API](https://polkadot.js.org/api/) helps with communicating with the JSON-RPC endpoint, and the clients and services never have to talk directly with the endpoint.

**Blocktime**

The blocktime is currently set to 4 seconds, but this setting is subject to change based on
further research. 

**Extrinsics and Block Storage**

In Substrate, the blockchain transactions are abstracted away and are generalised as
[extrinsics](https://docs.substrate.dev/docs/extrinsics) in the system. They are called extrinsics since they can represent any piece of information that is regarded as input from “the outside world” (i.e. from users of the network) to the blockchain logic. The blockchain transactions are implemented through these
general extrinsics, that are signed by the originator of the transaction. We use this framework
to write the protocol specific data entries on the blockchain: [DID],
[MTYPEhash], [Attestation] and [Delegation]. The processing of each of these entry types is
handled by our custom runtime modules.

Under the current consensus algorithm, authority validator nodes (whose addresses are listed
in the genesis block) can create new blocks. These nodes [validate](https://substrate.dev/docs/en/knowledgebase/learn-substrate/tx-pool#transaction-lifecycle) incoming transactions, put
them into the pool, and include them in a new block. While creating the block, the node
executes the transactions and stores the resulting state changes in its local storage. Note that
the size of the entry depends on the number of arguments the transaction, (i.e., the respective
extrinsic method) has. The size of the block is hence dynamic and will depend on the number
and type of transactions included in the new block. The valid new blocks are propagated
through the network and other nodes execute these blocks to update their local state (storage).

**Authoring & Consensus Algorithm**

We use [Aura](https://wiki.parity.io/Aura) as the authoring algorithm, since we are a permissioned blockchain mode.

For consensus we use [GRANDPA](https://github.com/paritytech/substrate#2-description).

### DID Module

The node runtime defines an DID module exposing:

#### Add

```rust
add(origin, sign_key: T::PublicSigningKey, box_key: T::PublicBoxKey, doc_ref: Option<Vec<u8>>) -> Result
```

This function takes the following parameters:

- origin: public [ss58](<https://wiki.parity.io/External-Address-Format-(SS58)>) address of the caller of the method
- sign_key: the [ed25519](http://ed25519.cr.yp.to/) public signing key of the owner
- box_key: the [x25519-xsalsa20-poly1305](http://nacl.cr.yp.to/valid.html) public encryption key of the owner
- doc_ref: Optional u8 byte vector representing the reference (URL) to the DID
  document

The node verifies the transaction signature corresponding to the owner and
inserts it to the blockchain storage by using a map:

```rust
T::AccountId => (T::PublicSigningKey, T::PublicBoxKey, Option<Vec<u8>>)
```

#### CRUD

As DID supports CRUD (Create, Read, Update, Delete) operations, a `get(dids)` method
reads a DID for an account address, the add function may also be used to update a DID and
a `remove(origin) -> Result` function that takes the owner as a single parameter removes the DID from the
map, so any later read operation call does not return the data of a removed DID.

### #MARK Type Module

The node runtime defines an MARK TYPE (Schema) module exposing

```rust
add(origin, hash: T::Hash) -> Result
```

This function takes following parameters:

- origin: public [ss58](https://substrate.dev/docs/en/knowledgebase/advanced/ss58-address-format) address of the caller of the method
- hash: MTYPE hash as a [blake2b](https://blake2.net/) string

The node verifies the transaction signature corresponding to the creator and
inserts it to the blockchain storage by using a map:

```rust
T::Hash => T::AccountId
```

### Attestation Module

The node runtime defines an Attestation module exposing functions to

- add an attestation (`add`)
- revoke an attestation (`revoke`)
- lookup an attestation (`lookup`)
- lookup attestations for a delegation (used later in Complex Trust Structures)
  on chain.

#### Add

```rust
add(origin, claim_hash: T::Hash, mtype_hash: T::Hash, delegation_id: Option<T::DelegationNodeId>) -> Result
```

The `add` function takes following parameters:

- origin: The caller of the method, i.e., public address ([ss58](https://substrate.dev/docs/en/knowledgebase/advanced/ss58-address-format)) of the Attester
- claim_hash: The Claim hash as [blake2b](https://blake2.net/) string used as the key of the entry
- mtype_hash: The [blake2b](https://blake2.net/) hash of MTYPE used when creating the Claim
- delegation_id: Optional reference to a delegation which this attestation is based
  on

The node verifies the transaction signature and insert it to the state, if the provided attester
didn’t already attest the provided claimHash. The attestation is stored by using a map:

```rust
T::Hash => (T::Hash,T::AccountId,Option<T::DelegationNodeId>,bool)
```

Delegated Attestations are stored in an additional map:

```rust
T::DelegationNodeId => Vec<T::Hash>
```

#### Revoke

```rust
revoke(origin, claim_hash: T::Hash) -> Result
```

The `revoke` function takes the claimHash (which is the key to lookup an attestation) as
argument. After looking up the attestation and checking invoker permissions, the revoked
flag is set to true and the updated attestation is stored on chain.

#### Lookup

The attestation lookup is performed with the `claimHash`, serving as the key to the
attestation store. The function `get_attestation(claimHash)` is exposed to the outside
clients and services on the blockchain for this purpose.

Similarly, as with the simple lookup, to query all attestations created by a certain delegate,
the runtime defines the function `get_delegated_attestations(DelegationNodeId)`
that is exposed to the outside.

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
create_root(origin, root_id: T::DelegationNodeId, mtype_hash: T::Hash) -> Result
```

The `create_root` function takes the following parameters:

- origin: The caller of the method, i.e., public address (ss58) of the owner of the
  trust hierarchy
- root_id: A V4 UUID identifying the trust hierarchy
- mtype_hash: The blake2b hash of the MTYPE the trust hierarchy is associated with

The node verifies the transaction signature and insert it to the state. The root is stored by using
a map:

```rust
T::DelegationNodeId => (T::Hash,T::AccountId,bool)
```

#### Add delegation

```rust
add_delegation(origin, delegation_id: T::DelegationNodeId, root_id: T::DelegationNodeId, parent_id: Option<T::DelegationNodeId>, delegate: T::AccountId, permissions: Permissions, delegate_signature: T::Signature) -> Result
```

The `add_delegation` function takes the following parameters:

- origin: The caller of the method, i.e., public address (ss58) of the delegator
- delegation_id: A V4 UUID identifying this delegation
- root_id: A V4 UUID identifying the associated trust hierarchy
- parent_id: Optional, a V4 UUID identifying the parent delegation this delegation is
  based on
- MTYPEHash: The blake2b hash of MTYPE used when creating the Claim
- delegate: The public address (ss58) of the delegate (ID receiving the delegation)
- permissions: The permission bit set (having 0001 for attesting permission and
  0010 for delegation permission)
- delegate_signature: ed25519 based signature by the delegate based on the
  delegationId, rootId, parentId and permissions

The node verifies the transaction signature and the delegate signature as well as all other data
to be valid and the delegator to be permitted and then inserts it to the state. The delegation is
stored by using a map:

```rust
T::DelegationNodeId => (T::DelegationNodeId,Option<T::DelegationNodeId>,T::AccountId,Permissions,bool)
```

Additionally, if the delegation has a parent delegation, the information about the children of its
parent is updated in the following map that relates parents to their children:

```rust
T::DelegationNodeId => Vec<T::DelegationNodeId>
```

#### Revoke

```rust
revoke_root(origin, root_id: T::DelegationNodeId) -> Result
```

and

```rust
revoke_delegation(origin, delegation_id: T::DelegationNodeId) -> Result
```
## bs58-cli Install
```
cargo install bs58-cli
```
## Substrate Documentation:
### [Substrate Tutorials](https://substrate.dev/en/tutorials)
### [Substrate JSON-RPC API](https://polkadot.js.org/docs/substrate/rpc)
### [Substrate Reference Rust Docs](https://substrate.dev/rustdocs/v2.0.0/sc_service/index.html)

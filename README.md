<!-- [![Build and Test](https://github.com/dhiway/cord/workflows/Build%20and%20Test/badge.svg)](https://github.com/dhiway/cord/actions) -->

# CORD

The CORD node implementation uses Parity Substrate as the underlying
technology stack with custome pallets and hierarchical trust Modules.

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

A valid account only requires a private key that can sign on one of the supported curves and signature schemes. CORD uses Sr25519 as the signature scheme for all inhjected accounts.

#### Polkadot{.js} Browser Plugin

The browser plugin is available for both [Google Chrome](https://chrome.google.com/webstore/detail/polkadot%7Bjs%7D-extension/mopnmbcafieddcagagdcbnhejhlodfdd?hl=en) (and Chromium based browsers like Brave) and [FireFox](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension).

#### Polkadot Web UI

Add the content from [custom-types.json](./custom-types.json) code to `Developer` -> `Settings`

### Building docker image

Clone this repo and navigate into it.

Build docker image:

```
docker build -t local/cord:main .
```

start, by running:

```
docker run -p 9944:9944 -p 9933:9933 local/cord:main --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

with persistent mount that will keep the chain data locally:

```
docker run -p 9944:9944 -p 9933:9933 -v /my/local/folder:/cord local/cord:main  --dev --ws-port 9944 --ws-external --rpc-external --rpc-methods Unsafe
```

## Node Modules functionalities

The CORD node provides an immutable transaction ledger for various workflows supported by the network.

**Remote Procedure Calls**

The [Polkadot API](https://polkadot.js.org/api/) helps with communicating with the JSON-RPC endpoint, and the clients and services never have to talk directly with the endpoint.

**Blocktime**

The blocktime is currently set to 4 seconds, but this setting is subject to change based on further research.

**Extrinsics and Block Storage**

In Substrate, the blockchain transactions are abstracted away and are generalised as [extrinsics](https://docs.substrate.dev/docs/extrinsics) in the system. They are called extrinsics since they can represent any piece of information that is regarded as input from “the outside world” (i.e. from users of the network) to the blockchain logic. The blockchain transactions are implemented through these general extrinsics, that are signed by the originator of the transaction. We use this framework to write the protocol specific data entries on the blockchain: [SCHEMA] and [STREAM]. The processing of each of these entry types is handled by our custom runtime modules.

Under the current consensus algorithm, authority validator nodes (whose addresses are listed
in the genesis block) can create new blocks. These nodes [validate](https://substrate.dev/docs/en/knowledgebase/learn-substrate/tx-pool#transaction-lifecycle) incoming transactions, put them into the pool, and include them in a new block. While creating the block, the node executes the transactions and stores the resulting state changes in its local storage. Note that the size of the entry depends on the number of arguments the transaction, (i.e., the respective extrinsic method) has. The size of the block is hence dynamic and will depend on the number and type of transactions included in the new block. The valid new blocks are propagated through the network and other nodes execute these blocks to update their local state (storage).

**Authoring & Consensus Algorithm**

For Authoring we use Babe.

For consensus we use [GRANDPA](https://github.com/paritytech/substrate#2-description).

**Governance**

- [TBD]

## Substrate Benchmarking -

To know more about generating weights for the specific pallets -
Go to - docs/benchmarking.md

or check out the weights file in the respective pallets.

### Substrate Documentation

#### [Substrate Tutorials](https://substrate.dev/en/tutorials)

#### [Substrate JSON-RPC API](https://polkadot.js.org/docs/substrate/rpc)

#### [Substrate Reference Rust Docs](https://substrate.dev/rustdocs/v2.0.0/sc_service/index.html)

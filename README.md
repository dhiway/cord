This repository contains the Rust implementation of a [CORD Network][cord-homepage] node based on the [Substrate][substrate-homepage] framework.

# CORD

CORD is a global public utility and trust framework designed to address trust gaps, manage transactions, and facilitate the exchange of value at scale.

It simplifies the management of information, making it easier for owners to control their data. Agencies and businesses can discover, access, and use this data to deliver networked public services. CORD provides a transparent history of information, protecting it from unauthorized tampering both inside and outside the system.

Building on the modular approach of the Substrate framework, CORD defines a rich set of primitives to foster exceptional innovation across various industries. These innovations support transactions and record maintenance in sectors such as finance, trade, health, energy, water resources, agriculture, and many more.

CORD now supports multiple runtimes, each tailored to different types of networks:

- **Braid**: Optimized for enterprise networks, Braid functions without on-chain governance. It’s ideal for private, high-performance environments where speed and efficiency are crucial.
- **Loom**: Designed for ecosystem-level networks, Loom incorporates on-chain governance. It excels at managing and coordinating multi-party networks, making it suitable for sectors that require robust governance and regulatory compliance.
- **Weave**: Targeted at permissionless networks, Weave is intended for open and decentralized environments where anyone can participate without prior authorization. Please note that Weave is currently a work in progress, with ongoing development to enhance its capabilities.

By offering these distinct runtimes, CORD provides a versatile foundation tailored to meet the specific needs of various industries and applications, enhancing its ability to deliver effective networked public services.

## Get Started

The first step in becoming a blockchain developer with CORD is to learn how to compile and launch a single local blockchain node. In this tutorial, you'll build and start a single node blockchain using the CORD framework.

The CORD repository provides everything you need to set up a fully functional single-node blockchain that you can run locally in your development environment. This setup includes several predefined components—such as user accounts, assets, smart-contracts, governance, identifiers, statements, chain-space—allowing you to experiment with common tasks right away. You can build and run the node as-is to produce blocks and facilitate transactions immediately.

### Steps to Get Started

1. **Clone the Repository**:

   ```sh
   git clone https://github.com/dhiway/cord.git
   cd cord
   ```

2. **Build CORD in Release Mode**:

   ```sh
   cargo build --release
   ```

   This will create the production binary in the `target/release` directory.

3. **Run a Single Node Local CORD Network**:

   Start the node in development mode:

   ```bash
   ./target/release/cord --dev
   ```

   The `--dev` option specifies that the node runs in development mode, using a predefined development chain specification. This ensures the node runs in a clean state every time it is restarted.

After starting your local CORD blockchain node, this tutorial will guide you on how to use the CORD front-end to view blockchain activity and submit transactions.

By following these steps, you'll gain hands-on experience with the foundational elements of CORD, setting you on the path to becoming a proficient blockchain developer.

### Prerequisites

Before you begin, ensure you have the [necessary packages](/docs/installation.md) to locally run CORD.

## Compile a CORD node

If you have already compiled the node on the local computer, you can skip this section and continue to [Start the local node](#start-the-local-node).

To compile the node :

1. Open a terminal shell on your computer.

1. Clone the cord node repository by running the following command:

   ```bash
   git clone https://github.com/dhiway/cord.git
   ```

   This command clones the `develop` branch.

1. Change to the root of the CORD node directory by running the following command:

   ```bash
   cd cord
   ```

   Create a new branch to contain your work:

   ```bash
   git switch -c my-branch-yyyy-mm-dd
   ```

   Replace `yyyy-mm-dd` with any identifying information that you desire, but we recommend a numerical year-month-day format. For example:

   ```bash
   git switch -c my-branch-2024-06-01
   ```

1. Compile the node by running the following command:

   ```bash
   cargo build --release
   ```

   You should always use the `--release` flag to build optimized artifacts.
   The first time you compile this, it takes some time to complete.

   It should complete with a line something like this:

   ```bash
   Finished release [optimized] target(s) in 5m 23s
   ```

## Start the local node

To get your local CORD node up and running, follow these steps:

1. Start the node in development mode:

   In the terminal where you compiled your node, run the following command:

   ```bash
   ./target/release/cord --dev
   ```

   This command starts the node in development mode using the predefined `loom` development chain specification. The `--dev` option ensures the node runs in a clean working state each time you restart it, deleting all active data such as keys, the blockchain database, and networking information. If you don't specify a runtime, the `loom` runtime is used by default.

2. Start the Braid node in development mode:

   If you want to run the Braid runtime, use the following command:

   ```bash
   ./target/release/cord braid --dev
   ```

3. Start the Loom node in development mode:

   To run the Loom runtime, use this command:

   ```bash
   ./target/release/cord loom --dev
   ```

4. Verify your node is up and running successfully by reviewing the output displayed in the terminal.

   The terminal should display output similar to this:

   ```text
   2024-06-11 16:22:52 Dhiway CORD
   2024-06-11 16:22:52 ✌️  version 0.9.3-5cd85df03fb
   2024-06-11 16:22:52 ❤️   by Dhiway Networks <info@dhiway.com>, 2019-2024
   2024-06-11 16:22:52 📋 Chain specification: Loom Development
   2024-06-11 16:22:52 🏷  Node name: low-pull-3415
   2024-06-11 16:22:52 👤 Role: AUTHORITY
   2024-06-11 16:22:52 💾 Database: ParityDb at /var/folders/ww/gtrj_81s6hj7p_6bd0b3qgb00000gn/T/substrateyVRCDn/chains/loom-dev/paritydb/full
   2024-06-11 16:22:55 🔨 Initializing Genesis block/state (state: 0x717a…e54e, header-hash: 0x1ae3…3314)
   2024-06-11 16:22:55 👴 Loading GRANDPA authority set from genesis on what appears to be first startup.
   2024-06-11 16:22:55 👶 Creating empty BABE epoch changes on what appears to be first startup.
   2024-06-11 16:22:55 🏷  Local node identity is: 12D3KooWButjQ1xjMkDM8BDLrJipvqBNcGhLnRWJTR5MbU1YaMUN
   ...
   ...
   ...
   ...
   2024-06-11 16:23:05 💤 Idle (0 peers), best: #3 (0x3a75…1901), finalized #1 (0x1ab2…ff17), ⬇ 0 ⬆ 0
   ```

   If the number after `finalized` is increasing, your blockchain is producing new blocks and reaching consensus about the state they describe.

5. Keep the terminal that displays the node output open to continue.

These steps will help you set up and experiment with different runtimes supported by CORD, each tailored to specific network requirements. Enjoy exploring the versatile capabilities of the CORD framework!

### Using Docker

> #### Install Docker
>
> If Docker is not installed on your system, you can check by running: `which docker`. To install Docker, follow the [official installation documentation](https://docs.docker.com/engine/install/).

1. First, let's check the version of CORD. The first time you run this command, the CORD Docker image will be downloaded. This may take some time and bandwidth, so please be patient:

   ```bash
   docker run --rm dhiway/cord --version
   ```

   You can also pass any argument or flag that CORD supports:

   ```bash
   docker run --name "CordDocker" --rm dhiway/cord --dev
   ```

2. Once you are done experimenting and picking the best node name, you can start CORD as a daemon, exposing the necessary ports and mounting a volume to store your blockchain data locally. Make sure to create a Docker volume for mounting or pass a separate mount (disk) for the process.

   Create a Docker volume:

   ```sh
   docker volume create cord
   ```

3. To start a CORD node with the default RPC port 9933, default P2P port 30333, and default Prometheus port 9615, use the following command:

   ```bash
   docker run -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord:develop --dev --rpc-external --rpc-cors all
   ```

4. If you want to specify a custom node name, add the arg `--name "YourName"` to the command:

   ```bash
   docker run --name "CordDocker" -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord --dev --rpc-external --rpc-cors all
   ```

### Note on Using Braid and Loom Runtimes

CORD supports multiple runtimes, including Braid and Loom. To specify a runtime, you can add the runtime name before the `--dev` flag. For example:

- To start a Braid runtime node:

  ```bash
  docker run --name "CordDocker" -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord:develop braid --dev --rpc-external --rpc-cors all
  ```

- To start a Loom runtime node:

  ```bash
  docker run --name "CordDocker" -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord:develop loom --dev --rpc-external --rpc-cors all
  ```

## Other projects of interest

This repository contains the complete code for the CORD blockchain framework. To effectively interact with the chain, you may need several additional components:

- **[CORD.js](https://github.com/dhiway/cord.js)**: This SDK is essential for building applications that use CORD. It provides methods to interact with the CORD node, enabling seamless integration and interaction with the network.

- **[Apps UI](https://apps.cord.network)**: This user interface project is managed through the [apps](https://github.com/dhiway/apps) repository, providing an intuitive way to interact with the network.

- **[Telemetry](https://telemetry.cord.network)**: Monitor the network through this telemetry interface.

- **_GraphQL Interface_**: Currently in beta, this interface facilitates advanced data queries and is under development in the [cord-subql](https://github.com/dhiway/cord-subql) repository.

- **[Demo Scripts](https://github.com/dhiway/cord-demo-scripts)**: Explore these demo scripts to connect and interact with the CORD Chain pallets/modules. They utilize the `cord.js` SDK to facilitate chain interactions.

## Contributing

If you would like to contribute, please fork the repository, follow the [contributions] guidelines, introduce your changes and submit a pull request. All pull requests are warmly welcome.

### Before submitting the PR

There are 3 tests which run as part of PR validation.

- Build - `cargo build --release`

- Clippy - `cargo clippy --all --no-deps --all-targets --features=runtime-benchmarks -- -D warnings`

- Test - `cargo test --release  --locked --features=runtime-benchmarks --no-fail-fast --verbose --color always --all --all-targets`

Note that each of these would take significant time to run, and hence, if you are working on a specific pallet, you can use `-p <pallet-name> --lib` instead of `--all`. That should be faster than normal full test locally.

## License

The code in this repository is licensed under the terms of the [GPL 3.0 licensed](LICENSE-GPL3).

[cord-homepage]: https://cord.network
[substrate-homepage]: https://substrate.io
[contributions]: ./CONTRIBUTING.md

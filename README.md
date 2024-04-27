This repository contains the Rust implementation of a [CORD Network][cord-homepage] node based on the [Substrate][substrate-homepage] framework.

# CORD

CORD is a global public utility and trust framework that is designed to address trust gaps, manage transactions and exchange value at scale.

It is designed to simplify the management of information, making it easier for owners to control; agencies and businesses to discover, access and use data to deliver networked public services. It provides a transparent history of information and protects it from unauthorized tampering from within or without the system.

CORD builds on the modular approach of the Substrate framework. It defines a rich set of primitives to help design exceptional levels of innovations for existing and emerging industries. Such innovative approaches cover conducting transactions and maintaining records across several sectors such as finance, trade, health, energy, water resources, agriculture and many more.

## Quick Start Guide

### Prerequisites

Before you begin, ensure you have the [necessary packages](/docs/installation.md) to locally run CORD.

### Clone the repository at your desired location

```sh
git clone https://github.com/dhiway/cord.git

cd cord
```

### Build CORD in release mode

To build the release version of the project, optimized for performance, execute the following command:

```bash
cargo build --release
```

This will create the production binary in the `target/release` directory.
When CORD is built using `cargo build` the binary will be present in `target/debug` directory.

### Run Single Node Local CORD Network

To run the production binary locally with the `--dev` flag, use the following command:

```bash
./target/production/cord help | less
./target/production/cord --dev
```

Detailed logs may be shown by running the chain with the following environment variables set:

```bash
RUST_LOG=debug RUST_BACKTRACE=1 ./target/production/cord --dev
```

### Using Docker

> #### Install Docker
>
>  Docker maybe not be installed on your system, so to check if docker is installed, run: `which docker`. To install docker on your system follow the [official installtion documentation](https://docs.docker.com/engine/install/) of docker.


1. Let's first check the version we have. The first time you run this command, the CORD docker image will be downloaded. This takes a bit of time and bandwidth, be patient:

```bash
docker run --rm dhiway/cord --version
```

You can also pass any argument/flag that CORD supports:

```bash
docker run --name "CordDocker" --rm dhiway/cord --dev 
```

Once you are done experimenting and picking the best node name :) you can start CORD as daemon, exposes the CORD ports and mount a volume that will keep your blockchain data locally.

Make sure that you create a docker volume to mount on, or pass the separate mount (disk) to use for the process.

```sh
docker volume create cord
```

2. To start a CORD node on default rpc port 9933 and default p2p port 30333 & default prometheus port is 9615 use the following command:

```bash
docker run -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord:develop --dev --rpc-external --rpc-cors all
```

3. Additionally if you want to have custom node name you can add the `--name "YourName"` at the end

```bash
docker run --name "CordDocker" -d -p 9944:9944 -p 30333:30333 -p 9933:9933 -p 9615:9615 -v cord:/data dhiway/cord --dev --rpc-external --rpc-cors all 
```


## Other projects of interest

This repository contains the complete code of the CORD Network Blockchain. To interact with the chain, one needs multiple things.

1. SDK to connect and build on top.

  - Check [CORD.js](https://github.com/dhiway/cord.js) repository for SDK development. You can interact with the pallets implemented here through the corresponding modules in the `cord.js` project.

2. A UI to explore the chain.

  - There are multiple projects which are required when we need to monitor the chain, and make use of the transactions.

  - [Telemetry](https://telemetry.cord.network) - This is hosted through [substrate-telemetry](https://github.com/dhiway/substrate-telemetry) project.

  - [Apps UI](https://apps.cord.network) - This project is managed through [apps](https://github.com/dhiway/apps) repository.

  - *GraphQL Interface* - This project is still in beta. Development work is happening at [cord-subql](https://github.com/dhiway/cord-subql) repository.

3. Demostratable scripts and services, so one can take and build upon

  - [Demo Scripts](https://github.com/dhiway/cord-demo-scripts) - Demo scripts are provided to connect and interact with the CORD Chain pallets/modules. This uses CORD.js SDK to interact with chain.


## Contributing

If you would like to contribute, please fork the repository, follow the [contributions] guidelines, introduce your changes and submit a pull request. All pull requests are warmly welcome.

### Before submitting the PR

There are 3 tests which run as part of PR validation.

* Build - `cargo build --release`

* Clippy - `cargo clippy --all --no-deps --all-targets --features=runtime-benchmarks -- -D warnings`

* Test - `cargo test --release  --locked --features=runtime-benchmarks --no-fail-fast --verbose --color always --all --all-targets`

Note that each of these would take significant time to run, and hence, if you are working on a specific pallet, you can use `-p <pallet-name> --lib` instead of `--all`. That should be faster than normal full test locally.


## License

The code in this repository is licensed under the terms of the [GPL 3.0 licensed](LICENSE-GPL3).

[cord-homepage]: https://cord.network
[substrate-homepage]: https://substrate.io
[contributions]: ./CONTRIBUTING.md
[discord]: https://discord.gg/bcwZFznb7Z

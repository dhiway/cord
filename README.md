This repository contains the Rust implementation of a [CORD Network][cord-homepage] node based on the [Substrate][substrate-homepage] framework.

# CORD

CORD is a global public utility and trust framework that is designed to address trust gaps, manage transactions and exchange value at scale.

It is designed to simplify the management of information, making it easier for owners to control; agencies and businesses to discover, access and use data to deliver networked public services. It provides a transparent history of information and protects it from unauthorized tampering from within or without the system.

CORD builds on the modular approach of the Substrate framework. It defines a rich set of primitives to help design exceptional levels of innovations for existing and emerging industries. Such innovative approaches cover conducting transactions and maintaining records across several sectors such as finance, trade, health, energy, water resources, agriculture and many more.

## Quick Start Guide

### Prerequisites

Before you begin, ensure you have the [necessary packages](/docs/installation.md) to locally run CORD.


### Build CORD in production mode

To build the production version of the project, optimized for performance, execute the following command:

```bash
make
```

This will create the production binary in the `target/production` directory.


### Build the CORD in debug mode:

To build the project in the `debug` profile, run the following command:

```bash
make build-debug
```

This will compile the project without optimization and generate the executable in the `target/debug` directory.

### Checking the CORD

To perform a static analysis of the entire project, use the following command:

```bash
make check
```

This will check the code and dependencies for errors without producing an executable.

### Running Tests

To run all tests in the project, including runtime benchmarks and without fail-fast, run the following command:

```bash
make test
```

### Run Single Node Local CORD Network

To run the production binary locally with the `--dev` flag, use the following command:

```bash
make run-local
```

Detailed logs may be shown by running the chain with the following environment variables set:

```bash
RUST_LOG=debug RUST_BACKTRACE=1 make run-local
```

### Displaying Help Message

To see the help message of the production binary, run the following command:

```bash
make help
```

### Clean Build Artifacts

To remove all build artifacts, use the following command:

```bash
make clean
```

This will clean up any compiled files and reset the project to a clean state.

## Specific Pallet Checks and Tests

The `Makefile` also provides specific rules to `check` and `test` individual `pallets`. 
You can use these commands to `check` and `test` specific parts of the project.

### Check Pallets

To check specific pallets, use the following commands:

```bash
make check-did
make check-authorship
make check-did-name
make check-registry
make check-schema
make check-stream
```

### Test Pallets

To run tests for specific pallets, use the following commands:

```bash
make test-did
make test-authorship
make test-did-name
make test-registry
make test-schema
make test-stream
```

---

### Using Docker

1. Let's first check the version we have. The first time you run this command, the CORD docker image will be downloaded. This takes a bit of time and bandwidth, be patient:

```bash
docker run --rm -it dhiway/cord:develop --version
```

You can also pass any argument/flag that CORD supports:

```bash
docker run --rm -it dhiway/cord:develop --dev --name "CordDocker"
```

Once you are done experimenting and picking the best node name :) you can start CORD as daemon, exposes the CORD ports and mount a volume that will keep your blockchain data locally. 
Make sure that you set the ownership of your local directory to the CORD user that is used by the container. Set user id 1000 and group id 1000, by running `chown 1000.1000 /my/local/folder -R` if you use a bind mount.

2. To start a CORD node on default rpc port 9933 and default p2p port 30333 use the following command. If you want to connect to rpc port 9933, then must add CORD startup parameter: `--rpc-external`.

```bash
docker run -d -p 30333:30333 -p 9933:9933 -v /my/local/folder:/cord dhiway/cord:develop --dev --rpc-external --rpc-cors all
```

3. Additionally if you want to have custom node name you can add the `--name "YourName"` at the end

```bash
docker run -d -p 30333:30333 -p 9933:9933 -v /my/local/folder:/cord dhiway/cord:develop --dev --rpc-external --rpc-cors all --name "CordDocker"
```

4. If you also want to expose the webservice port 9944 use the following command:

```bash
docker run -d -p 30333:30333 -p 9933:9933 -p 9944:9944 -v /my/local/folder:/cord dhiway/cord:develop --dev --ws-external --rpc-external --rpc-cors all --name "CordDocker"
```

5. To get up and running with the smallest footprint on your system, you may use the CORD Docker image.
You can build it yourself (it takes a while...).

```bash
docker build -t local/cord:develop .
```

## Other projects of interest

This repository contains the complete code of the CORD Network Blockchain. To interact with the chain, one needs multiple things.

1. SDK to connect and build on top.

  - Check [CORD.js](https://github.com/dhiway/cord.js) repository for SDK development. You can interact with the pallets implemented here through the corresponding modules in the `cord.js` project.

2. A UI to explore the chain.

  - There are multiple projects which are required when we need to monitor the chain, and make use of the transactions.

  - [Telemetry](https://telemetry.cord.network) - This is hosted through 'substrate-telemetry' project.

  - [Apps UI](https://apps.cord.network) - This project is managed through [apps](https://github.com/dhiway/apps) repository.

  - [GraphQL Interface](/) - This project is still in beta. Development work is happening at [cord-subql](https://github.com/dhiway/cord-subql) repository.

3. Demostratable scripts and services, so one can take and build upon

  - [Demo Scripts](https://github.com/dhiway/cord-demo-scripts) - Demo scripts are provided to connect and interact with the CORD Chain pallets/modules. This uses CORD.js SDK to interact with chain.

  - [Demo API Service](https://github.com/dhiway/cord-agent-app-v1) - Demo API Server using nodejs/express.js combination, which uses SDK to in backend and provides basic APIs and also provides a mechanism to store application data in DB, so data along with chain interactions can be managed.


## Contributing

If you would like to contribute, please fork the repository, follow the [contributions] guidelines, introduce your changes and submit a pull request. All pull requests are warmly welcome.

## License

The code in this repository is licensed under the terms of the [GPL 3.0 licensed](LICENSE-GPL3).

[cord-homepage]: https://cord.network
[substrate-homepage]: https://substrate.io
[contributions]: ./CONTRIBUTING.md
[discord]: https://discord.gg/bcwZFznb7Z

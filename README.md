This repository contains the Rust implementation of a [CORD Network][cord-homepage] node based on the [Substrate][substrate-homepage] framework.

# CORD

CORD is a global public utility and trust framework that is designed to address trust gaps, manage transactions and exchange value at scale.

It is designed to simplify the management of information, making it easier for owners to control; agencies and businesses to discover, access and use data to deliver networked public services. It provides a transparent history of information and protects it from unauthorized tampering from within or without the system.

CORD builds on the modular approach of the Substrate framework. It defines a rich set of primitives to help design exceptional levels of innovations for existing and emerging industries. Such innovative approaches cover conducting transactions and maintaining records across several sectors such as finance, trade, health, energy, water resources, agriculture and many more.

### Getting Started

This section will guide you through the **following steps** needed to prepare a computer for **CORD** development. Since CORD is built with [the Rust programming language](https://www.rust-lang.org/), the first thing you will need to do is prepare the computer for Rust development - these steps will vary based on the computer's operating system.

Once Rust is configured, you will use its toolchains to interact with Rust projects; the commands for Rust's toolchains will be the same for all supported, Unix-based operating systems.

Also, join and discover the various [discord] channels where you can engage, participate and keep up with the latest developments.

## 1. Install dependencies

### Ubuntu/Debian

Use a terminal shell to execute the following commands:

```bash
sudo apt update
# May prompt for location information
sudo apt install -y git clang curl libssl-dev llvm libudev-dev pkg-config
```

### Arch Linux

Run these commands from a terminal:

```bash
pacman -Syu --needed --noconfirm curl git clang
```

### Fedora

Run these commands from a terminal:

```bash
sudo dnf update
sudo dnf install clang curl git openssl-devel
```

### OpenSUSE

Run these commands from a terminal:

```bash
sudo zypper install clang curl git openssl-devel llvm-devel libudev-devel
```

### macOS

Open the Terminal application and execute the following commands:

```bash
# Install Homebrew if necessary https://brew.sh/
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"

# Make sure Homebrew is up-to-date, install openssl and protobuf
brew update
brew install openssl
brew install protobuf
```

## 2. Rust developer environment

This guide uses <https://rustup.rs> installer and the `rustup` tool to manage the Rust toolchain.
First, install and configure `rustup`:

```bash
# Install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Configure
source ~/.cargo/env
```

Configure the Rust toolchain to default to the latest stable version, add nightly and the nightly wasm target:

```bash
rustup default stable
rustup update
rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
```

## 3. Build the node

You can use rustup to install a specific version of rust, including its custom compilation targets. Using rustup, it should set a proper toolchain automatically while you call rustup show within project's root directory. Naturally, we can try to use different versions of these dependencies, i.e. delivered by system's default package manager. To compile the CORD node:

1. Clone this repository by running the following command:

   ```bash
   git clone https://github.com/dhiway/cord
   ```

1. Change to the root of the node template directory by running the following command:

   ```bash
   cd cord
   git checkout develop
   ```

1. Compile by running the following command:

   ```bash
   cargo build --release
   ```

   You should always use the `--release` flag to build optimized artifacts.

## 4. Run the node

1. Start the single-node development chain, by running:

   ```
   ./target/release/cord --dev
   ```

1. Detailed logs may be shown by running the chain with the following environment variables set:

   ```bash
   RUST_LOG=debug RUST_BACKTRACE=1 ./target/release/cord --dev
   ```

1. Additional CLI usage options

   ```
   ./target/release/cord --help
   ```

## 5. Using Docker

The easiest/faster option to run CORD in Docker is to use the latest release images. These are small images that use the latest official release of the CORD binary, pulled from our package repository.

1. Let's first check the version we have. The first time you run this command, the CORD docker image will be downloaded. This takes a bit of time and bandwidth, be patient:

   ```bash
    docker run --rm -it dhiway/cord:develop --version
   ```

   You can also pass any argument/flag that CORD supports:

   ```bash
    docker run --rm -it dhiway/cord:develop --dev --name "CordDocker"
   ```

   Once you are done experimenting and picking the best node name :) you can start CORD as daemon, exposes the CORD ports and mount a volume that will keep your blockchain data locally. Make sure that you set the ownership of your local directory to the CORD user that is used by the container. Set user id 1000 and group id 1000, by running `chown 1000.1000 /my/local/folder -R` if you use a bind mount.

1. To start a CORD node on default rpc port 9933 and default p2p port 30333 use the following command. If you want to connect to rpc port 9933, then must add CORD startup parameter: `--rpc-external`.

   ```bash
   docker run -d -p 30333:30333 -p 9933:9933 -v /my/local/folder:/cord dhiway/cord:develop --dev --rpc-external --rpc-cors all
   ```

1. Additionally if you want to have custom node name you can add the `--name "YourName"` at the end

   ```bash
   docker run -d -p 30333:30333 -p 9933:9933 -v /my/local/folder:/cord dhiway/cord:develop --dev --rpc-external --rpc-cors all --name "CordDocker"
   ```

1. If you also want to expose the webservice port 9944 use the following command:

   ```bash
   docker run -d -p 30333:30333 -p 9933:9933 -p 9944:9944 -v /my/local/folder:/cord dhiway/cord:develop --dev --ws-external --rpc-external --rpc-cors all --name "CordDocker"
   ```

1. To get up and running with the smallest footprint on your system, you may use the CORD Docker image.
   You can build it yourself (it takes a while...).

   ```
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

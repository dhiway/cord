### Checking the CORD

To perform a static analysis of the entire project, use the following command:

```bash
cargo check --all --locked --all-targets --features=runtime-benchmarks --color always
```

This will check the code and dependencies for errors without producing an executable.

### Running Tests

To run all tests in the project, including runtime benchmarks and without fail-fast, run the following command:

```bash
cargo test --all --no-fail-fast --locked --all-targets --features=runtime-benchmarks --color always
```

### Build the CORD in debug mode:

To build the project in the `debug` profile, run the following command:

```bash
cargo build --locked
```

This will compile the project without optimization and generate the executable in the `target/debug` directory.

### Displaying Help Message

To see the help message of the production binary, run the following command:

```bash
./target/production/cord --help
```

### Clean Build Artifacts

To remove all build artifacts, use the following command:

```bash
cargo clean
```

This will clean up any compiled files and reset the project to a clean state.


## Specific Pallet Checks and Tests

### Check Pallets

To check specific pallets, use the following commands:

```bash
cargo check --package pallet-did
cargo check --package pallet-did-name
cargo check --package pallet-network-membership
cargo check --package pallet-chain-space
cargo check --package pallet-statement
cargo check --package pallet-network-score
```

### Test Pallets

To run tests for specific pallets, use the following commands:

```bash
cargo test --package pallet-did
cargo test --package pallet-did-name
cargo test --package pallet-network-membership
cargo test --package pallet-chain-space
cargo test --package pallet-statement
cargo test --package pallet-network-score
```

---
## Build Docker images


```sh
docker buildx create --use

docker buildx build --file docker/Dockerfile --tag local/cord:latest --platform linux/amd64,linux/arm64 --build-arg profile=production --load .
```

### Quickly start a 4 Node CORD Network

NOTE: To start a local network, be sure that it takes more CPU and memory, and on some laptops, it may not be able to run. In such cases, for all purposes, running a single instance with `--dev` option is good enough.

#### Start the network

```sh
bash scripts/run-local-cluster.sh
```

#### Stop the network

```sh
bash scripts/stop-local-cluster.sh
```


# Makefile for CORD Project

# Flags for common Cargo commands
CARGO_FLAGS := --locked --all-targets --features=runtime-benchmarks --color always

# Define the binary name
BINARY := cord

# Define the output for production builds
PROD_DIR := target/production

TEST_CHECK_PALLETS := check \
    check-did \
    check-authorship \
    check-did-name \
    check-registry \
    check-schema \
    check-stream \
    test \
    test-did \
    test-authorship \
    test-did-name \
    test-registry \
    test-schema \
    test-stream


# Rule to build the production version of the project
build:
	cargo build --locked --profile production

# Default rule, runs the production build
all: build

# Rule to build the project (debug profile)
build-debug:
	cargo build --locked

# Rule to check the project with all targets
check:
	cargo check --all $(CARGO_FLAGS)

# Rule to check the DID pallet with all targets
check-did:
	cargo check --package pallet-did $(CARGO_FLAGS)

# Rule to check the Athorship pallet with all targets
check-authorship:
	cargo check --package pallet-extrinsic-authorship $(CARGO_FLAGS)

# Rule to check the DID NAME pallet with all targets
check-did-name:
	cargo check --package pallet-did-names $(CARGO_FLAGS)

# Rule to check the REGISTRY pallet with all targets
check-registry:
	cargo check --package pallet-registry $(CARGO_FLAGS)

# Rule to check the SCHEMA pallet with all targets
check-schema:
	cargo check --package pallet-schema $(CARGO_FLAGS)

# Rule to check the STREAM pallet with all targets
check-stream:
	cargo check --package pallet-stream $(CARGO_FLAGS)

# Rule to run tests with runtime benchmarks and without fail-fast
test:
	cargo test --all --no-fail-fast $(CARGO_FLAGS)

# Rule to test the DID pallet with all targets
test-did:
	cargo test --package pallet-did $(CARGO_FLAGS) 

# Rule to test the Athorship pallet with all targets
test-authorship:
	cargo test --package pallet-extrinsic-authorship $(CARGO_FLAGS)

# Rule to test the DID NAME pallet with all targets
test-did-name:
	cargo test --package pallet-did-names $(CARGO_FLAGS)

# Rule to test the REGISTRY pallet with all targets
test-registry:
	cargo test --package pallet-registry $(CARGO_FLAGS)

# Rule to test the SCHEMA pallet with all targets
test-schema:
	cargo test --package pallet-schema $(CARGO_FLAGS)

# Rule to test the STREAM pallet with all targets
test-stream:
	cargo test --package pallet-stream $(CARGO_FLAGS)

# Rule to run the production binary with --dev flag
run-local: build
	$(PROD_DIR)/$(BINARY) --dev

# Rule to display the help message of the production binary
help: build
	$(PROD_DIR)/$(BINARY) --help

# Rule to clean build artifacts
clean:
	cargo clean

.PHONY: build build-debug run-local help clean all $(TEST_CHECK_PALLETS)

# Start a 4 node CORD network using local chain spec
.PHONY: spinup spindown

NODE_1_CMD := ./target/production/cord --base-path /tmp/cord-data/alice --validator --chain local --alice --port 30333 --rpc-port 9933 --prometheus-port 9615 --node-key abe47f4e1065d4aa6fb0c1dd69a9a6b63c4551da63aad5f688976f77bd21622f --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0"

NODE_2_CMD := ./target/production/cord --base-path /tmp/cord-data/bob --validator --chain local --bob --port 30334 --rpc-port 9934 --node-key 7609333b3e2e2e0c1b4064f074a7396b53d213e08d356d1be2d48fab3a6cd25a --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWSNT7EqipGHpsAYptQfPNrMXdJcgjMd25hnQWwyvHxYnz

NODE_3_CMD := ./target/production/cord --base-path /tmp/cord-data/charlie --validator --chain local --charlie --port 30335 --rpc-port 9935 --node-key e18d2c105ad8188830979b7bf4e7779361beb9010b6574e1b35a0a354ce02e96 --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWSNT7EqipGHpsAYptQfPNrMXdJcgjMd25hnQWwyvHxYnz

NODE_4_CMD := ./target/production/cord --base-path /tmp/cord-data/dave --chain local --dave --port 30336 --rpc-port 9936 --node-key f21d3114273b5d6184f9e595dba1850eb64b1e4965cfd2c6130354c67f632f5d --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWSNT7EqipGHpsAYptQfPNrMXdJcgjMd25hnQWwyvHxYnz


spinup: build
	@echo "Starting nodes in the background..."
	@$(NODE_1_CMD) &
	@$(NODE_2_CMD) &
	@$(NODE_3_CMD) &
	@$(NODE_4_CMD) &
	@echo "Four CORD nodes (Alice, Bob, Charlie, Dave) have been successfully started."
	@echo "See them in \033[0;34mhttps://telemetry.cord.network\033[0m under Cord Spin tab."
	@echo "You can also watch this network details in \033[0;34mhttps://apps.cord.network/?rpc=ws://localhost:9933\033[0m "

spindown:
	@echo "Stopping all nodes..."
	@pkill -f "cord --base-path /tmp/cord-data/"
	@echo "All nodes stopped."
	@echo "Deleting /tmp/cord-data/ directory..."
	@rm -rf /tmp/cord-data/
	@echo "/tmp/cord-data/ directory deleted."
	@echo ""
	@echo "Commercial Support Services on CORD are offered by Dhiway \033[0;34m(sales@dhiway.com)\033[0m "
	@echo "CORD team recommends having a separate chain in production, because \033[0;34mlocal\033[0m chain uses the default keys, which are common."

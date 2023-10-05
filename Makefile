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
check-network-membership:
	cargo check --package pallet-network-membership $(CARGO_FLAGS)

# Rule to check the DID NAME pallet with all targets
check-did-name:
	cargo check --package pallet-did-name $(CARGO_FLAGS)

# Rule to check the REGISTRY pallet with all targets
check-registry:
	cargo check --package pallet-registry $(CARGO_FLAGS)

# Rule to check the SCHEMA pallet with all targets
check-schema:
	cargo check --package pallet-schema $(CARGO_FLAGS)

# Rule to check the STREAM pallet with all targets
check-stream:
	cargo check --package pallet-stream $(CARGO_FLAGS)

# Rule to check the UNIQUE pallet with all targets
check-unique:
	cargo check --package pallet-unique $(CARGO_FLAGS)

# Rule to run tests with runtime benchmarks and without fail-fast
test:
	cargo test --all --no-fail-fast $(CARGO_FLAGS)

# Rule to test the DID pallet with all targets
test-did:
	cargo test --package pallet-did $(CARGO_FLAGS)

# Rule to test the Athorship pallet with all targets
test-network-membership:
	cargo test --package pallet-network-membership $(CARGO_FLAGS)

# Rule to test the DID NAME pallet with all targets
test-did-name:
	cargo test --package pallet-did-name $(CARGO_FLAGS)

# Rule to test the REGISTRY pallet with all targets
test-registry:
	cargo test --package pallet-registry $(CARGO_FLAGS)

# Rule to test the SCHEMA pallet with all targets
test-schema:
	cargo test --package pallet-schema $(CARGO_FLAGS)

# Rule to test the STREAM pallet with all targets
test-stream:
	cargo test --package pallet-stream $(CARGO_FLAGS)

# Rule to test the STREAM pallet with all targets
test-unique:
	cargo test --package pallet-unique $(CARGO_FLAGS)

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

ALICE_NODE_CMD := ./target/production/cord --base-path /tmp/cord-data/alice --validator --chain local --alice --port 30333 --rpc-port 9933 --prometheus-port 9615 --node-key 0000000000000000000000000000000000000000000000000000000000000001 --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0"

BOB_NODE_CMD := ./target/production/cord --base-path /tmp/cord-data/bob --validator --chain local --bob --port 30334 --rpc-port 9934 --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp

CHARLIE_NODE_CMD := ./target/production/cord --base-path /tmp/cord-data/charlie --validator --chain local --charlie --port 30335 --rpc-port 9935 --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp

DAVE_NODE_CMD := ./target/production/cord --base-path /tmp/cord-data/dave --chain local --dave --port 30336 --rpc-port 9936 --rpc-methods=Safe --rpc-cors all --prometheus-external --telemetry-url "wss://telemetry.cord.network/submit/ 0" --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp

# Logs
LOG_DIR := /tmp/cord-logs
ALICE_LOG := $(LOG_DIR)/alice.log
BOB_LOG := $(LOG_DIR)/bob.log
CHARLIE_LOG := $(LOG_DIR)/charlie.log
DAVE_LOG := $(LOG_DIR)/dave.log

spinup:
	@echo "Compiling CORD binary..."
	@cargo build --locked --profile production > /dev/null 2>&1
	@echo "Creating log directory in \033[0;34m$(LOG_DIR)\033[0m"
	@mkdir -p $(LOG_DIR)
	@touch $(ALICE_LOG)
	@touch $(BOB_LOG)
	@touch $(CHARLIE_LOG)
	@touch $(DAVE_LOG)
	@chmod 666 $(ALICE_LOG) $(BOB_LOG) $(CHARLIE_LOG) $(DAVE_LOG)
	@echo "Starting all nodes in the background..."
	@$(ALICE_NODE_CMD) > $(ALICE_LOG) 2>&1 &
	@$(BOB_NODE_CMD) > $(BOB_LOG) 2>&1 &
	@$(CHARLIE_NODE_CMD) > $(CHARLIE_LOG) 2>&1 &
	@$(DAVE_NODE_CMD) > $(DAVE_LOG) 2>&1 &
	@echo "Four CORD nodes (Alice, Bob, Charlie, Dave) have been successfully started."
	@echo "See them in \033[0;34mhttps://telemetry.cord.network\033[0m under Cord Spin tab."
	@echo "You can also watch this network details in \033[0;34mhttps://apps.cord.network/?rpc=ws://localhost:9933\033[0m "
	@echo ""
	@echo "To view the logs, you can use the following commands:"
	@echo "Alice: tail -f $(ALICE_LOG)"
	@echo "Bob: tail -f $(BOB_LOG)"
	@echo "Charlie: tail -f $(CHARLIE_LOG)"
	@echo "Dave: tail -f $(DAVE_LOG)"
	@echo ""
	@echo "To stop all running nodes run: \033[0;34mmake spindown\033[0m"


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

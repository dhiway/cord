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
	cargo build $(CARGO_FLAGS) --profile production

# Default rule, runs the production build
all: build

# Rule to build the project (debug profile)
build-debug:
	cargo build $(CARGO_FLAGS)

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

# Orbis Solidity fixture

`Counter.sol` is a minimal EVM bytecode compatibility fixture for `pallet-revive`.
The runtime test `solidity_evm_fixture_deploys_and_executes_through_revive` deploys the generated
init bytecode, mutates contract storage, and verifies the returned bytes directly.

Regenerate the committed bytecode with Solidity compiler 0.8.36:

```sh
./build.sh
```

The fixture intentionally uses standard `solc` EVM output. PolkaVM compilation with
`resolc` is a separate tooling path and is not required to exercise Orbis's configured
`AllowEVMBytecode` compatibility envelope.

# Orbis Solidity fixture

`Counter.sol` is a minimal EVM bytecode compatibility fixture for `pallet-revive`.
The runtime test deploys the generated init bytecode, mutates contract storage, and
reads the result through its Solidity ABI.

Regenerate the committed ABI and bytecode with Solidity compiler 0.8.36:

```sh
./build.sh
```

The fixture intentionally uses standard `solc` EVM output. PolkaVM compilation with
`resolc` is a separate tooling path and is not required to exercise Orbis's configured
`AllowEVMBytecode` compatibility envelope.

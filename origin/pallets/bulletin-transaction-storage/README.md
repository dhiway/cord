# pallet-bulletin-transaction-storage

> [!WARNING]
> This is a reference implementation provided for research, experimentation, and developer education. This code has not been fully audited. It is actively under development and may contain bugs, vulnerabilities, or incomplete features. It is not recommended for production use without independent review. Use at your own risk.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](../../LICENSE)
[![Status: experimental](https://img.shields.io/badge/status-experimental-yellow.svg)](#)

> Part of the [Polkadot Bulletin Chain](https://github.com/paritytech/polkadot-bulletin-chain).

Transaction storage pallet for the Polkadot Bulletin Chain. Indexes transactions and manages storage proofs.

## Overview

This pallet provides distributed data storage on-chain with proof-of-storage guarantees. It is designed for chains with no transaction fees and data is retrievable via the Bitswap protocol using content-addressed CIDs.

Key features:
- Store arbitrary data on-chain via the `store` extrinsic
- Automatic data removal after a configurable `RetentionPeriod` (default: 14 days at 6s block time)
- Data renewal to extend retention via `renew`
- Validators submit proofs of storing random data chunks when producing blocks
- CID generation for content-addressed data retrieval via Bitswap

## Usage

### Storing data

Use the `transactionStorage.store` extrinsic to store data. A CID is generated from the content hash for retrieval via Bitswap.

### Renewing data

To prevent data from being removed after the retention period, use `transactionStorage.renew(block, index)` where `block` is the block number of the previous store or renew transaction, and `index` is the index of that transaction in the block.

### Retrieving data

Stored data is retrievable via the Bitswap protocol using the CID generated at storage time.

## Dependencies

- [`bulletin-transaction-storage-primitives`](primitives/) — CID utilities and shared types
- `sp-transaction-storage-proof` — Storage proof verification from Polkadot SDK

## Security

See the [root README](../../README.md#security) for security notices and responsible deployment guidance.

For Parity's security disclosure process and Bug Bounty program, visit: https://parity.io/bug-bounty

## License

Apache-2.0

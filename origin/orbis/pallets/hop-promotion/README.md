# pallet-bulletin-hop-promotion

Orbis vendors this pallet from Polkadot Bulletin Chain revision `b6c2827d2326`. Package and API
names remain upstream-compatible; dependencies resolve through the CORD `release-v1.24.0` SDK
workspace graph.

> [!WARNING]
> This is a reference implementation provided for research, experimentation, and developer education. This code has not been fully audited. It is actively under development and may contain bugs, vulnerabilities, or incomplete features. It is not recommended for production use without independent review. Use at your own risk.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](../../LICENSE)
[![Status: experimental](https://img.shields.io/badge/status-experimental-yellow.svg)](#)

> Part of the [Polkadot Bulletin Chain](https://github.com/paritytech/polkadot-bulletin-chain).

Promotes near-expiry HOP pool data to permanent chain storage on the Polkadot Bulletin Chain.

## Overview

HOP submissions are short-lived by default. This pallet lets near-expiry data be promoted into `pallet-bulletin-transaction-storage` via general (unsigned, fee-less, priority-0) transactions that only land in otherwise-unused blockspace. The submitter's Bulletin allowance is not debited — the trade-off is that promotion is best-effort, not guaranteed.

The authorize closure verifies the user's submit-time signature and the freshness of the submit timestamp, and rejects promotion for accounts whose Bulletin authorization is missing or expired.

## Security

See the [root README](../../README.md#security) for security notices and responsible deployment guidance.

For Parity's security disclosure process and Bug Bounty program, visit: https://parity.io/bug-bounty

## License

Apache-2.0

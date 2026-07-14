# Signer Guide

The SDK re-exports `tx::signer`, which wraps Subxt’s signer traits with Origin-friendly helpers.

## Keypair Abstraction

```rust
use oc::tx::signer::{DevAccount, KeyAlgorithm, Keypair};

// Default: sr25519
let maintainer = Keypair::dev(DevAccount::Alice);

// Explicit algorithm (ed25519)
let delegate = Keypair::dev_with(DevAccount::Bob, KeyAlgorithm::Ed25519);

// From a secret URI or mnemonic
let custom = Keypair::from_secret_uri(KeyAlgorithm::Sr25519, "//Charlie//demo", None)?;
```

`Keypair` implements `subxt::tx::Signer<OriginConfig>` (and `Signer<PolkadotConfig>`), so you can pass it to any Subxt builder or SDK helper without manual conversions.

## Account & Signature Introspection

```rust
let account_id = signer.account_id(); // subxt::utils::AccountId32
let scheme = signer.algorithm();      // sr25519 or ed25519
let signature = signer.sign_message(b"hello"); // sp_runtime::MultiSignature
```

The SDK automatically detects which `MultiSignature` variant was produced, so higher-level APIs (view authorizations, tx submission, meta tx) never ask for a signature scheme flag.

## Dev Accounts

`DevAccount` enumerates the standard Substrate test keys (Alice, Bob, Charlie, Dave, Eve, Ferdie). Use `Keypair::dev(account)` for sr25519 or `Keypair::dev_with(account, KeyAlgorithm::Ed25519)` for ed25519.

## Error Handling

Key constructors return `SigningError`, which distinguishes between invalid secret URIs (`InvalidSecret`) and unsupported seed lengths (`UnsupportedSeedLength`). Bubble this error up to your CLI/UI to surface actionable messages.

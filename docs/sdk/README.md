# origin-rs Documentation

This directory captures the developer-facing surface of `origin-rs` (aka the Origin Rust Client / oc). Each guide focuses on the APIs you interact with when building Origin/CORD tooling.

| Document | What it covers |
|----------|----------------|
| [client.md](client.md) | Connecting to nodes, metadata guards, accessing transaction/query builders. |
| [signers.md](signers.md) | Multi-algorithm key helpers (`Keypair`, `KeyAlgorithm`, `DevAccount`). |
| [transactions.md](transactions.md) | Extrinsic composition (`TxSubmitter`, `TxExecutor`, `MetaSigner`, utility batches). |
| [operations.md](operations.md) | End-to-end SDK surface: connecting, authorizations, entity/register/packet extrinsics, and all view helpers with examples. |
| [views.md](views.md) | Runtime view helpers (`AuthorizationBuilder`, `entity/register/token` queries). |
| [demos.md](demos.md) | How the runnable demos are structured and where to extend them. |

Each file includes code snippets that compile against the current SDK. If you add new modules or change interfaces, update the relevant doc.

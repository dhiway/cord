# `@cord-network/origin-sdk-assets`

Typed, finalized access to Commons balances, fungible assets, canonical NFTs, conversion pools, asset-fee selection, and transaction-payment estimates.

- Native Foundation and Commons asset-location helpers hide XCM location boilerplate.
- Reads and transaction preparation pin to verified finalized Commons state.
- `ChargeAssetTxPayment` options are explicit and default to native fees.
- `Nfts` is the sole application NFT surface; duplicate `Uniques` composition is excluded pending the P8 cleanup gate.
- Forced governance operations are intentionally excluded.
- Conversion quotes are not advertised: Commons does not yet implement `AssetConversionApi`.

# Origin and Orbis native capability matrix

This matrix is the completeness ledger for the enterprise Origin/Orbis network. It distinguishes
native capability ownership from source provenance: the reference chains are not runtime
dependencies, but selected pallets, configuration, APIs, tests, and node services must be maintained
inside this repository on CORD's single `release-v1.24.0` SDK graph.

## Reference snapshots

| Reference | Revision | Primary use |
|---|---:|---|
| Dhiway SDK / Polkadot SDK fork | `release-v1.24.0#cc190ea8` | FRAME, Cumulus, relay host, Assets, Revive, Broker and node interfaces |
| Individuality Community | `28b7d07dab05` | `next-asset-hub-paseo` and `next-people-paseo` behavior |
| Polkadot Bulletin Chain | `b6c2827d2326` | durable transaction storage, proof handling and hop promotion |
| Paseo runtimes | `ac99ed6c1122` | relay/system-chain configuration and XCM |
| Fellows runtimes | `477689fddba4` | system-chain production configuration |
| Web3 Storage | `a32f83aae7a2` | provider, drive and S3 storage service behavior |

## Ownership rules

- The physical ownership boundary is `pallets/` for CORD, `origin/pallets/` for the Origin relay,
  and `origin/orbis/pallets/` for Orbis. New Orbis parity work must not be added to a CORD pallet.
- Upstream pallets that are used without modification remain SDK dependencies. A fork or adapter is
  created below `origin/orbis/pallets/` only when Orbis-specific behavior is required.
- Origin owns relay consensus, permissioned authority sessions, parachain registration, availability,
  disputes, scheduling, coretime assignment, XCM routing and Sudo administration.
- Orbis owns all application-facing system-chain capabilities and the sole Coretime Broker.
- Orbis must not require Asset Hub, People, Bulletin, Coretime, or Storage to be deployed as separate
  chains for native operations.
- Staking, nomination pools, elections, councils, referenda, conviction voting, treasuries and public
  governance are excluded. Their administrative origins are replaced by Sudo/root where a retained
  capability needs administration.
- A pallet is only **complete** after runtime wiring, transaction extensions, runtime APIs, XCM/node
  support, generated Orbis weights, and end-to-end tests are present.

## Origin relay

| Capability | Native implementation | State |
|---|---|---|
| Permissioned validators | `AuthorityManager`, `Session`, BABE, GRANDPA, BEEFY; Sudo-managed, no bond | Present |
| Parachain host | configuration, initializer, inclusion, paras, inherent, scheduler, disputes, slashing, HRMP/DMP/UMP | Present |
| Elastic scheduling | Assignments V2, ElasticScalingMVP, candidate receipts V2/V3, async backing | Present and native-smoke tested |
| Core allocation | relay `Coretime`, on-demand assignment and claim queue | Present |
| Orbis broker trust | `BrokerId = 1006`, Orbis-origin XCM authorization | Present; full Broker-driven native E2E pending |
| Relay administration | `Sudo`, registrar/root origins | Present |
| Staking and public governance | none in the enterprise authority/control path | Excluded by policy |

## Orbis system-chain foundation

| Capability | Reference | Native pallet/configuration | State |
|---|---|---|---|
| Parachain execution | all system chains | ParachainSystem, Aura/AuraExt, Session, two collators | Present |
| Bundled-block accounting | Asset Hub/Bulletin/SDK | `WeightReclaim` plus outer `StorageWeightReclaim` transaction extension | Present |
| Elastic authoring | Bulletin/SDK | target rate 3, relay-parent offset 1, capacity 12, slot-based node | Present and native-smoke tested |
| Messaging | Asset Hub/People | XCMP, DMP, XCM, MessageQueue and safe-call filtering | Present; full native E2E pending |
| Safety and operations | system chains | Scheduler, Utility, Multisig, Proxy, TxPause, SafeMode, migrations | Present |
| Administration | enterprise policy | Sudo only | Present |

## Assets and Revive

| Capability/pallet | Asset Hub reference | Orbis state |
|---|---|---|
| Native balances and transaction payment | Balances, TransactionPayment | Present |
| Fungible local assets | `pallet_assets` | Present at index 80 |
| ERC-20 asset precompile | asset precompiles | Present and exercised from Solidity |
| Solidity/PolkaVM contracts | `pallet_revive` | Present at index 100; deploy/call fixture tested |
| Foreign assets | second `pallet_assets` instance | Present at index 83 with Location IDs, root creation and freezer |
| Pool assets | third `pallet_assets` instance | Present at index 84 with root creation and freezer |
| Asset holder and freezer | `pallet_assets_holder`, `pallet_assets_freezer` | Present at indices 82/81 and exercised against local assets |
| Asset conversion and pools | `pallet_asset_conversion` | Present at index 200 for native, local and foreign assets |
| Asset-denominated fees | asset conversion/asset transaction payment | Present at index 201 and compatible with feeless/meta-tx envelope |
| Asset rates | `pallet_asset_rate` | Present at index 89 with Location-based, Sudo-managed rates |
| NFTs and uniques | `pallet_nfts`, `pallet_uniques` | Present at indices 88/87 with native collection and mint lifecycle tests |
| PGAS/allowance integration | Individuality Asset Hub PGAS pallets | Gap; must remain compatible with meta-tx/feeless policy |
| Alias accounts, DOTNS gateway and origin restriction | Individuality Asset Hub adapters | Gap |
| Vesting and claims | Asset Hub | Policy gap: include without staking only if enterprise issuance requires them |
| Snowbridge/bridge frontend | Asset Hub | Deployment-policy gap; not required for Origin-native operation |

## People and identity

| Capability/pallet | People reference | Orbis state |
|---|---|---|
| Identity records, registrars and judgements | People/FRAME Identity | Orbis-owned compatibility fork at index 90; root-managed registrar lifecycle present |
| Aliases, usernames and subaccounts | People/People Lite | Present in the Orbis-owned People pallet |
| Sudo attestation and forced administration | enterprise adaptation | Present |
| Full ring-backed personhood | `indiv_pallet_people` | Native at index 95 with Sudo recognition, flexible membership and authenticated person origins |
| People Lite compatibility API and storage semantics | `indiv_pallet_people_lite` | Native at index 94 with Members-backed aliases, Sudo allowances and transaction authentication |
| Storage initialization | `indiv_pallet_storage_initialization` | Gap |
| Resources and chunk management | `indiv_pallet_resources`, `indiv_pallet_chunks_manager` | Chunk manager present at index 91 with Sudo-managed ring parameter hashes; Resources gap |
| Members and notifications | members/subscriber/notifier pallets | Ring Members 92 and Sudo-managed XCM Notifier 93 present; local subscriber excluded because native consumers bind Members directly (ADR 0007) |
| Coinage and airdrop | Individuality People | Gap; enterprise issuance policy required |
| Honour, proof-of-ink, score and game | Individuality People application pallets | Gap |
| Mob rule | Individuality People | Excluded where it constitutes governance; non-governance behavior requires explicit adaptation |

## Bulletin and storage

| Capability/pallet | Reference | Orbis state |
|---|---|---|
| Authorized durable storage | Bulletin TransactionStorage | Vendored and present at index 110 |
| Content hash/CID lookup | Bulletin | Present and tested |
| Retention, renewal and permanent accounting | Bulletin | Present and unit-tested |
| Storage transaction validation and anti-wrapper policy | Bulletin | Present in the Orbis transaction envelope |
| Runtime authorization/query API | Bulletin | Present |
| Proof inherent | Bulletin node/runtime | Runtime present; production node provider and retention-window E2E pending |
| Hop promotion | `pallet_bulletin_hop_promotion` | Vendored under Orbis, present at index 111 with `sp_hop` runtime API |
| Storage providers | Web3 Storage `pallet_storage_provider` | Gap; existing reference requires stake and must be adapted to Sudo authorization with no stake |
| Drive registry | Web3 Storage `pallet_drive_registry` | Gap |
| S3 registry | Web3 Storage `pallet_s3_registry` | Gap |
| Provider/drive/S3 runtime APIs and node services | Web3 Storage | Gap |

## Coretime, transactions, and policy

| Capability | Native implementation | State |
|---|---|---|
| Coretime Broker | `pallet_broker` at protocol index 50 | Present |
| Three-core reservations | complete `Task(para_id)` masks | Unit-tested; Broker-to-Origin E2E pending |
| Sponsored transactions | MetaTx with user signature/nonce and sponsor payment | Present and abuse-tested |
| Controlled zero-fee calls | Feeless allowlist, per-account quota, deny-by-default wrappers | Present and abuse-tested |
| Solidity actor preservation | Revive `SetOrigin` plus transaction envelope | Present |
| Bulletin call validation | recursive storage-call inspector | Present |
| Runtime upgrade safety | migrations, SafeMode and TxPause | Present; production try-runtime rehearsal pending |

## Completion order

1. Replace remaining Orbis dependencies on mutable CORD application pallets with Orbis-owned
   upstream-aligned pallets under `origin/orbis/pallets/`; shared primitives may remain shared.
   People identity is complete; Entity, Register, Token, Feeless, MetaTx and signature adapters remain.
2. Complete remaining Asset Hub application adapters (PGAS/allowance, aliases and origin policy);
   conversion, asset fees and rates are complete.
3. Decide and implement literal Individuality application-pallet parity for People-specific Game,
   Score, Honour, Resources, Coinage and related pallets without importing governance.
4. Adapt Web3 Storage providers to Sudo-authorized, zero-stake enterprise providers; then add Drive
   and S3 registries and their node/runtime APIs.
5. Generate Orbis-native weights for every retained pallet.
6. Run Origin-Orbis XCM, Broker lifecycle, storage-proof retention and unified application E2E suites.

This ledger must be updated in the same commit that adds, excludes, or replaces a referenced
capability.

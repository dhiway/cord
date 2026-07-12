# Origin and Orbis native capability matrix

This matrix is the completeness ledger for the enterprise Origin/Orbis network. It distinguishes
native capability ownership from source provenance: the reference chains are not runtime
dependencies, but selected pallets, configuration, APIs, tests, and node services must be maintained
inside this repository on CORD's single `release-v1.24.0` SDK graph.

## Reference snapshots

| Reference | Revision | License | Primary use |
|---|---:|---|---|
| Dhiway SDK / Polkadot SDK fork | `release-v1.24.0#cc190ea8` | Apache-2.0 | FRAME, Cumulus, relay host, Assets, Revive, Broker and node interfaces |
| Individuality Community | `28b7d07dab05` | Apache-2.0 | `next-asset-hub-paseo` and `next-people-paseo` behavior |
| Polkadot Bulletin Chain | `b6c2827d2326` | Apache-2.0 | durable transaction storage, proof handling and hop promotion |
| Paseo runtimes | `ac99ed6c1122` | GPL-3.0 | relay/system-chain configuration and XCM |
| Fellows runtimes | `477689fddba4` | GPL-3.0 | system-chain production configuration |
| Web3 Storage | `a32f83aae7a2` | Apache-2.0 except undeclared `file-system-primitives` | provider, drive and S3 storage service behavior; the undeclared crate requires legal clearance before copying |

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
| Origin application compatibility | CORD Origin | Orbis-owned Token 51, Register 52, Entity 53 and Feeless 54 packages | Present; call/storage metadata compatibility snapshotted |

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
| Resources and chunk management | `indiv_pallet_resources`, `indiv_pallet_chunks_manager` | Native Orbis Resources V1 present at index 96 with person/lite proof quotas, atomic isolated Bulletin reservations, account-bound paid MetaTx v6 claims and Root management; Chunk Manager remains at 91 |
| Members and notifications | members/subscriber/notifier pallets | Ring Members 92 and Sudo-managed XCM Notifier 93 present; local subscriber excluded because native consumers bind Members directly (ADR 0007) |
| Coinage and airdrop | Individuality People | Gap; enterprise issuance policy required |
| Honour, proof-of-ink, score and game | Individuality People application pallets | Gap |
| Mob rule | Individuality People | Excluded where it constitutes governance; non-governance behavior requires explicit adaptation |

## Bulletin and storage

| Capability/pallet | Reference | Orbis state |
|---|---|---|
| Authorized durable storage | Bulletin TransactionStorage | Vendored and present at index 110 |
| Person resource reservation and provenance | Orbis Resources/Bulletin V6 | Isolated capacity, exact `(block, transaction_index)` links, explicit storage actors, manual reserved renewal, deterministic expiry and tombstone audit are native; the independently bounded two-map/counter repair is registered and native at V7, while provider references remain the later Slice 10/V8 seam |
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

The exact retained Web3 crates, provider modules, HTTP routes, workers, bounded runtime API
signatures and pagination limits are frozen in manifest version 1. Anything else at that snapshot
is excluded unless replanned. `file-system-primitives` has no declared license at the pinned
revision and cannot be copied verbatim until provenance is resolved.

## Coretime, transactions, and policy

| Capability | Native implementation | State |
|---|---|---|
| Coretime Broker | `pallet_broker` at protocol index 50 | Present |
| Three-core reservations | complete `Task(para_id)` masks | Unit-tested; Broker-to-Origin E2E pending |
| Sponsored transactions | MetaTx with user signature/nonce and sponsor payment | Spec 28/tx 7 composes Verify→Consume, all seven account-bound router variants, bounded ingress, one-shot paid token/finalization and signed direct Resources payer adapters; manifest v4 freezes reproducible literals, metadata-hash modes and exact error classes |
| Controlled zero-fee calls | Feeless allowlist, per-account quota, deny-by-default wrappers | Present and abuse-tested |
| Solidity actor preservation | Revive `SetOrigin` plus transaction envelope | Present |
| Bulletin call validation | recursive storage-call inspector | Present |
| Runtime upgrade safety | migrations, SafeMode and TxPause | Present; production try-runtime rehearsal pending |

### Frozen remediation sequence

1. Historical spec-26/transaction-6 baseline and dormant support are retained only as compatibility evidence.
2. The reverse-index Bulletin V6→V7 repair was integrated and is present; storage version 7 is current.
3. The current runtime is spec 28 / transaction 7 with stable pallet indices and active Verify→Consume support.
4. Manifest v4 freezes executable, hashed evidence before any later native capability slice begins.
5. Provider composition is not implemented here: the provider-reference V7→V8 migration remains planned for Slice 10.

Completed Bulletin V7 reconstructs both `ResourceLinkByRef` and `ResourceLinkByContentHash` independently from
authoritative links and writes row/link counters before setting V7 last. REF and HASH missing,
partial, stale and duplicate rehearsals cannot substitute for each other. Its frozen budget is
`reads = A + T + L + I_ref + I_hash + 3L + 3` and
`writes = I_ref + I_hash + 2L + 2 + 1`. A later V7-to-V8 provider migration appends optional
provider allocation references as `None` while preserving all existing reservation and link fields.

## Completion order

1. Maintain the Orbis-owned Token, Register, Entity, Feeless and People packages under
   `origin/orbis/pallets/`; shared primitives may remain shared. MetaTx and signature verification
   remain pinned SDK dependencies because Orbis does not modify those pallets.
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

## Evidence v4

Manifest v4 separates the completed Bulletin V6→V7 reverse-index repair from the planned provider V7→V8 seam and freezes compiled-enabled, custom-loss, no-hash CannotLookup, and early-propagation metadata evidence.

Migration IDs are normative: present `PMIG-Bulletin-V6-to-V7` owns reverse-index/counter repair;
planned `PMIG-Bulletin-V7-to-V8` owns provider-reference composition in Slice 10.

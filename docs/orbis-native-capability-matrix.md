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
| Orbis broker trust | `BrokerId = 1006`, Orbis-origin XCM authorization, `CoretimeControl` replay/receipt envelope at index 221 | Present; full Broker-driven native E2E pending |
| Relay administration | `Sudo`, registrar/root origins | Present |
| Staking and public governance | none in the enterprise authority/control path | Excluded by policy |

## Orbis system-chain foundation

| Capability | Reference | Native pallet/configuration | State |
|---|---|---|---|
| Parachain execution | all system chains | ParachainSystem, Aura/AuraExt, Session, two collators | Present |
| Bundled-block accounting | Asset Hub/Bulletin/SDK | `WeightReclaim` plus outer `StorageWeightReclaim` transaction extension | Present |
| Elastic authoring | Bulletin/SDK | target rate 3, relay-parent offset 1, capacity 12, slot-based node | Present and native-smoke tested |
| Messaging | Asset Hub/People | XCMP, DMP, XCM, MessageQueue and safe-call filtering | Present; full native E2E pending |
| Safety and operations | system chains | Scheduler, Utility, Multisig, Proxy, TxPause, SafeMode and the generic future-upgrade migration framework | Present; no predecessor-state migration is wired at the new genesis |
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
| Score | Individuality People application pallet | Orbis-owned Apache-2.0 fork present at index 97; native Personhood/People integration, account-bound participant extension, Sudo-or-named-manager operations, conservative weights, benchmarks and upstream-derived tests |
| Honour | Individuality People application pallet | Orbis-owned Apache-2.0 fork present at index 99; native Members ring proofs, Timestamp mortality/freeze policy, voter-auth extension, conservative weights, benchmarks and upstream-derived tests |
| Native attestation/schema registry | Attestation Protocol semantics | Present at index 105 with delegated/batched/expiring/revocable issuance and finalized runtime APIs |
| Proof-of-ink and game | Individuality People application pallets | Gap; not admitted to the enterprise-first launch scope |
| Mob rule | Individuality People | Excluded where it constitutes governance; non-governance behavior requires explicit adaptation |

## Native naming

| Capability | Reference semantics | Orbis state |
|---|---|---|
| DotNS ownership and lifecycle | DotNS contracts/SDK | Native bounded pallet at index 116 with commit/reveal registration, renewal, transfer, controllers, reservations and root administration |
| Address/subject/attestation/content/text resolution | DotNS resolvers | Native records reference canonical identities, attestations and TransactionStorage commitments; no contract registry or ABI facade |
| Label policy | DotNS normalization semantics | Deterministic ASCII policy v1 exposed through the runtime API; non-ASCII and reserved forms fail closed |
| Rust/TypeScript access | DotNS SDK patterns | CORD-owned typed clients and exact finalized-hash runtime API surface; no changes to the reference DotNS repositories |

## Bulletin and storage

| Capability/pallet | Reference | Orbis state |
|---|---|---|
| Authorized durable storage | Bulletin TransactionStorage | Vendored and present at index 110 |
| Person resource reservation and provenance | Orbis Resources/Bulletin semantics | Clean-genesis TransactionStorage V8: isolated capacity, exact `(block, transaction_index)` links, explicit current-network actors, manual reserved renewal, deterministic expiry/tombstone audit and optional native provider agreement references |
| Content hash/CID lookup | Bulletin | Present and tested |
| Retention, renewal and permanent accounting | Bulletin | Present and unit-tested |
| Storage transaction validation and anti-wrapper policy | Bulletin | Present in the Orbis transaction envelope |
| Runtime authorization/query API | Bulletin | Present |
| Proof inherent | Bulletin node/runtime | TransactionStorageApi v2 and the SDK-v1.24 omni-node Aura provider (which accepts v1+) are composed; production retention-window E2E is deferred to P7 after feature completeness |
| Hop promotion | `pallet_bulletin_hop_promotion` | Vendored under Orbis, present at index 111 with `sp_hop` runtime API |
| Storage providers | Web3 Storage semantics | Native CORD pallet present at index 120; Sudo-authorized, zero-stake provider lifecycle, agreements, challenges and checkpoints |
| Drive registry | Web3 Storage semantics | Native bounded CORD pallet present at index 121 |
| S3 registry | Web3 Storage semantics | Native bounded CORD pallet present at index 122 |
| Provider/drive/S3 runtime APIs and node services | CORD-owned implementation | Versioned finalized-state runtime APIs and the `origin-orbis-provider` companion/outbox consumer are present; deployment hardening and live-network evidence remain P7 work |

The Web3 reference inventory originated in manifest version 1 and is now classified reference-only
and excluded in current candidate manifest version 17. CORD-owned Provider, Drive, S3, runtime API,
HTTP and worker surfaces are the implementation authority. Anything outside that boundary requires
replanning. `file-system-primitives` has no declared license at the pinned revision and cannot be
copied verbatim.

## Coretime, transactions, and policy

| Capability | Native implementation | State |
|---|---|---|
| Coretime Broker | `pallet_broker` at protocol index 50 plus stack-owned `CoretimeControl` at index 221 | Present; monotonic request IDs, replay rejection, delayed retry and Origin receipts do not fork the SDK Broker/Coretime primitives |
| Three-core reservations | complete `Task(para_id)` masks | Unit-tested; Broker-to-Origin live E2E pending |
| Sponsored transactions | MetaTx with user signature/nonce and sponsor payment | Spec 29/tx 8 composes Score participant and Honour voter authentication with Verify→Consume, the account-bound router, bounded ingress, one-shot paid token/finalization and signed direct Resources payer adapters; manifest v4 remains the immutable spec-28/tx-7 historical evidence boundary and the v8 fixtures are the active compatibility envelope |
| Controlled zero-fee calls | Feeless allowlist, per-account quota, deny-by-default wrappers | Present and abuse-tested |
| Solidity actor preservation | Revive `SetOrigin` plus transaction envelope | Present |
| Bulletin call validation | recursive storage-call inspector | Present |
| Runtime upgrade safety | generic future migrations, SafeMode and TxPause | New genesis starts directly at current migrated-domain pallet storage versions with no predecessor/data import; Orbis uses `Migrations = ()`, while Origin retains the pinned SDK permanent XCM maintenance migration. Only future post-genesis schema changes may add CORD-owned forward migrations |

### Clean-genesis feature sequence

1. TransactionStorage starts directly at storage version 8; no V0-V7 migration, backfill, tolerant legacy decode or `LegacyUnknown` provenance is part of the network.
2. `ReservationProviderRef` and `attach_provider` are native current-schema capabilities validated against active Provider agreements.
3. Provider 120, Drive 121 and S3 122 plus their versioned runtime APIs are composed from genesis.
4. Historical manifest-v4/V5 migration evidence remains non-buildable provenance only and is not a current launch obligation.
5. Broad proof-retention, recovery and performance campaigns run only after the provider node and both CORD-owned SDKs are feature complete.

## Completion order

1. Maintain the Orbis-owned Token, Register, Entity, Feeless and People packages under
   `origin/orbis/pallets/`; shared primitives may remain shared. MetaTx and signature verification
   remain pinned SDK dependencies because Orbis does not modify those pallets.
2. Keep PGAS/allowance aliases and public Asset Hub adapters excluded unless a later ADR admits
   them; conversion, asset fees and rates are the retained asset scope.
3. Keep Game, Proof of Ink, Coinage and other non-admitted Individuality applications excluded;
   Score and Honour are the native enterprise launch scope.
4. Maintain the delivered CORD-owned provider companion process and exact-finalized Provider,
   Drive and S3 Rust/TypeScript SDK surfaces.
5. Replace conservative nonzero weights with final benchmark-generated weights after the accepted
   P6 source diff.
6. Run Origin-Orbis XCM, Broker lifecycle, storage-proof retention and unified application E2E
   suites in P6/P7, after the native stack is feature complete.

This ledger must be updated in the same commit that adds, excludes, or replaces a referenced
capability.

## Evidence v4

Manifest v4 is immutable historical evidence for predecessor development iterations. Its migration rows and IDs are not normative for the new Origin/Orbis genesis and are not imported into the current runtime or launch manifest.

## Iteration-5 evidence gate

`GATE-5-EVIDENCE` is Present after independent Architect and Critic CLEAR verdicts and executable
source-marker evidence. This is a narrow runtime/evidence freeze, not whole-program completion. The
reviewed pre-closure state had 146 planned rows; only the Gate 5 row changed, leaving 145 planned
rows and every planned capability unchanged.

## P0 runtime-alignment reconciliation (`sm-update-sub-0x63`)

The machine-readable inventory for the current branch is
`docs/architecture/origin-foundation-commons-runtime-alignment.csv`; its capture report and fail-closed validator
are under `docs/evidence/p0/`. The ledger inventories the exact Origin/Orbis pallet indices, runtime
APIs, Orbis transaction-extension order, node/CLI/RPC and Rust/TypeScript SDK disposition without
changing this matrix's capability decisions.

The reconciliation records the independently reviewed historical Slice-2 evidence closure. Current
candidate manifest v17 tracks clean-genesis native feature implementation and remains unratified
until its independent transition/product/evidence reviews are recorded. Historical v5 closure
proves only its original bounded claims; it does not ratify the new provider/runtime/SDK surface or
claim production readiness. There is no legacy data
migration, Solidity-ABI compatibility or old-network cutover requirement: migrated domains start
native at new genesis, and superseded contract-era code is deleted under the native-cutover cleanup
gate.

# ADR 0008: Orbis transaction policy pipeline

- Status: accepted
- Date: 2026-07-12
- Supersedes: the target-order paragraph in ADR 0005

## Context

Orbis has four transaction construction surfaces: a normal signed extrinsic, an Ethereum
transaction translated by Revive, a signed inner MetaTx intent, and an authorized/offchain
transaction. Adding identity, feeless, storage, or application policy independently to those
surfaces can create a signature, origin, nonce, or payment bypass. The order is therefore protocol
surface, not an implementation detail.

The runtime currently implements `AsPerson`, `PeopleLiteAuth`, `AuthorizeCall`, asset/native
payment with a feeless gate, recursive Bulletin call validation, metadata-hash validation, and
Revive origin selection. Future policy names below reserve positions; they are not claims that the
corresponding feature exists.

## Decision

The frozen logical order is:

1. outer `StorageWeightReclaim`;
2. `AuthorizeValueTransfer`, `VerifySignature`, `AsPerson`,
   `AsProofOfInkParticipant`, `ScoreAsParticipant`, `GameAsInvited`, `PeopleLiteAuth`, `AsMember`,
   `AsCoinage`, `AsResources`, `VoterAuth`, `AuthorizeCall`, `AsPgas`, `AsRingAlias`, and
   `AsDotnsGateway`, in that order;
3. `RestrictOrigin`, after all dispatch-origin mutation;
4. nonzero-sender, spec-version, transaction-version, genesis, mortality, nonce, and weight checks;
5. deny-by-default `ChargeOrSkipFeeless<ChargePGAS<ChargeAssetTxPayment>>`;
6. recursive Bulletin storage-call validation, metadata-hash validation, and terminal Revive
   `SetOrigin`.

The type fixture in `origin/orbis/runtime/src/tests.rs` gives every unimplemented policy position a
typed `NoPolicy<N>` marker. It is intentionally **not** a `TransactionExtension`, so it cannot
authorize a call, mutate an origin, or accidentally enter the runtime tuple. The full fixture also
models reclaim, restriction, checks, the nested feeless/PGAS/asset payment policy, Bulletin,
metadata, and Revive origin selection. Tests destructure the concrete normal, Ethereum, MetaTx and
authorized types and assert their extension metadata order; the fixture is not a disconnected set
of surface aliases. Implemented full-transaction surfaces share `OriginPolicyExtensions`,
`InnerTxExtensions`, and the single `default_inner_tx_extensions` builder.

The currently implemented payment type is codec-transparent
`ExplicitPayment<ChargeOrSkipFeeless<ChargeAssetTxPayment>>`; the missing `ChargePGAS` position is a
typed fixture placeholder. `ExplicitPayment` contains no encoded skip request. It derives the
exemption only from the exact
`System::Authorized` origin produced by the preceding `AuthorizeCall`. Thus an authorized extrinsic
keeps its skip semantics after a wire round trip, while an arbitrary signed SCALE transaction
cannot request the skip and continues through ordinary payment validation.

Replacing a `NoPolicy` slot requires the same commit to:

- wire the extension at its reserved position on every applicable surface;
- increment `transaction_version`;
- update signing vectors and type assertions;
- add direct and MetaTx positive and negative tests;
- add Ethereum positive and negative tests when Revive can reach the capability, or prove that the
  default Ethereum construction cannot claim the custom origin; and
- update authorized construction tests when the pallet emits authorized/offchain transactions.

No `NoPolicy` marker may remain in the completion fixture at production readiness.

## Surface semantics

### Normal signed extrinsics

The signed account owns the system nonce and is the default logical actor. It pays the selected
native/asset fee unless a recursive call classification and prepared quota explicitly make the
call feeless. Identity extensions may replace the logical dispatch origin only with their own
validated proof or account-bound nonce.

### Ethereum/Revive extrinsics

The mapped Ethereum account owns the nonce and pays. Current policy constructors pass `None` to
`AsPerson` and `PeopleLiteAuth`, so an Ethereum envelope cannot silently claim either custom
origin. Terminal `SetOrigin::new_from_eth_transaction` establishes the Revive execution actor; it
does not widen the earlier FRAME dispatch origin or bypass Bulletin and payment validation.

### MetaTx inner intents

The inner signer owns the inner nonce and logical origin. The outer relayer owns the outer nonce and
is the sole ordinary native/asset fee payer. The inner payload has no second fee withdrawal. PGAS
may be selected only by an explicit authorization carried by the signed inner intent. Dispatch
failure uses the payment extension's normal correction/refund behavior, while nonces and any quota
prepared by validation follow FRAME transaction-validity semantics.

The current MetaTx type contains `VerifySignature`, `MetaTxMarker`, the shared `AsPerson`,
`PeopleLiteAuth`, and `AuthorizeCall` policy tuple, version/genesis/mortality/nonce, recursive
Bulletin validation, and metadata validation. Transaction version 4 binds this expanded signed
intent. It intentionally has no second payment withdrawal, `CheckWeight`, storage-weight reclaim,
or Revive `SetOrigin`: those belong to the outer sponsored extrinsic. `RestrictOrigin`,
protected-transfer, PGAS, and the other reserved policy slots remain explicit future gaps.

### Authorized/offchain extrinsics

`AuthorizeCall` is the sole authority source. Construction uses the documented zero nonce and
immortal era, a zero tip, and default Revive origin. The constructor still retains nonzero/version/
genesis/weight checks, recursive Bulletin validation, metadata validation, and the base call filter.
The payment wrapper derives its explicit skip after authorization. No native/asset withdrawal or
feeless quota preparation occurs. The same result is derived after decoding from the exact
authorized origin, and SCALE input cannot forge that origin. When PGAS is added, the outer payment
skip must continue to prevent both PGAS and asset/native charging.

## Indirection and failure rules

Unknown or opaque Utility, Proxy, Multisig, Scheduler, XCM, or Revive indirection is ineligible for
PGAS, feeless, or protected-transfer exemption. Such calls are not globally prohibited: ordinary
paid dispatch remains available when the base call filter permits it. Bulletin classification is
recursive for the supported transparent Utility wrappers. Payment correction refunds only the payer
selected during pre-dispatch; it never changes the actor or nonce owner.

## Verification and known gaps

The compile/runtime fixture checks the actual tuple projections and metadata ordering of normal
construction, `EthExtraImpl`, MetaTx configuration, and `CreateAuthorizedTransaction`. Behavior
tests cover normal nonce-owner payment, failed-dispatch correction, prepared feeless quota
consumption, Ethereum mapped nonce/payment and terminal Revive actor selection, authorized-call
validation with explicit payment/quota skip and Bulletin storage, recursive Bulletin inspection,
and MetaTx actor preservation, replay rejection, signature forgery rejection, and inner nonce
consumption, plus outer-relayer nonce, payment and correction. The protected-transfer and PGAS
placeholders are zero-sized passthrough-only fixture types and cannot enter a runtime extension
tuple.

P0 deliberately does not pretend that future extensions exist. Remaining acceptance work is bound
to the slices that implement it: positive custom-identity proof vectors on every reachable surface;
protected-transfer signatures; `RestrictOrigin`; and the PGAS charging layer. Until those slices
land, ADR 0008 freezes the intended contract and makes the present typed placeholders explicit.

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

The runtime currently implements `AsPerson`, `ScoreAsParticipant`, `PeopleLiteAuth`, `AsResources`,
`VoterAuth`, `AuthorizeCall`, asset/native
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

Changing an encoded policy slot requires the same commit to:

- wire the extension at its reserved position on every applicable surface;
- increment `transaction_version` when its SCALE schema changes (a composition-only behavioral
  transition increments `spec_version` instead);
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
`AsPerson`, `PeopleLiteAuth` and `AsResources`, so an Ethereum envelope cannot silently claim any custom
origin. Terminal `SetOrigin::new_from_eth_transaction` establishes the Revive execution actor; it
does not widen the earlier FRAME dispatch origin or bypass Bulletin and payment validation.

### MetaTx inner intents

The inner signer owns the inner nonce and logical origin. The outer relayer owns the outer nonce and
is the sole ordinary native/asset fee payer. The inner payload has no second fee withdrawal. PGAS
may be selected only by an explicit authorization carried by the signed inner intent. Dispatch
failure uses the payment extension's normal correction/refund behavior, while nonces and any quota
prepared by validation follow FRAME transaction-validity semantics.

The MetaTx v6 type contains `VerifySignature`, mandatory paid-ingress consumption, `MetaTxMarker`, signed system checks through nonce, the Orbis account-bound policy router, recursive Bulletin validation, and metadata validation. Resources Meta proofs bind the verified signer to the claim account before origin transformation. Transaction version 6 binds the account-bound Resources Meta policy and paid-ingress signed
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

The Resources slot is now concrete on normal and MetaTx surfaces, defaults to `None` for Ethereum
and authorized construction, and precedes `AuthorizeCall`. P0 deliberately does not pretend that
other future extensions exist. Remaining acceptance work is bound
to the slices that implement it: positive custom-identity proof vectors on every reachable surface;
protected-transfer signatures; `RestrictOrigin`; and the PGAS charging layer. Until those slices
land, ADR 0008 freezes the intended contract and makes the present typed placeholders explicit.

### Spec-29 Score and Honour transition

Spec 29 / transaction version 8 makes the reserved Score and Honour positions concrete.
`ScoreAsParticipant` precedes the existing account-bound policy router and rejects both unknown
accounts and suspended identities before nonce mutation. `VoterAuth` follows Resources and accepts
only a fresh proof against the active Orbis Members ring. Both extensions encode `None` in default,
Ethereum, and authorized constructors; this preserves deny-by-default semantics rather than
implicitly manufacturing a custom origin. Meta v8 fixtures bind the new ordered schema and the
current metadata hash. Manifest v4 and its spec-28/transaction-7 fixtures remain historical
evidence and are not rewritten by this transition.

The active MetaTx extension sequence is `VerifySignature`, `ConsumePaidMetaIngress`,
`MetaTxMarker`, the system checks through `CheckNonce`, `ScoreAsParticipant`,
`MetaAccountBoundPoliciesV6`, `VoterAuth`, Bulletin validation, and metadata validation. Thus a
sponsor can carry either explicit identity-bound application authorization inside the signed
inner payload; `None` remains the deny-by-default encoding for both new slots.

The four surfaces have an exact application contract. Normal Score calls require the signed
account to be an active, non-suspended participant; normal Honour calls require a proof explicitly
bound to that same signed account. MetaTx applies the same rules to the inner signer after
`VerifySignature`; the outer sponsor remains the sole payer and nonce owner for the outer
transaction, while the inner signer owns the inner nonce. Ethereum/Revive and authorized/offchain
constructors deliberately encode both application slots as `None`, so they cannot manufacture a
participant or voter origin; their ordinary mapped/authorized payer semantics are unchanged.
Origin transformation never changes the account selected by the account-aware nonce or payment
extensions, and failed validation occurs before nonce, fee, Score, Honour, or payout mutation.


## Clean-genesis boundary

The Origin/Orbis network defined by this repository is a new network. It starts directly with the
current transaction-policy tuple and TransactionStorage schema V8. The runtime deliberately wires
`Migrations = ()`: it does not execute a V5-to-V7 or V7-to-V8 Bulletin migration, import a
predecessor state, decode a legacy storage form, backfill provider references, or expose a
compatibility facade. A future post-launch schema change must introduce its own forward migration
and storage-version evidence when that change is designed.

The remainder of this ADR records historical Iteration-5 evidence used to derive the policy
pipeline. It is non-normative for genesis construction and must not be interpreted as executable
migration or launch work.

## Historical Iteration-5 evidence boundary

The historical `d75ff22a` runtime was spec 26 / transaction 6. The current runtime composes the
remediation at spec 28 and transaction version 7. The preceding dormant-support
and unregistered-migration commits preserved the historical runtime metadata and optimized Wasm. Canonical positive direct and Meta literals are spec 28. Spec-27 and spec-26 literals are legacy negative fixtures. Compatibility means the `d75` tuple schema and field order,
not whole-byte equality across a spec-version transition.

The exact Meta v6 alias at that integration boundary is:

```text
(VerifySignature, ConsumePaidMetaIngress, MetaTxMarker, CheckNonZeroSender,
 CheckSpecVersion, CheckTxVersion, CheckGenesis, CheckMortality, CheckNonce,
 MetaAccountBoundPoliciesV6, ValidateStorageCalls, CheckMetadataHash)
```

`IntentPreimageV7` commits with Blake2-256 over its SCALE encoding. Its ordered fields are the
domain `orbis/meta-intent/v7`, extension version, genesis hash, spec version exactly 28, transaction version exactly 7, inner signer, call hash, mortality, nonce, policy-proofs hash,
storage-extension hash and metadata-extension hash. Mutating any committed field invalidates the
intent. Personhood routes exactly `PersonalAliasAccount`, `PersonalIdentityAccount` and
`PersonalAliasAccountRevised`; People Lite routes exactly `LitePerson`, `LiteAliasAccount` and
`LiteAliasAccountRevised`; Resources routes exactly `ClaimLongTermStorage`. Each route has a frozen
`Val` and `Pre` state and binds its authority account to the verified inner signer before changing
the dispatch origin.

A direct Resources claim begins as `Signed(call.account_id)`. Its transaction signature authorizes
the account-aware nonce and fee debit/refund; its Resources proof separately authorizes the custom
identity origin. A bad signature, signer/call-account mismatch or `None` origin is rejected before
nonce/payment side effects.

Meta-tree inspection is allocation-free and bounded by the three manifest constants
`MaxMetaEnvelopeDepth`, `MaxMetaEnvelopeCalls` and `MaxMetaEnvelopeBytes`. Transparent direct,
Utility batch/batch-all/force-batch, Proxy proxy/proxy-announced and concrete Multisig
as-multi/threshold-one ingress are the complete eligible set. Nested or multiple Meta envelopes,
dispatch-as/derivative, Sudo, Scheduler/preimage, hash-only Multisig approval, XCM/sovereign,
authorized/offchain, Revive/Ethereum, payerless and opaque ingress are denied or ineligible as
enumerated by manifest v4. The BaseCallFilter remains a leaf defense rather than the sole envelope
guard.

The paid-ingress token is one-shot and key-bound. Outer preparation performs all fallible checks
before writing it; `ConsumePaidMetaIngress` removes the exact token before dispatch.
`MetaTokenMustBeEmpty`, `PostTransactions`, Executive finalization and try-state enforce an empty
slot after every validation, preparation, dispatch, post-dispatch and bypass failure. Frozen weight
ownership is paid scope `2R + 2W`, base leaf `1R`, consumer `1R + 1W`, plus the selected router
variant and bounded inspection coefficients. Generated benchmarks must replace conservative values
without changing that ownership.

The predecessor development iteration modelled Bulletin V6-to-V7 as a separate two-phase repair. A
read-only bounded preflight derives both `ResourceLinkByRef` and
`ResourceLinkByContentHash` from authoritative links, validates duplicates/dangling ownership and
derives row/link counters before any write. The infallible phase clears and rebuilds both maps,
writes both counters, then writes storage version 7 last. With active, tombstone, authoritative
link, old ref-index and old hash-index counts `A,T,L,I_ref,I_hash`, its exact database budget is
`reads = A + T + L + I_ref + I_hash + 3L + 3` and
`writes = I_ref + I_hash + 2L + 2 + 1`. The separate REF/HASH missing, partial, stale and duplicate
rehearsals, simultaneous partial/bad-counter state, historical 4c/640 states, empty state and maximum
valid state are mandatory. Invalid preflight performs zero writes and leaves storage V6.

That historical repair did not include provider composition. The current clean-genesis runtime
instead declares V8 directly and stores the optional provider reference in its canonical schema;
there is no V7-to-V8 backfill or predecessor-state preservation requirement.


## Historical evidence-v4 completion status

The spec-28/transaction-7 runtime composes the Verify-to-Consume alias, signed and reciprocally bound direct
Resources payer, account-aware nonzero/nonce/payment adapters, all seven account-bound Meta routes,
bounded allocation-free ingress inspection, XCM/authorized denial, one-shot token finalization and
the Bulletin V5-to-V7 composed migration. Bulletin declares storage V7 only at this boundary. Pallet indices remain unchanged. Manifest v4 records normalized checked artifacts and hashes; Bulletin V7 is present while provider V8 remains planned.

Metadata implicit evidence is split into compiled-enabled RFC-78 reproduction, custom-hash wire-loss detection, isolated no-hash `CannotLookup`, and early propagation before token or business-state mutation.

### Historical migration identity cross-check

`PMIG-Bulletin-V6-to-V7` and `PMIG-Bulletin-V7-to-V8` identify predecessor-development evidence
only. Neither is registered by the current runtime. Provider references are part of the canonical
V8 genesis schema and do not depend on a completed V7 migration.

### Iteration-5 Gate 5 closure

Independent Architect and Critic verdicts cleared only the Iteration-5 runtime/evidence freeze. Gate
`GATE-5-EVIDENCE` moves from planned to present with source-emitted evidence; no other planned row
changes. The reviewers explicitly did **not** clear the whole program: their reviewed pre-closure
report contained 146 planned rows. Moving the gate itself leaves 145 normalized planned rows after
closure, including all four provider V8 contracts and every previously planned capability row.

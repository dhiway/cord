# Orbis identity topology contract

Status: **implemented native topology; P6/P7 production evidence remains outstanding**. This
extends ADR 0007 and does not replace the active identity pallets.

| Concept | Sole authority | Referenced by | Forbidden duplicate |
|---|---|---|---|
| entity/controller/accounts | Entity with Register/Token history | applications by SubjectId | copied profile/personhood root |
| profile/aliases/judgements | People | SDK composite views | membership or attestation status |
| lightweight authorization | People-Lite | bounded policy extension | profile/root copy or privilege escalation |
| personhood roots/epochs | Members | Personhood, Resources, Score/Honour and future native apps | local MembersSubscriber root |
| outbound root notification | MembersNotifier | approved sibling consumers | native Orbis dependency on notification |
| anonymous proof/context alias | Personhood/verifier | scoped application call | raw private proof/claim storage |
| quota/reservation | Resources | Orbis Storage/application authorization | personhood/profile truth |
| credential schema/status | native Attestation pallet 105 | DotNS/apps/SDK | Revive contract status or private claim |
| names/content records | native DotNS pallet 116 | resolved references | copied identity/attestation/content records |

## Normative invariants

- I1 SubjectId/account mapping is deterministic per genesis/network.
- I2 Entity/People references do not imply personhood.
- I3 Judgement cannot mint membership; membership cannot imply judgement.
- I4 People-Lite cannot escalate authority.
- I5 Alias/reverse-name controller and normalization are canonical and collision-free.
- I6 Proofs bind genesis, spec/application, collection, root/context version, payload hash, nonce and expiry.
- I7 Root rotation declares finalized activation, bounded overlap and revocation.
- I8 Attestation revocation and dependent reads resolve at the same finalized hash.
- I9 Cross-pallet links use stable IDs/traits/APIs and never mirror mutable records.
- I10 Genesis contains validated public/bootstrap state only and imports no private or legacy contract claims/proofs.

A composite SDK read pins one finalized block hash for every component or calls a versioned atomic-snapshot runtime API. A missing historical state/version or mixed hash returns a typed error; clients never assemble a partial cross-block answer.

Private claims, presentations, BCTS/envelopes, content bytes, host consent and keys stay off-chain. On-chain data is limited to identifiers, commitments, status/revocation, public policy, roots/epochs and bounded audit events. P2 threat tests cover proof replay, issuer escalation, alias collision, root rotation, stale status, cross-hash assembly and privacy leakage.

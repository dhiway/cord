# Origin/Orbis authority contract

Status: **proposed for P0 ratification**. Network: Origin spec 9901; Orbis para 1006, spec 29/tx 8.

## Invariants

1. Origin root owns validator membership, relay upgrade, registration and bootstrap/emergency cores; these are separate keys and ceremonies.
2. Orbis root owns collators, Orbis upgrades and scoped domain administration. Domain keys are expiring/rotatable and cannot upgrade either chain.
3. Orbis Broker pallet 50 is the sole normal core-lifecycle writer. Origin Coretime accepts Broker management only from para-origin 1006; arbitrary signed/XCM origins fail.
4. TxPause/SafeMode is an expiring, two-person break-glass path. It cannot create a second domain authority.
5. Runtime metadata/APIs own chain semantics. A host owns user permission, active consent, signer and transport, never chain administration.
6. Off-chain providers own content availability and bytes; they cannot rewrite finalized commitments, authorization or capacity.
7. Unknown origin, key, XCM location/call/version, extension slot or consent fails closed.

## Decision matrix

| Decision | Writer | Accepted origin/key | Activation | Required record |
|---|---|---|---|---|
| Origin validator admit/remove | AuthorityManager | Origin governance multisig/HSM | Session boundary | proposal, PoP, simulation, quorum, finality |
| validator session keys | Session | member validator HSM | Session boundary | PoP, dual-control rotation |
| Origin upgrade | AuthorizedUpgrade/Sudo | distinct Origin upgrade multisig | authorized enactment | reproducible WASM and runtime hashes |
| para code/genesis/registration | Origin registrar/Sudo | Origin governance multisig | finalized relay block | para/genesis/code hashes |
| bootstrap/emergency core | Origin Coretime | Origin root/break-glass | bounded incident | reason, expiry, postmortem |
| normal core lifecycle | Orbis Broker 50 | Orbis root policy; emitted from para 1006 | Broker schedule | request/reserve/assign/renew/release audit |
| Orbis collators | CollatorSelection | Orbis operations multisig | Session boundary | PoP and rotation record |
| Orbis upgrade | AuthorizedUpgrade/Sudo | distinct Orbis upgrade multisig | authorized enactment | WASM/metadata/SDK/vector hashes |
| pause/resume | TxPause/SafeMode | incident multisig | immediate/expiring | two-person approval, expiry, postmortem |
| DMP execution | Orbis XCM barrier/filter | Origin location plus allowed call/version | finalized Orbis block | message/outcome/weight |
| Broker UMP | Origin Coretime | para-origin 1006 only | finalized Origin block | Broker state/message/outcome |
| domain issuer/registrar/provider | scoped pallet origin | named Orbis domain multisig | bounded term | grant/scope/rotation/revocation |
| host signing | local host | active user consent and secure signer | one request/session | app/scope/nonce/expiry/result |

## Ceremony contract

Every privileged action records proposal hash, change class, approvers/quorum, HSM identifiers without secrets, simulation and runtime/call/genesis hashes, window, signatures, enactment/finality block, post-check and retention location. Key rotation is rehearsed before launch. No one key may simultaneously satisfy Origin upgrade, Orbis upgrade and emergency quorum.

## Threat and privacy inventory

| Asset/flow | Principal threat | Mandatory control |
|---|---|---|
| validator/collator keys | theft, correlated loss, stale membership | HSM, separate operators, PoP, session rotation/recovery rehearsal |
| runtime WASM | supply-chain substitution, malicious upgrade | locked SDK, reproducible build, independent review, hash ceremony |
| Broker/XCM | forged para, replay, call widening, weight exhaustion | para 1006 conversion, call/version allowlist, rate/weight bounds, duplicate tests |
| issuer/name/provider roles | escalation, stale delegation | scoped origin, expiry, rotation/revocation, bounded audit events |
| user proof/credential | linkability, replay, disclosure | context/root/nonce/expiry binding; private claims off-chain |
| content | corruption, censorship, private-data disclosure | CID verification, provider failover; bytes and encryption off-chain |
| host request | malicious app, stale consent, signer substitution | app scope, active consent, nonce/expiry, secure signer, cancellation |

Verification command is frozen in `docs/evidence/verification/p0/index.json`. P1 exercises every control-plane row; P2-P4 exercise scoped-domain rows. Missing named owners or signatures block activation.

# ADR 0009: Origin/Orbis operational security

- Status: proposed
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Origin is a dedicated enterprise relay and Orbis para 1006 is its unified application system chain. Deterministic admission, upgrades, capacity and recovery are the primary reason to operate this topology. A small permissioned validator set increases correlated-key, quorum and operational risks compared with a strong external relay.

## Decision

Keep separate authority planes and keys: Origin validator membership/session keys, Origin upgrade/registrar/bootstrap-core, Orbis collator membership, Orbis upgrade/domain administration, and an expiring two-person break-glass pause. Offline governance multisig/HSM ceremonies record proposal, simulation and code/genesis hashes, quorum, enactment/finality block and post-check. Orbis Broker 50 is the normal core-lifecycle authority; Origin root is bootstrap/emergency only; Origin accepts Broker management only from para 1006.

P0 must price three years of both the dedicated and external-relay alternatives and record operator/quorum assumptions. P1 must test key loss/compromise, admission/removal/session rotation, pause/resume, failed upgrade and recovery. Reopen the relay choice on any plan R1-R4 trigger.

## Drivers

Deterministic control; separation of duties; least privilege; recoverability; auditable ceremonies.

## Alternatives

Use an external relay (stronger shared security, weaker deterministic control); dual deployment (rejected for R1 operational/test cost); one omnibus Sudo key (rejected).

## Authority and security

Root/Sudo remains intentionally powerful for the enterprise-first network, so key separation, HSM custody, quorum and rehearsal are launch gates.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

`docs/architecture/authority-contract.md` validates one authority per decision. P1 control and Broker/XCM scenarios must pass; unresolved high/critical threats block release.

## Reversal

Move Orbis to an external relay only via a new ADR and new-genesis/re-registration plan; never silently widen Origin/XCM authority.

## Follow-ups

Name key owners and quorum; perform ceremonies; complete cost evidence and P1 recovery campaign.

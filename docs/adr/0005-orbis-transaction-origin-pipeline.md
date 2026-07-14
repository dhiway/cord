# ADR 0005: Orbis transaction, origin, and payment pipeline

- Status: accepted
- Date: 2026-07-11

## Decision

Orbis preserves separate actors: outer relayer, inner meta-transaction signer, nonce owner,
sponsor payer, People identity subject, and Orbis Storage authorizer. Meta-transactions use the existing
`pallet_meta_tx` marker and signature extension; they do not convert the relayer into the actor.

Before payment, one centralized recursive call inspector classifies direct and nested Utility,
Proxy, Multisig, Sudo, and MetaTx calls. Unknown or opaque calls are denied **only from zero-fee
exemption** and may still use normal paid dispatch. Identity and Orbis Storage authorization run against
the inner actor before fee exemption. Sponsored transactions charge the sponsor under explicit
quota; controlled zero-fee transactions require an allowlisted account and allowlisted call tree.
Failed dispatch still consumes nonce and the configured sponsor/quota charge.

Target validation order is: call authorization, signature/meta-tx actor recovery, version/genesis/
era, actor nonce, weight, recursive identity/storage/fee policy, payment or exemption, metadata
hash, then dispatch. Post-dispatch correction must refund only the payer selected before dispatch.
This ADR is incomplete in code until FEE-01 through FEE-05 adversarial tests pass.

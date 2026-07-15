# Unified Identity contract v2

Identity is one developer facade, not one joined authority or necessarily one pallet. Every operation
has its own grant, audience, consent, response, and finalized-state rule:

| Operation | Result boundary |
|---|---|
| `identity.account` | selected account/session metadata; no signing authority |
| `identity.profile.read` | selected public finalized chain fields |
| `identity.profile.disclose` | separately consented selected host-held fields for one audience |
| `identity.humanity.status` | minimal status and freshness only |
| `identity.humanity.prove` | audience/challenge-bound proof with separate consent |
| `identity.subject.derive` | opaque contextual subject only |
| `identity.entitlements.read` | decision, scope, policy version, expiry/freshness; no proof/profile |
| `transaction.sign` | separate signing grant and fresh consent; never an Identity read grant |

Operations may share a finalized block hash but MUST NOT return a composite response. Account/session
is host state, not a runtime Identity database. A grant for one row confers no operation in another.

## Contextual subject and proof

The host is issuer and sole master-secret holder. `SubjectContextV2` is deterministic CBOR with integer
keys `{0:2,1:genesis:bstr32,2:product_id:tstr .size(1..128),3:context:tstr .size(1..256),
4:verifier_audience:tstr .size(1..256),5:epoch:u32,6:recovery_incarnation:bstr32,
7:continuity:bool}`. There is no V1 decoder. HKDF-SHA256 uses the current 32-byte master seed as IKM,
`SHA-256("cord.identity.subject.kdf.v2")` as salt, and the exact deterministic CBOR context as `info`
to derive a 32-byte Ed25519 seed. Subject is
`BLAKE2b-256("cord.identity.subject.id.v2" || recovery_incarnation || derived_public_key)`.

`SubjectProofV2` signed bytes are the deterministic CBOR map
`{0:2,1:genesis,2:product_id,3:context,4:audience,5:epoch,6:recovery_incarnation,7:continuity,
8:subject,9:derived_public_key,10:challenge:bstr(16..64),11:issued_at:u64,12:expires_at:u64,
13:nonce:bstr16}`, prefixed by `cord.identity.subject.proof.v2`, with detached Ed25519-64 signature.
Expiry is positive and at most 128 finalized blocks. Verifier checks every bound field and consumes its
challenge/nonce atomically. Before signing, the host durably consumes
`(genesis,audience,incarnation,epoch,SHA-256(challenge),nonce)`. Replay is
`IDENTITY_CHALLENGE_REPLAY`; wrong incarnation is `IDENTITY_OLD_INCARNATION`.

## Recovery: fresh root, no continuity

A proven same-store restart atomically restores encrypted master seed, random 32-byte incarnation,
epoch/counter, and the complete unexpired replay journal/integrity root before issuance. Only this
path may preserve continuity.

Every backup import, cross-device/disaster recovery, seed-only restore, stale/incomplete store, or
store without proven monotonicity MUST discard the restored subject root. It generates a new 32-byte
CSPRNG master seed and independent 32-byte incarnation, sets epoch zero, and atomically commits
`RecoveryInstallV2` plus byte-exact `RecoveryReceiptV2` before derivation or success. The 16-byte
recovery operation ID is idempotent: a retry returns the identical receipt/root. All-zero entropy or a
current/retired collision retries three times then returns 408. Failed atomic install returns 409 and
permits no derivation.

The retired set stores sorted `SHA-256(seed || incarnation)` tombstones, at most 16,384 (512 KiB hash
material). It never auto-evicts. Full capacity returns `411 IDENTITY_RETIRED_SET_FULL` before install.
Authorized reset/export may remove local collision tombstones only while forcing `continuity=false`;
it cannot revive a root.

Recovered proofs/responses carry `continuity=false`; old subjects, Humanity sessions, and entitlements
are not inherited and apps re-enrol. Grants bind incarnation. Reachable authority revocation reports
`pending|finalized`; unreachable authority reports `unavailable`. The platform does not promise global
revocation, device shutdown, or link continuity. Old proofs can remain usable until their at-most
128-block expiry. Disconnected imports intentionally create different roots.

## Threat and privacy boundary

Proof transcripts and subjects remain off chain and transient buffers are erased. Tests cover
colluding apps/hosts/providers, audience substitution, replay, recovery/split brain, transcript
retention, logs/metrics, and account/fee/extrinsic/event correlation. Different subject bytes alone
do not prove unlinkability: shared accounts, fee payers, timing, extrinsics, or events remain
correlatable and MUST be disclosed to developers.

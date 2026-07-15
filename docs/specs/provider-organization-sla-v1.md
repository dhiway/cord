# Provider organizational identity and SLA authority v1

Provider authority is organizational and MUST remain separate from Humanity, app-scoped subjects,
personal profiles, and person entitlements.

`ProviderOrganizationRefV1` contains `entity_id:Ss58Identifier`, `attestation_id:Hash`,
`schema_id:Hash`, `sla_commitment:Hash`, `sla_version:u16`, `valid_from:BlockNumber`,
`valid_until:BlockNumber`, and an optional rotation predecessor. Foundation/Commons provider-admin
governance controls admission. Only an allowlisted organizational-attestation issuer can attest the
organization. The confidential SLA body remains off chain; Commons stores versioned schema and
commitment only.

Activation is evaluated at finalized state and requires a live Entity, issued/final/non-revoked
attestation, validity interval, authorized issuer, admitted SLA schema/version, and service key bound
to the organization. Failures map exactly to codes 256–260. Expiry or revocation suspends new
agreements and writes at the next finalized observation and starts deterministic failover, while
preserving hashes, signed evidence, and status events. Rotation links references append-only and MUST
NOT rewrite evidence. No provider organization check may satisfy a Humanity, subject, or person
entitlement operation.

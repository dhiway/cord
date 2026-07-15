# Bucket, agreement, capacity and deletion contract v1

A bucket has an owner, monotonic version, policy, one primary, two to four replicas, and at most 256
membership grants. Roles are `reader`, `writer`, and `admin`; bucket admin never implies chain signing.
Every mutation supplies `expected_version`.

Agreement states are `Proposed -> Active -> Suspended -> Active` and terminal `Cancelled | Expired`.
Only an eligible governed provider plus owner acceptance activates an agreement. Proposal reserves
capacity; activation allocates it. Terminal state blocks new writes immediately. Capacity is released
only after the bounded tombstone/evidence window. A provider has at most 1,024 agreements; an
agreement has at most four replicas. Events contain prior state, new state, and version.

Reads are strongly consistent with the latest finalized publishable checkpoint. Pending uploads are a
separate status. Lists pin a snapshot and encode its version in the cursor; version drift returns
`STORAGE_CURSOR_STALE` rather than mixing snapshots.

Deletion is a versioned tombstone followed by acknowledgements/evidence from every CORD-controlled
active replica. Success means those replicas no longer serve the object. It does not erase audit
hashes, chain events, legal-retention copies, or third-party copies and MUST NOT be described as
cryptographic erasure, GDPR compliance, or SLA compliance.

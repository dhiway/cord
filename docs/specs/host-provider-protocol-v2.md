# `cord.origin.host/2` and provider protocol contract

## Registry and negotiation

`origin-host-registry-v2.cddl` is the normative protocol registry. Its exact SHA-256 is the descriptor
hash. A new operation is a minor change; changing a field, type, bound, or meaning is a major change.
Version 1 is not supported. Negotiation binds major/minor, Commons genesis, finalized runtime
spec/transaction versions, descriptor hash, and exact feature intersection. Major, genesis, or hash
mismatch is terminal. Every token binds the negotiated tuple.

All signed/hashed values and desktop IPC use RFC 8949 deterministic CBOR: definite lengths, shortest
integer/length encoding, and map keys sorted first by encoded-key length then unsigned lexical bytes.
Floats, tags, undefined, duplicate keys, invalid UTF-8, non-NFC text, and indefinite lengths are
rejected before authorization. Registry maps use unsigned integer keys/discriminants. Desktop frames
have a four-byte big-endian length and are at most 4,194,304 bytes. Browser structured-clone values
must round-trip through the same codec. Mobile JSON is only a projection and must canonicalize to the
same CBOR.

Object and S3 GET/range bytes are emitted only as typed progress chunks of at most 4 MiB; their terminal result carries verified CID/length/range/checkpoint metadata. This is required so a 64 MiB object never violates the 4 MiB frame bound.

`EventV2` starts with Accepted at sequence zero; subsequent events increment by one. Result, error,
and cancelled are terminal. Duplicate, skipped, or post-terminal sequence closes transport with 105.
Subscriptions retain 256 events, use strictly increasing cursor sequence, and require exactly one
unsubscribe acknowledgment. No callback continues after acknowledgment.

## Durable pre-send outbox

No provider-visible request, capability, resume/successor token, cancel, or response ack may leave the
host before its byte-exact `HostOutboxEntryV1` is durably committed. This applies to provider reads and
writes; plain chain RPC reads are outside the outbox. The encrypted record contains exact request and
authority bytes, fingerprint, IDs/generation/cursor, registry/genesis/negotiation, provider endpoint
hash, expected response, timestamps, recovery bound, and wrapping-key version.

At rest it is `0x01 || nonce[24] || XChaCha20-Poly1305(ciphertext || tag[16])` under a random 32-byte
host-profile outbox key distinct from content, account, delegation, and subject keys. AAD binds host
profile, outbox ID, key version, registry hash, and genesis. Desktop durability is one atomic WAL/DB
transaction followed by file and directory fsync. Browser durability is an origin-private IndexedDB
transaction requested strict and awaited through commit. Unavailable durable commit or key returns
113 before send. Corruption quarantines the entry, returns 115, and neither sends nor regenerates
authority.

The authoritative state law is:

1. `Prepared` is durable before the first network byte. `Sent` is advisory only.
2. Restart/loss resends byte-identical stored request and authority; IDs, nonce, token, and fingerprint
   are never regenerated.
3. The authenticated response bytes/hash, successor token, cursor, and `ResponseInstalled` are one
   atomic install before `ResponseAckV1` is sent.
4. Ack confirmation persists `AckConfirmed`. A successor becomes live only in the install transaction;
   continuation requires its own Prepared entry.
5. Terminal/cancel response and ack confirmation persist before authority erasure. Body GC requires a
   durable successor or durable terminal; fingerprint/response-hash tombstone survives the TTL.
6. Recovery closes at authority expiry plus 256 finalized blocks (at most 384 from issue), or terminal
   block plus 256. Expiry erases authority, retains tombstone, and returns 116; it never creates a new
   request.

Capacity is 4,096 entries, 268,435,456 encrypted bytes total, and 4,456,448 bytes/entry. Capacity is
reserved before authority consumption. GC prioritizes acknowledged old/terminal bodies and cannot
evict the sole recoverable Prepared/ResponseInstalled record.

## Capability, resume, acceptance and cleanup

`ProviderCapabilityV1` is deterministic CBOR, domain `cord.provider.capability.v1`, with detached
Ed25519-64 by a finalized host delegation key. It binds registry/genesis, grant/issuer/product,
bucket/agreement/provider audience, methods, optional CID, maximum bytes, issue/expiry (at most 128
blocks), and nonce. Rotation/revocation becomes effective at finality.

`ResumeTokenV1` is deterministic CBOR, domain `cord.provider.resume.v1`, Ed25519-64 by the finalized
provider service key. It binds registry/genesis/provider/host audience, operation/bucket/CID/length,
last durable contiguous chunk, generation, issue/expiry, nonce, and cancelled flag. It is
single-effect and monotonically advancing. Resume starts at cursor+1; unacknowledged partial chunks
are discarded. Provider change requires a fresh capability and token after the new provider proves
its local verified cursor. Cancellation durably revokes token and journal.

Before delivering any Accepted/progress/successor/cancel/terminal response, provider atomically stores
`RecoveryEntryV1`, journal effect, and replay index. Fingerprint is
`SHA-256(canonical RequestV2 || canonical authority)`. The recovery key is
`(host_delegation_key, operation_id, generation, consumed_nonce)`. Identical authenticated retry gets
the byte-identical response without repeating the effect. Changed fingerprint/audience/descriptor/
provider/cursor under the nonce returns replay. Host atomically installs response before ack; provider
atomically acknowledges. Recovery entries are at most 8,192/provider and survive through
`max(expiry,terminal)+256`; exact recovery remains available when full while new work returns 238
before consumption.

Backpressure is four chunks and 1 MiB. Cancel is idempotent only in Accepted/Progress, creates one
terminal cancelled event, revokes resume state, and forbids later progress. Browser Worker
MessagePort and desktop framed CBOR are mandatory and run identical canonical/noncanonical vectors.
Mobile consumes schemas/fixtures only. No normal log, metric, or evidence record contains raw payload,
token, fingerprint, authority, or operation metadata.

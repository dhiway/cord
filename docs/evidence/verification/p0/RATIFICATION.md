# Independent P0 target ratification

The signed object is the exact byte sequence in `p0-ratification.payload.json`.
Its SHA-256 must equal `payload_sha256` in `ratification-envelope.json`. Signatures
approve only P0 targets/client contracts. They do not approve production genesis,
network activation, or a performance result.

The five role owners are tracked as `P0-APPROVER-RUNTIME`, `P0-APPROVER-SDK`,
`P0-APPROVER-SECURITY`, `P0-APPROVER-PERFORMANCE`, and
`P0-APPROVER-ARCHITECTURE`. No private or test approval key belongs in this repo.

## Key registration

Each owner creates and retains an Ed25519 key outside the repository:

```sh
openssl genpkey -algorithm ED25519 -out "$PRIVATE_KEY"
openssl pkey -in "$PRIVATE_KEY" -pubout -outform DER -out "$ROLE.spki.der"
openssl dgst -sha256 "$ROLE.spki.der"
base64 < "$ROLE.spki.der" | tr -d '\n'
```

Through the corresponding tracker, record the role, DER SHA-256 fingerprint,
SPKI DER base64, `valid_from`, `valid_until`, and `revoked_at: null` in the signed
payload's `authorized_keys`. Change that role's slot to `READY` and bind the same
fingerprint. Keys must be unique across roles. After all five public keys are
reviewed, and before any signature exists, run:

```sh
npm --prefix product-sdk run update:ratification
npm --prefix product-sdk run generate:descriptors
npm --prefix product-sdk run update:ratification
shasum -a 256 docs/evidence/verification/p0/p0-ratification.payload.json
```

The printed/file SHA must exactly equal the envelope payload hash. Any contract
digest or key-registry change requires all signatures to be collected again.

## Detached owner signature

Each owner independently verifies the contract digests and signs the frozen file:

```sh
openssl pkeyutl -sign -rawin \
  -inkey "$PRIVATE_KEY" \
  -in docs/evidence/verification/p0/p0-ratification.payload.json \
  -out "$ROLE.sig"
base64 < "$ROLE.sig" | tr -d '\n'
```

Add only the detached signature record—role, registered fingerprint, payload hash,
SPKI DER base64, signature base64, and UTC `signed_at`—outside `payload`. Do not
rerun the update command after signing. Verify:

```sh
npm --prefix product-sdk run validate:ratification
```

The verifier requires the exact nonempty five-role set, unique allowlisted SPKIs,
valid key windows, no effective revocation, matching payload hashes, and valid
Ed25519 signatures. P0 target ratification is distinct from production activation;
final genesis/campaign authorization requires a later payload and five fresh
signatures.

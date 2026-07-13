# P5 native SDK freeze ratification

`sdk-freeze-ratification-envelope.json` binds the spec-29/transaction-8 bootstrap descriptor,
the generated 124-route HostRequest schema, and the other existing SDK contracts. Its canonical
payload is `sdk-freeze-ratification.payload.json` with SHA-256
`2b029154ce67c4b2a5d6391937bc9b110193606f17bf068270f0198f6c3297cb`.

This envelope is deliberately unsigned. All five required approval slots are `PENDING`, its
derived target-ratification status is false, and production activation is blocked. The historical
signed P0 envelope remains unchanged; none of its signatures were copied. P5 becomes ratified only
after the runtime, SDK, security, performance, and architecture owners produce new valid Ed25519
signatures over this exact canonical payload.

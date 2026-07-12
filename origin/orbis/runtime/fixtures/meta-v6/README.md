# Orbis Meta v6 wire fixtures

These checked-in binary files freeze the spec-27 transaction wire contract:

- an actual directly signed `UncheckedExtrinsic`;
- the actual `MetaTxFor<Runtime>` tuple from `VerifySignature` through
  `ConsumePaidMetaIngress` and the remaining inner extensions;
- one `PolicyProofsV6` value for each of the seven router variants;
- the intent preimage and commitment, paid-token key, and accepted 32-call/depth-4 envelope;
- an immutable spec-26 negative intent (`spec26-negative.json` plus its raw SCALE `.bin`);
- one independently encoded intent mutation for every signed field.

`meta_v6_fixtures::checked_in_meta_v6_fixtures_decode_all_and_recompute` decodes complete SCALE
values, verifies both signatures, rebuilds the vectors from runtime types, checks the max envelope
with the production inspector, and recomputes the commitment and token key..

The three cryptographic proof fixtures and direct Resources extrinsic are captured from the corresponding green Executive E2E builders; the dedicated fixture test verifies their complete SCALE decoding and signatures.

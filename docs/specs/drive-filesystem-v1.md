# Drive and File System contract v1

A Drive root is a raw-CID-addressed SCALE `DriveManifestV1`. Root replacement is atomic and carries
`(expected_version, previous_root, new_root)`. The root may change only when all referenced objects are
publishable.

Names are 1–256 UTF-8 bytes and MUST be NFC; `/`, NUL, `.`, and `..` are invalid. Children are unique
and sorted by unsigned raw UTF-8 bytes. A directory has at most 1,024 children. An entry has at most
64 metadata pairs; keys are at most 64 bytes and values 256 bytes. Paths are at most 4,096 bytes and
64 components deep. A file has at most 256 chunks. MIME is at most 128 bytes; serialized encryption
parameters at most 512 bytes.

A file manifest records ordered chunk CIDs, exact plaintext length, ciphertext length, MIME, and
encryption parameters. Checkpoint strategy defaults to `batched(100 blocks)`; `immediate` and explicit
`manual` are the only alternatives. Sharing maps only to bucket roles/grants. Stale mutation returns
`DRIVE_VERSION_CONFLICT`; invalid name, depth, child limit, ordering, missing/unpublishable reference,
and metadata limit have distinct registry errors.

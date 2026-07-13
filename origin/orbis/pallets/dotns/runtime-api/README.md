# Orbis DotNS runtime API

Stable, versioned, bounded SCALE views for the native Orbis DotNS pallet. The API supports name
metadata, normalized root-label lookup, bounded owner pagination, record resolution, primary names,
and active/expiry status.

Clients choose the invocation block. Production clients should invoke the API at a finalized block
hash; finalized-head selection is intentionally not runtime logic.

This crate has no smart-contract ABI or legacy compatibility surface.

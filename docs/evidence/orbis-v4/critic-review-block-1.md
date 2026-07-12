# Critic BLOCK — fourth remediation record

The Critic blocked v4 because row accounting was verifier-derived rather than source-owned, evidence
could be accepted from dirty or untracked files, and the production runtime baseline was conflated
with later test-only marker commits.

This remediation restores `audited_runtime_commit` to production baseline `f88aa3f`, introduces a
separate `evidence_marker_commit`, freezes all 645 manifest identities in a source-owned inventory,
requires committed/clean evidence, and records a same-path reproducible compact-runtime Wasm build
showing byte identity between the baseline and marker commit. This is a BLOCK record, not clearance.
Critic remains pending and Gate 5 remains planned.

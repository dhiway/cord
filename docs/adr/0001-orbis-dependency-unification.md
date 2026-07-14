# ADR 0001: Orbis dependency unification and source provenance

- Status: accepted
- Date: 2026-07-11

## Decision

Orbis has exactly one FRAME/SP/Cumulus dependency graph: the CORD workspace graph pinned to
Dhiway SDK `release-v1.24.0` (resolved by the current lockfile to SDK revision `cc190ea8`). External
runtime repositories are design and test references, not direct build dependencies.

Reviewed source snapshots are:

| Source | Revision | License role |
|---|---:|---|
| Dhiway SDK working tree | `55e62434c4b8` | upstream compatibility reference |
| Fellows runtimes | `477689fddba4` | runtime behavior reference |
| Paseo runtimes | `ac99ed6c1122` | runtime behavior reference |
| Individuality Community | `28b7d07dab05` | Apache-2.0 People/Asset Hub reference |
| upstream transaction-storage reference | `b6c2827d2326` | GPL-3.0 storage reference |

People behavior is temporarily adapted onto CORD's maintained identity pallet pending an
Orbis-owned upstream-aligned replacement. Orbis Storage transaction-storage, primitives, runtime API,
common helpers, and Hop Promotion are vendored under `origin/orbis/pallets/` from revision
`b6c2827d2326`; their manifests use CORD `workspace = true` dependencies. Adding upstream runtime crates directly remains prohibited
because the reviewed snapshots resolve a different SDK generation. Copied source retains its
original license header and provenance.

## Verification

`cargo tree -p origin-commons-runtime -d --depth 0` contains ordinary ecosystem duplicates but no
duplicate `frame-*`, `sp-*`, or `cumulus-*` roots. `frame-support 48.0.0`, `sp-runtime 48.0.0`, and
`cumulus-pallet-parachain-system 0.29.0` all resolve from Dhiway SDK `release-v1.24.0#cc190ea8`.

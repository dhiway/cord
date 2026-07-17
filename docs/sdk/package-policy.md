# Origin SDK package and release policy

## Frozen decisions

- The package scope is `@cord-network`; the umbrella is `@cord-network/origin-sdk`.
- Registry probes on 2026-07-14 returned `E404` for both the umbrella and the existing private
  workspace name. Absence does not prove organization ownership. Publication is blocked until an
  administrator records npm organization access outside source control.
- Packages are ESM-only and ship compiled JavaScript, TypeScript declarations and declaration maps.
  No public export may point to a `.ts` source file.
- Node 22 is the initial build/test floor. Browser, hosted, iOS and Android consumers use portable
  package or transport surfaces; they do not run repository TypeScript directly.
- Versions remain below 1.0 while the first app journeys stabilize. A minor version may break only
  with a Changeset and migration note; patches are compatible fixes. Version 1.0 adopts normal
  semantic versioning.
- Generated descriptors and route projections are checked in and must be byte-identical after their
  documented generator runs. Hand edits to generated output are prohibited.
- Each publishable package declares `files`, `exports`, `sideEffects`, runtime/development
  dependencies, supported engines and a `/testing` subpath when a fake exists.
- The product workspace does not publish a contracts package. Commons-native capabilities and the
  host-v2 boundary are the only supported application path; contract deployment, ABI wrappers, and
  native-versus-contract switches are intentionally absent.

## Release gates

A package is not publishable until it builds from a clean checkout, packs without source/tests or
secrets, installs into the consumer fixture, exposes no raw TypeScript, and stays within its recorded
bundle budget. Changesets create release intent; they never imply production network approval.

# ADR 0007: Orbis unified personhood topology

- Status: accepted
- Date: 2026-07-12

## Decision

Orbis stores authoritative ring-membership roots in its native `Members` pallet and all native
People, Assets, PGAS, alias, and application pallets consume `Members` directly. Orbis does not
instantiate `indiv-pallet-members-subscriber` for its own application path.

`MembersSubscriber` exists to copy roots from a remote People chain into a separate consumer chain.
Instantiating it beside the authoritative `Members` pallet would create duplicate state, delayed
consistency, and a self-XCM trust path without adding capability. This conflicts with the selected
unified-hub architecture.

Orbis retains `MembersNotifier` so separately deployed enterprise parachains can subscribe to
selected collections. Subscription administration is Root/Sudo-only, replay requests are accepted
only from the requesting sibling parachain, and native Orbis consumers never depend on notifier
delivery.

## Consequences

- Native personhood verification remains available when HRMP/XCM delivery is unavailable.
- PGAS and alias-account integrations must configure `Members` as their membership prover.
- External parachains may deploy the upstream-aligned subscriber and trust Orbis as notifier.
- A future split-chain deployment requires a new ADR and migration plan before adding a subscriber
  instance to Orbis.

# Orbis provenance

Vendored from `paritytech/individuality` commit `28b7d07dab05bbd05f6b664278b5c83841e212d3`.
Subscription administration is Root/Sudo-only. Replay requests must originate from the requesting
sibling parachain. The unused upstream `indiv-pallet-people` manifest dependency is removed because
Orbis supplies ring roots through its native `indiv-pallet-members` implementation.

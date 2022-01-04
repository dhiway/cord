# Chain Parameters

Many of these parameter values can be updated via on-chain governance. If you require absolute certainty of these parameter values, it is recommended you directly check the constants by looking at the chain state and/or storage.

Periods of common actions and attributes

- Slot: 6 seconds \*(generally one block per slot, although see note below)
- Epoch: 4 hours (2_400 slots x 6 seconds)
- Session: 4 hours (Session and Epoch lengths are the same)
- Era: 24 hours (6 sessions per Era, 2_400 slots x 6 epochs x 6 seconds)

| Cord    | Time      | Slots\* |
| ------- | --------- | ------- |
| Slot    | 6 seconds | 1       |
| Epoch   | 4 hours   | 2_400   |
| Session | 4 hours   | 2_400   |
| Era     | 24 hours  | 259_200 |

\*A maximum of one block per slot can be in a canonical chain. Occasionally, a slot will be without a block in the chain. Thus, the times given are estimates.

# Staking, Validating, and Nominating

| Cord                 | Time    | Slots   | Description                                                                                                                                                                                         |
| -------------------- | ------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Term duration        | 1 day   | 14_400  | The time for which a validator is in the set after being elected. Note, this duration can be shortened in the case the a validator misbehaves.                                                      |
| Nomination period    | 1 day   | 14_400  | How often a new validator set is elected.                                                                                                                                                           |
| Bonding duration     | 28 days | 403_200 | How long until your funds will be transferrable after unbonding. Note that the bonding duration is defined in eras, not directly by slots.                                                          |
| Slash defer duration | 28 days | 403_200 | Prevents overslashing and validators "escaping" and getting their nominators slashed with no repercussions to themselves. Note that the bonding duration is defined in eras, not directly by slots. |

# Treasury

| Treasury               | Time    | Slots   | Description                                                  |
| ---------------------- | ------- | ------- | ------------------------------------------------------------ |
| Periods between spends | 24 days | 345_600 | When the treasury can spend again after spending previously. |

Burn percentage is currently 0.20%.

# Precision

WAYT have 12 decimals of precision. In other words, 1e12 (1_000_000_000_000, or one trillion) Plancks make up a single WAYT.

# Only for Demo

## Governance

| Democracy        | Time   | Slots   | Description                                                                                                                                                   |
| ---------------- | ------ | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Voting period    | 7 days | 100_800 | How long the public can vote on a referendum.                                                                                                                 |
| Launch period    | 7 days | 100_800 | How long the public can select which proposal to hold a referendum on, i.e., every week, the highest-weighted proposal will be selected to have a referendum. |
| Enactment period | 8 days | 115_200 | Time it takes for a successful referendum to be implemented on the network.                                                                                   |

| Council       | Time   | Slots   | Description                                                          |
| ------------- | ------ | ------- | -------------------------------------------------------------------- |
| Term duration | 7 days | 100_800 | The length of a council member's term until the next election round. |
| Voting period | 7 days | 100_800 | The council's voting period for motions.                             |

The Cord Council consists of up to 10 members and up to 5 runners up.

| Technical committee     | Time    | Slots   | Description                                                                                    |
| ----------------------- | ------- | ------- | ---------------------------------------------------------------------------------------------- |
| Cool-off period         | 7 days  | 100_800 | The time a veto from the technical committee lasts before the proposal can be submitted again. |
| Emergency voting period | 3 hours | 1_800   | The voting period after the technical committee expedites voting.                              |

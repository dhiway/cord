# Chain Parameters

Many of these parameter values can be updated via on-chain governance. If you require absolute certainty of these parameter values, it is recommended you directly check the constants by looking at the chain state and/or storage.

Periods of common actions and attributes

Slot: 4 seconds \*(generally one block per slot, although see note below)
Session: 2 days (6 sessions per Era)
Era: 12 days (259_200 slots x 4 seconds)

| CORD    | Time      | Slots\* |
| ------- | --------- | ------- |
| Slot    | 4 seconds | 1       |
| Session | 2 days    | 43_200  |

\*A maximum of one block per slot can be in a canonical chain. Occasionally, a slot will be without a block in the chain. Thus, the times given are estimates.

# Treasury

| Treasury               | Time    | Slots   | Description                                                  |
| ---------------------- | ------- | ------- | ------------------------------------------------------------ |
| Periods between spends | 12 days | 259_200 | When the treasury can spend again after spending previously. |

Burn percentage is currently 0.10%.

# Precision

WAYT have 12 decimals of precision. In other words, 1e12 (1_000_000_000_000, or one trillion) Plancks make up a single WAYT.

# Only for Demo

## Governance

| Democracy        | Time    | Slots   | Description                                                                                                                                                   |
| ---------------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Voting period    | 12 days | 259_200 | How long the public can vote on a referendum.                                                                                                                 |
| Launch period    | 12 days | 259_200 | How long the public can select which proposal to hold a referendum on, i.e., every week, the highest-weighted proposal will be selected to have a referendum. |
| Enactment period | 12 days | 259_200 | Time it takes for a successful referendum to be implemented on the network.                                                                                   |

| Council       | Time   | Slots   | Description                                                          |
| ------------- | ------ | ------- | -------------------------------------------------------------------- |
| Term duration | 7 days | 151_200 | The length of a council member's term until the next election round. |
| Voting period | 7 days | 151_200 | The council's voting period for motions.                             |

The CORD Council consists of up to 10 members and up to 5 runners up.

| Technical committee     | Time    | Slots   | Description                                                                                    |
| ----------------------- | ------- | ------- | ---------------------------------------------------------------------------------------------- |
| Cool-off period         | 7 days  | 151_200 | The time a veto from the technical committee lasts before the proposal can be submitted again. |
| Emergency voting period | 3 hours | 2_700   | The voting period after the technical committee expedites voting.                              |

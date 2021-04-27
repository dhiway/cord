## Cord benchmarking outputs -


## 1. Pallet Mtype - 

```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mtype --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mtype/src/weights.rs 
2021-04-16 10:23:27  💸 new validator set of size 1 has been elected via ElectionCompute::OnChain for era 0    
Pallet: "pallet_mtype", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    98.11
              µs

Reads = 1
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    98.11
              µs

Reads = 1
Writes = 1
```
## 2. Pallet Delegation - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_delegation --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/delegation/src/weights.rs 
```

```
💸 new validator set of size 1 has been elected via ElectionCompute::OnChain for era 0    
Pallet: "pallet_delegation", Extrinsic: "create_root", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    116.6
              µs

Reads = 2
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    116.6
              µs

Reads = 2
Writes = 1
Pallet: "pallet_delegation", Extrinsic: "revoke_root", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    136.7
    + r    158.9
              µs

Reads = 2 + (2 * r)
Writes = 1 + (1 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1     296.7     7.304    2.4%
    2     468.9     20.34    4.3%
    3       610      2.19    0.3%
    4     772.2     3.682    0.4%
    5     931.6     2.819    0.3%

Quality and confidence:
param     error
r         1.531

Model:
Time ~=      144
    + r    157.2
              µs

Reads = 2 + (2 * r)
Writes = 1 + (1 * r)
Pallet: "pallet_delegation", Extrinsic: "add_delegation", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    331.9
              µs

Reads = 4
Writes = 2
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    331.9
              µs

Reads = 4
Writes = 2
Pallet: "pallet_delegation", Extrinsic: "revoke_delegation_root_child", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=     49.5
    + r      162
              µs

Reads = 0 + (2 * r)
Writes = 0 + (1 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1     208.7     1.208    0.5%
    2     375.8     1.946    0.5%
    3     541.7     6.218    1.1%
    4       703     10.32    1.4%
    5     857.1       2.1    0.2%

Quality and confidence:
param     error
r         0.897

Model:
Time ~=    50.12
    + r    162.3
              µs

Reads = 0 + (2 * r)
Writes = 0 + (1 * r)
Pallet: "pallet_delegation", Extrinsic: "revoke_delegation_leaf", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    190.7
    + r    58.02
              µs

Reads = 2 + (1 * r)
Writes = 1 + (0 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1       246     2.276    0.9%
    2     308.1     2.183    0.7%
    3     366.9     1.961    0.5%
    4     423.3      6.75    1.5%
    5     479.1     3.755    0.7%

Quality and confidence:
param     error
r         0.567

Model:
Time ~=    190.3
    + r    58.12
              µs

Reads = 2 + (1 * r)

```

## 2. Pallet Mark - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mark --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mark/src/weights.rs 
```

```
Pallet: "pallet_mark", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    224.6
              µs

Reads = 5
Writes = 2
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    224.6
              µs

Reads = 5
Writes = 2

```

## Pallet DID - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_did --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/did/src/weights.rs 

```
## Cord benchmarking outputs -


## 1. Pallet Mtype - 

```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mtype --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mtype/src/weights.rs --template=./.maintain/weight-template.hbs
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
 ./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_delegation --extrinsic='*' --steps=20  --output=./pallets/delegation/src/weights.rs --template=./.maintain/weight-template.hbs

```

```
Pallet: "pallet_delegation", Extrinsic: "create_root", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    119.2
              µs

Reads = 2
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    119.2
              µs

Reads = 2
Writes = 1
Pallet: "pallet_delegation", Extrinsic: "revoke_root", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=      137
    + r    158.1
              µs

Reads = 2 + (2 * r)
Writes = 1 + (1 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1     316.2         0    0.0%
    2     448.9         0    0.0%
    3     611.5         0    0.0%
    4     769.3         0    0.0%
    5     927.7         0    0.0%

Quality and confidence:
param     error
r         2.969

Model:
Time ~=    151.7
    + r    154.3
              µs

Reads = 2 + (2 * r)
Writes = 1 + (1 * r)
Pallet: "pallet_delegation", Extrinsic: "add_delegation", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    330.5
              µs

Reads = 4
Writes = 2
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    330.5
              µs

Reads = 4
Writes = 2
Pallet: "pallet_delegation", Extrinsic: "revoke_delegation_root_child", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    53.14
    + r    159.9
              µs

Reads = 0 + (2 * r)
Writes = 0 + (1 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1     208.4         0    0.0%
    2     374.4         0    0.0%
    3     534.3         0    0.0%
    4     692.9         0    0.0%
    5       851         0    0.0%

Quality and confidence:
param     error
r         0.894

Model:
Time ~=    51.15
    + r    160.3
              µs

Reads = 0 + (2 * r)
Writes = 0 + (1 * r)
Pallet: "pallet_delegation", Extrinsic: "revoke_delegation_leaf", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    186.8
    + r    57.27
              µs

Reads = 2 + (1 * r)
Writes = 1 + (0 * r)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    r   mean µs  sigma µs       %
    1     241.7         0    0.0%
    2       536         0    0.0%
    3     703.8         0    0.0%
    4     415.9         0    0.0%
    5     470.7         0    0.0%

Quality and confidence:
param     error
r         58.48

Model:
Time ~=    372.2
    + r     33.8
              µs

Reads = 2 + (1 * r)
Writes = 1 + (0 * r)

```

## 3. Pallet Mark - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mark --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mark/src/weights.rs --template=./.maintain/weight-template.hbs
```

```
Compute::OnChain for era 0    
Pallet: "pallet_mark", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    237.2
              µs

Reads = 5
Writes = 2
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    237.2
              µs

Reads = 5
Writes = 2
Pallet: "pallet_mark", Extrinsic: "revoke", Lowest values: [], Highest values: [], Steps: [20], Repeat: 1
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    158.7
    + d    50.05
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    d   mean µs  sigma µs       %
    1     208.7         0    0.0%
    2     265.7         0    0.0%
    3     310.7         0    0.0%
    4     358.7         0    0.0%
    5     408.8         0    0.0%
    6       459         0    0.0%
    7     508.8         0    0.0%
    8     559.9         0    0.0%
    9     609.2         0    0.0%
   10       664         0    0.0%

Quality and confidence:
param     error
d         0.289

Model:
Time ~=    160.4
    + d    49.99
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)


```

## Pallet DID - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_did --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/did/src/weights.rs --template=./.maintain/weight-template.hbs

```
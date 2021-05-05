# Cord benchmarking outputs -

## How to use this file - 
File has been divided into  - pallets name -> Each pallet consists of two parts. 
```
1. Command to generate weights file.
2. Output from that command.
```
If you want to generate the weights of specific pallets, copy the command of the respective pallet and run at the root of repository.


## 1. Pallet Mtype - 

```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mtype --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mtype/src/weights.rs --template=./.maintain/weight-template.hbs
```

```
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
Pallet: "pallet_mark", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    311.2
              µs

Reads = 5
Writes = 2
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    311.2
              µs

Reads = 5
Writes = 2
Pallet: "pallet_mark", Extrinsic: "revoke", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    224.7
    + d    44.09
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    d   mean µs  sigma µs       %
    1     272.8     3.627    1.3%
    2     301.3     28.33    9.4%
    3     384.3     17.25    4.4%
    4     416.1     38.66    9.2%
    5     486.6     28.78    5.9%
    6     462.4     1.276    0.2%
    7     519.3     4.956    0.9%
    8     566.5     2.066    0.3%
    9       618     1.337    0.2%
   10     668.7     2.026    0.3%

Quality and confidence:
param     error
d         1.213

Model:
Time ~=    237.1
    + d    42.28
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)
Pallet: "pallet_mark", Extrinsic: "restore", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    165.4
    + d    50.48
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    d   mean µs  sigma µs       %
    1     256.9     8.585    3.3%
    2       271     10.89    4.0%
    3     339.2     22.72    6.6%
    4     361.2     1.108    0.3%
    5     409.5     0.471    0.1%
    6     541.9     46.95    8.6%
    7     522.5     11.16    2.1%
    8     563.9     2.375    0.4%
    9     634.4     18.41    2.9%
   10     698.6     26.56    3.8%

Quality and confidence:
param     error
d         1.448

Model:
Time ~=    184.6
    + d    50.05
              µs

Reads = 2 + (1 * d)
Writes = 1 + (0 * d)



```

## 4. Pallet DID - 
```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_did --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/did/src/weights.rs --template=./.maintain/weight-template.hbs

```

```
Pallet: "pallet_did", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    341.1
              µs

Reads = 0
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    341.1
              µs

Reads = 0
Writes = 1
Pallet: "pallet_did", Extrinsic: "remove", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    274.5
              µs

Reads = 0
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=    274.5
              µs

Reads = 0
Writes = 1


```

## 5. Pallet-Digest - 

```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_digest --extrinsic='*' --steps=20  --repeat=10 --output=./pallets/digest/src/weights.rs --template=./.maintain/weight-template.hbs
```

```
Pallet: "pallet_digest", Extrinsic: "anchor", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=      140
              µs

Reads = 2
Writes = 1
Min Squares Analysis
========
-- Extrinsic Time --

Model:
Time ~=      140
              µs

Reads = 2
Writes = 1
Pallet: "pallet_digest", Extrinsic: "revoke", Lowest values: [], Highest values: [], Steps: [20], Repeat: 10
Median Slopes Analysis
========
-- Extrinsic Time --

Model:
Time ~=    122.8
    + d        0
              µs

Reads = 1 + (0 * d)
Writes = 1 + (0 * d)
Min Squares Analysis
========
-- Extrinsic Time --

Data points distribution:
    d   mean µs  sigma µs       %
    1     121.9     0.413    0.3%
    2     122.4     0.441    0.3%
    3     120.5      1.64    1.3%
    4     115.9     1.136    0.9%
    5     113.4     1.415    1.2%
    6     111.2     0.291    0.2%
    7     111.1     0.199    0.1%
    8       111     0.275    0.2%
    9     110.9     0.246    0.2%
   10     111.2     0.278    0.2%

Quality and confidence:
param     error
d         0.096

Model:
Time ~=      123
    + d        0
              µs

Reads = 1 + (0 * d)
Writes = 1 + (0 * d)


```
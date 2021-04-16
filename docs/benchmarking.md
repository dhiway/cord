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
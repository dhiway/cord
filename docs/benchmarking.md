# CORD benchmarking outputs -

## How to use this file -

File has been divided into - pallets name -> Each pallet consists of two parts.

```
1. Command to generate weights file.
2. Output from that command.
```

If you want to generate the weights of specific pallets, copy the command of the respective pallet and run at the root of repository.

## Compile cord with benchmark flag -

```
1. cd node/
2. Run->   `cargo build --release --features runtime-benchmarks`
```

## 1. Pallet mark -

```
./target/release/cord benchmark --chain=dev --execution=wasm --pallet=pallet_mark --extrinsic='*' --steps=20 --repeat=10 --output=./pallets/mark/src/weights.rs --template=./.maintain/weight-template.hbs
```

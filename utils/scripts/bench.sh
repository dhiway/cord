#! /bin/bash

# create node binary with benchmarks enabled
# run inside the node directory
# cargo build --release --features=runtime-benchmarks
# this binary should only be used for benchmarking.

pallets=$(./target/release/cord benchmark --chain dev --pallet "*" --extrinsic "*" --repeat 0 |  cut -d "\"" -f 2 | uniq)

for pallet in $pallets
do 
    echo "Benchmarking ${pallet}\n"
    
    case $pallet in
    pallet_did)
        weightOut="./pallets/did/src/weights.rs"
        ;;
    pallet_mtype)
        weightOut="./pallets/mtype/src/weights.rs"
        ;;
    pallet_mark)
        weightOut="./pallets/mark/src/weights.rs"
        ;;
    pallet_delegation)
        weightOut="./pallets/delegation/src/weights.rs"
        ;;
    pallet_digest)
        weightOut="./pallets/digest/src/weights.rs"
        ;;                
    *)
        weightOut="./runtime/src/weights/"
        ;;    
    esac

    ./target/release/cord benchmark \
    --chain=dev \
    --steps=3 \
    --repeat=2 \
    --pallet=${pallet} \
    --extrinsic=* \
    --execution=Wasm \
    --wasm-execution=Interpreted \
    --heap-pages=4096 \
    --output=${weightOut} \
    --template=./.maintain/weight-template.hbs
done



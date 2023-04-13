# More on CORD Chain

In this document, lets see how to connect with the [running cord staging network](https://staging.cord.network) chain.

This document assumes you have `docker` installed.

## Download the latest cord container image (or you can choose to build locally)

```sh
docker pull dhiway/cord:develop
```

## Generate `node-key`

In a blockchain, every participating node should be uniquely identified. Hence, one need to generate a new key for the node, which needn't be shared with anyone.

```sh
docker run --rm --volume $(pwd):/tmp parity/subkey:2.0.0 generate-node-key --file /tmp/node-key
```

## Start

Assuming `/data` is the partition where chain data would reside, and one is running the command from the current directory, run below command.

```sh
docker run --detach --name cord -v /data:/data -v $(pwd):/home/ubuntu --network host dhiway/cord:develop --base-path /data --chain=/home/ubuntu/chainSpec-staging-0.7.7.json --node-key-file /home/ubuntu/node-key --prometheus-external --unsafe-rpc-external --unsafe-ws-external --rpc-methods Safe --rpc-cors all --pruning 1024 --bootnodes /ip4/3.110.229.240/tcp/30333/ws/p2p/12D3KooWLT35hGPSBx29vnkTGFRT9pLdeNV2f6Ggtbky2kJZuLjo
```

NOTE: currently bootnode address is same as above. But it can change, and there can be more bootnodes available for the same chain. Once you start this, you can see that your node will start discovering the nodes in the network, and will start participating as a 'FULL' node. To become 'validator' connect with Dhiway team.


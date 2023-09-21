# More on CORD Chain

In this document, let's see how to connect with the [running cord staging network](https://staging.cord.network) chain.

This document assumes you have `docker` installed.

## Download the latest cord container image (or you can choose to build locally)

```sh
docker pull dhiway/cord
```

## Create a Volume to store CORD data

```sh
docker volume create cord
```

## Generate `node.key`

In a blockchain, every participating node should be uniquely identified. Hence, one needs to generate a new key for the node, which needn't be shared with anyone.

```sh
docker run --rm --mount source=cord,target=/data dhiway/cord key generate-node-key --file /data/node.key
```

## Start

Assuming the chain data would reside in the `cord` volume, start cord by running the below command:

```sh
docker run --detach --restart unless-stopped --mount source=cord,target=/data --name cord dhiway/cord --base-path /data/ --chain spark --node-key-file /data/node.key --port 30333 --rpc-port 9933 --rpc-methods=Safe --rpc-cors all --state-pruning 100 --blocks-pruning 100 --prometheus-external --prometheus-port 9615 --bootnodes /ip4/34.131.139.143/tcp/30333/ws/p2p/12D3KooWDUdBdGbjEoPw6Wk4N1MQCRNV1sDfGU7EPipjYt8hMyKM /ip4/34.100.197.57/tcp/30333/ws/p2p/12D3KooWFJWcacayRNpEbGqsSzSyD2tChJ4PTEV14etpLVzrqeWU 
```

NOTE: currently bootnode address is same as above. But it can change, and there can be more bootnodes available for the same chain. Once you start this, you can see that your node will start discovering the nodes in the network, and will start participating as a 'FULL' node. To become 'validator' connect with Dhiway team.

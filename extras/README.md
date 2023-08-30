# More on CORD Chain

In this document, let's see how to connect with the [running cord staging network](https://staging.cord.network) chain.

This document assumes you have `docker` installed.

## Download the latest cord container image (or you can choose to build locally)

```sh
docker pull dhiway/cord
```

## Generate `node.key`

In a blockchain, every participating node should be uniquely identified. Hence, one needs to generate a new key for the node, which needn't be shared with anyone.
```sh
sudo mkdir /tmp/cord && sudo chown $USER /tmp/cord && sudo chmod 777 /tmp/cord
```

```sh
docker run --rm -v /tmp/cord:/cord dhiway/cord key generate-node-key --file /cord/node.key
```

## Start

Assuming `/tmp/cord` is the partition where chain data would reside, and one is running the command from the current directory, run below command.

```sh
docker run --detach --restart unless-stopped -v /tmp/cord:/cord --name cord dhiway/cord --base-path /cord/ --chain spark --node-key-file /cord/node.key --port 30333 --rpc-port 9933 --prometheus-port 9615 --rpc-methods=Safe --rpc-cors all --state-pruning 100 --blocks-pruning 100 --prometheus-external --bootnodes /ip4/34.131.139.143/tcp/30333/ws/p2p/12D3KooWDUdBdGbjEoPw6Wk4N1MQCRNV1sDfGU7EPipjYt8hMyKM /ip4/34.100.197.57/tcp/30333/ws/p2p/12D3KooWFJWcacayRNpEbGqsSzSyD2tChJ4PTEV14etpLVzrqeWU 
```

NOTE: currently bootnode address is same as above. But it can change, and there can be more bootnodes available for the same chain. Once you start this, you can see that your node will start discovering the nodes in the network, and will start participating as a 'FULL' node. To become 'validator' connect with Dhiway team.

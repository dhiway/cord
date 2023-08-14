## Keys for CORD

CORD is configured to work with default keys in `--dev` mode. When you are connected to test-net, ask for transfer of tokens from Dhiway.

## Default Keys

//Alice, //Bob etc are support when you run the CORD node in `--dev` mode (ie, in a standalone mode).

### User Accounts

## Session Keys
The keys are generated using the [prep_node_keys](scripts/prep_node_keys.sh) script with the help of a secret. 

### Babe and Grandpa Keys - Dev Node

### Chain Spec Entries - Dev Node
The babe and grandpa keys need to be inserted to each nodes through the polka UI (Developer- RPC calls) or via `curl`. Choose "author" and "insertKey". The fields can be filled like this:

#### BABE Session Key

`keytype: babe`

#### Grandpa Session Key
`keytype: gran`

## Creating New Keys

``` bash
subkey generate --scheme sr25519
```

#### Export the secret key

```bash
export SECRET="<Hex key>"
```
Run the [Script](scripts/prep_node_keys.sh) with number of validators
``` bash 
./prep_node_keys.sh <no. of nodes>

eg: ./prep_node_keys.sh 3 //for testnet
    ./prep_node_keys.sh 1 //for local dev node
```

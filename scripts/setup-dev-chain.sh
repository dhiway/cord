#!/bin/bash

SCRIPTDIR=$(dirname "$0")
echo $SCRIPTDIR

NODE_SEED="0x37531f33c12c9a344e982b5919b53ab3b093f39a515cbd7a1ffe21d7673c9a89" $SCRIPTDIR/prep_node_keys.sh 1

curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-aura.json"
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-gran.json"

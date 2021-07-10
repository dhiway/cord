#/*
# * This file is part of the CORD
# * Copyright (C) 2020-21  Dhiway
# *
# */

# Generating a new NODE_SEED
# use the CMD:- subkey generate -n cord --words 24
# Secret seed of this account is the node seed

#!/bin/bash

SCRIPTDIR=$(dirname "$0")
echo $SCRIPTDIR

NODE_SEED="0x37531f33c12c9a344e982b5919b53ab3b093f39a515cbd7a1ffe21d7673c9a89" $SCRIPTDIR/prep_node_keys.sh 1

curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-babe.json"
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-gran.json"

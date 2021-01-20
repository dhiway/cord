#/*
# * This file is part of the CORD
# * Copyright (C) 2021  Dhiway
# *
# */

#!/usr/bin/env bash

set -e

if [ "$#" -ne 1 ]; then
	echo "Either you haven't provided peerID or provided more that one"
	exit 1
fi

echo -n $1 | bs58 -d | xxd -p -c 80

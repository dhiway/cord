#
#/ Copyright 2019-2021 Dhiway.
# This file is part of CORD Platform.
# 

# Generating a new NODE_SEED
# use the CMD - subkey generate -n cord --words 24
# Secret seed of this account is the node seed

#!/usr/bin/env bash
set -e

if [ "$#" -ne 1 ]; then
	echo "Please provide the number of initial validators!"
	exit 1
fi


if [ -z "$NODE_SEED" ]; then
	echo "NODE_SEED Empty!"
	exit 1
fi

generate_account_id() {
	./target/release/cord key inspect -n cord ${3:-} ${4:-} "$NODE_SEED//$1//$2" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	./target/release/cord key inspect -n cord ${3:-} ${4:-} "$NODE_SEED//$1//$2" | grep "SS58 Address" | awk '{ print $3 }'
}


generate_address_and_account_id() {
	ACCOUNT=$(generate_account_id $1 $2 $3)
  	ADDRESS=$(generate_address $1 $2 $3)
	
  	if ${4:-false}; then
		INTO="unchecked_into"

	else
		INTO="into"
	fi

	printf "//$ADDRESS\nhex![\"${ACCOUNT#'0x'}\"].$INTO(),"
}

V_NUM=$1
DETAILS=""
AUTHORITIES="\nInitial Authorities \n"
AUTHORITIES_RPC="\nInitial Authorities (RPC) \n"
AUTHORITY_ACCOUNTS="\nInitial Authorities (Controller Accounts) (\n"

for i in $(seq 1 $V_NUM); do
	AUTHORITY_ACCOUNTS+="$(generate_address_and_account_id $i controller)\n"
	
	AUTHORITIES+="(\n"
	AUTHORITIES+="$(generate_address_and_account_id $i stash)\n"
	AUTHORITIES+="$(generate_address_and_account_id $i controller)\n"
	AUTHORITIES+="$(generate_address_and_account_id $i grandpa '--scheme ed25519' true)\n"
	AUTHORITIES+="$(generate_address_and_account_id $i aura '--scheme sr25519' true)\n"

	AUTHORITIES+="),\n"

	AUTHORITIES_RPC+="//$(generate_address $i controller) (\n"
	AUTHORITIES_RPC+="key type: aura\n"
	AUTHORITIES_RPC+="suri: $NODE_SEED//$i//aura\n"
	AUTHORITIES_RPC+="public key: $(generate_account_id $i aura '--scheme sr25519')\n"
	AUTHORITIES_RPC+="key type: gran\n"
	AUTHORITIES_RPC+="suri: $NODE_SEED//$i//grandpa\n"
	AUTHORITIES_RPC+="public key: $(generate_account_id $i grandpa '--scheme ed25519')\n"
	AUTHORITIES_RPC+="),\n"
done

AUTHORITY_ACCOUNTS+="),\n"

printf "$AUTHORITIES"
printf "$AUTHORITY_ACCOUNTS"
printf "$AUTHORITIES_RPC"

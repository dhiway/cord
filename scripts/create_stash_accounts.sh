/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

#!/usr/bin/env bash
set -e

if [ "$#" -ne 1 ]; then
	echo "Please provide the number of stash accounts!"
	exit 1
fi

generate_account_id() {
	subkey inspect -n cord ${3:-} ${4:-} "$STASH_SEED//$1//$2" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	subkey inspect -n cord ${3:-} ${4:-} "$STASH_SEED//$1//$2" | grep "SS58 Address" | awk '{ print $3 }'
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
ENDOWED_ACCOUNTS+="\nEndowed Accounts (Stash) (\n"
ENDOWED_SEED+="\nEndowed Seeds (Stash) (\n"

for i in $(seq 1 $V_NUM); do
	ENDOWED_ACCOUNTS+="$(generate_address_and_account_id $i stash)\n"
done
ENDOWED_ACCOUNTS+=")\n"

printf "$ENDOWED_ACCOUNTS"

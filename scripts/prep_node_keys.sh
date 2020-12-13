#/*
# * This file is part of the CORD
# * Copyright (C) 2020  Dhiway
# *
# */

#!/usr/bin/env bash
set -e

if [ "$#" -ne 1 ]; then
	echo "Please provide the number of initial validators!"
	exit 1
fi

generate_account_id() {
	subkey inspect ${3:-} ${4:-} "$SECRET//$1//$2" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	subkey inspect ${3:-} ${4:-} "$SECRET//$1//$2" | grep "SS58 Address" | awk '{ print $3 }'
   # printf "$SECRET//$1//$2\n"
   # printf "Secret - $SECRET \n"
}

generate_address_detail() {
	subkey inspect  ${3:-} ${4:-} "$SECRET//$1//$2"
}

generate_address_details() {
	generate_address_detail $1 $2 $3
    # DETAILS=$(generate_address_detail $1 $2 $3)

}

generate_address_and_account_id() {
	ACCOUNT=$(generate_account_id $1 $2 $3)
    #printf "one-$1, two- $2, three- $3\n"
	ADDRESS=$(generate_address $1 $2 $3)
   # printf DETAILS
    #printf "one-$1, two- $2, three- $3\n"
	if ${4:-false}; then
		INTO="unchecked_into"
	else
		INTO="into"
	fi

	printf "//$ADDRESS\nhex![\"${ACCOUNT#'0x'}\"].$INTO(),"
}

V_NUM=$1
DETAILS=""
AUTHORITIES=""

for i in $(seq 1 $V_NUM); do
	AUTHORITIES+="(\n"
	AUTHORITIES+="$(generate_address_and_account_id $i controller)\n"
	AUTHORITIES+="$(generate_address_and_account_id $i aura '--scheme sr25519' true)\n"
	AUTHORITIES+="$(generate_address_and_account_id $i grandpa '--scheme ed25519' true)\n"
	AUTHORITIES+="),\n"
done

for i in $(seq 1 $V_NUM); do
    DETAILS+="\nAccount Details (\n"
    DETAILS+="$(generate_address_details $i controller)\n\n"
    DETAILS+="$(generate_address_details $i aura '--scheme sr25519' true)\n\n"
    DETAILS+="$(generate_address_details $i grandpa '--scheme ed25519' true)\n"
    DETAILS+="),\n"
done

printf "$AUTHORITIES"
printf "$DETAILS"
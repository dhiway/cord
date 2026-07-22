# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

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

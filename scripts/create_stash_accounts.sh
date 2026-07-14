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
	echo "Please provide the number of stash accounts!"
	exit 1
fi

if [ -z "$STASH_SEED" ]; then
	echo "STASH_SEED Empty!"
	exit 1
fi

generate_account_id() {
	printf "$STASH_SEED//$1//$2"
	./target/release/cord key inspect -n cord ${3:-} ${4:-} "$STASH_SEED//$1//$2" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	./target/release/cord key inspect -n cord ${3:-} ${4:-} "$STASH_SEED//$1//$2" | grep "SS58 Address" | awk '{ print $3 }'
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

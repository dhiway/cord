#!/usr/bin/env python3
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

"""Give the disposable proof-retention network its own protocol namespace.

Zombienet invokes this program as a chain-spec post-processor.  The plain
chain spec is read from stdin and the updated spec is written to stdout.
Genesis isolation itself comes from the topology's unique validator and
collator names: Zombienet derives both authority/session keys and libp2p node
keys from those names before invoking this script.
"""

from __future__ import annotations

import json
import sys


RELAY_ID = "origin_proof_isolated"
RELAY_PROTOCOL = "origin-proof-retention-v1"
ORBIS_ID = "orbis-proof-isolated"
ORBIS_PROTOCOL = "orbis-proof-retention-v1"


def main() -> int:
	spec = json.load(sys.stdin)
	if "para_id" in spec:
		spec.update(
			{
				"name": "Orbis Proof Retention Isolated",
				"id": ORBIS_ID,
				"protocolId": ORBIS_PROTOCOL,
				"forkId": ORBIS_PROTOCOL,
				"relay_chain": RELAY_ID,
			}
		)
	else:
		spec.update(
			{
				"name": "Origin Proof Retention Isolated",
				"id": RELAY_ID,
				"protocolId": RELAY_PROTOCOL,
				"forkId": RELAY_PROTOCOL,
			}
		)

	json.dump(spec, sys.stdout, separators=(",", ":"))
	return 0


if __name__ == "__main__":
	raise SystemExit(main())

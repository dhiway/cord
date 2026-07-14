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

SCRIPTDIR=$(dirname "$0")
echo $SCRIPTDIR

NODE_SEED="0x37531f33c12c9a344e982b5919b53ab3b093f39a515cbd7a1ffe21d7673c9a89" $SCRIPTDIR/prep_node_keys.sh 1

curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-babe.json"
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d "@config/author-key-gran.json"

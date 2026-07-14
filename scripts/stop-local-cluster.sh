#!/bin/bash
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

echo "Stopping all nodes..."
pkill -f "cord --base-path /tmp/cord-data/"
echo "All nodes stopped."
echo "Deleting /tmp/cord-data/ directory..."
rm -rf /tmp/cord-data/
echo "/tmp/cord-data/ directory deleted."
echo ""
echo -e "Commercial Support Services on CORD are offered by Dhiway \033[0;34m(sales@dhiway.com)\033[0m "
echo -e "CORD team recommends having a separate chain in production, because \033[0;34mlocal\033[0m chain uses the default keys, which are common."

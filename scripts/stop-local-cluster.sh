#!/bin/bash

echo "Stopping all nodes..."
pkill -f "cord --base-path /tmp/cord-data/"
echo "All nodes stopped."
echo "Deleting /tmp/cord-data/ directory..."
rm -rf /tmp/cord-data/
echo "/tmp/cord-data/ directory deleted."
echo ""
echo -e "Commercial Support Services on CORD are offered by Dhiway \033[0;34m(sales@dhiway.com)\033[0m "
echo -e "CORD team recommends having a separate chain in production, because \033[0;34mlocal\033[0m chain uses the default keys, which are common."

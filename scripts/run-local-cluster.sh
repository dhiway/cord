#!/bin/bash

CORD_BINARY=./target/release/cord
ALICE_NODE_CMD="${CORD_BINARY} --base-path /tmp/cord-data/alice --validator --chain local --alice --port 30333 --rpc-port 9933 --prometheus-port 9615 --node-key 0000000000000000000000000000000000000000000000000000000000000001 --rpc-methods=Safe --rpc-cors all --prometheus-external "

BOB_NODE_CMD="${CORD_BINARY} --base-path /tmp/cord-data/bob --validator --chain local --bob --port 30334 --rpc-port 9934 --rpc-methods=Safe --rpc-cors all --prometheus-external --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

CHARLIE_NODE_CMD="${CORD_BINARY} --base-path /tmp/cord-data/charlie --validator --chain local --charlie --port 30335 --rpc-port 9935 --rpc-methods=Safe --rpc-cors all --prometheus-external --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

DAVE_NODE_CMD="${CORD_BINARY} --base-path /tmp/cord-data/dave --chain local --dave --port 30336 --rpc-port 9936 --rpc-methods=Safe --rpc-cors all --prometheus-external --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# Logs
LOG_DIR=/tmp/cord-logs
ALICE_LOG=${LOG_DIR}/alice.log
BOB_LOG=${LOG_DIR}/bob.log
CHARLIE_LOG=${LOG_DIR}/charlie.log
DAVE_LOG=${LOG_DIR}/dave.log

echo -e "Creating log directory in \\033[0;34m${LOG_DIR}\\033[0m"
mkdir -p ${LOG_DIR}
touch ${ALICE_LOG}
touch ${BOB_LOG}
touch ${CHARLIE_LOG}
touch ${DAVE_LOG}
chmod 666 ${ALICE_LOG} ${BOB_LOG} ${CHARLIE_LOG} ${DAVE_LOG}
echo "Starting all nodes in the background..."
echo -n -e "1..\033[0K\r"
sleep 1
echo -n -e "1....\033[0K\r"
$ALICE_NODE_CMD > ${ALICE_LOG} 2>&1 &
sleep 1
echo -n -e "1..\033[0K\r"
sleep 1
echo -n -e "1....\033[0K\r"
sleep 1
echo -n -e "2..\033[0K\r"
$BOB_NODE_CMD > ${BOB_LOG} 2>&1 &
sleep 1
echo -n -e "2....\033[0K\r"
sleep 1
echo -n -e "3..\033[0K\r"
$CHARLIE_NODE_CMD > ${CHARLIE_LOG} 2>&1 &
sleep 2
echo -n -e "4..\033[0K\r"
$DAVE_NODE_CMD > ${DAVE_LOG} 2>&1 &
sleep 1
echo
echo -e "Four CORD nodes {Alice, Bob, Charlie, Dave} have been successfully started."
echo -e "See them in \033[0;34mhttps://telemetry.cord.network\033[0m under Cord Spin tab."
echo -e "You can also watch this network details in \033[0;34mhttps://apps.cord.network/?rpc=ws://localhost:9933\033[0m "
echo ""
echo "To view the logs, you can use the following commands:"
echo "Alice: tail -f ${ALICE_LOG}"
echo "Bob: tail -f ${BOB_LOG}"
echo "Charlie: tail -f ${CHARLIE_LOG}"
echo "Dave: tail -f ${DAVE_LOG}"
echo ""
echo -e "To stop all running nodes run: \033[0;34mbash ./scripts/stop-local-cluster.sh\033[0m"


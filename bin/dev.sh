#!/bin/bash

# Development script for running JAX nodes in tmux with watch mode
# This sets up a tmux session with three panes demonstrating different configurations:
#   - App only (full UI + API, no gateway)
#   - App + Gateway (full UI + API + gateway on separate port)
#   - Gateway only (minimal content serving, no UI/API)

set -e

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Change to project root
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Setting up JAX development environment...${NC}"

# Initialize nodes if they haven't been initialized yet
init_node() {
    local node_dir=$1
    local node_name=$2
    local app_port=$3
    local peer_port=$4
    local gateway_port=$5

    if [ ! -d "$node_dir" ]; then
        echo -e "${YELLOW}Initializing $node_name (first run)...${NC}"
        cargo run --bin jax -- --config-path "$node_dir" init \
            --app-port "$app_port" \
            --peer-port "$peer_port" \
            --gateway-port "$gateway_port"
        echo -e "${GREEN}$node_name initialized with APP:$app_port PEER:$peer_port GW:$gateway_port${NC}"
    else
        echo -e "${GREEN}$node_name already initialized${NC}"
    fi
}

# Initialize all three nodes with distinct ports
# Node1: App only (gateway port configured but not used)
init_node "./data/node1" "Node1 (app)" 8080 9000 9090
# Node2: App + Gateway
init_node "./data/node2" "Node2 (app+gw)" 8081 9001 9091
# Node3: Gateway only
init_node "./data/node3" "Node3 (gw-only)" 8082 9002 9092

# Check if tmux session already exists
if tmux has-session -t jax-dev 2>/dev/null; then
    echo -e "${BLUE}Killing existing jax-dev tmux session...${NC}"
    tmux kill-session -t jax-dev
fi

# Check if cargo-watch is installed
if ! command -v cargo-watch &>/dev/null; then
    echo -e "${BLUE}cargo-watch not found, installing...${NC}"
    cargo install cargo-watch
fi

echo -e "${GREEN}Starting tmux session 'jax-dev'...${NC}"

# Create new tmux session with nodes window
tmux new-session -d -s jax-dev -n "nodes"

# Split into 3 panes (top-left, top-right, bottom)
tmux split-window -h -t jax-dev:0
tmux split-window -v -t jax-dev:0.0

# Pane 0.0 (top-left): App only
tmux send-keys -t jax-dev:0.0 "cd $PROJECT_ROOT && echo '=== Node1: App Only ===' && echo 'App: http://localhost:8080 (UI + API)' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node1 daemon'" C-m

# Pane 0.1 (top-right): App + Gateway (uses --with-gateway to enable both)
tmux send-keys -t jax-dev:0.1 "cd $PROJECT_ROOT && echo '=== Node2: App + Gateway ===' && echo 'App: http://localhost:8081 (UI + API) | GW: http://localhost:9091' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node2 daemon --with-gateway --gateway-url http://localhost:9091'" C-m

# Pane 0.2 (bottom): Gateway only
tmux send-keys -t jax-dev:0.2 "cd $PROJECT_ROOT && echo '=== Node3: Gateway Only ===' && echo 'GW: http://localhost:9092' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node3 daemon --gateway'" C-m

# Create a new window for database inspection
tmux new-window -t jax-dev:1 -n "db"
tmux send-keys -t jax-dev:1 "cd $PROJECT_ROOT && echo 'Node1 DB: ./data/node1/db.sqlite' && echo 'Node2 DB: ./data/node2/db.sqlite' && echo 'Node3 DB: ./data/node3/db.sqlite' && echo '' && echo 'Use: ./bin/db.sh node1 or ./bin/db.sh node2 or ./bin/db.sh node3'" C-m

# Create a new window for API testing
tmux new-window -t jax-dev:2 -n "info"
tmux send-keys -t jax-dev:2 "cd $PROJECT_ROOT && cat << 'EOF'
JAX Development Environment
============================

Node Configurations:
--------------------
Node1 - App Only:
  App:  http://localhost:8080 (UI + API on same port)
  Gateway: (not enabled)
  Use case: Standard daemon without gateway

Node2 - App + Gateway:
  App:  http://localhost:8081 (UI + API on same port)
  Gateway: http://localhost:9091
  Use case: Full daemon with integrated gateway on separate port
  Share links in UI will point to http://localhost:9091

Node3 - Gateway Only:
  Gateway: http://localhost:9092
  Use case: Minimal read-only content serving (no UI, no API)
  Can mirror buckets from other nodes

Testing:
--------
1. Create a bucket on Node1 or Node2 via UI
2. Add files via the UI
3. Test share links:
   - Node1: No gateway (share links won't work for direct download)
   - Node2: Share links use http://localhost:9091/gw/...
4. Test gateway-only access on Node3:
   - Visit http://localhost:9092 to see identity page
   - Mirror a bucket from Node1/Node2
   - Access via http://localhost:9092/gw/{bucket_id}/path
5. Check identity endpoints:
   - curl http://localhost:9091/_status/identity
   - curl http://localhost:9092/_status/identity
6. Verify API works on same port as UI:
   - curl http://localhost:8080/api/v0/bucket/list
   - curl http://localhost:8080/buckets  # HTML
EOF" C-m

# Go back to first window
tmux select-window -t jax-dev:0

echo -e "${GREEN}Tmux session 'jax-dev' started!${NC}"
echo ""
echo "Usage:"
echo "  tmux attach -t jax-dev         # Attach to the session"
echo "  tmux kill-session -t jax-dev   # Kill the session"
echo ""
echo "Windows:"
echo "  0: nodes - Three node configurations"
echo "  1: db    - Database inspection"
echo "  2: info  - Configuration info and testing guide"
echo ""
echo "Node Configurations:"
echo "  Node1 (app):         App=8080 (UI+API)"
echo "  Node2 (app+gw):      App=8081 (UI+API) + GW=9091"
echo "  Node3 (gw-only):     GW=9092"
echo ""
echo -e "${BLUE}Attaching to session...${NC}"

# Attach to the session
tmux attach -t jax-dev

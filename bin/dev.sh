#!/bin/bash

# Development script for running JAX nodes in tmux with watch mode
# This sets up a tmux session with three panes demonstrating different configurations:
#   - App only (full UI + API, no gateway)
#   - App + Gateway (full UI + API + gateway on separate port)
#   - Gateway only (minimal content serving, no UI/API)
#
# New blob store commands:
#   ./bin/dev.sh blob-stores    - Run gateways with different blob store backends
#   ./bin/dev.sh minio          - Start MinIO container for S3 testing

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

# Default development setup with three node configurations
run_default() {
echo -e "${BLUE}Setting up JAX development environment...${NC}"

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
}

# Function to start MinIO container
start_minio() {
    echo -e "${BLUE}Starting MinIO container...${NC}"

    # Check if docker is available
    if ! command -v docker &>/dev/null; then
        echo -e "${YELLOW}Docker not found. Please install Docker to use MinIO.${NC}"
        exit 1
    fi

    # Stop existing MinIO container if running
    if docker ps -q -f name=jax-minio | grep -q .; then
        echo -e "${YELLOW}Stopping existing MinIO container...${NC}"
        docker stop jax-minio >/dev/null
    fi

    # Remove container if it exists
    if docker ps -aq -f name=jax-minio | grep -q .; then
        docker rm jax-minio >/dev/null
    fi

    # Create data directory
    mkdir -p ./data/minio

    # Start MinIO
    echo -e "${GREEN}Starting MinIO on http://localhost:9000${NC}"
    echo -e "${GREEN}Console: http://localhost:9001${NC}"
    echo -e "${GREEN}Credentials: minioadmin / minioadmin${NC}"
    docker run -d \
        --name jax-minio \
        -p 9000:9000 \
        -p 9001:9001 \
        -v "$(pwd)/data/minio:/data" \
        -e "MINIO_ROOT_USER=minioadmin" \
        -e "MINIO_ROOT_PASSWORD=minioadmin" \
        minio/minio server /data --console-address ":9001"

    # Wait for MinIO to start
    echo -e "${YELLOW}Waiting for MinIO to start...${NC}"
    sleep 3

    # Create the jax-blobs bucket if mc is available
    if command -v mc &>/dev/null; then
        mc alias set jax-minio http://localhost:9000 minioadmin minioadmin 2>/dev/null || true
        mc mb --ignore-existing jax-minio/jax-blobs 2>/dev/null || true
        echo -e "${GREEN}Created bucket: jax-blobs${NC}"
    else
        echo -e "${YELLOW}Install 'mc' (MinIO client) to auto-create buckets:${NC}"
        echo "  brew install minio/stable/mc"
        echo ""
        echo "Or create bucket manually via console: http://localhost:9001"
    fi

    echo -e "${GREEN}MinIO is running!${NC}"
}

# Function to run blob store demo
run_blob_stores_demo() {
    echo -e "${BLUE}Setting up blob store demo environment...${NC}"

    # Initialize nodes for blob store testing
    init_node "./data/blob-legacy" "Legacy" 8080 9010 9080
    init_node "./data/blob-filesystem" "Filesystem" 8081 9011 9081
    init_node "./data/blob-s3" "S3" 8082 9012 9082

    # Check if tmux session already exists
    if tmux has-session -t jax-blob-stores 2>/dev/null; then
        echo -e "${BLUE}Killing existing jax-blob-stores tmux session...${NC}"
        tmux kill-session -t jax-blob-stores
    fi

    echo -e "${GREEN}Starting tmux session 'jax-blob-stores'...${NC}"

    # Create new tmux session
    tmux new-session -d -s jax-blob-stores -n "gateways"

    # Split into 4 panes (2x2 grid)
    tmux split-window -h -t jax-blob-stores:0
    tmux split-window -v -t jax-blob-stores:0.0
    tmux split-window -v -t jax-blob-stores:0.1

    # Pane 0.0 (top-left): Legacy blob store
    tmux send-keys -t jax-blob-stores:0.0 "cd $PROJECT_ROOT && echo '=== Legacy Blob Store (iroh FsStore) ===' && echo 'Gateway: http://localhost:9080' && echo '' && RUST_LOG=info cargo run --bin jax -- --config-path ./data/blob-legacy daemon --gateway --blob-store legacy" C-m

    # Pane 0.1 (top-right): Filesystem blob store
    tmux send-keys -t jax-blob-stores:0.1 "cd $PROJECT_ROOT && echo '=== Filesystem Blob Store (SQLite + local) ===' && echo 'Gateway: http://localhost:9081' && echo '' && RUST_LOG=info cargo run --bin jax -- --config-path ./data/blob-filesystem daemon --gateway --blob-store filesystem" C-m

    # Pane 0.2 (bottom-left): S3 blob store
    tmux send-keys -t jax-blob-stores:0.2 "cd $PROJECT_ROOT && echo '=== S3 Blob Store (SQLite + MinIO) ===' && echo 'Gateway: http://localhost:9082' && echo 'Requires: ./bin/dev.sh minio' && echo '' && RUST_LOG=info cargo run --bin jax -- --config-path ./data/blob-s3 daemon --gateway --blob-store s3 --s3-endpoint http://localhost:9000 --s3-bucket jax-blobs --s3-access-key minioadmin --s3-secret-key minioadmin" C-m

    # Pane 0.3 (bottom-right): Info
    tmux send-keys -t jax-blob-stores:0.3 "cd $PROJECT_ROOT && cat << 'EOF'
Blob Store Demo
===============

Gateway Ports:
  Legacy (iroh):     http://localhost:9080
  Filesystem:        http://localhost:9081
  S3 (MinIO):        http://localhost:9082

For S3 mode, run MinIO first:
  ./bin/dev.sh minio

MinIO Console:
  http://localhost:9001
  Credentials: minioadmin / minioadmin

Testing:
  # Upload via each gateway
  curl -X POST http://localhost:9080/gw/upload -F 'file=@README.md'
  curl -X POST http://localhost:9081/gw/upload -F 'file=@README.md'
  curl -X POST http://localhost:9082/gw/upload -F 'file=@README.md'

  # Check S3 bucket contents
  mc ls jax-minio/jax-blobs
EOF" C-m

    echo -e "${GREEN}Tmux session 'jax-blob-stores' started!${NC}"
    echo ""
    echo "Usage:"
    echo "  tmux attach -t jax-blob-stores         # Attach to the session"
    echo "  tmux kill-session -t jax-blob-stores   # Kill the session"
    echo ""
    echo "Gateway Ports:"
    echo "  Legacy (iroh):     http://localhost:9080"
    echo "  Filesystem:        http://localhost:9081"
    echo "  S3 (MinIO):        http://localhost:9082"
    echo ""
    echo -e "${BLUE}Attaching to session...${NC}"

    tmux attach -t jax-blob-stores
}

# Main command handling
case "${1:-}" in
    minio)
        start_minio
        ;;
    blob-stores)
        run_blob_stores_demo
        ;;
    *)
        run_default
        ;;
esac

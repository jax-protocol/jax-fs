#!/bin/bash

# Development script for running JAX nodes in tmux with watch mode
#
# Usage:
#   ./bin/dev.sh          # Default: 3 nodes with legacy blob store
#   ./bin/dev.sh s3       # 3 nodes with S3 blob store (starts MinIO)
#   ./bin/dev.sh clean    # Remove all dev data and start fresh

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
RED='\033[0;31m'
NC='\033[0m' # No Color

# S3/MinIO configuration
S3_URL='s3://minioadmin:minioadmin@localhost:9000/jax-blobs'

# Initialize a node with specified blob store
init_node() {
    local node_dir=$1
    local node_name=$2
    local app_port=$3
    local peer_port=$4
    local gateway_port=$5
    local blob_store=${6:-legacy}
    local extra_args=${7:-}

    if [ ! -d "$node_dir" ]; then
        echo -e "${YELLOW}Initializing $node_name ($blob_store)...${NC}"
        cargo run --bin jax -- --config-path "$node_dir" init \
            --app-port "$app_port" \
            --peer-port "$peer_port" \
            --gateway-port "$gateway_port" \
            --blob-store "$blob_store" \
            $extra_args
        echo -e "${GREEN}$node_name initialized${NC}"
    else
        echo -e "${GREEN}$node_name already initialized${NC}"
    fi
}

# Ensure MinIO is running
ensure_minio() {
    echo -e "${BLUE}Checking MinIO...${NC}"

    if ! "$SCRIPT_DIR/minio.sh" status &>/dev/null; then
        echo -e "${YELLOW}Starting MinIO...${NC}"
        "$SCRIPT_DIR/minio.sh" up
    else
        echo -e "${GREEN}MinIO already running${NC}"
    fi
}

# Clean all dev data
clean() {
    echo -e "${YELLOW}Cleaning dev data...${NC}"
    rm -rf ./data/node1 ./data/node2 ./data/node3
    echo -e "${GREEN}Dev data cleaned${NC}"
}

# Setup and run tmux session
run_tmux() {
    local blob_store=$1

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

    echo -e "${GREEN}Starting tmux session 'jax-dev' (blob store: $blob_store)...${NC}"

    # Create new tmux session with nodes window
    tmux new-session -d -s jax-dev -n "nodes"

    # Split into 3 panes (top-left, top-right, bottom)
    tmux split-window -h -t jax-dev:0
    tmux split-window -v -t jax-dev:0.0

    # Pane 0.0 (top-left): App only
    tmux send-keys -t jax-dev:0.0 "cd $PROJECT_ROOT && echo '=== Node1: App Only ($blob_store) ===' && echo 'App: http://localhost:8080 (UI + API)' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node1 daemon'" C-m

    # Pane 0.1 (top-right): App + Gateway
    tmux send-keys -t jax-dev:0.1 "cd $PROJECT_ROOT && echo '=== Node2: App + Gateway ($blob_store) ===' && echo 'App: http://localhost:8081 (UI + API) | GW: http://localhost:9091' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node2 daemon --with-gateway --gateway-url http://localhost:9091'" C-m

    # Pane 0.2 (bottom): Gateway only
    tmux send-keys -t jax-dev:0.2 "cd $PROJECT_ROOT && echo '=== Node3: Gateway Only ($blob_store) ===' && echo 'GW: http://localhost:9092' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x 'run --bin jax -- --config-path ./data/node3 daemon --gateway'" C-m

    # Create info window
    tmux new-window -t jax-dev:1 -n "info"
    tmux send-keys -t jax-dev:1 "cd $PROJECT_ROOT && cat << 'EOF'
JAX Development Environment (blob store: $blob_store)
======================================================

Node Configurations:
--------------------
Node1 - App Only:      http://localhost:8080
Node2 - App + Gateway: http://localhost:8081 + http://localhost:9091
Node3 - Gateway Only:  http://localhost:9092

Quick Test:
-----------
1. Open http://localhost:8080 in browser
2. Create a bucket, add files
3. Check blob store is working

Commands:
---------
tmux kill-session -t jax-dev   # Stop everything
./bin/dev.sh clean             # Remove all data
./bin/dev.sh s3                # Restart with S3 backend
EOF" C-m

    # Go back to first window
    tmux select-window -t jax-dev:0

    echo ""
    echo -e "${GREEN}Started!${NC} Nodes using $blob_store blob store"
    echo ""
    echo "  Node1: http://localhost:8080 (app)"
    echo "  Node2: http://localhost:8081 (app) + http://localhost:9091 (gateway)"
    echo "  Node3: http://localhost:9092 (gateway only)"
    echo ""
    echo -e "${BLUE}Attaching to tmux session...${NC}"

    tmux attach -t jax-dev
}

# Run with legacy blob store (default)
run_legacy() {
    echo -e "${BLUE}Setting up JAX dev environment (legacy blob store)...${NC}"

    init_node "./data/node1" "Node1" 8080 9000 9090 legacy
    init_node "./data/node2" "Node2" 8081 9001 9091 legacy
    init_node "./data/node3" "Node3" 8082 9002 9092 legacy

    run_tmux "legacy"
}

# Run with S3 blob store
run_s3() {
    echo -e "${BLUE}Setting up JAX dev environment (S3 blob store)...${NC}"

    # Start MinIO
    ensure_minio

    # Initialize nodes with S3 backend
    init_node "./data/node1" "Node1" 8080 9000 9090 s3 "--s3-url '$S3_URL'"
    init_node "./data/node2" "Node2" 8081 9001 9091 s3 "--s3-url '$S3_URL'"
    init_node "./data/node3" "Node3" 8082 9002 9092 s3 "--s3-url '$S3_URL'"

    run_tmux "s3"
}

# Help
help() {
    echo "JAX Development Environment"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  (default)  Start 3 nodes with legacy blob store"
    echo "  s3         Start 3 nodes with S3 blob store (MinIO)"
    echo "  clean      Remove all dev data (./data/node*)"
    echo "  help       Show this help"
    echo ""
    echo "Examples:"
    echo "  $0         # Quick start with legacy blobs"
    echo "  $0 s3      # Test S3 integration with MinIO"
    echo "  $0 clean && $0 s3  # Fresh S3 setup"
}

# Main
case "${1:-}" in
    s3)
        run_s3
        ;;
    clean)
        clean
        ;;
    help|--help|-h)
        help
        ;;
    *)
        run_legacy
        ;;
esac

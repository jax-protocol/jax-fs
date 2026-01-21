#!/bin/bash
# Node management commands

TMUX_SESSION="jax-dev"

cmd_clean() {
    echo -e "${YELLOW}Cleaning dev data...${NC}"
    for node in $(get_node_names); do
        rm -rf "$DATA_DIR/$node"
    done
    echo -e "${GREEN}Done${NC}"
}

cmd_kill() {
    local force=false
    if [[ "$1" == "--force" ]] || [[ "$1" == "-f" ]]; then
        force=true
    fi

    echo -e "${YELLOW}Killing $TMUX_SESSION tmux session...${NC}"
    tmux kill-session -t "$TMUX_SESSION" 2>/dev/null && \
        echo -e "${GREEN}Done${NC}" || \
        echo -e "${YELLOW}No session found${NC}"

    # Kill any orphaned jax processes on our ports
    if $force; then
        echo -e "${YELLOW}Force killing processes on dev ports...${NC}"
        kill_dev_ports
    fi
}

# Kill processes on dev environment ports
kill_dev_ports() {
    local killed=0
    for node in $(get_node_names); do
        local app_port=$(get_app_port "$node")
        local gw_port=$(get_gateway_port "$node")

        for port in $app_port $gw_port; do
            # Find PID using the port (macOS lsof)
            local pid=$(lsof -ti tcp:$port 2>/dev/null)
            if [[ -n "$pid" ]]; then
                echo -e "  Killing process $pid on port $port"
                kill -9 $pid 2>/dev/null && killed=$((killed + 1))
            fi
        done
    done

    if [[ $killed -eq 0 ]]; then
        echo -e "${GREEN}No orphaned processes found${NC}"
    else
        echo -e "${GREEN}Killed $killed process(es)${NC}"
    fi
}

cmd_status() {
    echo -e "${BLUE}Node Status${NC}"
    echo ""

    # Check tmux session
    if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
        echo -e "tmux session: ${GREEN}running${NC}"
    else
        echo -e "tmux session: ${YELLOW}not running${NC}"
    fi
    echo ""

    list_nodes
    echo ""

    # Check if nodes are responding
    echo "Health checks:"
    for node in $(get_node_names); do
        local type=$(get_node_type "$node")
        local port

        if [[ "$type" == "gateway" ]]; then
            port=$(get_gateway_port "$node")
        else
            port=$(get_app_port "$node")
        fi

        local nick=$(toml_get "$node" "nick")
        printf "  %-8s %-6s " "$node" "($nick)"

        if curl -s --connect-timeout 1 "http://localhost:$port/_status/livez" >/dev/null 2>&1; then
            echo -e "${GREEN}healthy${NC}"
        else
            echo -e "${RED}not responding${NC}"
        fi
    done
}

# Initialize a node if it doesn't exist
init_node() {
    local node="$1"
    local data_path="$DATA_DIR/$node"

    if [[ -d "$data_path" ]]; then
        return 0
    fi

    local name=$(get_node_name "$node")
    local blob_store=$(get_blob_store "$node")
    local app_port=$(get_app_port "$node")
    local gw_port=$(get_gateway_port "$node")
    local peer_port=$(toml_get "$node" "peer_port")

    echo -e "${YELLOW}Initializing $node ($name)...${NC}"

    local init_args="--config-path $data_path init"
    init_args="$init_args --app-port $app_port"
    if [[ -n "$peer_port" ]]; then
        init_args="$init_args --peer-port $peer_port"
    fi
    init_args="$init_args --gateway-port $gw_port"
    init_args="$init_args --blob-store $blob_store"

    if [[ "$blob_store" == "s3" ]]; then
        local s3_url=$(get_s3_url "$node")
        init_args="$init_args --s3-url $s3_url"
    fi

    cargo run --bin jax -- $init_args
}

# Build the daemon command for a node (for use with cargo watch -x)
# Returns just the cargo subcommand (without 'cargo ' prefix)
get_daemon_cmd() {
    local node="$1"
    local type=$(get_node_type "$node")
    local data_path="$DATA_DIR/$node"
    local log_dir="$data_path/logs"

    local cmd="run --bin jax -- --config-path $data_path daemon --log-dir $log_dir"

    case "$type" in
        full)    cmd="$cmd --with-gateway" ;;
        gateway) cmd="$cmd --gateway" ;;
        # app type needs no extra flags
    esac

    echo "$cmd"
}

cmd_run() {
    local background=false
    if [[ "$1" == "--background" ]] || [[ "$1" == "-b" ]]; then
        background=true
        shift
    fi

    # Check if session already exists and nodes are healthy
    if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
        echo -e "${BLUE}Tmux session already exists, checking nodes...${NC}"
        local all_healthy=true
        for node in $(get_node_names); do
            local type=$(get_node_type "$node")
            local port
            if [[ "$type" == "gateway" ]]; then
                port=$(get_gateway_port "$node")
            else
                port=$(get_app_port "$node")
            fi
            if ! curl -s --connect-timeout 1 "http://localhost:$port/_status/livez" >/dev/null 2>&1; then
                all_healthy=false
                break
            fi
        done

        if $all_healthy; then
            echo -e "${GREEN}All nodes healthy${NC}"
            if ! $background; then
                tmux attach -t "$TMUX_SESSION"
            fi
            return 0
        else
            echo -e "${YELLOW}Nodes not healthy, recreating session...${NC}"
        fi
    fi

    echo -e "${BLUE}Setting up JAX dev environment...${NC}"

    # Check cargo-watch
    if ! command -v cargo-watch &>/dev/null; then
        echo -e "${YELLOW}Installing cargo-watch...${NC}"
        cargo install cargo-watch
    fi

    # Start MinIO for s3 nodes
    local need_minio=false
    for node in $(get_node_names); do
        if [[ "$(get_blob_store $node)" == "s3" ]]; then
            need_minio=true
            break
        fi
    done

    if $need_minio; then
        echo -e "${BLUE}Starting MinIO...${NC}"
        "$PROJECT_ROOT/bin/minio" up || true
    fi

    # Initialize nodes
    for node in $(get_node_names); do
        init_node "$node"
    done

    # Kill existing session (if unhealthy)
    tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true

    # Count nodes for pane layout
    local node_count=$(get_node_names | wc -l | tr -d ' ')

    # Create tmux session
    tmux new-session -d -s "$TMUX_SESSION" -n nodes

    # Split into panes
    for ((i=1; i<node_count; i++)); do
        tmux split-window -v -t "$TMUX_SESSION:0"
    done

    # Make panes equal size
    tmux select-layout -t "$TMUX_SESSION:0" even-vertical

    # Start each node in its pane
    local pane=0
    for node in $(get_node_names); do
        local name=$(get_node_name "$node")
        local nick=$(toml_get "$node" "nick")
        local type=$(get_node_type "$node")
        local app_port=$(get_app_port "$node")
        local gw_port=$(get_gateway_port "$node")
        local daemon_cmd=$(get_daemon_cmd "$node")

        # Build info line
        local info=""
        case "$type" in
            full)    info="App: http://localhost:$app_port | Gateway: http://localhost:$gw_port" ;;
            app)     info="App: http://localhost:$app_port" ;;
            gateway) info="Gateway: http://localhost:$gw_port" ;;
        esac

        # Send commands to pane
        tmux send-keys -t "$TMUX_SESSION:0.$pane" "cd $PROJECT_ROOT && echo -e '${GREEN}=== $node ($nick): $name ===${NC}' && echo '$info' && echo '' && RUST_LOG=info cargo watch --why --ignore 'data/*' --ignore '*.sqlite*' --ignore '*.db*' -x '$daemon_cmd'" C-m

        pane=$((pane + 1))
    done

    # Print summary
    echo ""
    echo -e "${GREEN}Started nodes:${NC}"
    for node in $(get_node_names); do
        local nick=$(toml_get "$node" "nick")
        local name=$(get_node_name "$node")
        local type=$(get_node_type "$node")
        local app_port=$(get_app_port "$node")
        local gw_port=$(get_gateway_port "$node")

        case "$type" in
            full)    echo "  $node ($nick): http://localhost:$app_port (app) / http://localhost:$gw_port (gateway)" ;;
            app)     echo "  $node ($nick): http://localhost:$app_port (app)" ;;
            gateway) echo "  $node ($nick): http://localhost:$gw_port (gateway)" ;;
        esac
    done
    echo ""

    if $need_minio; then
        echo "MinIO console: http://localhost:9001"
        echo ""
    fi

    echo "Logs: ./data/<node>/logs/jax.log.*"
    echo ""

    # Wait for nodes to be healthy and apply fixtures
    wait_for_nodes
    apply_fixtures_on_startup

    echo "Use './bin/dev logs' to view logs"
    echo "Use './bin/dev api <node> <command>' for API commands"
    echo ""

    if ! $background; then
        tmux attach -t "$TMUX_SESSION"
    else
        echo -e "${GREEN}Dev environment running in background${NC}"
        echo "Use './bin/dev status' to check health"
        echo "Use 'tmux attach -t $TMUX_SESSION' to attach"
    fi
}

# Wait for all nodes to be healthy
wait_for_nodes() {
    echo -e "${BLUE}Waiting for nodes to be healthy...${NC}"
    local max_attempts=30
    local attempt=0

    while [[ $attempt -lt $max_attempts ]]; do
        local all_healthy=true

        for node in $(get_node_names); do
            local type=$(get_node_type "$node")
            local port

            if [[ "$type" == "gateway" ]]; then
                port=$(get_gateway_port "$node")
            else
                port=$(get_app_port "$node")
            fi

            if ! curl -s --connect-timeout 1 "http://localhost:$port/_status/livez" >/dev/null 2>&1; then
                all_healthy=false
                break
            fi
        done

        if $all_healthy; then
            echo -e "${GREEN}All nodes healthy${NC}"
            return 0
        fi

        attempt=$((attempt + 1))
        printf "."
        sleep 1
    done

    echo ""
    echo -e "${YELLOW}Warning: Some nodes may not be ready${NC}"
    return 1
}

# Apply fixtures after nodes start
apply_fixtures_on_startup() {
    if [[ ! -f "$FIXTURES_FILE" ]]; then
        return 0
    fi

    echo ""
    fixtures_apply
}

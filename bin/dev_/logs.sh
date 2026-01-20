#!/bin/bash
# Log viewing commands

# Get the log directory for a node
log_dir() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    echo "$DATA_DIR/$node/logs"
}

# Find the latest log file for a node
latest_log() {
    local node=$(resolve_node "$1")
    local dir=$(log_dir "$node")
    if [[ -d "$dir" ]]; then
        ls -t "$dir"/jax.log.* 2>/dev/null | head -1
    fi
}

# Get all log files across all nodes
all_log_files() {
    for node in $(get_node_names); do
        local dir=$(log_dir "$node")
        if [[ -d "$dir" ]]; then
            ls "$dir"/jax.log.* 2>/dev/null
        fi
    done
}

logs_tail() {
    local node="$1"

    if [[ -n "$node" ]]; then
        # Tail specific node
        local resolved=$(resolve_node "$node")
        if [[ -z "$resolved" ]]; then
            echo -e "${RED}Unknown node: $node${NC}"
            list_nodes
            return 1
        fi

        local dir=$(log_dir "$resolved")
        if [[ ! -d "$dir" ]]; then
            echo -e "${YELLOW}No logs directory for $resolved${NC}"
            return 1
        fi

        local nick=$(toml_get "$resolved" "nick")
        echo -e "${GREEN}Tailing logs for $resolved ($nick)...${NC}"
        tail -F "$dir"/jax.log.* 2>/dev/null || echo -e "${YELLOW}No log files found${NC}"
    else
        # Tail all nodes
        local log_files=$(all_log_files)

        if [[ -z "$log_files" ]]; then
            echo -e "${YELLOW}No log files found. Start the dev environment first.${NC}"
            return 1
        fi

        echo -e "${GREEN}Tailing all node logs (Ctrl+C to stop)...${NC}"
        tail -F $log_files
    fi
}

logs_grep() {
    local term="$1"

    if [[ -z "$term" ]]; then
        echo "Usage: logs grep <term>"
        return 1
    fi

    local log_files=$(all_log_files)

    if [[ -z "$log_files" ]]; then
        echo -e "${YELLOW}No log files found${NC}"
        return 1
    fi

    echo -e "${GREEN}Searching for '$term' in logs...${NC}"
    grep -h --color=always "$term" $log_files 2>/dev/null || echo -e "${YELLOW}No matches found${NC}"
}

logs_cat() {
    local node="${1:-$(get_default_node)}"
    local resolved=$(resolve_node "$node")
    local log_file=$(latest_log "$resolved")

    if [[ -z "$log_file" ]]; then
        echo -e "${YELLOW}No log files found for $resolved${NC}"
        return 1
    fi

    local nick=$(toml_get "$resolved" "nick")
    echo -e "${GREEN}Latest log for $resolved ($nick): $log_file${NC}"
    cat "$log_file"
}

logs_list() {
    echo -e "${GREEN}Log files:${NC}"
    echo ""

    for node in $(get_node_names); do
        local nick=$(toml_get "$node" "nick")
        local dir=$(log_dir "$node")

        echo "$node ($nick):"
        if [[ -d "$dir" ]]; then
            ls -lh "$dir"/jax.log.* 2>/dev/null | sed 's/^/  /' || echo "  (no files)"
        else
            echo "  (no logs directory)"
        fi
        echo ""
    done
}

logs_help() {
    echo "Log viewer - helper for viewing jax-bucket logs"
    echo ""
    echo "Usage: ./bin/dev logs <command> [args...]"
    echo ""
    echo "Commands:"
    echo "  tail [node]     Tail logs (all nodes if no node specified)"
    echo "  grep <term>     Search all logs for a term"
    echo "  cat [node]      Show latest log file (default: node0)"
    echo "  list            List all log files"
    echo "  help            Show this help"
    echo ""
    echo "Node can be specified by:"
    echo "  - ID: node0, node1, node2"
    echo "  - Nickname: full, app, gw"
    echo ""
    echo "Log location: ./data/<node>/logs/jax.log.YYYY-MM-DD"
    echo ""
    echo "Examples:"
    echo "  ./bin/dev logs tail           # Tail all logs"
    echo "  ./bin/dev logs tail full      # Tail node0 logs"
    echo "  ./bin/dev logs grep ERROR     # Search for errors"
    echo "  ./bin/dev logs cat gw         # View node2 latest log"
}

cmd_logs() {
    case "${1:-tail}" in
        tail)   shift; logs_tail "$@" ;;
        grep)   shift; logs_grep "$@" ;;
        cat)    shift; logs_cat "$@" ;;
        list)   logs_list ;;
        help|-h|--help) logs_help ;;
        # If first arg looks like a node, assume tail
        *)
            if resolve_node "$1" >/dev/null 2>&1; then
                logs_tail "$1"
            else
                echo -e "${RED}Unknown logs command: $1${NC}"
                logs_help
                return 1
            fi
            ;;
    esac
}

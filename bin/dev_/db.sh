#!/bin/bash
# Database inspection commands

cmd_db() {
    local node="${1:-}"
    local query="${2:-}"

    if [[ -z "$node" ]]; then
        db_help
        return 1
    fi

    if [[ "$node" == "help" ]]; then
        db_help
        return 0
    fi

    # Resolve node name to data path
    local data_path="$DATA_DIR/$node"
    if [[ ! -d "$data_path" ]]; then
        # Try by nickname
        local resolved=$(resolve_node_by_nick "$node")
        if [[ -n "$resolved" ]]; then
            data_path="$DATA_DIR/$resolved"
        else
            echo -e "${RED}Unknown node: $node${NC}"
            echo "Available nodes:"
            for n in $(get_node_names); do
                local nick=$(toml_get "$n" "nick")
                echo "  $n ($nick)"
            done
            return 1
        fi
    fi

    local db_path="$data_path/db.sqlite"
    if [[ ! -f "$db_path" ]]; then
        echo -e "${RED}Database not found: $db_path${NC}"
        return 1
    fi

    if [[ -z "$query" ]]; then
        # Interactive mode
        echo -e "${BLUE}Opening SQLite shell for $node${NC}"
        echo "Database: $db_path"
        echo ""
        sqlite3 "$db_path"
    else
        # Run query
        sqlite3 "$db_path" "$query"
    fi
}

# Resolve node nickname to node name
resolve_node_by_nick() {
    local nick="$1"
    for node in $(get_node_names); do
        local node_nick=$(toml_get "$node" "nick")
        if [[ "$node_nick" == "$nick" ]]; then
            echo "$node"
            return 0
        fi
    done
    return 1
}

db_help() {
    echo "Database inspection helper"
    echo ""
    echo "Usage: ./bin/dev db <node> [query]"
    echo ""
    echo "Arguments:"
    echo "  node     Node name (node0, node1, node2) or nickname (full, app, gw)"
    echo "  query    Optional SQL query (opens interactive shell if omitted)"
    echo ""
    echo "Examples:"
    echo "  ./bin/dev db full                     # Interactive shell for node0"
    echo "  ./bin/dev db full '.tables'           # List tables"
    echo "  ./bin/dev db app 'SELECT * FROM bucket_log LIMIT 5'"
    echo "  ./bin/dev db gw 'SELECT bucket_id, current_link FROM bucket_log ORDER BY height DESC LIMIT 3'"
    echo ""
    echo "Common queries:"
    echo "  .tables                               # List all tables"
    echo "  .schema bucket_log                    # Show table schema"
    echo "  SELECT * FROM bucket_log LIMIT 10    # View recent entries"
}

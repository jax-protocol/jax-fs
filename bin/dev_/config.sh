#!/bin/bash
# Config parsing utilities for dev environment

# Parse a value from TOML config
# Usage: toml_get <section> <key>
toml_get() {
    local section="$1"
    local key="$2"
    local in_section=false

    while IFS= read -r line; do
        # Skip comments
        [[ "$line" =~ ^[[:space:]]*# ]] && continue

        # Check for section header
        if [[ "$line" =~ ^\[([a-zA-Z0-9_]+)\]$ ]]; then
            if [[ "${BASH_REMATCH[1]}" == "$section" ]]; then
                in_section=true
            else
                in_section=false
            fi
            continue
        fi

        # Parse key=value in current section
        if $in_section && [[ "$line" =~ ^${key}[[:space:]]*=[[:space:]]*(.+)$ ]]; then
            local value="${BASH_REMATCH[1]}"
            # Remove inline comments (after #)
            value="${value%%#*}"
            # Trim trailing whitespace
            value="${value%"${value##*[![:space:]]}"}"
            # Remove quotes if present
            value="${value%\"}"
            value="${value#\"}"
            echo "$value"
            return 0
        fi
    done < "$CONFIG_FILE"

    return 1
}

# Get all node names from config
get_node_names() {
    grep -E '^\[node[0-9]+\]$' "$CONFIG_FILE" | tr -d '[]'
}

# Resolve a node reference (id, nick, or name) to node id
# Usage: resolve_node <ref>
# Returns: node id (e.g., "node0") or empty if not found
resolve_node() {
    local ref="$1"

    # If it's already a node id, return it
    if [[ "$ref" =~ ^node[0-9]+$ ]]; then
        echo "$ref"
        return 0
    fi

    # Search by nickname or name
    for node in $(get_node_names); do
        local nick=$(toml_get "$node" "nick")
        local name=$(toml_get "$node" "name")

        if [[ "$nick" == "$ref" ]] || [[ "$name" == "$ref" ]]; then
            echo "$node"
            return 0
        fi
    done

    return 1
}

# Get the default node (first app node, not gateway-only)
get_default_node() {
    for node in $(get_node_names); do
        local type=$(toml_get "$node" "type")
        if [[ "$type" != "gateway" ]]; then
            echo "$node"
            return 0
        fi
    done
    # Fallback to first node if all are gateways (shouldn't happen)
    get_node_names | head -1
}

# Get app port for a node
get_app_port() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "app_port"
}

# Get gateway port for a node
get_gateway_port() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "gateway_port"
}

# Get node type (full, app, gateway)
get_node_type() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "type"
}

# Get node display name
get_node_name() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "name"
}

# Get blob store type
get_blob_store() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "blob_store"
}

# Get S3 URL (for s3 blob store)
get_s3_url() {
    local node=$(resolve_node "${1:-$(get_default_node)}")
    toml_get "$node" "s3_url"
}

# List all nodes with their info
list_nodes() {
    echo "Available nodes:"
    echo ""
    for node in $(get_node_names); do
        local nick=$(toml_get "$node" "nick")
        local name=$(toml_get "$node" "name")
        local type=$(toml_get "$node" "type")
        local app_port=$(toml_get "$node" "app_port")
        local gw_port=$(toml_get "$node" "gateway_port")

        printf "  %-8s %-6s %-30s" "$node" "($nick)" "$name"

        case "$type" in
            full)    echo " App:$app_port Gateway:$gw_port" ;;
            app)     echo " App:$app_port" ;;
            gateway) echo " Gateway:$gw_port" ;;
        esac
    done
}

#!/bin/bash
# API helper commands
# Usage: ./bin/dev api <node> <command> [args...]

# Current node (set by cmd_api)
API_NODE=""

# Get the API URL for a node
# Usage: api_url [node] - uses API_NODE if not specified
api_url() {
    local node_arg="${1:-$API_NODE}"
    local node=$(resolve_node "$node_arg")
    local type=$(get_node_type "$node")
    local port

    if [[ "$type" == "gateway" ]]; then
        port=$(get_gateway_port "$node")
    else
        port=$(get_app_port "$node")
    fi

    echo "http://localhost:$port/api/v0"
}

# Get the status URL for a node
# Usage: status_url [node] - uses API_NODE if not specified
status_url() {
    local node_arg="${1:-$API_NODE}"
    local node=$(resolve_node "$node_arg")
    local type=$(get_node_type "$node")
    local port

    if [[ "$type" == "gateway" ]]; then
        port=$(get_gateway_port "$node")
    else
        port=$(get_app_port "$node")
    fi

    echo "http://localhost:$port/_status"
}

api_health() {
    local url=$(status_url)
    echo -e "${BLUE}GET $url/livez${NC}"
    curl -s "$url/livez" | jq .
}

api_ready() {
    local url=$(status_url)
    echo -e "${BLUE}GET $url/readyz${NC}"
    curl -s "$url/readyz" | jq .
}

api_identity() {
    local url=$(status_url)
    echo -e "${BLUE}GET $url/identity${NC}"
    curl -s "$url/identity" | jq .
}

api_version() {
    local url=$(status_url)
    echo -e "${BLUE}GET $url/version${NC}"
    curl -s "$url/version" | jq .
}

api_list() {
    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/list${NC}"
    curl -s -X POST "$url/bucket/list" \
        -H "Content-Type: application/json" \
        -d '{}' | jq .
}

api_create() {
    local name="$1"

    if [[ -z "$name" ]]; then
        echo "Usage: ./bin/dev api <node> create <name>"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket${NC}"
    curl -s -X POST "$url/bucket" \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$name\"}" | jq .
}

api_ls() {
    local bucket_id="$1"
    local path="${2:-/}"

    if [[ -z "$bucket_id" ]]; then
        echo "Usage: ./bin/dev api <node> ls <bucket_id> [path]"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/ls${NC}"
    local response
    response=$(curl -s -X POST "$url/bucket/ls" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"path\": \"$path\"}")

    if echo "$response" | jq . 2>/dev/null; then
        :  # jq succeeded, output already printed
    else
        echo -e "${RED}$response${NC}"
    fi
}

api_cat() {
    local bucket_id="$1"
    local path="$2"

    if [[ -z "$bucket_id" ]] || [[ -z "$path" ]]; then
        echo "Usage: ./bin/dev api <node> cat <bucket_id> <path>"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/cat${NC}"
    curl -s -X POST "$url/bucket/cat" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"path\": \"$path\"}"
}

api_upload() {
    local bucket_id="$1"
    local remote_path="$2"
    local local_file="$3"

    if [[ -z "$bucket_id" ]] || [[ -z "$remote_path" ]] || [[ -z "$local_file" ]]; then
        echo "Usage: ./bin/dev api <node> upload <bucket_id> <remote_path> <local_file>"
        return 1
    fi

    if [[ ! -f "$local_file" ]]; then
        echo -e "${RED}Error: File not found: $local_file${NC}"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/add${NC}"
    curl -s -X POST "$url/bucket/add" \
        -F "bucket_id=$bucket_id" \
        -F "mount_path=$remote_path" \
        -F "file=@$local_file" | jq .
}

api_mkdir() {
    local bucket_id="$1"
    local path="$2"

    if [[ -z "$bucket_id" ]] || [[ -z "$path" ]]; then
        echo "Usage: ./bin/dev api <node> mkdir <bucket_id> <path>"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/mkdir${NC}"
    curl -s -X POST "$url/bucket/mkdir" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"path\": \"$path\"}" | jq .
}

api_delete() {
    local bucket_id="$1"
    local path="$2"

    if [[ -z "$bucket_id" ]] || [[ -z "$path" ]]; then
        echo "Usage: ./bin/dev api <node> delete <bucket_id> <path>"
        return 1
    fi

    local url=$(api_url)
    echo -e "${BLUE}POST $url/bucket/delete${NC}"
    curl -s -X POST "$url/bucket/delete" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"path\": \"$path\"}" | jq .
}

# Get gateway base URL for a node
gateway_url() {
    local node_arg="${1:-$API_NODE}"
    local node=$(resolve_node "$node_arg")
    local port=$(get_gateway_port "$node")
    echo "http://localhost:$port"
}

api_fetch() {
    local bucket_id="$1"
    local path="${2:-}"

    if [[ -z "$bucket_id" ]]; then
        echo "Usage: ./bin/dev api <node> fetch <bucket_id> [path]"
        return 1
    fi

    local base=$(gateway_url)
    local url="$base/gw/$bucket_id$path"
    echo -e "${BLUE}GET $url${NC}"

    # Use Accept: application/json for directory listings
    local response
    response=$(curl -s -H "Accept: application/json" "$url")

    # Try to parse as JSON, otherwise show raw
    if echo "$response" | jq . 2>/dev/null; then
        :  # jq succeeded
    else
        echo "$response"
    fi
}

api_help() {
    echo "API helper - curl commands for interacting with jax-bucket"
    echo ""
    echo "Usage: ./bin/dev api <node> <command> [args...]"
    echo ""
    echo "Nodes:"
    echo "  full, app    - App nodes (support all commands)"
    echo "  gw           - Gateway only (health + fetch commands)"
    echo ""
    echo "Health commands (all nodes):"
    echo "  health                        Check node health"
    echo "  ready                         Check node readiness"
    echo "  identity                      Get node identity"
    echo "  version                       Get node version"
    echo ""
    echo "Gateway commands (all nodes with gateway port):"
    echo "  fetch <bucket_id> [path]      Fetch content from gateway"
    echo ""
    echo "Bucket commands (app nodes only - full, app):"
    echo "  list                          List all buckets"
    echo "  create <name>                 Create a new bucket"
    echo "  ls <bucket_id> [path]         List directory contents"
    echo "  cat <bucket_id> <path>        Read file contents"
    echo "  upload <bucket_id> <path> <file>  Upload a file"
    echo "  mkdir <bucket_id> <path>      Create directory"
    echo "  delete <bucket_id> <path>     Delete file/directory"
    echo ""
    echo "Examples:"
    echo "  ./bin/dev api full health     # Health check on full node"
    echo "  ./bin/dev api gw health       # Health check on gateway"
    echo "  ./bin/dev api gw fetch abc-123 /  # Fetch from gateway"
    echo "  ./bin/dev api app list        # List buckets on app node"
    echo "  ./bin/dev api full create test # Create bucket"
    echo "  ./bin/dev api full ls abc-123 / # List files"
}

# Check if command requires the bucket API (not available on gateways)
requires_bucket_api() {
    local cmd="$1"
    case "$cmd" in
        list|create|ls|cat|upload|mkdir|delete) return 0 ;;
        *) return 1 ;;
    esac
}

cmd_api() {
    # First arg must be node
    local node="$1"

    if [[ -z "$node" ]] || [[ "$node" == "help" ]] || [[ "$node" == "-h" ]] || [[ "$node" == "--help" ]]; then
        api_help
        return 0
    fi

    # Validate node
    if ! resolve_node "$node" >/dev/null 2>&1; then
        echo -e "${RED}Unknown node: $node${NC}"
        echo "Valid nodes: full, app, gw (or node0, node1, node2)"
        return 1
    fi

    API_NODE="$node"
    shift

    local cmd="${1:-help}"
    local resolved_node=$(resolve_node "$API_NODE")
    local node_type=$(get_node_type "$resolved_node")

    # Check if gateway node is trying to use bucket API
    if [[ "$node_type" == "gateway" ]] && requires_bucket_api "$cmd"; then
        echo -e "${RED}Error: Gateway nodes do not expose the bucket API${NC}"
        echo "Gateway nodes only support: health, ready, identity, version"
        echo ""
        echo "Use an app node (full, app) for bucket operations:"
        echo "  ./bin/dev api full $cmd ..."
        echo "  ./bin/dev api app $cmd ..."
        return 1
    fi

    # Second arg is command
    case "$cmd" in
        health)   shift; api_health "$@" ;;
        ready)    shift; api_ready "$@" ;;
        identity) shift; api_identity "$@" ;;
        version)  shift; api_version "$@" ;;
        fetch)    shift; api_fetch "$@" ;;
        list)     shift; api_list "$@" ;;
        create)   shift; api_create "$@" ;;
        ls)       shift; api_ls "$@" ;;
        cat)      shift; api_cat "$@" ;;
        upload)   shift; api_upload "$@" ;;
        mkdir)    shift; api_mkdir "$@" ;;
        delete)   shift; api_delete "$@" ;;
        help|-h|--help) api_help ;;
        *)        echo -e "${RED}Unknown command: $cmd${NC}"; api_help; return 1 ;;
    esac
}

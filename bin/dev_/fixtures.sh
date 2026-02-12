#!/bin/bash
# Fixtures - set up initial data in the dev environment

FIXTURES_FILE="$SCRIPT_DIR/fixtures.toml"

# State file for tracking created buckets (name -> id mapping)
BUCKET_CACHE_FILE="${TMPDIR:-/tmp}/jax-dev-bucket-cache-$$"

# Clean up cache on exit
trap 'rm -f "$BUCKET_CACHE_FILE"' EXIT

# Store bucket id in cache
cache_bucket() {
    local name="$1"
    local id="$2"
    echo "$name=$id" >> "$BUCKET_CACHE_FILE"
}

# Get bucket id from cache
get_cached_bucket() {
    local name="$1"
    if [[ -f "$BUCKET_CACHE_FILE" ]]; then
        grep "^$name=" "$BUCKET_CACHE_FILE" 2>/dev/null | head -1 | cut -d= -f2
    fi
}

# Parse fixture entries from TOML
# Returns lines like: "type|bucket|name|path|content|source|node|role|peer|from|to|mount_point"
parse_fixtures() {
    local in_fixture=false
    local type="" bucket="" name="" path="" content="" source="" node="" role="" peer="" from="" to="" mount_point=""
    local in_multiline=false
    local multiline_content=""

    while IFS= read -r line; do
        # Check for fixture start
        if [[ "$line" =~ ^\[\[fixture\]\]$ ]]; then
            # Output previous fixture if exists
            if $in_fixture && [[ -n "$type" ]]; then
                echo "$type|$bucket|$name|$path|$content|$source|$node|$role|$peer|$from|$to|$mount_point"
            fi
            in_fixture=true
            type="" bucket="" name="" path="" content="" source="" node="" role="" peer="" from="" to="" mount_point=""
            in_multiline=false
            multiline_content=""
            continue
        fi

        # Skip if not in a fixture
        if ! $in_fixture; then
            continue
        fi

        # Handle multiline string end
        if $in_multiline; then
            if [[ "$line" =~ ^\"\"\"$ ]]; then
                content="$multiline_content"
                in_multiline=false
            else
                if [[ -n "$multiline_content" ]]; then
                    multiline_content="$multiline_content\n$line"
                else
                    multiline_content="$line"
                fi
            fi
            continue
        fi

        # Parse key = value
        if [[ "$line" =~ ^([a-z_]+)[[:space:]]*=[[:space:]]*(.+)$ ]]; then
            local key="${BASH_REMATCH[1]}"
            local value="${BASH_REMATCH[2]}"

            # Check for multiline string start
            if [[ "$value" =~ ^\"\"\" ]]; then
                in_multiline=true
                multiline_content=""
                continue
            fi

            # Remove quotes
            value="${value%\"}"
            value="${value#\"}"

            case "$key" in
                type)        type="$value" ;;
                bucket)      bucket="$value" ;;
                name)        name="$value" ;;
                path)        path="$value" ;;
                content)     content="$value" ;;
                source)      source="$value" ;;
                node)        node="$value" ;;
                role)        role="$value" ;;
                peer)        peer="$value" ;;
                from)        from="$value" ;;
                to)          to="$value" ;;
                mount_point) mount_point="$value" ;;
            esac
        fi
    done < "$FIXTURES_FILE"

    # Output last fixture
    if $in_fixture && [[ -n "$type" ]]; then
        echo "$type|$bucket|$name|$path|$content|$source|$node|$role|$peer|$from|$to|$mount_point"
    fi
}

# Create a bucket and store its ID
fixture_bucket() {
    local name="$1"
    local node="$2"

    echo -e "${BLUE}Creating bucket: $name${NC}"
    local result=$(curl -s -X POST "$(api_url "$node")/bucket" \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$name\"}")

    local bucket_id=$(echo "$result" | jq -r '.bucket_id // empty')
    if [[ -n "$bucket_id" ]]; then
        cache_bucket "$name" "$bucket_id"
        echo -e "  ${GREEN}Created: $bucket_id${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Resolve bucket name to ID
resolve_bucket() {
    local name="$1"
    local node="$2"

    # Check cache first
    local cached=$(get_cached_bucket "$name")
    if [[ -n "$cached" ]]; then
        echo "$cached"
        return 0
    fi

    # Query the API
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/list" \
        -H "Content-Type: application/json" \
        -d '{}')

    local bucket_id=$(echo "$result" | jq -r ".buckets[] | select(.name == \"$name\") | .bucket_id")
    if [[ -n "$bucket_id" ]]; then
        cache_bucket "$name" "$bucket_id"
        echo "$bucket_id"
        return 0
    fi

    return 1
}

# Create a directory in a bucket
fixture_dir() {
    local bucket="$1"
    local path="$2"
    local node="$3"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    echo -e "${BLUE}Creating directory: $bucket:$path${NC}"
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/mkdir" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"path\": \"$path\"}")

    if echo "$result" | jq -e '.link' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Created${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Upload a file to a bucket
fixture_file() {
    local bucket="$1"
    local path="$2"
    local content="$3"
    local source="$4"
    local node="$5"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    echo -e "${BLUE}Creating file: $bucket:$path${NC}"

    local dir=$(dirname "$path")
    local filename=$(basename "$path")
    local tmp_file=""

    # Determine content source
    if [[ -n "$source" ]]; then
        # File from disk
        if [[ ! -f "$PROJECT_ROOT/$source" ]]; then
            echo -e "  ${RED}Source file not found: $source${NC}"
            return 1
        fi
        tmp_file="$PROJECT_ROOT/$source"
    elif [[ -n "$content" ]]; then
        # Inline content - create temp file
        tmp_file=$(mktemp)
        # Handle escaped newlines
        echo -e "$content" > "$tmp_file"
    else
        echo -e "  ${RED}No content or source specified${NC}"
        return 1
    fi

    # Upload
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/add" \
        -F "bucket_id=$bucket_id" \
        -F "mount_path=$dir" \
        -F "file=@$tmp_file;filename=$filename")

    # Clean up temp file if we created one
    if [[ -n "$content" ]] && [[ -f "$tmp_file" ]]; then
        rm -f "$tmp_file"
    fi

    if echo "$result" | jq -e '.successful_files > 0' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Created${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Share a bucket with a peer
fixture_share() {
    local bucket="$1"
    local peer="$2"
    local role="${3:-owner}"
    local node="$4"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    # Get peer's public key
    local peer_node=$(resolve_node "$peer")
    local peer_port=$(get_api_port "$peer_node")

    local peer_identity=$(curl -s "http://localhost:$peer_port/_status/identity")
    local peer_public_key=$(echo "$peer_identity" | jq -r '.node_id // empty')

    if [[ -z "$peer_public_key" ]]; then
        echo -e "${RED}Could not get public key for peer: $peer${NC}"
        return 1
    fi

    echo -e "${BLUE}Sharing bucket: $bucket with $peer as $role${NC}"
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/share" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"peer_public_key\": \"$peer_public_key\", \"role\": \"$role\"}")

    if echo "$result" | jq -e '.new_bucket_link' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Shared with $peer as $role${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Publish a bucket (grant decryption to mirrors)
fixture_publish() {
    local bucket="$1"
    local node="$2"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    echo -e "${BLUE}Publishing bucket: $bucket${NC}"
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/publish" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\"}")

    if echo "$result" | jq -e '.published' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Published${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Move/rename a file or directory
fixture_mv() {
    local bucket="$1"
    local from="$2"
    local to="$3"
    local node="$4"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    echo -e "${BLUE}Moving: $bucket:$from -> $to${NC}"
    local result=$(curl -s -X POST "$(api_url "$node")/bucket/mv" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"source_path\": \"$from\", \"dest_path\": \"$to\"}")

    if echo "$result" | jq -e '.link' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Moved${NC}"
    else
        echo -e "  ${RED}Failed: $result${NC}"
        return 1
    fi
}

# Mount a bucket via FUSE
fixture_mount() {
    local bucket="$1"
    local mount_point="$2"
    local node="$3"

    local bucket_id=$(resolve_bucket "$bucket" "$node")
    if [[ -z "$bucket_id" ]]; then
        echo -e "${RED}Bucket not found: $bucket${NC}"
        return 1
    fi

    # Expand ~ in mount_point
    mount_point="${mount_point/#\~/$HOME}"

    echo -e "${BLUE}Mounting bucket: $bucket at $mount_point${NC}"

    # Create mount point if needed
    mkdir -p "$mount_point" 2>/dev/null || true

    # Create mount via API (response has .mount.mount_id structure)
    local result=$(curl -s -X POST "$(api_url "$node")/mounts" \
        -H "Content-Type: application/json" \
        -d "{\"bucket_id\": \"$bucket_id\", \"mount_point\": \"$mount_point\"}")

    # Try both response formats: .mount.mount_id (nested) or .mount_id (flat)
    local mount_id=$(echo "$result" | jq -r '.mount.mount_id // .mount_id // empty')
    if [[ -z "$mount_id" ]]; then
        echo -e "  ${RED}Failed to create mount: $result${NC}"
        return 1
    fi

    echo -e "  ${GREEN}Created mount config: $mount_id${NC}"

    # Start the mount (response has .started field)
    local start_result=$(curl -s -X POST "$(api_url "$node")/mounts/$mount_id/start" \
        -H "Content-Type: application/json" \
        -d '{}')

    if echo "$start_result" | jq -e '.started // .success' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Mounted at $mount_point${NC}"
        # Store mount_id for later unmount
        echo "$bucket=$mount_id" >> "${TMPDIR:-/tmp}/jax-dev-mount-cache-$$"
        # Give FUSE a moment to initialize
        sleep 1
    else
        echo -e "  ${RED}Failed to start mount: $start_result${NC}"
        return 1
    fi
}

# Unmount a bucket
fixture_unmount() {
    local bucket="$1"
    local node="$2"

    # Get mount_id from cache
    local mount_id=""
    if [[ -f "${TMPDIR:-/tmp}/jax-dev-mount-cache-$$" ]]; then
        mount_id=$(grep "^$bucket=" "${TMPDIR:-/tmp}/jax-dev-mount-cache-$$" 2>/dev/null | head -1 | cut -d= -f2)
    fi

    if [[ -z "$mount_id" ]]; then
        echo -e "${YELLOW}No cached mount for bucket: $bucket${NC}"
        return 0
    fi

    echo -e "${BLUE}Unmounting bucket: $bucket${NC}"

    local result=$(curl -s -X POST "$(api_url "$node")/mounts/$mount_id/stop" \
        -H "Content-Type: application/json" \
        -d '{}')

    if echo "$result" | jq -e '.success' >/dev/null 2>&1; then
        echo -e "  ${GREEN}Unmounted${NC}"
    else
        echo -e "  ${YELLOW}Unmount result: $result${NC}"
    fi
}

# Verify FUSE mount by checking filesystem operations
fixture_mount_verify() {
    local bucket="$1"
    local mount_point="$2"
    local node="$3"

    # Expand ~ in mount_point
    mount_point="${mount_point/#\~/$HOME}"

    echo -e "${BLUE}Verifying FUSE mount: $mount_point${NC}"

    # Wait a moment for mount to be ready
    sleep 1

    # Check mount point exists and is accessible
    if [[ ! -d "$mount_point" ]]; then
        echo -e "  ${RED}Mount point not found: $mount_point${NC}"
        return 1
    fi

    # Try to list directory
    if ls "$mount_point" >/dev/null 2>&1; then
        echo -e "  ${GREEN}Directory listing: OK${NC}"
    else
        echo -e "  ${RED}Directory listing: FAILED${NC}"
        return 1
    fi

    # Try to read a file if docs/readme.md exists
    if [[ -f "$mount_point/docs/readme.md" ]]; then
        if head -1 "$mount_point/docs/readme.md" >/dev/null 2>&1; then
            echo -e "  ${GREEN}File read: OK${NC}"
        else
            echo -e "  ${RED}File read: FAILED${NC}"
            return 1
        fi
    fi

    # Try to create a test file
    local test_file="$mount_point/.fuse-test-$$"
    if echo "fuse test" > "$test_file" 2>/dev/null; then
        echo -e "  ${GREEN}File write: OK${NC}"
        rm -f "$test_file" 2>/dev/null
        echo -e "  ${GREEN}File delete: OK${NC}"
    else
        echo -e "  ${YELLOW}File write: SKIPPED (read-only or permission denied)${NC}"
    fi

    echo -e "  ${GREEN}FUSE mount verified${NC}"
}

fixtures_help() {
    echo "Fixtures - set up initial data in dev environment"
    echo ""
    echo "Fixtures are applied automatically on './bin/dev' startup."
    echo ""
    echo "Usage: ./bin/dev fixtures [command]"
    echo ""
    echo "Commands:"
    echo "  apply           Apply all fixtures from fixtures.toml (default)"
    echo "  list            List fixtures without applying"
    echo "  help            Show this help"
    echo ""
    echo "Fixture config: $FIXTURES_FILE"
    echo ""
    echo "Fixture types in TOML:"
    echo "  bucket        - Create a bucket"
    echo "  dir           - Create a directory"
    echo "  file          - Upload a file (inline content or from disk)"
    echo "  share         - Share a bucket with a peer (role: owner/mirror)"
    echo "  publish       - Publish a bucket (grant decryption to mirrors)"
    echo "  mv            - Move/rename a file or directory"
    echo "  mount         - Mount a bucket via FUSE"
    echo "  mount_verify  - Verify FUSE mount filesystem operations"
    echo "  unmount       - Unmount a FUSE-mounted bucket"
    echo ""
    echo "All nodes run both API and gateway servers."
}

fixtures_list() {
    echo -e "${GREEN}Fixtures to apply:${NC}"
    echo ""

    parse_fixtures | while IFS='|' read -r type bucket name path content source node role peer from to mount_point; do
        case "$type" in
            bucket)       echo "  [bucket]       name=$name node=$node" ;;
            dir)          echo "  [dir]          bucket=$bucket path=$path node=$node" ;;
            file)         echo "  [file]         bucket=$bucket path=$path node=$node" ;;
            share)        echo "  [share]        bucket=$bucket peer=$peer role=${role:-owner} node=$node" ;;
            publish)      echo "  [publish]      bucket=$bucket node=$node" ;;
            mv)           echo "  [mv]           bucket=$bucket from=$from to=$to node=$node" ;;
            mount)        echo "  [mount]        bucket=$bucket mount_point=$mount_point node=$node" ;;
            mount_verify) echo "  [mount_verify] bucket=$bucket mount_point=$mount_point node=$node" ;;
            unmount)      echo "  [unmount]      bucket=$bucket node=$node" ;;
        esac
    done
}

fixtures_apply() {
    if [[ ! -f "$FIXTURES_FILE" ]]; then
        echo -e "${YELLOW}No fixtures file found: $FIXTURES_FILE${NC}"
        return 0
    fi

    echo -e "${BLUE}Applying fixtures...${NC}"
    echo ""

    local errors=0

    parse_fixtures | while IFS='|' read -r type bucket name path content source node role peer from to mount_point; do
        node="${node:-$(get_default_node)}"

        case "$type" in
            bucket)       fixture_bucket "$name" "$node" || ((errors++)) ;;
            dir)          fixture_dir "$bucket" "$path" "$node" || ((errors++)) ;;
            file)         fixture_file "$bucket" "$path" "$content" "$source" "$node" || ((errors++)) ;;
            share)        fixture_share "$bucket" "$peer" "${role:-owner}" "$node" || ((errors++)) ;;
            publish)      fixture_publish "$bucket" "$node" || ((errors++)) ;;
            mv)           fixture_mv "$bucket" "$from" "$to" "$node" || ((errors++)) ;;
            mount)        fixture_mount "$bucket" "$mount_point" "$node" || ((errors++)) ;;
            mount_verify) fixture_mount_verify "$bucket" "$mount_point" "$node" || ((errors++)) ;;
            unmount)      fixture_unmount "$bucket" "$node" || ((errors++)) ;;
        esac
    done

    echo ""
    if [[ $errors -eq 0 ]]; then
        echo -e "${GREEN}Fixtures applied successfully${NC}"
    else
        echo -e "${YELLOW}Completed with $errors errors${NC}"
    fi
}

cmd_fixtures() {
    case "${1:-apply}" in
        apply)  fixtures_apply ;;
        list)   fixtures_list ;;
        help|-h|--help) fixtures_help ;;
        *)      echo -e "${RED}Unknown fixtures command: $1${NC}"; fixtures_help; return 1 ;;
    esac
}

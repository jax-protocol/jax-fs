# Final Template Structure - Jax Bucket

## Directory Organization

```
crates/app/templates/
├── layouts/
│   ├── base.html              # Base layout with nav, footer, CodeMirror, etc.
│   └── explorer.html          # Explorer layout with sidebar (extends base.html)
├── pages/
│   ├── index.html            # Bucket list (/ and /buckets)
│   └── buckets/              # Bucket-specific pages (/buckets/:id/*)
│       ├── index.html        # Bucket file browser (/buckets/:id)
│       ├── viewer.html       # File viewer (/buckets/:id/view)
│       ├── editor.html       # Markdown/text editor (/buckets/:id/edit)
│       ├── logs.html         # History viewer (/buckets/:id/logs)
│       ├── pins.html         # Pin management (/buckets/:id/pins)
│       ├── peers.html        # Peer management (/buckets/:id/peers)
│       ├── syncing.html      # Bucket syncing state page
│       └── not_found.html    # Bucket not found error page
└── components/
    ├── inline_editor.html     # Reusable inline editing component
    ├── historical_banner.html # Read-only version banner
    ├── explorer_sidebar.html  # Bucket explorer sidebar
    ├── file_viewer_sidebar.html # File viewer sidebar
    ├── manifest_modal.html    # Manifest details modal
    └── share_modal.html       # Bucket sharing modal
```

## Layout Hierarchy

### Base Layout (`layouts/base.html`)
- Common navigation, footer, fonts, CSS frameworks, CodeMirror
- Used by: bucket list, editor, error pages, syncing pages
- Provides blocks: `title`, `head`, `content`

### Explorer Layout (`layouts/explorer.html`)
- Extends `base.html`
- Adds sidebar structure for bucket-specific pages
- Mobile menu overlay support
- Used by: bucket explorer, file viewer, logs, peers
- Provides blocks: `sidebar`, `content`

## URL Structure Matches Template Structure

| Route | Template | Layout | Description |
|-------|----------|--------|-------------|
| `/` | `pages/index.html` | base | List all buckets (home page) |
| `/buckets` | `pages/index.html` | base | List all buckets |
| `/buckets/:id` | `pages/buckets/index.html` | explorer | Browse bucket files (bucket home) |
| `/buckets/:id/view` | `pages/buckets/viewer.html` | explorer | View file content |
| `/buckets/:id/edit` | `pages/buckets/editor.html` | base | Edit markdown/text files |
| `/buckets/:id/logs` | `pages/buckets/logs.html` | explorer | Browse bucket history |
| `/buckets/:id/pins` | `pages/buckets/pins.html` | base | Manage pins |
| `/buckets/:id/peers` | `pages/buckets/peers.html` | explorer | Manage peers |
| `/gw/:bucket_id/*path` | (JSON/binary response) | - | Gateway API for direct file access |

**Conditional Templates:**
- `pages/buckets/syncing.html` - Shown when bucket is syncing (auto-refreshes every 5s)
- `pages/buckets/not_found.html` - Shown when bucket doesn't exist

## Design Rationale

### Logical Grouping
- **Root page** (`pages/index.html`) shows bucket list - this is the entry point
- **Bucket-specific pages** nested in `/pages/buckets/` mirror the `/buckets/:id/*` URL structure
- `buckets/index.html` is the bucket explorer because `/buckets/:id` is the index page for a bucket

### Index Files Pattern
- `pages/index.html` → Entry point for the app (bucket list)
- `pages/buckets/index.html` → Entry point for a specific bucket (file browser)
- This mirrors standard web conventions where `index.html` is the default page

### No Redundant Naming
- ❌ `bucket_explorer.html` → ✅ `buckets/index.html`
- ❌ `bucket_logs.html` → ✅ `buckets/logs.html`
- ❌ `file_viewer.html` → ✅ `buckets/viewer.html`
- ❌ `peers_explorer.html` → ✅ `buckets/peers.html`

Context is provided by folder structure, not file naming.

## Handler Mappings

**Root page:**
```rust
// src/daemon/http_server/html/buckets.rs
#[template(path = "pages/index.html")]
pub struct BucketsTemplate { /* ... */ }
// Used for both "/" and "/buckets" routes
```

**Bucket-specific pages:**
```rust
// src/daemon/http_server/html/bucket_explorer.rs
#[template(path = "pages/buckets/index.html")]
pub struct BucketExplorerTemplate { /* ... */ }

#[template(path = "pages/buckets/syncing.html")]
pub struct SyncingTemplate { /* ... */ }

#[template(path = "pages/buckets/not_found.html")]
pub struct BucketNotFoundTemplate { /* ... */ }

// src/daemon/http_server/html/file_viewer.rs
#[template(path = "pages/buckets/viewer.html")]
pub struct FileViewerTemplate { /* ... */ }

// src/daemon/http_server/html/file_editor.rs
#[template(path = "pages/buckets/editor.html")]
pub struct FileEditorTemplate { /* ... */ }

// src/daemon/http_server/html/bucket_logs.rs
#[template(path = "pages/buckets/logs.html")]
pub struct BucketLogsTemplate { /* ... */ }

// src/daemon/http_server/html/pins_explorer.rs
#[template(path = "pages/buckets/pins.html")]
pub struct PinsExplorerTemplate { /* ... */ }

// src/daemon/http_server/html/peers_explorer.rs
#[template(path = "pages/buckets/peers.html")]
pub struct PeersExplorerTemplate { /* ... */ }

// src/daemon/http_server/html/gateway.rs
// No template - returns JSON directory listings or binary file content
pub async fn handler(...) -> Response { /* ... */ }
```

## Component Usage

### Modals (included at page bottom)
- `manifest_modal.html` - Inspect bucket manifest details (JSON structure, links, shares)
- `share_modal.html` - Share bucket with peers (add/remove peer access)

### Sidebars (included in explorer layout)
- `explorer_sidebar.html` - Bucket info, manifest details, navigation (index, logs, peers, pins)
- `file_viewer_sidebar.html` - File info, navigation, edit/download actions

### Banners (included conditionally)
- `historical_banner.html` - Shown when `viewing_history=true` (yellow banner with "return to current" link)

### Editors (inline components)
- `inline_editor.html` - Inline file editing (deprecated in favor of dedicated editor page)

## Extending the Structure

### Adding a new root page
```bash
touch crates/app/templates/pages/settings.html
```

```rust
// src/daemon/http_server/html/settings.rs
#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate { /* ... */ }
```

```html
{% extends "layouts/base.html" %}
{% block title %}Settings - Jax{% endblock %}
{% block content %}<!-- content -->{% endblock %}
```

### Adding a new bucket-specific page
```bash
touch crates/app/templates/pages/buckets/stats.html
```

```rust
// src/daemon/http_server/html/bucket_stats.rs
#[derive(Template)]
#[template(path = "pages/buckets/stats.html")]
pub struct BucketStatsTemplate { /* ... */ }
```

Route: `/buckets/:bucket_id/stats`

Use explorer layout if you need a sidebar:
```html
{% extends "layouts/explorer.html" %}
{% block sidebar %}{% include "components/explorer_sidebar.html" %}{% endblock %}
{% block content %}<!-- content -->{% endblock %}
```

### Adding a new layout
```bash
touch crates/app/templates/layouts/minimal.html
```

```html
<!DOCTYPE html>
<html>
<head>
    <title>{% block title %}{% endblock %}</title>
    {% block head %}{% endblock %}
</head>
<body>
    {% block content %}{% endblock %}
</body>
</html>
```

Use in templates:
```html
{% extends "layouts/minimal.html" %}
```

### Adding a new component
```bash
touch crates/app/templates/components/file_card.html
```

```html
<!-- Component with parameters -->
<div class="file-card">
    <h3>{{ file_name }}</h3>
    <p>{{ file_size }}</p>
</div>
```

Include in pages (parameters come from template struct):
```html
{% include "components/file_card.html" %}
```

## Benefits of This Structure

1. **URL-to-File Mapping**: Clear relationship between routes and templates
2. **Logical Nesting**: Bucket pages grouped together, clear hierarchy
3. **No Redundancy**: Folder context eliminates need for prefixes
4. **Scalable**: Easy to add new pages at appropriate levels
5. **Maintainable**: Find files by thinking about URL structure
6. **Layout Reuse**: Two layouts (base + explorer) cover all use cases

## Migration Summary

- ✅ Organized templates into layouts/pages/components
- ✅ Nested bucket-specific pages under pages/buckets/
- ✅ Removed redundant naming (bucket_, _explorer suffixes)
- ✅ Added explorer layout for sidebar-based pages
- ✅ Created dedicated editor page for markdown/text files
- ✅ Added error state templates (syncing, not_found)
- ✅ Built reusable modal and sidebar components
- ✅ Updated all Rust handler template paths
- ✅ Updated all template extends directives
- ✅ All builds passing with 0 warnings

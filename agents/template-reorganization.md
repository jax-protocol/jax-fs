# Template Reorganization - Jax Bucket

## Final Template Structure

```
crates/app/templates/
├── layouts/
│   ├── base.html              # Base layout with nav, footer, head includes
│   └── explorer.html          # Explorer layout with sidebar (extends base)
├── pages/
│   ├── index.html            # Bucket list (/ and /buckets)
│   └── buckets/              # Bucket-specific pages (/buckets/:id/*)
│       ├── index.html        # Bucket file browser
│       ├── viewer.html       # File content viewer
│       ├── editor.html       # Markdown/text editor
│       ├── logs.html         # History viewer
│       ├── pins.html         # Pin management
│       ├── peers.html        # Peer management
│       ├── syncing.html      # Bucket syncing state page
│       └── not_found.html    # Bucket not found error page
└── components/
    ├── inline_editor.html     # Inline editing (deprecated)
    ├── historical_banner.html # Read-only version banner
    ├── explorer_sidebar.html  # Bucket explorer sidebar
    ├── file_viewer_sidebar.html # File viewer sidebar
    ├── manifest_modal.html    # Manifest details modal
    └── share_modal.html       # Bucket sharing modal
```

## Changes Made

### 1. Directory Structure
- Created `layouts/` for base templates
  - `base.html` - Primary layout with nav/footer
  - `explorer.html` - Extends base, adds sidebar structure
- Created `pages/` for page templates
  - Root: `index.html` (bucket list)
  - Buckets: Nested under `buckets/` subdirectory
- Expanded `components/` for reusable components
  - Modals: manifest_modal, share_modal
  - Sidebars: explorer_sidebar, file_viewer_sidebar
  - Banners: historical_banner
  - Editors: inline_editor (deprecated)

### 2. Template Paths Updated

**All page templates extend appropriate layout:**
```html
<!-- Bucket list, editor, pins -->
{% extends "layouts/base.html" %}

<!-- Bucket explorer, file viewer, logs, peers -->
{% extends "layouts/explorer.html" %}
```

### 3. Rust Handler Updates

All template struct attributes updated to new paths:
```rust
// Pages
#[template(path = "pages/index.html")]              // BucketsTemplate
#[template(path = "pages/buckets/index.html")]      // BucketExplorerTemplate
#[template(path = "pages/buckets/viewer.html")]     // FileViewerTemplate
#[template(path = "pages/buckets/editor.html")]     // FileEditorTemplate
#[template(path = "pages/buckets/logs.html")]       // BucketLogsTemplate
#[template(path = "pages/buckets/pins.html")]       // PinsExplorerTemplate
#[template(path = "pages/buckets/peers.html")]      // PeersExplorerTemplate
#[template(path = "pages/buckets/syncing.html")]    // SyncingTemplate
#[template(path = "pages/buckets/not_found.html")]  // BucketNotFoundTemplate
```

### 4. Files Modified

**Rust handlers:**
- `src/daemon/http_server/html/buckets.rs` - Bucket list page
- `src/daemon/http_server/html/bucket_explorer.rs` - Explorer, syncing, not_found
- `src/daemon/http_server/html/file_viewer.rs` - File viewer
- `src/daemon/http_server/html/file_editor.rs` - Editor page
- `src/daemon/http_server/html/bucket_logs.rs` - History viewer
- `src/daemon/http_server/html/pins_explorer.rs` - Pins page
- `src/daemon/http_server/html/peers_explorer.rs` - Peers page
- `src/daemon/http_server/html/gateway.rs` - Gateway (no template)

**Templates:**
- All page templates updated to extend appropriate layout
- All components properly included where needed

## Benefits

1. **Better Organization**: Clear separation of layouts, pages, and components
2. **Scalability**: Easy to add new layouts or component types
3. **Maintainability**: Logical grouping makes finding templates easier
4. **Conventions**: Follows common templating patterns (Rails, Django, etc.)
5. **DRY Principle**: Explorer layout reused across multiple bucket pages
6. **Component Reuse**: Modal and sidebar components shared across pages

## Layout Hierarchy

### Base Layout (`layouts/base.html`)
- Used by: bucket list, editor, pins, error pages
- Provides: navigation, footer, CodeMirror, CSS/JS includes
- Blocks: `title`, `head`, `content`

### Explorer Layout (`layouts/explorer.html`)
- Extends: `base.html`
- Used by: bucket explorer, file viewer, logs, peers
- Adds: sidebar structure, mobile menu overlay
- Blocks: `sidebar`, `content` (inherits `title`, `head` from base)

## Component Usage Patterns

### Modals (included at page bottom)
```html
{% include "components/manifest_modal.html" %}
{% include "components/share_modal.html" %}
```

### Sidebars (included in sidebar block)
```html
{% block sidebar %}
{% include "components/explorer_sidebar.html" %}
{% endblock %}
```

### Banners (included conditionally)
```html
{% if viewing_history %}
{% include "components/historical_banner.html" %}
{% endif %}
```

## Creating New Templates

### New Page (extends base layout)
```bash
touch crates/app/templates/pages/my_page.html
```

```html
{% extends "layouts/base.html" %}

{% block title %}My Page - Jax{% endblock %}

{% block content %}
<div class="max-w-6xl mx-auto px-8 py-6">
    <h1>My Page</h1>
    <p>{{ data }}</p>
</div>
{% endblock %}
```

### New Bucket Page (extends explorer layout)
```bash
touch crates/app/templates/pages/buckets/stats.html
```

```html
{% extends "layouts/explorer.html" %}

{% block title %}{{ bucket_name }} - Stats - Jax{% endblock %}

{% block sidebar %}
{% include "components/explorer_sidebar.html" %}
{% endblock %}

{% block content %}
<div class="px-8 py-6">
    <h1>Bucket Statistics</h1>
    <!-- stats content -->
</div>
{% endblock %}
```

### New Layout
```bash
touch crates/app/templates/layouts/minimal.html
```

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{% endblock %}</title>
    {% block head %}{% endblock %}
</head>
<body>
    {% block content %}{% endblock %}
</body>
</html>
```

### New Component
```bash
touch crates/app/templates/components/my_component.html
```

```html
<!-- Component with expected parameters -->
<div class="my-component">
    <h3>{{ param1 }}</h3>
    <p>{{ param2 }}</p>
</div>
```

Include it:
```html
{% include "components/my_component.html" %}
```

## Route → Template → Layout Mapping

| Route | Handler | Template | Layout | Sidebar |
|-------|---------|----------|--------|---------|
| `/` | buckets | `pages/index.html` | base | - |
| `/buckets` | buckets | `pages/index.html` | base | - |
| `/buckets/:id` | bucket_explorer | `pages/buckets/index.html` | explorer | explorer_sidebar |
| `/buckets/:id/view` | file_viewer | `pages/buckets/viewer.html` | explorer | file_viewer_sidebar |
| `/buckets/:id/edit` | file_editor | `pages/buckets/editor.html` | base | - |
| `/buckets/:id/logs` | bucket_logs | `pages/buckets/logs.html` | explorer | explorer_sidebar |
| `/buckets/:id/pins` | pins_explorer | `pages/buckets/pins.html` | base | - |
| `/buckets/:id/peers` | peers_explorer | `pages/buckets/peers.html` | explorer | explorer_sidebar |

**Conditional Pages:**
- Syncing: `pages/buckets/syncing.html` (base layout)
- Not Found: `pages/buckets/not_found.html` (base layout)

## Migration Status

### ✅ Completed
- Organized templates into layouts/pages/components
- Nested bucket-specific pages under pages/buckets/
- Removed redundant naming (bucket_, _explorer suffixes)
- Added explorer layout for sidebar-based pages
- Created dedicated editor page for markdown/text files
- Added error state templates (syncing, not_found)
- Built reusable modal and sidebar components
- Updated all Rust handler template paths
- Updated all template extends directives
- All builds passing with 0 compilation warnings

### 📦 Components Created
1. **Modals** (2):
   - `manifest_modal.html` - Inspect bucket manifest
   - `share_modal.html` - Share buckets with peers

2. **Sidebars** (2):
   - `explorer_sidebar.html` - Bucket navigation/info
   - `file_viewer_sidebar.html` - File actions/info

3. **Banners** (1):
   - `historical_banner.html` - Historical version indicator

4. **Editors** (1):
   - `inline_editor.html` - Deprecated (use editor page)

### 🗑️ Deprecated
- `inline_editor.html` component - Use `/buckets/:id/edit` page instead
- Component kept for backward compatibility but not actively used

### 🎯 Current State
- **9 pages** (1 root + 8 bucket pages)
- **2 layouts** (base + explorer)
- **6 components** (modals, sidebars, banners, deprecated editor)
- **0 compilation warnings**
- **Type-safe** template rendering
- **Compile-time** template validation

## Best Practices Established

### Template Organization
✅ Clear separation: layouts / pages / components
✅ Logical nesting: bucket pages under buckets/ folder
✅ Consistent naming: no redundant prefixes
✅ Layout inheritance: base → explorer hierarchy
✅ Component reuse: modals and sidebars shared across pages

### Rust Handler Patterns
✅ All handlers in `src/daemon/http_server/html/`
✅ Template paths match directory structure
✅ Conditional templates for error states
✅ Consistent struct field naming
✅ Tracing instrumentation on all handlers

### Styling & Assets
✅ CSS variables for theming
✅ Dark mode support throughout
✅ Embedded static assets (rust-embed)
✅ CDN dependencies for external libraries
✅ Module pattern for JavaScript organization

## Migration Complete ✨

The template reorganization is complete and production-ready. All templates follow the new structure, all handlers have been updated, and the codebase compiles with zero warnings. The system now has:

- A clear, scalable template hierarchy
- Reusable layouts and components
- Logical URL → file path mapping
- Type-safe, compile-time validated templates
- Comprehensive documentation

Future additions should follow the established patterns documented here.

# Templating and UI System - Jax Bucket

## Template Engine: Askama

- Jinja2-like template syntax
- Compile-time template checking
- Type-safe template rendering
- Integration with Axum via `askama_axum`

## Template Structure

### Layouts

**Base Template: `templates/layouts/base.html`**
All pages extend the base template which provides:
- **Blocks:** `title`, `head`, `content`
- Common head elements (fonts, CSS frameworks, CodeMirror)
- Navigation bar
- Footer
- Dark mode support

**Explorer Template: `templates/layouts/explorer.html`**
Extends base.html and adds:
- **Blocks:** `sidebar`, `content`
- Sidebar structure for bucket-specific pages
- Mobile menu overlay support
- Used by bucket explorer, file viewer, logs, peers pages

### Current Template Hierarchy

```
templates/
├── layouts/
│   ├── base.html (parent layout)
│   └── explorer.html (extends base, adds sidebar)
├── pages/
│   ├── index.html (bucket list)
│   └── buckets/
│       ├── index.html (file browser)
│       ├── viewer.html (file content viewer)
│       ├── editor.html (markdown/text editor)
│       ├── logs.html (history viewer)
│       ├── pins.html (pin management)
│       ├── peers.html (peer management)
│       ├── syncing.html (bucket syncing state)
│       └── not_found.html (bucket not found error)
└── components/
    ├── inline_editor.html (reusable inline editing - deprecated)
    ├── historical_banner.html (read-only version banner)
    ├── explorer_sidebar.html (bucket info sidebar)
    ├── file_viewer_sidebar.html (file viewer sidebar)
    ├── manifest_modal.html (manifest inspection modal)
    └── share_modal.html (bucket sharing modal)
```

## UI Framework

### Franken UI (Primary)
- Modern UI component library based on UIKit
- CDN-hosted components
- Modal dialogs, tables, forms, grid system

### Custom CSS: `static/style.css`

**Design System:**
- HSL-based CSS variables for theming
- Light/Dark mode support
- Monochromatic palette (black/white/grays)
- Tailwind-like utility classes

**CSS Variables:**
```css
:root {
  --background: 0 0% 100%;
  --foreground: 0 0% 0%;
  --primary: 0 0% 0%;
  --muted: 0 0% 95%;
  /* etc. */
}

.dark {
  --background: 0 0% 0%;
  --foreground: 0 0% 100%;
  /* inverted */
}
```

## Static Assets

### Organization
```
static/
├── 404.html
├── app.js (main JavaScript - 451 lines)
├── style.css (custom styles - 1,763 lines)
└── js/
    ├── inline-editor.js (inline file editing - 208 lines)
    ├── note-editor.js (full-screen note editor - 337 lines)
    └── tree-viewer.js (history tree visualization - 271 lines)
```

### Asset Serving: `rust-embed`
- Embeds static files into binary at compile time
- Route: `/static/*path`
- No runtime file system access needed
- Handler: `src/daemon/http_server/mod.rs`

### External CDN Dependencies
- Inter Font (rsms.me)
- Franken UI (CSS framework)
- Font Awesome 5 (icons)
- CodeMirror 6 (ES modules from esm.sh)
- Marked.js (Markdown rendering)

## JavaScript Integration

### Global CodeMirror Loading (base.html)
```html
<script type="module">
  import { EditorView, basicSetup } from "https://esm.sh/codemirror@6.0.1";
  import { markdown } from "https://esm.sh/@codemirror/lang-markdown@6.2.4";
  import { oneDark } from "https://esm.sh/@codemirror/theme-one-dark@6.1.2";

  window.CodeMirror = { EditorView, basicSetup, markdown, oneDark };
</script>
```

### Main JavaScript: `app.js`

**Modules:**
- BucketCreation
- FileUpload
- BucketShare
- FileRename
- FileDelete
- NewFile

**Initialization Pattern:**
```javascript
document.addEventListener("DOMContentLoaded", function() {
  const apiUrl = window.JAX_API_URL || "http://localhost:3000";
  const bucketId = window.JAX_BUCKET_ID;

  BucketCreation.init(apiUrl);
  if (bucketId) {
    FileUpload.init(apiUrl, bucketId);
    // etc.
  }
});
```

### ES6 Modules: `static/js/`

**inline-editor.js** (deprecated - use editor page instead)
```javascript
export function initInlineEditor(bucketId, filePath, isMarkdown) { }
export function renderMarkdown(content) { }
```

**note-editor.js** (full-screen editor for `/buckets/:id/edit`)
```javascript
export function initNoteEditor(config) {
  // config: { bucketId, filePath, currentPath, isNewFile, originalFilename, apiUrl, backUrl }
  // Creates CodeMirror 6 editor with markdown support
  // Split view with live preview
  // Save/cancel buttons
}
```

**tree-viewer.js** (history visualization for `/buckets/:id/logs`)
```javascript
export function renderHistoryTree(logs) {
  // Renders ASCII-art tree visualization of bucket history
  // Links are clickable to navigate to specific versions
}
```

### Global Configuration Pattern

Templates inject config via window variables:
```html
<script>
window.JAX_API_URL = '{{ api_url }}';
window.JAX_BUCKET_ID = '{{ bucket_id }}';
window.JAX_FILE_PATH = '{{ file_path }}';
window.JAX_IS_MARKDOWN = {{ is_markdown }};
</script>
```

## Routing and Template Rendering

### Route Handler Pattern

**File:** `src/daemon/http_server/html/[handler].rs`

```rust
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{State, Path, Query};
use tracing::instrument;

#[derive(Template)]
#[template(path = "pages/template_name.html")]
pub struct TemplateStruct {
    pub field1: String,
    pub field2: Vec<Item>,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(id): Path<Uuid>,
) -> askama_axum::Response {
    let template = TemplateStruct { /* ... */ };
    template.into_response()
}
```

### Routes Map

| Route | Handler | Template | Layout |
|-------|---------|----------|--------|
| `/` | `buckets::handler` | `pages/index.html` | base |
| `/buckets` | `buckets::handler` | `pages/index.html` | base |
| `/buckets/:id` | `bucket_explorer::handler` | `pages/buckets/index.html` | explorer |
| `/buckets/:id/view` | `file_viewer::handler` | `pages/buckets/viewer.html` | explorer |
| `/buckets/:id/edit` | `file_editor::handler` | `pages/buckets/editor.html` | base |
| `/buckets/:id/logs` | `bucket_logs::handler` | `pages/buckets/logs.html` | explorer |
| `/buckets/:id/pins` | `pins_explorer::handler` | `pages/buckets/pins.html` | base |
| `/buckets/:id/peers` | `peers_explorer::handler` | `pages/buckets/peers.html` | explorer |
| `/gw/:bucket_id/*path` | `gateway::handler` | (none - JSON/binary) | - |

**Conditional templates (rendered by bucket_explorer::handler):**
- `pages/buckets/syncing.html` - When bucket is syncing
- `pages/buckets/not_found.html` - When bucket doesn't exist

## Creating New Pages

### 1. Create Template
```bash
touch crates/app/templates/pages/my_page.html
```

```html
{% extends "layouts/base.html" %}

{% block title %}My Page - Jax{% endblock %}

{% block content %}
<div class="max-w-6xl mx-auto px-8 py-6 space-y-6">
    <h1 class="text-3xl font-bold">My Page</h1>
    <p>{{ data }}</p>
</div>
{% endblock %}
```

### 2. Create Handler
```bash
touch crates/app/src/daemon/http_server/html/my_page.rs
```

```rust
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::ServiceState;

#[derive(Template)]
#[template(path = "pages/my_page.html")]
pub struct MyPageTemplate {
    pub data: String,
}

pub async fn handler(
    State(state): State<ServiceState>
) -> askama_axum::Response {
    let template = MyPageTemplate {
        data: "Hello".to_string()
    };
    template.into_response()
}
```

### 3. Register Module
In `src/daemon/http_server/html/mod.rs`:
```rust
mod my_page;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/my-page", get(my_page::handler))
        // ...existing routes...
}
```

## Creating Reusable Components

### 1. Create Component
```bash
touch crates/app/templates/components/my_component.html
```

```html
<!-- Required parameters: param1, param2 -->
<div class="card p-4">
    <h3 class="text-lg font-semibold">{{ param1 }}</h3>
    <p class="text-muted-foreground">{{ param2 }}</p>
</div>
```

### 2. Include in Template
```html
{% block content %}
{% include "components/my_component.html" %}
{% endblock %}
```

### 3. Pass Parameters via Template Struct
Handler provides data:
```rust
#[derive(Template)]
#[template(path = "pages/parent.html")]
pub struct ParentTemplate {
    pub param1: String,
    pub param2: String,
}
```

The component automatically accesses `param1` and `param2` from the parent template's context.

## Best Practices

### Template Organization
✅ Single base template for consistency
✅ Explorer layout for sidebar-based pages
✅ Components in dedicated directory
✅ Consistent block structure
✅ Breadcrumb navigation

### Styling
✅ CSS variable theming system
✅ Dark mode support
✅ Utility-first CSS
✅ Component-scoped styles

### JavaScript
✅ Module pattern organization
✅ Global configuration via window variables
✅ ES6 modules for new code
✅ Progressive enhancement

### Security
✅ Type-safe templates (compile-time)
✅ Embedded assets (no file system access)
✅ CORS configuration
✅ Read-only mode support

## API Endpoints for Frontend

All API calls go to `/api/v0/bucket/*`:

### Bucket Management
- `POST /api/v0/bucket/` - Create bucket (expects: name)
- `POST /api/v0/bucket/list` - List buckets (optional: limit, offset)
- `POST /api/v0/bucket/share` - Share bucket (expects: bucket_id, peer_id, role)
- `POST /api/v0/bucket/export` - Export bucket (expects: bucket_id)
- `POST /api/v0/bucket/ping` - Ping bucket (expects: bucket_id)

### File Operations
- `POST /api/v0/bucket/add` - Add file (multipart: bucket_id, path, files[])
- `POST /api/v0/bucket/update` - Update file (multipart: bucket_id, mount_path, file)
- `POST /api/v0/bucket/rename` - Rename file (expects: bucket_id, old_path, new_path)
- `POST /api/v0/bucket/delete` - Delete file (expects: bucket_id, path)
- `POST /api/v0/bucket/mkdir` - Create directory (expects: bucket_id, path)
- `POST /api/v0/bucket/ls` - List files (expects: bucket_id, path)
- `POST /api/v0/bucket/cat` - Read file (expects: bucket_id, path)
- `GET /api/v0/bucket/cat?bucket_id=X&path=Y` - Read file (GET variant)

All POST endpoints expect either JSON or multipart form data.
File upload endpoints (add, update) require multipart/form-data.

### Gateway Endpoint
- `GET /gw/:bucket_id/*file_path` - Direct file/directory access
  - Returns JSON for directories (file listing)
  - Returns file content with proper MIME type for files
  - No authentication required (public gateway)

## Development Workflow

1. **Edit templates** in `crates/app/templates/`
2. **Edit static assets** in `crates/app/static/`
3. **Build**: `cargo build` (embeds static assets at compile time)
4. **Run**: `cargo run` or restart daemon
5. **Templates are compiled at build time** - errors caught early
6. **Hot reload**: Edit templates and rebuild for changes

## Historical Version Support

Templates support viewing historical bucket states via `?at={hash}` query parameter:

- `bucket_explorer.html` (buckets/index.html) - Browse files at specific version
- `file_viewer.html` (buckets/viewer.html) - View file content at specific version
- Historical views are automatically set to read-only
- Yellow banner component (`historical_banner.html`) indicates historical mode
- "Return to current version" links provided in banner
- History navigation in logs page (`buckets/logs.html`)

## Component Library

### Modals

**manifest_modal.html** (396 lines)
- Displays bucket manifest details (JSON structure)
- Shows: version, height, entry link, pins link, previous link
- Lists shares (peers and their roles)
- Copy-to-clipboard buttons for all links
- Accessible via "Manifest" button in explorer sidebar

**share_modal.html** (262 lines)
- Share bucket with peers
- Add peer by ID with role selection (reader/writer/owner)
- Remove existing shares
- Form validation and error handling
- Accessible via "Share" button in bucket pages

### Sidebars

**explorer_sidebar.html** (102 lines)
- Bucket information (ID, name, manifest link)
- Navigation: Index, History, Peers, Pins
- Manifest and Share buttons
- Used in: bucket explorer, logs, peers pages

**file_viewer_sidebar.html** (123 lines)
- File information (name, size, type)
- Actions: Edit, Download, Delete
- Navigation back to parent directory
- Manifest and Share buttons
- Used in: file viewer page

### Banners

**historical_banner.html**
- Yellow warning banner
- Displays when `viewing_history=true`
- Shows current version hash being viewed
- "Return to current version" link
- Auto-included in explorer layout when appropriate

### Editors (Deprecated)

**inline_editor.html** (208 lines)
- Inline file editing with CodeMirror
- **Status**: Deprecated - use `/buckets/:id/edit` page instead
- Kept for backward compatibility

## Editor Implementation

The markdown/text editor (`/buckets/:id/edit`) uses:
- **CodeMirror 6** for editing
- **Marked.js** for markdown preview
- **Split view**: Edit | Preview | Split modes
- **Auto-save drafts** (localStorage)
- **Filename editing** inline
- **Back navigation** to previous page or directory

Initialization in `static/js/note-editor.js`:
```javascript
initNoteEditor({
  bucketId: '...',
  filePath: '/path/to/file.md',
  currentPath: '/path/to',
  isNewFile: true,
  originalFilename: 'untitled-1.md',
  apiUrl: 'http://localhost:3000',
  backUrl: '/buckets/abc123'
});
```

## State Management

Templates receive state through handler structs:
- `read_only: bool` - Enables/disables edit features
- `viewing_history: bool` - Triggers historical banner
- `at_hash: Option<String>` - Current version hash for historical views
- `api_url: String` - API endpoint for JavaScript

All state flows from Rust handlers → Template structs → Rendered HTML → JavaScript global vars.

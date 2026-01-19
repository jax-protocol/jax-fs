# Tauri Migration

**Status:** Future
**Track:** Local
**Reference:** `amiller68/tauri-app-explore` branch, `issues/tauri-desktop-app.md`

## Objective

Replace native Rust desktop integration with Tauri app, providing a full SolidJS GUI while keeping the existing Askama web UI accessible.

## Background

Ticket 4 implements basic desktop integration using native Rust crates (tray-icon, auto-launch). This future ticket explores migrating to Tauri for:
- Richer native GUI (not just web UI in browser)
- SolidJS frontend with better UX
- Native file dialogs
- Better platform integration

## Architecture

```
jax daemon --gateway-only (gateway)
├── Read-only HTML file explorer
├── Gateway handler serves published buckets
└── No bucket management UI

jax (Tauri - local app)
├── Full SolidJS frontend
├── Tauri IPC → Rust backend
├── Backend includes full P2P peer
├── Does NOT serve Askama UI
└── All bucket management via SolidJS
```

**Key decision**: Askama templates stay in gateway for serving published content. Tauri app uses SolidJS exclusively for local bucket management - no Askama UI served from the local app.

## Implementation Steps (if pursued)

1. Set up Tauri 2.0 project structure in `crates/tauri/`
2. Integrate SolidJS with Vite
3. Implement IPC commands mirroring REST API
4. Build SolidJS UI pages (buckets list, explorer, viewer, editor)
5. Add tray integration via Tauri
6. Remove native desktop deps from ticket 4
7. Remove Askama UI routes from local app (keep only in gateway)
8. Move Askama templates to gateway-specific location

## Reference

See `amiller68/tauri-app-explore` branch for working prototype with:
- SolidJS frontend
- Tauri IPC commands
- System tray
- Full P2P peer

## Acceptance Criteria

TBD based on architectural decision.

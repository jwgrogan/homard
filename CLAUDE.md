# CLAUDE.md

## Project Overview

arcctl (Agent Run Control) — a macOS menu bar assistant that wraps Claude Code CLI, providing session management, settings GUI, autonomous scheduling via launchd, and messaging bridges (Telegram, email).

## Architecture

Cargo workspace with three crates sharing a core library:
- `crates/arcctl-core/` — Shared Rust library (config, store, profiles, process management, launchd)
- `src-tauri/` — Tauri 2 GUI app (system tray, IPC commands, React frontend)
- `cli/` — CLI binary (`arcctl run-job`, `arcctl status`, `arcctl switch`)

Frontend: React 18 + TypeScript + Vite + Tailwind CSS 4

## Build & Test Commands

```bash
cargo build                    # Build all Rust crates
cargo test                     # Run all Rust tests
cargo tauri dev                # Launch Tauri app in dev mode
npm run dev                    # Frontend dev server only
npm run build                  # Build frontend for production
cargo tauri build              # Build production app bundle
```

## Key Directories

- `~/.arcctl/` — App data (config, profiles, schedules, logs, SQLite DB)
- `~/.claude/` — Claude Code config (settings.json, agents/, commands/)
- `~/Library/LaunchAgents/com.arcctl.job.*.plist` — Scheduled job plists

## Code Conventions

- Rust: standard formatting (rustfmt), clippy clean
- TypeScript: strict mode
- All secrets stored in macOS Keychain via security CLI
- Config files are JSON on disk

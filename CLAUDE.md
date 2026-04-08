# CLAUDE.md

## Project Overview

Homard — a lightweight macOS personal AI assistant. Always-on daemon with Tauri tray app, Telegram bridge, and ReAct agent loop. Supports OpenAI OAuth PKCE and Anthropic OAuth for LLM providers.

## Architecture

Cargo workspace with three crates:
- `crates/homard-core/` — Core library (agent loop, LLM client, tools, security, REST API, Telegram, scheduler)
- `src-tauri/` — Thin Tauri 2 tray app (just tray icon + React webview, no business logic)
- `cli/` — CLI binary (`homard serve`, `homard chat`, `homard status`, `homard stop`, `homard install`)

Two-binary split: daemon (`homard serve`) runs agent + API on localhost:17700, tray app talks to it via REST.

Frontend: React 19 + TypeScript + Vite + Tailwind CSS 4 (light mode only, coastal palette)

## Build & Test Commands

```bash
cargo build                    # Build all Rust crates
cargo test                     # Run all Rust tests
cargo tauri dev                # Launch Tauri tray app in dev mode
npm run dev                    # Frontend dev server only
npm run build                  # Build frontend for production
cargo tauri build              # Build production app bundle
./target/debug/homard serve    # Start daemon directly
./target/debug/homard chat     # Interactive CLI chat
```

## Key Directories

- `~/.homard/` — App data (config, identity files, schedules, logs, SQLite DB)
- `~/.homard/*.md` — Identity files (SOUL.md, USER.md, MEMORY.md, AGENTS.md, TOOLS.md, HEARTBEAT.md, IDENTITY.md, BOOTSTRAP.md)
- `~/Library/LaunchAgents/com.homard.daemon.plist` — Always-on daemon plist

## Code Conventions

- Rust: standard formatting (rustfmt), clippy clean
- TypeScript: strict mode
- All secrets stored in macOS Keychain via security-framework crate
- Config files are JSON on disk
- Identity files are Markdown

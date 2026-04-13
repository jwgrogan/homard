# Homard Development Session Notes

**Date:** 2026-04-08 to 2026-04-10
**Duration:** ~2 days of intensive development

---

## What Was Built

Homard — a lightweight macOS personal AI assistant. Complete rewrite from homard (a Claude Code CLI wrapper) into an autonomous agent platform.

### Architecture
- **Daemon** (`homard serve`): Rust, axum REST API on localhost:17700, codex app-server for sub-second chat, Telegram poller, cron scheduler
- **Tray app** (`Homard.app`): Tauri 2 + React, thin UI shell that talks to daemon via REST
- **CLI** (`homard`): setup wizard, chat, status, install/uninstall

### Key Technical Decisions

1. **Codex app-server** (not subprocess per message): JSON-RPC over stdio, persistent WebSocket to OpenAI inside codex. 15s → <1s per message.

2. **CLI backends** (not OAuth): OpenAI OAuth can't hit chatgpt.com/backend-api from REST (CloudFlare). Anthropic OAuth doesn't work with Messages API. Solution: run `codex exec` / `claude -p` as backends, using users' existing CLI auth. Bills to their subscription.

3. **Identity files** (not just a system prompt): SOUL.md, USER.md, MEMORY.md, AGENTS.md, TOOLS.md, HEARTBEAT.md — OpenClaw-style deep context. Agent maintains USER.md and MEMORY.md over time.

4. **Telegram username allowlist** (not pairing codes): Standard pairing flows suck. Just add your @username in settings, bot auto-accepts your messages.

5. **Server mode toggle**: launchd KeepAlive for crash recovery + boot start, but easily toggled off. Unlike OpenClaw where always-on is hard to disable.

6. **Two-binary split**: Daemon has all logic, tray is pure UI. Daemon survives tray quit/crash. Can run headless (server + Telegram only).

### Performance
- Chat (codex, warm): <1s
- Chat (codex, cold/first): ~2s (pre-warmed at startup)
- Chat (claude CLI): ~7s (subprocess per message, --continue helps)
- API endpoints: <15ms
- Memory: 10-14 MB daemon
- Binary: 12 MB
- DMG: 3.9 MB

### Security
- API bearer token (48-char random, stored in ~/.homard/api.token)
- CORS restricted to tauri://localhost
- Shell sandbox (29 blocked patterns + normalized whitespace)
- File write restrictions (~/.homard/ only)
- Hard iteration cap (50) on agent loop
- Codex app-server crash recovery (auto-restart)

### What Didn't Work
- **OAuth for OpenAI**: chatgpt.com/backend-api is CloudFlare-protected. Can't call it via REST. The Codex CLI handles it internally via WebSocket but that's not exportable.
- **OAuth for Anthropic**: Tokens don't work with Messages API. Only with Claude Code/Claude.ai.
- **Overlay title bar**: One-drag-then-stuck bug in Tauri. Switched to standard decorations.
- **Custom drag handling**: CSS -webkit-app-region broken with transparent windows. Tauri startDragging() API unreliable. Standard title bar is the answer.
- **Chat bubbles vs flat**: Tried flat Slack-style, user hated it. Went back to iMessage-style bubbles.

### Codebase
- 39 Rust source files, ~16K LOC total
- 34 tests
- 40+ commits since rewrite
- React frontend: 5 pages (Chat, Activity, Settings with 6 sub-tabs)

---

## Product Direction

### Core Thesis
Homard is an **ultralightweight OpenClaw** — personal AI daemon on your machine, reachable via Telegram, that dispatches to Claude/Codex CLIs. It's not an IDE, it's a control plane.

### Competitive Position
- OpenClaw: too heavy, too complex
- Claude Channels: not shipping, gated
- Codex/Claude desktop: no remote access, no always-on
- Telegram bots: too simple, no tools/memory/identity

### Roadmap
- v1.0: Daily driver (DONE)
- v1.1: Multi-machine (Windows, separate bots per machine)
- v2.0: Telegram Mini App (web UI inside Telegram, multi-machine dispatch)
- v2.1: Smart assistant (MCP integration, todos, proactive notifications)
- v3.0: Platform (multi-user, shared memory, agent marketplace)

### Distribution
- Primary: `brew install homard` → `homard setup` → done
- Windows: `.exe` + `homard setup`
- Key insight: Telegram is the distribution channel, not an app store

---

## User Preferences (James Grogan)

- Wants terse, direct communication
- Prefers vertical slice MVPs
- Rust for systems, Swift for iOS
- Doesn't like Python for always-on (memory issues)
- Uses Codex CLI with ChatGPT subscription as primary
- Claude CLI for coding dispatch
- OpenClaw user but frustrated with complexity
- Wants things that "just work" with minimal config
- Likes iMessage-style chat UI, not flat/Slack-style
- Standard macOS window chrome > custom title bars
- Telegram pairing should be dead simple (username allowlist)
- Always-on server mode should be easily togglable
- The "install should install the CLI tools too" insight is key for non-technical users

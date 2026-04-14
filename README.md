# 🦞 Homard

A lightweight, always-on macOS personal AI assistant that lives in your menu bar and is reachable via Telegram.

Homard is your personal control plane — it can chat, run shell commands, search the web, manage files, maintain long-term memory, and spin up full Claude Code or Codex CLI sessions for complex coding tasks. Configure it via markdown identity files, not YAML configs.

## Features

- **ReAct agent loop** with tool calling (shell, web, files, memory, CLI sessions)
- **LLM providers**: OpenAI (OAuth PKCE), Anthropic (OAuth PKCE), OpenRouter (API key)
- **Telegram integration**: full bidirectional chat, pair via code, remote `/stop`, `/server` toggle
- **CLI session orchestration**: spawn Claude Code or Codex sessions, track output, kill remotely
- **Identity files**: SOUL.md, USER.md, MEMORY.md, AGENTS.md — the agent learns and remembers
- **Cron scheduler**: HEARTBEAT.md for natural-language periodic tasks with health metrics
- **Server mode**: launchd KeepAlive — survives crashes, starts on boot, toggleable on/off
- **Security**: supervised/autonomous/locked permission levels, shell sandbox, file path restrictions
- **Tray app**: Tauri popover with Chat, Activity, and Settings views
- **Coastal design**: sage, cream, coral, navy — not another dark-mode dev tool

## Quick Start

```bash
# Build
cargo build --release

# Start the daemon
./target/release/homard serve

# Interactive chat
./target/release/homard chat

# One-shot
./target/release/homard chat -m "what's the weather in Boston?"

# Enable server mode (survives reboots)
./target/release/homard install

# Launch the tray app
cargo tauri dev
```

## Architecture

```
Telegram ──→
              ┌─────────────────────────────────────┐
Chat UI  ──→ │          Homard Daemon               │
              │  ┌──────────────┐                    │
Cron     ──→ │  │  Agent Loop   │──→ LLM (OpenAI/   │
              │  │  (ReAct)      │    Anthropic/      │
              │  │               │    OpenRouter)     │
              │  │  Tools:       │                    │
              │  │  · shell_exec │                    │
              │  │  · web_search │                    │
              │  │  · file_read/write                 │
              │  │  · memory_save/search              │
              │  │  · spawn_session ──→ Claude Code   │
              │  │  · spawn_session ──→ Codex CLI     │
              │  │  · kill_session│                    │
              │  └──────────────┘                    │
              │  REST API :17700                     │
              └───────────┬─────────────────────────┘
                          │
              ┌───────────┴─────────────────────────┐
              │        Homard.app (Tray)             │
              │  Chat │ Activity │ Settings          │
              └─────────────────────────────────────┘
```

Two binaries: the **daemon** (`homard serve`) runs the agent, Telegram poller, cron scheduler, and REST API. The **tray app** (`Homard.app`) is a thin Tauri shell that talks to the daemon via localhost.

## Identity Files

Homard's personality, knowledge, and behavior are configured through markdown files in `~/.homard/`:

| File | Purpose | Who edits it |
|------|---------|-------------|
| `IDENTITY.md` | Name, emoji, tagline | User (or agent on request) |
| `SOUL.md` | Personality, tone, communication style | User |
| `USER.md` | Everything about you | Agent (learns over time) |
| `AGENTS.md` | Operational policies, coding delegation rules | User |
| `TOOLS.md` | Environment-specific tool notes | User |
| `MEMORY.md` | Learned patterns, preferences, events | Agent (self-maintains) |
| `HEARTBEAT.md` | Periodic task checklists | User |
| `BOOTSTRAP.md` | First-run setup prompt (runs once) | System |

## Coding: Self vs Delegate

Homard can code directly (file_read/write + shell_exec) or delegate to Claude Code/Codex. The decision framework in `AGENTS.md`:

- **Direct**: small edits, single commands, quick fixes
- **Delegate**: multi-file changes, debugging, new features, refactoring
- **Rule of thumb**: if it takes >3-4 tool calls, spin up a session

You can customize this policy in your `~/.homard/AGENTS.md`.

## Telegram Commands

| Command | Action |
|---------|--------|
| (any text) | Chat with the agent |
| `/status` | Daemon health |
| `/stop` | Stop current run |
| `/pair <code>` | Pair this chat |
| `/perms <level>` | Change permission level |
| `/server on\|off` | Toggle server mode |

## Server Mode

Toggle in Settings, via CLI (`homard install/uninstall`), or Telegram (`/server on|off`).

- **ON**: launchd KeepAlive — restarts on crash, starts on boot
- **OFF**: daemon stops when you close it, plist removed

Unlike some alternatives, the off switch actually works.

## Security

Three permission levels (toggle in Settings or via Telegram):

- **Supervised** (default): safe actions run automatically, commands that still need approval are blocked until the approval flow exists
- **Autonomous**: everything auto-approved, hang alerts but never pauses
- **Locked**: read-only, no shell/file writes/outbound messages

Shell sandbox blocks dangerous patterns. File writes restricted to `~/.homard/`. All tool executions logged to audit table.

## CLI

```
homard serve       Start the daemon (foreground)
homard chat        Interactive chat (or -m "..." for one-shot)
homard status      Show daemon health
homard stop        Stop current agent run
homard install     Enable server mode (launchd)
homard uninstall   Disable server mode
```

## Tech Stack

- **Daemon**: Rust (tokio, axum, reqwest, rusqlite, teloxide)
- **Tray app**: Tauri 2 + React 19 + TypeScript + Tailwind CSS 4
- **LLM**: OpenAI chat/completions + Anthropic Messages API adapter
- **Auth**: OAuth 2.0 PKCE (no client secrets), tokens in macOS Keychain
- **Storage**: SQLite WAL + FTS5 for memory search
- **Scheduling**: launchd (macOS native) + internal cron loop

## License

MIT

# Activity Tab Redesign — Session Manager + Command Center

**Date:** 2026-04-10
**Status:** Design spec, pending implementation

---

## Problem

The Activity tab is currently a flat log of agent runs — timestamps, status dots, durations. It's a debug view, not a user tool. Nobody looks at it.

What users actually need from this space:

1. **See what's running right now** — Claude sessions, cron jobs, agent tasks
2. **Control running sessions** — resume in Claude app, kill, view output
3. **See what happened recently** — not raw runs, but meaningful events
4. **Quick actions** — launch a new Claude session, check on a cron, view a todo

---

## Design

### Three sections, priority-ordered

#### 1. Active Sessions (top, always visible)

Shows currently running Claude Code sessions and codex conversations. Each entry:

```
┌─────────────────────────────────────────┐
│ ● fix-the-tests         2m running      │
│   ~/GitHub/d1201        [Resume] [Kill] │
├─────────────────────────────────────────┤
│ ● codex chat            idle            │
│   persistent session    [Clear]         │
└─────────────────────────────────────────┘
```

- **Claude sessions**: show name, directory, duration. "Resume" opens terminal with `claude --resume`. "Kill" sends SIGTERM.
- **Codex session**: show the persistent app-server thread status. "Clear" starts a new thread.
- When nothing is running: "No active sessions" (one line, not a big empty state)

#### 2. Recent Events (middle, scrollable)

Not raw runs — **meaningful events** grouped by time. Each event is one line:

```
Today
  ✓ Deployed site-factory                    2:30 PM
  ✓ Fixed login bug (Claude, 3 files)        1:15 PM  
  ● Morning brief ran                        9:00 AM
  
Yesterday  
  ✗ CI check failed on d1201                 4:45 PM
  ✓ Refactored auth module (Claude)          2:00 PM
```

Events are derived from:
- Completed agent runs (chat + Telegram)
- Completed Claude sessions (with file change summary from git)
- Cron job completions
- Errors and failures

Each event: status icon (✓/✗/●) + short description + time. Tap to expand and see details/output.

#### 3. Cron Health (bottom, compact)

One-line-per-job summary:

```
Morning brief    ✓ 12/12 runs    last: 9:00 AM
Weekly summary   ✓ 2/2 runs      next: Fri 5 PM
```

Only shows if there are scheduled jobs. Otherwise hidden.

---

## Data Sources

| Event type | Source | Description generation |
|-----------|--------|----------------------|
| Chat completion | `runs` table | Last user message, truncated |
| Claude session | `cli_sessions` table | Session name + `git diff --stat` after completion |
| Cron job | `cron_runs` table | Schedule name + status |
| Telegram message | `runs` where channel starts with `telegram_` | "Telegram: " + first line of user message |
| Error | Any table where status = "error" | Error message, truncated |

---

## API Changes

### `GET /activity` — redesigned response

```json
{
  "active_sessions": [
    {
      "type": "claude",
      "name": "fix-the-tests",
      "directory": "~/GitHub/d1201",
      "status": "running",
      "started_at": "2026-04-10T14:30:00Z",
      "duration_secs": 120,
      "pid": 12345
    },
    {
      "type": "codex",
      "thread_id": "019d76d5-...",
      "status": "idle",
      "messages": 15
    }
  ],
  "recent_events": [
    {
      "type": "chat_complete",
      "description": "Deployed site-factory",
      "status": "complete",
      "timestamp": "2026-04-10T14:30:00Z",
      "details": "..."
    }
  ],
  "cron_health": [
    {
      "name": "Morning brief",
      "success_rate": 1.0,
      "total_runs": 12,
      "last_run": "2026-04-10T09:00:00Z",
      "next_run": "2026-04-11T09:00:00Z"
    }
  ]
}
```

---

## UI Layout (420px wide)

```
┌──────────────────────────────────┐
│ [toolbar: Homard | Chat Activity Settings] │
├──────────────────────────────────┤
│ Active Sessions                  │
│ ● fix-tests  2m     [Resume][×] │
│ ● codex      idle   [Clear]     │
├──────────────────────────────────┤
│ Today                            │
│ ✓ Deployed site-factory    2:30p │
│ ✓ Fixed login bug          1:15p │
│ ● Morning brief            9:00a │
│                                  │
│ Yesterday                        │
│ ✗ CI check failed          4:45p │
│ ✓ Refactored auth          2:00p │
├──────────────────────────────────┤
│ Cron                             │
│ Morning brief  ✓ 12/12  9:00a   │
│ Weekly summary ✓ 2/2    Fri 5p  │
└──────────────────────────────────┘
```

- 11px for secondary text, 13px for event descriptions
- 0.5px dividers between sections
- No cards, just rows with thin separators
- macOS-native density
- Status icons: ✓ green, ✗ red, ● navy (running), ◦ muted (pending)

---

## Future additions (not in v1)

- **Todo lists** — agent-created tasks ("I noticed X needs Y, want me to add it?")
- **Reminders** — "remind me to check CI at 3pm" → shows in Activity
- **Git integration** — show recent commits per project
- **MCP server status** — connected services with health indicators

---

## Implementation approach

1. Add `GET /activity/v2` endpoint with the new response format
2. Rewrite `Activity.tsx` to render the three sections
3. Add "Resume" action (opens terminal with `claude --resume --name <name>`)
4. Keep the old `GET /activity` for backwards compatibility
5. Add event description generation from run data

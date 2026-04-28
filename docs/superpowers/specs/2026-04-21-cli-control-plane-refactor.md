# Homard CLI Control Plane Refactor

**Date:** 2026-04-21
**Status:** Proposed
**Owner:** JWG

---

## Overview

Homard should stop trying to be a native agent platform and instead become a thin control plane around enterprise-approved coding CLIs, starting with Claude Code CLI.

The product thesis is:

- Enterprises already trust, buy, and govern the underlying coding CLI.
- Homard adds remote reachability, orchestration, mobile dispatch, session routing, notifications, and audit.
- Homard does **not** claim to be a stronger execution sandbox than the CLI it wraps.

This is a narrowing move, not a retreat. It trades native agent ambition for a cleaner security story, lower implementation burden, and a more credible enterprise posture.

---

## Problem Statement

The current architecture is hybrid:

- Sometimes Homard is a thin wrapper around a CLI backend.
- Sometimes Homard is its own local agent runtime with native tools (`shell_exec`, `file_read`, `file_write`, memory mutation, web fetch, session spawning).

That hybrid posture creates three problems:

1. **Security ambiguity**
   - Users can reasonably think Homard inherits CLI controls, but in direct API modes it does not.
   - Permission labels like `supervised` overstate what the current code actually enforces.

2. **Product ambiguity**
   - It is unclear whether Homard's value is "better remote interface for Claude/Codex" or "new personal agent runtime."
   - Competing on native agent capability is expensive and unlikely to beat the platform vendors.

3. **Implementation drag**
   - Native tool security, prompt-injection handling, file sandboxing, approval UX, and remote auth become Homard's problem.
   - These are exactly the controls the underlying CLI vendors are already investing in.

---

## Product Decision

Homard becomes a **remote orchestration layer for enterprise-sanctioned coding CLIs**.

### Core Claim

Homard inherits the execution model and execution limits of the selected CLI backend for privileged work.

### Homard's Value

- Telegram and lightweight mobile dispatch
- Fast session launch into the right repo/worktree/profile
- Session status, history, and resumption
- Schedules, heartbeats, and notifications
- Lightweight policy checks before dispatch
- Audit and transcript stitching across channels

### Explicit Non-Claim

Homard is not an independent secure agent sandbox.

---

## Goals

- Make the security model honest and legible.
- Minimize Homard-native privileged execution.
- Preserve the fast Telegram-first control experience.
- Work well with standard enterprise Claude Code subscriptions.
- Keep the local daemon simple enough to maintain.

## Non-Goals

- Competing with Claude/Codex/Gemini on native agent capability
- Building a richer local tool sandbox than the underlying CLI
- Supporting unrestricted remote shell/file execution as a first-class feature
- Keeping the current direct-API + native-tools architecture as the long-term default

---

## Architectural Principle

**Privileged work is routed to the selected CLI.**

If a request would edit files, run commands, browse with side effects, invoke MCP tools, or otherwise change local state, Homard should create or resume a CLI session rather than execute that action itself.

Homard may still perform limited local orchestration tasks, but those tasks must be:

- non-privileged,
- easy to audit,
- easy to reason about,
- and clearly outside the category of "agent execution."

---

## Trust Boundaries

### Trusted Execution Plane

- Claude Code CLI / Codex CLI / future sanctioned coding CLI
- The enterprise policy, auth, MCP configuration, and approval model attached to that CLI

### Homard Trust Responsibilities

Homard still owns:

- Telegram pairing and channel authorization
- local API authentication
- session routing and repo/worktree targeting
- whether a remote request is allowed to start a CLI session
- schedule/heartbeat triggering
- transcript and audit storage
- notification delivery

Homard does **not** own:

- the full execution sandbox for coding tasks
- local file mutation policy for arbitrary model tool calls
- its own generalized secure tool runtime

---

## Target Architecture

```
Telegram / Tray / Local API / Scheduler
                |
                v
        Homard Control Plane
  - authn/authz
  - policy routing
  - repo/worktree selection
  - session launch/resume/stop
  - audit + transcripts
  - notifications
                |
                v
     Enterprise-approved CLI backend
      (Claude Code first, others later)
                |
                v
   Filesystem / shell / MCP / model execution
   under the CLI's own controls and policies
```

The important line is that Homard stops sitting in the middle as a tool-calling executor for privileged actions.

---

## What Homard Should Keep

- Telegram bot integration
- Tauri tray shell and settings UI
- session list, activity, status, and cancellation
- scheduled prompts / heartbeat wakeups
- provider/profile selection
- worktree/repo targeting
- local storage for metadata, audit, and conversation summaries
- identity/context files that shape dispatch behavior

These are orchestration features and remain aligned with the product thesis.

---

## What Homard Should Remove or Quarantine

The following should not remain in the default enterprise path:

- native `shell_exec`
- native `file_write`
- native arbitrary `file_read`
- native web fetch/search used as part of a privileged reasoning loop
- configured arbitrary `shell_tools`
- direct API providers that expose Homard's own tool runtime as the execution layer

If any of these remain for local power users, they should be:

- explicitly marked `experimental`,
- disabled by default,
- unavailable from Telegram and scheduled jobs,
- and excluded from the enterprise security story.

---

## Channel Model

### Tray / Local Desktop

Highest-trust channel.

Allowed capabilities:

- full session launch and session management
- local settings changes
- provider/profile management
- Telegram pairing
- optional access to experimental native mode if it still exists

### Telegram

Remote dispatch channel, not a remote shell.

Allowed capabilities:

- ask status
- start a session in an approved repo/worktree/profile
- resume a session
- stop a session
- receive summaries, errors, and notifications

Not allowed by default:

- direct file writes
- direct shell commands
- arbitrary local settings mutation
- switching into a more privileged execution mode without a local confirmation path

### Local API

Should be treated as a local control plane, not a trusted origin shortcut.

Requirements:

- bearer token for all privileged routes
- no `Origin`-only trust
- optional future support for a Unix domain socket or other tighter local transport

---

## Permission Model

The current `supervised / autonomous / locked` naming only makes sense if it maps to real routing behavior.

### Recommended Replacement

#### `locked`

- read status only
- no new sessions
- no provider/settings changes
- no schedule execution

#### `dispatch`

- can launch or resume CLI sessions in approved scopes
- cannot invoke Homard-native privileged tools
- uses the CLI's own approval and auth model for actual execution

#### `automation`

- can run pre-approved scheduled or templated CLI jobs
- still no Homard-native arbitrary shell/file tool execution
- intended for trusted recurring workflows only

If retaining the old labels is important for compatibility:

- `supervised` should map to `dispatch`
- `autonomous` should map to `automation`
- `locked` can stay `locked`

But the underlying semantics must change to routing policy, not pretend local approvals.

---

## Provider Strategy

### Primary Provider

Claude Code CLI should be the primary and first-class backend for the enterprise path.

### Secondary Providers

Codex CLI and Gemini CLI can be added when they fit the same control-plane pattern:

- authenticated and enterprise-governed outside Homard
- resumable session model
- clear CLI invocation and transcript collection

### Direct API Providers

Direct `OpenAI` / `Anthropic` / `OpenRouter` providers should not be part of the enterprise default if they require Homard to host its own privileged tool loop.

Options:

1. remove them from the default product path,
2. keep them behind an explicit experimental flag,
3. or scope them to non-privileged chat-only use.

---

## Security Implications

This refactor does not magically remove risk. It changes where risk lives.

### Risks Reduced

- misleading local approval claims
- Homard-native sandbox bypasses
- prompt-injection through Homard tool loops
- file write boundary bugs in Homard
- arbitrary configured shell shortcuts exposed to remote channels

### Risks That Remain

- compromised Telegram account or weak pairing flow
- compromised local machine or same-user local process abuse
- misuse of the underlying CLI if it is configured permissively
- overbroad repo/worktree allowlists
- insufficient audit on who launched what from which channel

### New Security Posture

The central question becomes:

**Who is allowed to dispatch a CLI session, in what scope, under which profile, and with what audit trail?**

That is a much narrower and more defensible problem than "how do we secure a homegrown agent runtime."

---

## Proposed Refactor Plan

### Phase 1: Make the product stance explicit

- update README and settings copy to describe Homard as a CLI control plane
- mark native tool execution as legacy/experimental
- stop claiming Homard provides a stronger approval boundary than it does

### Phase 2: Restrict remote channels

- remove Telegram access to native privileged actions
- make Telegram launch or resume CLI sessions instead of invoking native tools
- block remote permission escalations without local confirmation

### Phase 3: Harden the control plane

- require bearer auth for privileged local API routes
- tighten Telegram authorization and pairing
- add repo/worktree allowlists for remote dispatch
- add per-channel and per-profile audit logging

### Phase 4: De-scope native execution

- disable native `shell_exec`, `file_write`, `shell_tools`, and direct tool loops in enterprise mode
- quarantine direct API provider modes behind an explicit flag if retained

### Phase 5: Improve orchestration UX

- one-tap Telegram commands for common dispatch patterns
- session summaries and notifications
- better resume/status flows
- schedule templates for recurring CLI jobs

---

## Migration Notes

The cleanest implementation path is probably to introduce an explicit feature mode in config:

```json
{
  "product_mode": "cli_control_plane"
}
```

Possible values:

- `cli_control_plane` — default, enterprise-oriented
- `native_agent_experimental` — legacy/power-user mode

This allows:

- a clean default posture,
- backwards compatibility during migration,
- and a clear line in documentation and support.

---

## Open Questions

1. Should Telegram be allowed to choose arbitrary directories, or only named approved workspaces?
2. Should scheduled automations run only predefined prompt templates, or may they carry free-form prompts?
3. Does Claude Code provide enough session metadata and resume hooks for the full desired UX?
4. Is chat-only direct API mode still valuable if it has no privileged tool access?
5. Do we keep a developer-only native mode in-tree, or remove it entirely?

---

## Recommendation

Proceed with the CLI control plane refactor.

The product is stronger if it is clearly positioned as:

> Homard is the lightweight remote dispatch and orchestration layer for enterprise-approved coding CLIs.

That story is more credible than a hybrid native-agent pitch, easier to defend in security review, and more likely to survive contact with enterprise buyers.

---

## Recovery Note

This memo was reconstructed from Codex session transcripts on 2026-04-28 after the original `~/.codex/worktrees/d589/arcctl/` worktree was cleaned up. Source: parallel Apr 21 Codex sessions `019daf96-0f7f-7821-866a-1326eb355c9a` (adversarial review of this memo) and `019daf97-d9b3-7a73-aa5c-05ef3b9325c0` (the security review that produced it). Companion findings from those sessions:

- **Adversarial review (via $claude sidecar) identified three first-class subsystems still under-specified:** workspace allowlists, dispatch/audit policy, bearer-auth local control-plane auth. Recommended next step is a Claude session launch/resume/audit spike before any deletion work.
- **Five P1/P2 findings** that motivated this refactor:
  1. P1 — `Supervised` mode is not an approval boundary ([crates/homard-core/src/security/mod.rs:47-66](../../../crates/homard-core/src/security/mod.rs))
  2. P1 — Relative file writes escape `~/.homard/workspace` ([crates/homard-core/src/tools/files.rs:121-129](../../../crates/homard-core/src/tools/files.rs))
  3. P1 — `config.shell_tools` bypass the shell sandbox ([crates/homard-core/src/tools/registry.rs:67-103](../../../crates/homard-core/src/tools/registry.rs))
  4. P2 — Tool outputs fed unguarded into next model turn ([crates/homard-core/src/agent/loop.rs:192-221](../../../crates/homard-core/src/agent/loop.rs))
  5. P2 — Local API trusts `Origin: tauri://localhost` as auth ([crates/homard-core/src/api/mod.rs:29-50](../../../crates/homard-core/src/api/mod.rs))

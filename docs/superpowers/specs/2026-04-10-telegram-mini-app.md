# Telegram Mini App — Multi-Machine Dispatch UI

**Date:** 2026-04-10
**Status:** Idea, not yet designed

---

## Concept

A Telegram Mini App (web app inside Telegram) that replaces the native tray app for mobile users. No App Store publishing needed — it's a URL the bot links to.

## Why

- The tray app competes with Claude/Codex desktop and loses on polish
- Telegram is where users already interact with Homard
- Mini Apps run inside Telegram on iOS/Android/desktop — zero install
- No Apple Developer Program, no Play Store review

## Features

- **Machine picker**: "Send to Mac / Windows / Server" when dispatching jobs
- **Active sessions**: see what's running on each machine
- **Settings**: provider config, permissions, Telegram allowlist
- **Cron management**: create/edit/delete scheduled tasks
- **Chat history**: view conversations per machine

## Architecture

- Each Homard instance registers with a lightweight cloud backend (Supabase)
- Machine ID + online status synced to cloud
- Mini App reads from cloud to show available machines
- User picks machine → Mini App tells that machine's bot to execute
- Or: each machine has its own bot, Mini App is just a router UI

## Implementation

- Telegram Bot API: `setChatMenuButton` to add the Mini App button
- Web app: React (same stack as tray app) deployed to Vercel/Cloudflare Pages
- Backend: Supabase for machine registry + status sync
- Identity files (SOUL, USER, MEMORY) sync via Supabase storage

## Distribution

User flow:
1. `homard setup` on their machine
2. Bot shows "Open Homard" button in Telegram
3. Tap → Mini App opens with full UI
4. No install, no app store, works everywhere

## Not in scope

- Chat inside the Mini App (Telegram IS the chat)
- Code editing (that's Claude/Codex desktop)
- File browsing (that's the terminal)

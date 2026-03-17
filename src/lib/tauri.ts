import { invoke } from "@tauri-apps/api/core";
import type { HealthStatus, Profile, SessionInfo } from "./types";

export async function runHealthCheck(): Promise<HealthStatus> {
  return invoke("run_health_check");
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

export async function listProfiles(): Promise<Profile[]> {
  return invoke("list_profiles");
}

export async function readClaudeSettings(path: string): Promise<unknown> {
  return invoke("read_claude_settings_cmd", { path });
}

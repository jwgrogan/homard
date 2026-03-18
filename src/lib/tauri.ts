import { invoke } from "@tauri-apps/api/core";
import type {
  HealthStatus,
  Profile,
  Session,
  SessionTree,
  ClaudeSettings,
  McpServerConfig,
  AgentInfo,
  CommandInfo,
  Run,
  Schedule,
  DiscoveredPlist,
  TelegramStatus,
  CredentialHealth,
} from "./types";

export async function runHealthCheck(): Promise<HealthStatus> {
  return invoke("run_health_check");
}

export async function listSessions(): Promise<Session[]> {
  return invoke("list_sessions");
}

export async function listProfiles(): Promise<Profile[]> {
  return invoke("list_profiles");
}

export async function readClaudeSettings(path: string): Promise<unknown> {
  return invoke("read_claude_settings_cmd", { path });
}

// Settings
export async function getClaudeSettings(scope: string, projectDir?: string): Promise<ClaudeSettings> {
  return invoke("get_claude_settings", { scope, projectDir });
}

export async function addPermission(scope: string, list: string, pattern: string, projectDir?: string): Promise<void> {
  return invoke("add_permission", { scope, list, pattern, projectDir });
}

export async function removePermission(scope: string, list: string, pattern: string, projectDir?: string): Promise<void> {
  return invoke("remove_permission", { scope, list, pattern, projectDir });
}

export async function setDefaultMode(scope: string, mode: string | null, projectDir?: string): Promise<void> {
  return invoke("set_default_mode", { scope, mode, projectDir });
}

export async function addMcpServer(scope: string, name: string, config: McpServerConfig, projectDir?: string): Promise<void> {
  return invoke("add_mcp_server", { scope, name, config, projectDir });
}

export async function removeMcpServer(scope: string, name: string, projectDir?: string): Promise<void> {
  return invoke("remove_mcp_server", { scope, name, projectDir });
}

export async function setEnvVar(scope: string, key: string, value: string, projectDir?: string): Promise<void> {
  return invoke("set_env_var", { scope, key, value, projectDir });
}

export async function removeEnvVar(scope: string, key: string, projectDir?: string): Promise<void> {
  return invoke("remove_env_var", { scope, key, projectDir });
}

// Agents & Commands
export async function getAgents(projectDir?: string): Promise<AgentInfo[]> {
  return invoke("get_agents", { projectDir });
}

export async function getCommands(projectDir?: string): Promise<CommandInfo[]> {
  return invoke("get_commands", { projectDir });
}

// Profiles
export async function switchProfile(name: string): Promise<void> {
  return invoke("switch_profile", { name });
}

export async function importProfile(name: string): Promise<Profile> {
  return invoke("import_profile", { name });
}

export async function checkAllProfileHealth(): Promise<[string, CredentialHealth][]> {
  return invoke("check_all_profile_health");
}

export async function detectClaudeSwitch(): Promise<boolean> {
  return invoke("detect_claude_switch");
}

// Sessions
export async function spawnSession(directory: string, provider: string, profile?: string, agent?: string, prompt?: string): Promise<Session> {
  return invoke("spawn_session", { directory, provider, profile, agent, prompt });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function resumeSession(sessionId: string): Promise<Session> {
  return invoke("resume_session", { sessionId });
}

export async function forkSession(sessionId: string): Promise<Session> {
  return invoke("fork_session", { sessionId });
}

export async function getSessionTree(sessionId: string): Promise<SessionTree | null> {
  return invoke("get_session_tree", { sessionId });
}

export async function listRuns(limit?: number, offset?: number): Promise<Run[]> {
  return invoke("list_runs", { limit, offset });
}

export async function listSessionsFiltered(directory?: string, provider?: string, limit?: number, offset?: number): Promise<Session[]> {
  return invoke("list_sessions_filtered", { directory, provider, limit, offset });
}

// Scheduler
export async function createSchedule(schedule: Schedule): Promise<void> {
  return invoke("create_schedule", { schedule });
}

export async function updateSchedule(schedule: Schedule): Promise<void> {
  return invoke("update_schedule", { schedule });
}

export async function deleteSchedule(id: string): Promise<void> {
  return invoke("delete_schedule", { id });
}

export async function getSchedule(id: string): Promise<Schedule> {
  return invoke("get_schedule", { id });
}

export async function listSchedules(): Promise<Schedule[]> {
  return invoke("list_schedules");
}

export async function pauseSchedule(id: string): Promise<void> {
  return invoke("pause_schedule", { id });
}

export async function resumeSchedule(id: string): Promise<void> {
  return invoke("resume_schedule", { id });
}

export async function discoverLaunchdJobs(): Promise<DiscoveredPlist[]> {
  return invoke("discover_launchd_jobs");
}

export async function importLaunchdJob(label: string): Promise<Schedule> {
  return invoke("import_launchd_job_cmd", { label });
}

export async function listScheduleRuns(scheduleId: string, limit?: number, offset?: number): Promise<Run[]> {
  return invoke("list_schedule_runs", { scheduleId, limit, offset });
}

// --- Managed MCP Sync ---
export async function listManagedMcps(): Promise<Record<string, McpServerConfig>> {
  const result = await invoke<Record<string, McpServerConfig>>("list_managed_mcps");
  return result;
}

export async function addManagedMcp(name: string, config: McpServerConfig): Promise<void> {
  return invoke("add_managed_mcp", { name, config });
}

export async function removeManagedMcp(name: string): Promise<void> {
  return invoke("remove_managed_mcp", { name });
}

export async function syncAllMcps(): Promise<void> {
  return invoke("sync_all_mcps");
}

// --- Telegram ---
export async function saveTelegramToken(token: string): Promise<void> {
  return invoke("save_telegram_token_cmd", { token });
}

export async function verifyTelegramToken(token: string): Promise<string> {
  return invoke("verify_telegram_token", { token });
}

export async function getTelegramStatus(): Promise<TelegramStatus> {
  return invoke("get_telegram_status");
}

export async function addPairedChat(chatId: string): Promise<void> {
  return invoke("add_paired_chat_cmd", { chatId });
}

export async function removePairedChat(chatId: string): Promise<void> {
  return invoke("remove_paired_chat_cmd", { chatId });
}

export async function generatePairingCode(): Promise<string> {
  return invoke("generate_pairing_code_cmd");
}

export async function startTelegramPolling(): Promise<void> {
  return invoke("start_telegram_polling");
}

export async function stopTelegramPolling(): Promise<void> {
  return invoke("stop_telegram_polling");
}

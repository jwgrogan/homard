import { invoke } from "@tauri-apps/api/core";
import type {
  HealthStatus,
  Profile,
  SessionInfo,
  ClaudeSettings,
  McpServerConfig,
  AgentInfo,
  CommandInfo,
  Run,
  Schedule,
  DiscoveredPlist,
} from "./types";

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

export async function setBypassPermissions(scope: string, bypass: boolean, projectDir?: string): Promise<void> {
  return invoke("set_bypass_permissions", { scope, bypass, projectDir });
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

// Sessions
export async function spawnSession(prompt: string, directory: string, profile?: string, agent?: string): Promise<SessionInfo> {
  return invoke("spawn_session", { prompt, directory, profile, agent });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function listRuns(limit?: number, offset?: number): Promise<Run[]> {
  return invoke("list_runs", { limit, offset });
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

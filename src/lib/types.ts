export interface HealthStatus {
  claude_cli_installed: boolean;
  claude_cli_version: string | null;
  active_profile: Profile | null;
  telegram_connected: boolean;
  email_configured: boolean;
  arcctl_dir_exists: boolean;
  checked_at: string;
}

export interface Profile {
  name: string;
  email: string | null;
  is_active: boolean;
}

export interface SessionInfo {
  id: string;
  agent: string | null;
  profile: string | null;
  directory: string;
  trigger: string;
  started_at: string;
  pid: number | null;
}

// --- Settings ---
export interface ClaudeSettings {
  permissions: PermissionsConfig;
  env: Record<string, string>;
  mcpServers: Record<string, McpServerConfig>;
}

export interface PermissionsConfig {
  allow: string[];
  deny: string[];
  bypassPermissions: boolean;
}

export interface McpServerConfig {
  command?: string;
  args?: string[];
  url?: string;
  type?: string;
  [key: string]: unknown;
}

// --- Agents ---
export interface AgentInfo {
  name: string;
  path: string;
  scope: "global" | "project";
  description: string | null;
  model: string | null;
  tools: string[];
}

export interface CommandInfo {
  name: string;
  path: string;
  scope: "global" | "project";
  description: string | null;
}

// --- Runs ---
export interface Run {
  id: string;
  schedule_id: string | null;
  agent: string | null;
  profile: string | null;
  directory: string | null;
  trigger: "manual" | "cron" | "telegram" | "email";
  status: "running" | "complete" | "error" | "killed";
  started_at: string;
  finished_at: string | null;
  duration_ms: number | null;
  error_message: string | null;
}

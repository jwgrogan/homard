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
  provider: "claude" | "gemini";
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
  enabledPlugins?: Record<string, boolean>;
  enabledMcpjsonServers?: string[];
  defaultMode?: string;
  [key: string]: unknown;
}

export interface PermissionsConfig {
  allow: string[];
  deny: string[];
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

// --- Scheduler ---
export interface Schedule {
  id: string;
  name: string;
  schedule: string;
  timezone: string | null;
  agent: string | null;
  prompt: string | null;
  directory: string;
  profile: string | null;
  timeout_minutes: number | null;
  session_mode: "fresh" | "persistent";
  last_session_id: string | null;
  delivery: DeliveryConfig;
  retry: RetryConfig;
  enabled: boolean;
}

export interface DeliveryConfig {
  channels: string[];
  on: string[];
}

export interface RetryConfig {
  max_attempts: number;
  backoff_seconds: number[];
}

export interface DiscoveredPlist {
  label: string;
  path: string;
  program_args: string[];
  hour: number | null;
  minute: number | null;
}

// --- Telegram ---
export interface TelegramStatus {
  enabled: boolean;
  bot_username: string | null;
  paired_chat_ids: string[];
  is_polling: boolean;
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

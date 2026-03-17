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

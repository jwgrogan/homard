const BASE = "http://localhost:17700";

export interface ChatMessage {
  role: string;
  content: string;
  tool_call_id?: string;
  tool_calls?: Array<{ id: string; name: string; arguments: unknown }>;
  timestamp?: string;
}

export interface AgentRun {
  id: string;
  channel: string;
  trigger: string;
  status: string;
  started_at: string;
  finished_at?: string;
  duration_ms?: number;
  error_message?: string;
  iterations: number;
}

export interface DaemonStatus {
  running: boolean;
  uptime_secs?: number;
  active_provider?: string;
  active_model?: string;
  permission_level: string;
  telegram_connected: boolean;
  current_run?: string;
}

export async function sendChat(message: string, channel = "chat"): Promise<{ response: string }> {
  const res = await fetch(`${BASE}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message, channel }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getConversation(channel: string, limit = 50): Promise<ChatMessage[]> {
  const res = await fetch(`${BASE}/conversations/${channel}?limit=${limit}`);
  if (!res.ok) return [];
  return res.json();
}

export async function getStatus(): Promise<DaemonStatus | null> {
  try {
    const res = await fetch(`${BASE}/status`);
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getActivity(): Promise<AgentRun[]> {
  try {
    const res = await fetch(`${BASE}/activity`);
    if (!res.ok) return [];
    return res.json();
  } catch {
    return [];
  }
}

export async function stopRun(): Promise<void> {
  await fetch(`${BASE}/stop`, { method: "POST" });
}

export async function getSettings(): Promise<Record<string, unknown>> {
  const res = await fetch(`${BASE}/settings`);
  if (!res.ok) return {};
  return res.json();
}

export async function updateSettings(settings: Record<string, unknown>): Promise<void> {
  await fetch(`${BASE}/settings`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(settings),
  });
}

export async function setPermissions(level: string): Promise<void> {
  await fetch(`${BASE}/settings/permissions`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ level }),
  });
}

export async function startAuth(provider: string): Promise<{ auth_url: string; verifier: string; port: number }> {
  const res = await fetch(`${BASE}/auth/${provider}/start`, { method: "POST" });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function generatePairingCode(): Promise<string> {
  const res = await fetch(`${BASE}/telegram/pair`, { method: "POST" });
  const data = await res.json();
  return data.code;
}

export async function getTelegramStatus(): Promise<{ enabled: boolean; paired_chats: number }> {
  const res = await fetch(`${BASE}/telegram/status`);
  return res.json();
}

export async function readFile(name: string): Promise<string> {
  const res = await fetch(`${BASE}/files/${name}`);
  if (!res.ok) return "";
  return res.text();
}

export async function writeFile(name: string, content: string): Promise<void> {
  await fetch(`${BASE}/files/${name}`, {
    method: "PUT",
    headers: { "Content-Type": "text/plain" },
    body: content,
  });
}

export interface CliSession {
  id: string;
  cli: string;
  prompt: string;
  directory: string;
  status: string;
  pid?: number;
  started_at: string;
  finished_at?: string;
  duration_ms?: number;
  error?: string;
}

export interface CronHealthEntry {
  name: string;
  total_runs: number;
  successes: number;
  failures: number;
  last_run?: string;
  avg_duration_ms?: number;
}

export async function getSessions(): Promise<CliSession[]> {
  try {
    const res = await fetch(`${BASE}/sessions`);
    if (!res.ok) return [];
    return res.json();
  } catch { return []; }
}

export async function killSession(id: string): Promise<void> {
  await fetch(`${BASE}/sessions/${id}`, { method: "DELETE" });
}

export async function getCronHealth(): Promise<CronHealthEntry[]> {
  try {
    const res = await fetch(`${BASE}/cron/health`);
    if (!res.ok) return [];
    return res.json();
  } catch { return []; }
}

export async function getServerMode(): Promise<{ mode: string; launchd_installed: boolean }> {
  const res = await fetch(`${BASE}/server`);
  return res.json();
}

export async function setServerMode(mode: "on" | "off"): Promise<{ status: string; message: string }> {
  const res = await fetch(`${BASE}/server`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ mode }),
  });
  return res.json();
}

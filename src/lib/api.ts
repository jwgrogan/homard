const BASE = "http://localhost:17700";

async function headers(): Promise<HeadersInit> {
  // Auth is handled by origin-based check in the daemon middleware.
  // The Tauri webview and local Vite dev origins are allowed directly.
  // External API consumers use the bearer token from ~/.homard/api.token.
  return { "Content-Type": "application/json" };
}

export async function apiFetch(path: string, options: RequestInit = {}): Promise<Response> {
  const h = new Headers(await headers());
  // Merge any caller-provided headers
  if (options.headers) {
    const extra = new Headers(options.headers);
    extra.forEach((v, k) => h.set(k, v));
  }
  return fetch(`${BASE}${path}`, { ...options, headers: h });
}

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

export type ProviderAvailability = Record<string, boolean>;

export async function sendChat(message: string, channel = "chat"): Promise<{ response: string }> {
  const res = await apiFetch("/chat", {
    method: "POST",
    body: JSON.stringify({ message, channel }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getConversation(channel: string, limit = 50): Promise<ChatMessage[]> {
  const res = await apiFetch(`/conversations/${channel}?limit=${limit}`);
  if (!res.ok) return [];
  return res.json();
}

export async function getStatus(): Promise<DaemonStatus | null> {
  try {
    const res = await apiFetch("/status");
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getActivity(): Promise<AgentRun[]> {
  try {
    const res = await apiFetch("/activity");
    if (!res.ok) return [];
    return res.json();
  } catch {
    return [];
  }
}

export async function stopRun(): Promise<void> {
  await apiFetch("/stop", { method: "POST" });
}

export async function getSettings(): Promise<Record<string, unknown>> {
  const res = await apiFetch("/settings");
  if (!res.ok) return {};
  return res.json();
}

export async function updateSettings(settings: Record<string, unknown>): Promise<void> {
  await apiFetch("/settings", {
    method: "PUT",
    body: JSON.stringify(settings),
  });
}

export async function setPermissions(level: string): Promise<void> {
  await apiFetch("/settings/permissions", {
    method: "PUT",
    body: JSON.stringify({ level }),
  });
}

export async function startAuth(provider: string): Promise<{ auth_url: string; verifier: string; port: number }> {
  const res = await apiFetch(`/auth/${provider}/start`, { method: "POST" });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function saveProviderApiKey(
  provider: string,
  apiKey: string,
  model: string,
): Promise<{ status: string; message: string }> {
  const res = await apiFetch(`/providers/${provider}/api-key`, {
    method: "POST",
    body: JSON.stringify({ api_key: apiKey, model, activate: true }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getProviderAvailability(): Promise<ProviderAvailability> {
  try {
    const res = await apiFetch("/providers/availability");
    if (!res.ok) return {};
    return res.json();
  } catch {
    return {};
  }
}

export async function generatePairingCode(): Promise<string> {
  const res = await apiFetch("/telegram/pair", { method: "POST" });
  const data = await res.json();
  return data.code;
}

export async function getTelegramStatus(): Promise<{ enabled: boolean; paired_chats: number }> {
  const res = await apiFetch("/telegram/status");
  return res.json();
}

export async function readFile(name: string): Promise<string> {
  const res = await apiFetch(`/files/${name}`);
  if (!res.ok) return "";
  return res.text();
}

export async function writeFile(name: string, content: string): Promise<void> {
  await apiFetch(`/files/${name}`, {
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
    const res = await apiFetch("/sessions");
    if (!res.ok) return [];
    return res.json();
  } catch { return []; }
}

export async function killSession(id: string): Promise<void> {
  await apiFetch(`/sessions/${id}`, { method: "DELETE" });
}

export async function getCronHealth(): Promise<CronHealthEntry[]> {
  try {
    const res = await apiFetch("/cron/health");
    if (!res.ok) return [];
    return res.json();
  } catch { return []; }
}

export async function getServerMode(): Promise<{ mode: string; launchd_installed: boolean }> {
  const res = await apiFetch("/server");
  return res.json();
}

export async function setServerMode(mode: "on" | "off"): Promise<{ status: string; message: string }> {
  const res = await apiFetch("/server", {
    method: "PUT",
    body: JSON.stringify({ mode }),
  });
  return res.json();
}

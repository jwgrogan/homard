import { useEffect, useState } from "react";
import { apiFetch, getStatus, setPermissions, getTelegramStatus, readFile, writeFile, type DaemonStatus } from "../lib/api";

type SettingsTab = "providers" | "permissions" | "telegram" | "identity" | "daemon";

const providerModels: Record<string, string[]> = {
  codex_cli: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  claude_cli: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openai: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  anthropic: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openrouter: ["anthropic/claude-sonnet-4-6", "openai/gpt-5.4"],
};

function ProviderCard({ name, status, connected, currentModel, onRefresh }: {
  name: string;
  status: DaemonStatus | null;
  connected: boolean;
  currentModel?: string;
  onRefresh: () => void;
}) {
  const isActive = status?.active_provider === name;
  const [connecting, setConnecting] = useState(false);
  const [model, setModel] = useState(currentModel || providerModels[name]?.[0] || "");

  const isCli = name === "codex_cli" || name === "claude_cli";
  const isApiKey = name === "openrouter" || name === "openai" || name === "anthropic";
  const [apiKey, setApiKey] = useState("");

  const handleConnect = async () => {
    setConnecting(true);
    try {
      const res = await apiFetch("/settings");
      const cfg = await res.json();
      const providers = cfg.providers || {};

      if (isCli) {
        providers[name] = { kind: name, auth_type: "cli", model: model || providerModels[name]?.[0] || "" };
      } else if (isApiKey && apiKey) {
        providers[name] = {
          kind: name,
          auth_type: "api_key",
          model: model || providerModels[name]?.[0] || "",
          api_key_keychain_ref: `homard.${name}.api_key`,
        };
      }

      await apiFetch("/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ...cfg, providers, active_provider: name }),
      });
      setConnecting(false);
      onRefresh();
    } catch (e) {
      console.error(e);
      setConnecting(false);
    }
  };

  return (
    <div
      className="px-3 py-2"
      style={{ borderBottom: `0.5px solid ${isActive ? "var(--coral)" : "var(--border)"}` }}
    >
      <div className="flex items-center justify-between">
        <div>
          <div className="text-[13px] font-medium" style={{ color: "var(--navy)" }}>
            {name === "codex_cli" ? "Codex CLI" : name === "claude_cli" ? "Claude CLI" : name.charAt(0).toUpperCase() + name.slice(1)}
          </div>
          <div className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
            {name === "codex_cli" ? "Uses your Codex login" :
             name === "claude_cli" ? "Uses your Claude login" :
             name === "anthropic" ? "Extra usage billing" :
             name === "openai" ? "API key billing" : "Per-token billing"}
          </div>
        </div>
        <div className="flex gap-1.5 items-center">
          <select
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="text-[11px] rounded px-1.5 py-0.5 outline-none"
            style={{ background: "var(--sage)", color: "var(--navy)", border: "0.5px solid var(--border)" }}
          >
            {(providerModels[name] || []).map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </select>
          <button
            onClick={handleConnect}
            disabled={connecting || connected}
            className="px-2 py-0.5 rounded text-[11px] font-medium transition-colors"
            style={{
              background: connected ? "var(--success-bg)" : "var(--sage)",
              color: connected ? "var(--success)" : "var(--navy)",
            }}
          >
            {connecting ? "..." : connected ? (isActive ? "Active" : "Connected") : isCli ? "Use" : "Save"}
          </button>
        </div>
      </div>
      {isApiKey && !connected && (
        <div className="mt-1.5">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={`${name === "openrouter" ? "sk-or-..." : "sk-..."} API key`}
            className="w-full text-[11px] rounded px-2 py-1 outline-none"
            style={{ background: "var(--cream)", color: "var(--navy)", border: "0.5px solid var(--border)" }}
          />
        </div>
      )}
    </div>
  );
}

function PermissionsPanel() {
  const [level, setLevel] = useState("supervised");
  useEffect(() => {
    apiFetch("/settings/permissions")
      .then(res => res.json())
      .then(data => {
        if (typeof data === "string") setLevel(data);
      })
      .catch(() => {});
  }, []);

  const levels = [
    { id: "supervised", label: "Supervised", desc: "Approve dangerous actions" },
    { id: "autonomous", label: "Autonomous", desc: "Full auto, alerts only" },
    { id: "locked", label: "Locked", desc: "Read-only, no actions" },
  ];

  const handleChange = async (newLevel: string) => {
    setLevel(newLevel);
    await setPermissions(newLevel);
  };

  return (
    <div className="flex flex-col">
      {levels.map((l) => (
        <button
          key={l.id}
          onClick={() => handleChange(l.id)}
          className="px-3 py-2 text-left transition-colors"
          style={{
            background: level === l.id ? "rgba(232, 240, 236, 0.5)" : "transparent",
            borderBottom: "0.5px solid var(--border)",
          }}
        >
          <div className="flex items-center gap-2">
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ background: level === l.id ? "var(--coral)" : "var(--border)" }}
            />
            <span className="text-[13px] font-medium" style={{ color: "var(--navy)" }}>{l.label}</span>
            <span className="text-[11px]" style={{ color: "var(--navy-muted)" }}>{l.desc}</span>
          </div>
        </button>
      ))}
    </div>
  );
}

function UsernameAllowlist() {
  const [usernames, setUsernames] = useState<string[]>([]);
  const [newUsername, setNewUsername] = useState("");

  useEffect(() => {
    apiFetch("/settings").then(r => r.json()).then(cfg => {
      setUsernames(cfg.telegram?.allowed_usernames || []);
    }).catch(() => {});
  }, []);

  const addUsername = async () => {
    const name = newUsername.trim().replace(/^@/, "");
    if (!name) return;
    const updated = [...usernames, name];
    setUsernames(updated);
    setNewUsername("");
    // Save to config
    const res = await apiFetch("/settings");
    const cfg = await res.json();
    cfg.telegram = cfg.telegram || {};
    cfg.telegram.allowed_usernames = updated;
    await apiFetch("/settings", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(cfg),
    });
  };

  const removeUsername = async (name: string) => {
    const updated = usernames.filter(u => u !== name);
    setUsernames(updated);
    const res = await apiFetch("/settings");
    const cfg = await res.json();
    cfg.telegram = cfg.telegram || {};
    cfg.telegram.allowed_usernames = updated;
    await apiFetch("/settings", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(cfg),
    });
  };

  return (
    <div>
      <div className="flex gap-1.5 mb-1.5">
        <input
          value={newUsername}
          onChange={e => setNewUsername(e.target.value)}
          onKeyDown={e => e.key === "Enter" && addUsername()}
          placeholder="@username"
          className="flex-1 text-[11px] rounded px-2 py-1 outline-none"
          style={{ background: "var(--cream)", color: "var(--navy)", border: "0.5px solid var(--border)" }}
        />
        <button
          onClick={addUsername}
          disabled={!newUsername.trim()}
          className="px-2 py-1 rounded text-[11px] font-medium disabled:opacity-30"
          style={{ background: "var(--sage)", color: "var(--navy)" }}
        >
          Add
        </button>
      </div>
      {usernames.length > 0 ? (
        <div className="flex flex-wrap gap-1">
          {usernames.map(u => (
            <span
              key={u}
              className="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded"
              style={{ background: "var(--sage)", color: "var(--navy)" }}
            >
              @{u}
              <button onClick={() => removeUsername(u)} className="opacity-50 hover:opacity-100">&times;</button>
            </span>
          ))}
        </div>
      ) : (
        <div className="text-[10px]" style={{ color: "var(--navy-muted)" }}>
          No usernames added. Anyone who messages the bot will be ignored.
        </div>
      )}
    </div>
  );
}

function TelegramPanel() {
  const [botToken, setBotToken] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [tgStatus, setTgStatus] = useState<{ enabled: boolean; paired_chats: number; bot_name?: string } | null>(null);

  useEffect(() => {
    getTelegramStatus().then(setTgStatus).catch(() => {});
  }, []);

  const handleSaveToken = async () => {
    if (!botToken.trim()) return;
    setSaving(true);
    setMessage("");
    try {
      const res = await apiFetch("/telegram/token", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token: botToken.trim() }),
      });
      const data = await res.json();
      if (res.ok) {
        setMessage(data.message || "Connected!");
        setBotToken("");
        getTelegramStatus().then(setTgStatus);
      } else {
        setMessage(data || "Failed to connect");
      }
    } catch {
      setMessage("Failed to connect");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col">
      {/* Status */}
      <div className="px-3 py-2" style={{ borderBottom: "0.5px solid var(--border)" }}>
        <div className="text-[13px] font-medium" style={{ color: tgStatus?.enabled ? "var(--success)" : "var(--navy)" }}>
          {tgStatus?.enabled
            ? tgStatus.bot_name
              ? `@${tgStatus.bot_name} connected`
              : "Bot connected"
            : "Not connected"}
        </div>
        {tgStatus?.enabled && (
          <div className="text-[10px] mt-0.5" style={{ color: "var(--navy-muted)" }}>
            {tgStatus.paired_chats} paired chats
          </div>
        )}
      </div>

      {!tgStatus?.enabled ? (
        <>
          {/* Setup instructions */}
          <div className="px-3 py-2 text-[11px] leading-relaxed" style={{ color: "var(--navy-muted)", borderBottom: "0.5px solid var(--border)" }}>
            <div className="font-medium mb-1" style={{ color: "var(--navy)" }}>Setup</div>
            <ol className="list-decimal pl-3.5 space-y-0.5">
              <li>Open <a href="https://t.me/BotFather" target="_blank" rel="noreferrer" style={{ color: "var(--coral)" }}>@BotFather</a> in Telegram</li>
              <li>Send <code className="font-mono" style={{ color: "var(--coral)" }}>/newbot</code> and follow the prompts</li>
              <li>Paste the bot token below and click Connect</li>
              <li>Open your new bot in Telegram and send <code className="font-mono" style={{ color: "var(--coral)" }}>/start</code></li>
            </ol>
          </div>

          {/* Bot token input + save */}
          <div className="px-3 py-2" style={{ borderBottom: "0.5px solid var(--border)" }}>
            <div className="text-[11px] font-medium mb-1" style={{ color: "var(--navy)" }}>Bot Token</div>
            <div className="flex gap-1.5">
              <input
                type="password"
                value={botToken}
                onChange={e => setBotToken(e.target.value)}
                placeholder="123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
                className="flex-1 text-[11px] rounded px-2 py-1 outline-none font-mono"
                style={{ background: "var(--cream)", color: "var(--navy)", border: "0.5px solid var(--border)" }}
              />
              <button
                onClick={handleSaveToken}
                disabled={saving || !botToken.trim()}
                className="px-2.5 py-1 rounded text-[11px] font-medium transition-colors shrink-0 disabled:opacity-30"
                style={{ background: "var(--coral)", color: "white" }}
              >
                {saving ? "..." : "Connect"}
              </button>
            </div>
          </div>
        </>
      ) : (
        <div className="px-3 py-2 text-[11px]" style={{ color: "var(--navy-muted)", borderBottom: "0.5px solid var(--border)" }}>
          Bot connected. Messages from allowed usernames are accepted automatically.
        </div>
      )}

      {/* Username allowlist */}
      <div className="px-3 py-2" style={{ borderBottom: "0.5px solid var(--border)" }}>
        <div className="text-[11px] font-medium mb-1" style={{ color: "var(--navy)" }}>Allowed Usernames</div>
        <UsernameAllowlist />
      </div>

      {message && (
        <div className="px-3 py-1.5 text-[11px]" style={{ background: "var(--sage)", color: "var(--navy)", borderBottom: "0.5px solid var(--border)" }}>
          {message}
        </div>
      )}
    </div>
  );
}

function DaemonPanel() {
  const [serverMode, setServerMode] = useState<string>("off");
  const [launchdInstalled, setLaunchdInstalled] = useState(false);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    apiFetch("/server")
      .then(res => res.json())
      .then(data => {
        setServerMode(data.mode || "off");
        setLaunchdInstalled(data.launchd_installed || false);
      })
      .catch(() => {});
  }, []);

  const toggleServer = async () => {
    setLoading(true);
    setMessage("");
    const newMode = serverMode === "on" ? "off" : "on";
    try {
      const res = await apiFetch("/server", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mode: newMode }),
      });
      const data = await res.json();
      setServerMode(data.status || newMode);
      setMessage(data.message || "");
      setLaunchdInstalled(data.status === "on");
    } catch {
      setMessage("Failed to toggle server mode");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between px-3 py-2" style={{ borderBottom: "0.5px solid var(--border)" }}>
        <div>
          <div className="text-[13px] font-medium" style={{ color: "var(--navy)" }}>Server Mode</div>
          <div className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
            {serverMode === "on" ? "Restarts on crash, starts on boot" : "Stops when you close it"}
          </div>
        </div>
        <button
          onClick={toggleServer}
          disabled={loading}
          className="relative w-9 h-5 rounded-full transition-colors"
          style={{ background: serverMode === "on" ? "var(--coral)" : "var(--border)" }}
        >
          <span
            className="absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform"
            style={{ left: serverMode === "on" ? "calc(100% - 18px)" : "2px" }}
          />
        </button>
      </div>

      {message && (
        <div className="px-3 py-1.5 text-[11px]" style={{ background: "var(--sage)", color: "var(--navy)", borderBottom: "0.5px solid var(--border)" }}>
          {message}
        </div>
      )}

      <div className="px-3 py-2" style={{ borderBottom: "0.5px solid var(--border)" }}>
        <div className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
          launchd: {launchdInstalled ? "installed" : "not installed"} | Daemon: running
        </div>
      </div>
    </div>
  );
}

function IdentityPanel() {
  const files = ["SOUL.md", "AGENTS.md", "TOOLS.md", "HEARTBEAT.md", "USER.md", "MEMORY.md"];
  const [selected, setSelected] = useState("SOUL.md");
  const [content, setContent] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    readFile(selected).then(setContent);
  }, [selected]);

  const handleSave = async () => {
    setSaving(true);
    await writeFile(selected, content);
    setSaving(false);
  };

  return (
    <div className="flex flex-col h-full">
      <div className="flex gap-0.5 px-3 py-1.5" style={{ borderBottom: "0.5px solid var(--border)" }}>
        {files.map((f) => (
          <button
            key={f}
            onClick={() => setSelected(f)}
            className="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors"
            style={{
              background: selected === f ? "var(--coral)" : "transparent",
              color: selected === f ? "white" : "var(--navy-muted)",
            }}
          >
            {f}
          </button>
        ))}
      </div>
      <textarea
        value={content}
        onChange={(e) => setContent(e.target.value)}
        className="flex-1 px-3 py-2 text-[12px] font-mono resize-none outline-none"
        style={{
          background: "transparent",
          color: "var(--navy)",
        }}
      />
      <div className="px-3 py-1.5" style={{ borderTop: "0.5px solid var(--border)" }}>
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-2.5 py-1 rounded text-[11px] font-medium transition-colors"
          style={{ background: "var(--coral)", color: "white" }}
        >
          {saving ? "Saving..." : "Save"}
        </button>
      </div>
    </div>
  );
}

export default function Settings() {
  const [tab, setTab] = useState<SettingsTab>("providers");
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const [connectedProviders, setConnectedProviders] = useState<Record<string, { model?: string }>>({});

  const refreshStatus = async () => {
    const s = await getStatus();
    setStatus(s);
    try {
      const res = await apiFetch("/settings");
      const cfg = await res.json();
      if (cfg.providers) {
        const connected: Record<string, { model?: string }> = {};
        for (const [name, p] of Object.entries(cfg.providers)) {
          connected[name] = { model: (p as { model?: string }).model };
        }
        setConnectedProviders(connected);
      }
    } catch { /* daemon down */ }
  };

  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "providers", label: "Providers" },
    { id: "permissions", label: "Permissions" },
    { id: "telegram", label: "Telegram" },
    { id: "identity", label: "Identity" },
    { id: "daemon", label: "Daemon" },
  ];

  return (
    <div className="flex flex-col h-full">
      {/* Settings sub-tabs — macOS underline style */}
      <div
        className="flex px-3"
        style={{ borderBottom: "0.5px solid var(--border)" }}
      >
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className="px-2.5 py-1.5 text-[11px] font-medium transition-colors relative"
            style={{
              color: tab === t.id ? "var(--navy)" : "var(--navy-muted)",
              borderBottom: tab === t.id ? "2px solid var(--coral)" : "2px solid transparent",
              marginBottom: "-0.5px",
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto">
        {tab === "providers" && (
          <div className="flex flex-col">
            <div className="px-3 py-1.5 text-[11px] font-medium" style={{ color: "var(--navy-muted)", borderBottom: "0.5px solid var(--border)" }}>
              CLI Login (recommended)
            </div>
            <ProviderCard name="codex_cli" status={status} connected={"codex_cli" in connectedProviders} currentModel={connectedProviders["codex_cli"]?.model} onRefresh={refreshStatus} />
            <ProviderCard name="claude_cli" status={status} connected={"claude_cli" in connectedProviders} currentModel={connectedProviders["claude_cli"]?.model} onRefresh={refreshStatus} />

            <div className="px-3 py-1.5 text-[11px] font-medium" style={{ color: "var(--navy-muted)", borderBottom: "0.5px solid var(--border)" }}>
              API Key
            </div>
            <ProviderCard name="openrouter" status={status} connected={"openrouter" in connectedProviders} currentModel={connectedProviders["openrouter"]?.model} onRefresh={refreshStatus} />
          </div>
        )}
        {tab === "permissions" && <PermissionsPanel />}
        {tab === "telegram" && <TelegramPanel />}
        {tab === "identity" && <IdentityPanel />}
        {tab === "daemon" && <DaemonPanel />}
      </div>
    </div>
  );
}

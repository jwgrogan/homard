import { useEffect, useState } from "react";
import { getStatus, setPermissions, startAuth, generatePairingCode, getTelegramStatus, readFile, writeFile, type DaemonStatus } from "../lib/api";

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

  const handleConnect = async () => {
    setConnecting(true);
    try {
      if (isCli) {
        // CLI backends just need config written — no OAuth
        const res = await fetch("http://localhost:17700/settings");
        const cfg = await res.json();
        const providers = cfg.providers || {};
        providers[name] = {
          kind: name,
          auth_type: "cli",
          model: providerModels[name]?.[0] || "",
        };
        await fetch("http://localhost:17700/settings", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ ...cfg, providers, active_provider: name }),
        });
        setConnecting(false);
        onRefresh();
      } else {
        const { auth_url } = await startAuth(name);
        try {
          const { open } = await import("@tauri-apps/plugin-shell");
          await open(auth_url);
        } catch {
          window.open(auth_url, "_blank");
        }
        let attempts = 0;
        const poll = setInterval(async () => {
          attempts++;
          if (attempts > 40) { clearInterval(poll); setConnecting(false); return; }
          try {
            const res = await fetch("http://localhost:17700/settings");
            const cfg = await res.json();
            if (cfg.providers && cfg.providers[name]) {
              clearInterval(poll);
              setConnecting(false);
              onRefresh();
            }
          } catch { /* daemon may be busy */ }
        }, 3000);
      }
    } catch (e) {
      console.error(e);
      setConnecting(false);
    }
  };

  return (
    <div
      className="px-3 py-3 rounded-xl"
      style={{ background: "var(--cream-card)", border: `1px solid ${isActive ? "var(--coral)" : "var(--border)"}` }}
    >
      <div className="flex items-center justify-between">
        <div>
          <div className="text-sm font-medium" style={{ color: "var(--navy)" }}>
            {name === "codex_cli" ? "Codex CLI" : name === "claude_cli" ? "Claude CLI" : name.charAt(0).toUpperCase() + name.slice(1)}
          </div>
          <div className="text-xs mt-0.5" style={{ color: "var(--navy-muted)" }}>
            {name === "codex_cli" ? "Uses your Codex login (subscription)" :
             name === "claude_cli" ? "Uses your Claude login" :
             name === "anthropic" ? "Extra usage billing" :
             name === "openai" ? "API key billing" : "Per-token billing"}
          </div>
        </div>
        <div className="flex gap-2">
          <select
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="text-xs rounded-lg px-2 py-1 outline-none"
            style={{ background: "var(--sage)", color: "var(--navy)", border: "1px solid var(--border)" }}
          >
            {(providerModels[name] || []).map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </select>
          <button
            onClick={handleConnect}
            disabled={connecting || connected}
            className="px-3 py-1 rounded-lg text-xs font-medium transition-colors"
            style={{
              background: connected ? "#E8F5E9" : "var(--sage)",
              color: connected ? "#2E7D32" : "var(--navy)",
            }}
          >
            {connecting ? "..." : connected ? (isActive ? "Active" : "Connected") : isCli ? "Use" : "Sign in"}
          </button>
        </div>
      </div>
    </div>
  );
}

function PermissionsPanel() {
  const [level, setLevel] = useState("supervised");
  useEffect(() => {
    fetch("http://localhost:17700/settings/permissions")
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
    <div className="flex flex-col gap-2">
      {levels.map((l) => (
        <button
          key={l.id}
          onClick={() => handleChange(l.id)}
          className="px-3 py-3 rounded-xl text-left transition-colors"
          style={{
            background: level === l.id ? "var(--sage)" : "var(--cream-card)",
            border: `1px solid ${level === l.id ? "var(--coral)" : "var(--border)"}`,
          }}
        >
          <div className="text-sm font-medium" style={{ color: "var(--navy)" }}>{l.label}</div>
          <div className="text-xs mt-0.5" style={{ color: "var(--navy-muted)" }}>{l.desc}</div>
        </button>
      ))}
    </div>
  );
}

function TelegramPanel() {
  const [pairingCode, setPairingCode] = useState("");
  const [tgStatus, setTgStatus] = useState<{ enabled: boolean; paired_chats: number } | null>(null);

  useEffect(() => {
    getTelegramStatus().then(setTgStatus).catch(() => {});
  }, []);

  const handleGenerate = async () => {
    const code = await generatePairingCode();
    setPairingCode(code);
  };

  return (
    <div className="flex flex-col gap-3">
      <div
        className="px-3 py-3 rounded-xl"
        style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
      >
        <div className="text-sm font-medium" style={{ color: "var(--navy)" }}>
          Status: {tgStatus?.enabled ? `Connected (${tgStatus.paired_chats} chats)` : "Not connected"}
        </div>
      </div>
      <button
        onClick={handleGenerate}
        className="px-3 py-2 rounded-xl text-sm font-medium transition-colors"
        style={{ background: "var(--coral)", color: "white" }}
      >
        Generate Pairing Code
      </button>
      {pairingCode && (
        <div
          className="px-3 py-3 rounded-xl text-center"
          style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
        >
          <div className="text-xs" style={{ color: "var(--navy-muted)" }}>Send this in Telegram:</div>
          <div className="text-lg font-mono font-bold mt-1" style={{ color: "var(--coral)" }}>
            /pair {pairingCode}
          </div>
          <div className="text-xs mt-1" style={{ color: "var(--navy-muted)" }}>Expires in 10 minutes</div>
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
    fetch("http://localhost:17700/server")
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
      const res = await fetch("http://localhost:17700/server", {
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
    <div className="flex flex-col gap-3">
      <div
        className="px-3 py-3 rounded-xl"
        style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
      >
        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium" style={{ color: "var(--navy)" }}>
              Server Mode
            </div>
            <div className="text-xs mt-0.5" style={{ color: "var(--navy-muted)" }}>
              {serverMode === "on"
                ? "Homard restarts on crash, starts on boot"
                : "Daemon stops when you close it"}
            </div>
          </div>
          <button
            onClick={toggleServer}
            disabled={loading}
            className="relative w-12 h-6 rounded-full transition-colors"
            style={{
              background: serverMode === "on" ? "var(--coral)" : "var(--border)",
            }}
          >
            <span
              className="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform"
              style={{
                left: serverMode === "on" ? "calc(100% - 22px)" : "2px",
              }}
            />
          </button>
        </div>
      </div>

      {message && (
        <div
          className="px-3 py-2 rounded-xl text-xs"
          style={{ background: "var(--sage)", color: "var(--navy)" }}
        >
          {message}
        </div>
      )}

      <div
        className="px-3 py-3 rounded-xl"
        style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
      >
        <div className="text-sm font-medium" style={{ color: "var(--navy)" }}>Status</div>
        <div className="text-xs mt-1 space-y-1" style={{ color: "var(--navy-muted)" }}>
          <div>launchd plist: {launchdInstalled ? "installed" : "not installed"}</div>
          <div>Daemon: running (you're seeing this page)</div>
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
    <div className="flex flex-col gap-2 h-full">
      <div className="flex gap-1 flex-wrap">
        {files.map((f) => (
          <button
            key={f}
            onClick={() => setSelected(f)}
            className="px-2 py-1 rounded-lg text-xs transition-colors"
            style={{
              background: selected === f ? "var(--coral)" : "var(--sage)",
              color: selected === f ? "white" : "var(--navy)",
            }}
          >
            {f}
          </button>
        ))}
      </div>
      <textarea
        value={content}
        onChange={(e) => setContent(e.target.value)}
        className="flex-1 rounded-xl px-3 py-2 text-xs font-mono resize-none outline-none"
        style={{
          background: "var(--cream-card)",
          border: "1px solid var(--border)",
          color: "var(--navy)",
          minHeight: "150px",
        }}
      />
      <button
        onClick={handleSave}
        disabled={saving}
        className="px-3 py-2 rounded-xl text-sm font-medium transition-colors"
        style={{ background: "var(--coral)", color: "white" }}
      >
        {saving ? "Saving..." : "Save"}
      </button>
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
    // Also fetch config to see which providers are connected
    try {
      const res = await fetch("http://localhost:17700/settings");
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
      <div
        className="px-4 py-3 border-b"
        style={{ borderColor: "var(--border)", background: "var(--sage)" }}
      >
        <span className="font-semibold text-sm" style={{ color: "var(--navy)" }}>
          Settings
        </span>
      </div>

      {/* Setting tabs */}
      <div className="flex gap-1 px-4 pt-3">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className="px-2.5 py-1 rounded-lg text-xs font-medium transition-colors"
            style={{
              background: tab === t.id ? "var(--navy)" : "transparent",
              color: tab === t.id ? "var(--cream)" : "var(--navy-muted)",
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {tab === "providers" && (
          <div className="flex flex-col gap-2">
            <div className="text-xs font-medium mb-1" style={{ color: "var(--navy-muted)" }}>CLI Backends (use your existing login)</div>
            <ProviderCard name="codex_cli" status={status} connected={"codex_cli" in connectedProviders} currentModel={connectedProviders["codex_cli"]?.model} onRefresh={refreshStatus} />
            <ProviderCard name="claude_cli" status={status} connected={"claude_cli" in connectedProviders} currentModel={connectedProviders["claude_cli"]?.model} onRefresh={refreshStatus} />
            <div className="text-xs font-medium mb-1 mt-3" style={{ color: "var(--navy-muted)" }}>API Keys & OAuth</div>
            <ProviderCard name="openai" status={status} connected={"openai" in connectedProviders} currentModel={connectedProviders["openai"]?.model} onRefresh={refreshStatus} />
            <ProviderCard name="anthropic" status={status} connected={"anthropic" in connectedProviders} currentModel={connectedProviders["anthropic"]?.model} onRefresh={refreshStatus} />
            <ProviderCard name="openrouter" status={status} connected={"openrouter" in connectedProviders} currentModel={connectedProviders["openrouter"]?.model} onRefresh={refreshStatus} />
          </div>
        )}
        {tab === "permissions" && <PermissionsPanel />}
        {tab === "telegram" && <TelegramPanel />}
        {tab === "identity" && <IdentityPanel />}
        {tab === "daemon" && <DaemonPanel />}
      </div>

      {/* Daemon status footer */}
      <div
        className="px-4 py-2 text-xs border-t flex items-center gap-2"
        style={{ borderColor: "var(--border)", color: "var(--navy-muted)" }}
      >
        <span
          className="w-2 h-2 rounded-full"
          style={{ background: status?.running ? "#4CAF50" : "#E53935" }}
        />
        {status?.running ? `Daemon running \u00B7 ${status.active_provider}` : "Daemon offline"}
      </div>
    </div>
  );
}

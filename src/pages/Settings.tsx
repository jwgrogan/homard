import { useEffect, useState } from "react";
import { getStatus, setPermissions, startAuth, generatePairingCode, getTelegramStatus, readFile, writeFile, type DaemonStatus } from "../lib/api";

type SettingsTab = "providers" | "permissions" | "telegram" | "identity";

function ProviderCard({ name, status }: { name: string; status: DaemonStatus | null }) {
  const isActive = status?.active_provider === name;
  const [connecting, setConnecting] = useState(false);

  const handleConnect = async () => {
    setConnecting(true);
    try {
      const { auth_url } = await startAuth(name);
      window.open(auth_url, "_blank");
    } catch (e) {
      console.error(e);
    } finally {
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
            {name.charAt(0).toUpperCase() + name.slice(1)}
          </div>
          <div className="text-xs mt-0.5" style={{ color: "var(--navy-muted)" }}>
            {name === "anthropic" ? "Extra usage billing" : name === "openai" ? "Subscription credits" : "Per-token billing"}
          </div>
        </div>
        <button
          onClick={handleConnect}
          disabled={connecting}
          className="px-3 py-1 rounded-lg text-xs font-medium transition-colors"
          style={{ background: "var(--sage)", color: "var(--navy)" }}
        >
          {connecting ? "..." : "Sign in"}
        </button>
      </div>
    </div>
  );
}

function PermissionsPanel() {
  const [level, setLevel] = useState("supervised");

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

  useEffect(() => {
    getStatus().then(setStatus);
  }, []);

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "providers", label: "Providers" },
    { id: "permissions", label: "Permissions" },
    { id: "telegram", label: "Telegram" },
    { id: "identity", label: "Identity" },
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
            <ProviderCard name="openai" status={status} />
            <ProviderCard name="anthropic" status={status} />
            <ProviderCard name="openrouter" status={status} />
          </div>
        )}
        {tab === "permissions" && <PermissionsPanel />}
        {tab === "telegram" && <TelegramPanel />}
        {tab === "identity" && <IdentityPanel />}
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

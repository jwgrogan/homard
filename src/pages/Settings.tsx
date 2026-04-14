import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-shell";
import {
  apiFetch,
  getStatus,
  getTelegramStatus,
  readFile,
  setPermissions,
  startAuth,
  stopRun,
  writeFile,
  type DaemonStatus,
} from "../lib/api";

const providerModels: Record<string, string[]> = {
  codex_cli: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  claude_cli: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openai: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  anthropic: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openrouter: ["anthropic/claude-sonnet-4-6", "openai/gpt-5.4"],
};

function Section({
  label,
  title,
  body,
  children,
}: {
  label: string;
  title: string;
  body: string;
  children: React.ReactNode;
}) {
  return (
    <section className="settings-block">
      <div className="settings-block__header">
        <div className="subtle-label">{label}</div>
        <h3 className="section-title mt-1">{title}</h3>
        <p className="section-meta mt-1">{body}</p>
      </div>
      <div className="settings-block__body">{children}</div>
    </section>
  );
}

function ProviderRow({
  name,
  status,
  connected,
  currentModel,
  onRefresh,
  onMessage,
}: {
  name: string;
  status: DaemonStatus | null;
  connected: boolean;
  currentModel?: string;
  onRefresh: () => void;
  onMessage: (message: string) => void;
}) {
  const isActive = status?.active_provider === name;
  const [connecting, setConnecting] = useState(false);
  const [model, setModel] = useState(currentModel || providerModels[name]?.[0] || "");
  const [apiKey, setApiKey] = useState("");

  const isCli = name === "codex_cli" || name === "claude_cli";
  const isOAuth = name === "openai" || name === "anthropic";
  const isApiKey = name === "openrouter";
  const label =
    name === "codex_cli" ? "Codex CLI" :
    name === "claude_cli" ? "Claude CLI" :
    name === "openai" ? "OpenAI" :
    name === "openrouter" ? "OpenRouter" :
    "Anthropic";
  const meta =
    name === "codex_cli" ? "Uses your local Codex login." :
    name === "claude_cli" ? "Uses your local Claude login." :
    name === "openai" ? "Connects through browser OAuth." :
    name === "anthropic" ? "Connects through browser OAuth." :
    "Stores one API key and routes requests through OpenRouter.";

  const handleConnect = async () => {
    setConnecting(true);
    try {
      if (isOAuth && !connected) {
        const { auth_url } = await startAuth(name);
        await open(auth_url);
        onMessage(`Finish ${label} sign-in in your browser. Homard will pick it up automatically.`);
        window.setTimeout(onRefresh, 2500);
        return;
      }

      const res = await apiFetch("/settings");
      const cfg = await res.json();
      const providers = cfg.providers || {};

      if (isCli) {
        providers[name] = { kind: name, auth_type: "cli", model: model || providerModels[name]?.[0] || "" };
      } else if (isApiKey && apiKey.trim()) {
        providers[name] = {
          kind: name,
          auth_type: "api_key",
          model: model || providerModels[name]?.[0] || "",
          api_key_keychain_ref: `homard.${name}.api_key`,
        };
      } else if (isOAuth && connected) {
        providers[name] = { ...providers[name], model: model || providerModels[name]?.[0] || "" };
      }

      await apiFetch("/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ...cfg, providers, active_provider: name }),
      });
      onRefresh();
      onMessage(isActive ? `${label} updated.` : `${label} is now the active provider.`);
    } catch (e) {
      console.error(e);
      onMessage(`${label} could not be configured.`);
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <div className="text-[14px] font-medium">{label}</div>
          {isActive && (
            <span className="pill" style={{ background: "var(--accent-soft)", borderColor: "transparent", color: "var(--accent)" }}>
              Active
            </span>
          )}
        </div>
        <div className="mt-1 text-[12px]" style={{ color: "var(--ink-soft)" }}>{meta}</div>
        <div className="mt-3 flex gap-2">
          <select value={model} onChange={(e) => setModel(e.target.value)} className="field">
            {(providerModels[name] || []).map((item) => (
              <option key={item} value={item}>{item}</option>
            ))}
          </select>
          <button onClick={handleConnect} disabled={connecting || (isApiKey && !connected && !apiKey.trim())} className="cta disabled:opacity-40">
            {connecting ? "Working..." : connected ? (isActive ? "Active" : "Use") : isOAuth ? "Connect" : isCli ? "Use" : "Save"}
          </button>
        </div>
        {isApiKey && !connected && (
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="OpenRouter API key"
            className="field is-mono mt-2"
          />
        )}
      </div>
    </div>
  );
}

function PermissionsSection() {
  const [level, setLevel] = useState("supervised");

  useEffect(() => {
    apiFetch("/settings/permissions")
      .then((res) => res.json())
      .then((data) => {
        if (typeof data === "string") setLevel(data);
      })
      .catch(() => {});
  }, []);

  const levels = [
    { id: "supervised", label: "Supervised", desc: "Run safe actions automatically and block anything that still needs approval." },
    { id: "autonomous", label: "Autonomous", desc: "Run without prompts and notify only on outcomes." },
    { id: "locked", label: "Locked", desc: "Keep the assistant read-only." },
  ];

  return (
    <div className="row-list">
      {levels.map((item) => (
        <button
          key={item.id}
          onClick={async () => {
            setLevel(item.id);
            await setPermissions(item.id);
          }}
          className="row-item grid-cols-[minmax(0,1fr)_auto]"
        >
          <div>
            <div className="text-[14px] font-medium">{item.label}</div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>{item.desc}</div>
          </div>
          <span
            className="pill"
            style={{
              background: level === item.id ? "var(--accent-soft)" : "rgba(22, 48, 75, 0.06)",
              borderColor: "transparent",
              color: level === item.id ? "var(--accent)" : "var(--ink-soft)",
            }}
          >
            {level === item.id ? "Current" : "Select"}
          </span>
        </button>
      ))}
    </div>
  );
}

function TelegramSection({ onMessage }: { onMessage: (message: string) => void }) {
  const [botToken, setBotToken] = useState("");
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<{ enabled: boolean; paired_chats: number; allowed_usernames?: string[]; bot_name?: string } | null>(null);
  const [newUsername, setNewUsername] = useState("");
  const usernames = status?.allowed_usernames || [];

  const refresh = () => getTelegramStatus().then((data) => setStatus(data as typeof status)).catch(() => {});

  useEffect(() => {
    refresh();
  }, []);

  const persistUsernames = async (updated: string[]) => {
    const res = await apiFetch("/settings");
    const cfg = await res.json();
    cfg.telegram = cfg.telegram || {};
    cfg.telegram.allowed_usernames = updated;
    await apiFetch("/settings", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(cfg),
    });
    await refresh();
  };

  return (
    <div className="settings-grid">
      <div>
        <div className="text-[14px] font-medium">
          {status?.enabled ? (status.bot_name ? `@${status.bot_name}` : "Bot connected") : "Not connected"}
        </div>
        <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
          {status?.enabled ? `${status.paired_chats} paired chats connected.` : "Connect a bot token, then restrict who can talk to it."}
        </div>
      </div>

      {!status?.enabled && (
        <>
          <input
            type="password"
            value={botToken}
            onChange={(e) => setBotToken(e.target.value)}
            placeholder="Telegram bot token"
            className="field is-mono"
          />
          <div className="flex justify-end">
            <button
              onClick={async () => {
                if (!botToken.trim()) return;
                setSaving(true);
                try {
                  const res = await apiFetch("/telegram/token", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ token: botToken.trim() }),
                  });
                  const data = await res.json();
                  onMessage(res.ok ? data.message || "Telegram connected." : data || "Failed to connect.");
                  if (res.ok) {
                    setBotToken("");
                    await refresh();
                  }
                } catch {
                  onMessage("Failed to connect Telegram.");
                } finally {
                  setSaving(false);
                }
              }}
              disabled={saving || !botToken.trim()}
              className="cta disabled:opacity-40"
            >
              {saving ? "Connecting..." : "Connect"}
            </button>
          </div>
        </>
      )}

      <div className="settings-grid">
        <div className="text-[12px] font-medium">Allowed usernames</div>
        <div className="flex gap-2">
          <input
            value={newUsername}
            onChange={(e) => setNewUsername(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newUsername.trim()) {
                persistUsernames([...usernames, newUsername.trim().replace(/^@/, "")]);
                setNewUsername("");
              }
            }}
            placeholder="@username"
            className="field"
          />
          <button
            onClick={() => {
              if (!newUsername.trim()) return;
              persistUsernames([...usernames, newUsername.trim().replace(/^@/, "")]);
              setNewUsername("");
            }}
            disabled={!newUsername.trim()}
            className="secondary-cta disabled:opacity-40"
          >
            Add
          </button>
        </div>
        {usernames.length > 0 ? (
          <div className="flex flex-wrap gap-2">
            {usernames.map((item) => (
              <span key={item} className="pill">
                <span>@{item}</span>
                <button onClick={() => persistUsernames(usernames.filter((name) => name !== item))} style={{ color: "var(--ink-soft)" }}>
                  Remove
                </button>
              </span>
            ))}
          </div>
        ) : (
          <p className="section-meta">Messages from Telegram are ignored until at least one username is allowed.</p>
        )}
      </div>
    </div>
  );
}

function DaemonSection({ onMessage }: { onMessage: (message: string) => void }) {
  const [serverMode, setServerMode] = useState<string>("off");
  const [launchdInstalled, setLaunchdInstalled] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    apiFetch("/server")
      .then((res) => res.json())
      .then((data) => {
        setServerMode(data.mode || "off");
        setLaunchdInstalled(data.launchd_installed || false);
      })
      .catch(() => {});
  }, []);

  return (
    <div className="row-list">
      <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
        <div>
          <div className="text-[14px] font-medium">Server mode</div>
          <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
            {serverMode === "on" ? "Starts on boot and restarts after crashes." : "Runs only while you keep it open."}
          </div>
        </div>
        <button
          onClick={async () => {
            setLoading(true);
            const newMode = serverMode === "on" ? "off" : "on";
            try {
              const res = await apiFetch("/server", {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ mode: newMode }),
              });
              const data = await res.json();
              setServerMode(data.status || newMode);
              setLaunchdInstalled(data.status === "on");
              onMessage(data.message || "Daemon mode updated.");
            } finally {
              setLoading(false);
            }
          }}
          disabled={loading}
          className={serverMode === "on" ? "cta" : "secondary-cta"}
        >
          {loading ? "Working..." : serverMode === "on" ? "Turn off" : "Turn on"}
        </button>
      </div>
      <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
        <div>
          <div className="text-[14px] font-medium">Launch agent</div>
          <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
            launchd is {launchdInstalled ? "installed" : "not installed"}.
          </div>
        </div>
        <span className="pill" style={{ background: launchdInstalled ? "var(--success-bg)" : "rgba(22, 48, 75, 0.06)", borderColor: "transparent", color: launchdInstalled ? "var(--success)" : "var(--ink-soft)" }}>
          {launchdInstalled ? "Installed" : "Missing"}
        </span>
      </div>
    </div>
  );
}

function IdentitySection() {
  const [botName, setBotName] = useState("Homard");
  const [botEmoji, setBotEmoji] = useState("🦞");
  const [botTagline, setBotTagline] = useState("Your personal crustacean");
  const [userName, setUserName] = useState("");
  const [userRole, setUserRole] = useState("");
  const [selected, setSelected] = useState("SOUL.md");
  const [content, setContent] = useState("");
  const [saving, setSaving] = useState(false);
  const files = ["SOUL.md", "AGENTS.md", "TOOLS.md", "HEARTBEAT.md", "USER.md", "MEMORY.md"];

  useEffect(() => {
    apiFetch("/files/IDENTITY.md").then((r) => r.text()).then((text) => {
      const n = text.match(/name:\s*(.+)/); if (n) setBotName(n[1].trim());
      const e = text.match(/emoji:\s*(.+)/); if (e) setBotEmoji(e[1].trim());
      const t = text.match(/tagline:\s*(.+)/); if (t) setBotTagline(t[1].trim());
    }).catch(() => {});
    apiFetch("/files/USER.md").then((r) => r.text()).then((text) => {
      const n = text.match(/[Nn]ame:\s*(.+)/); if (n) setUserName(n[1].trim());
      const r = text.match(/[Rr]ole:\s*(.+)/); if (r) setUserRole(r[1].trim());
    }).catch(() => {});
  }, []);

  useEffect(() => {
    readFile(selected).then(setContent);
  }, [selected]);

  return (
    <div className="settings-grid">
      <div className="settings-grid">
        <div className="text-[12px] font-medium">Assistant</div>
        <input value={botName} onChange={(e) => setBotName(e.target.value)} placeholder="Name" className="field" />
        <input value={botTagline} onChange={(e) => setBotTagline(e.target.value)} placeholder="Tagline" className="field" />
        <div className="text-[11px]" style={{ color: "var(--ink-soft)" }}>Symbol on file: {botEmoji}</div>
        <div className="flex justify-end">
          <button
            onClick={async () => {
              setSaving(true);
              await writeFile("IDENTITY.md", `name: ${botName}\nemoji: ${botEmoji}\ntagline: ${botTagline}\n`);
              setSaving(false);
            }}
            disabled={saving}
            className="cta disabled:opacity-40"
          >
            {saving ? "Saving..." : "Save assistant"}
          </button>
        </div>
      </div>

      <div className="settings-grid">
        <div className="text-[12px] font-medium">User</div>
        <input value={userName} onChange={(e) => setUserName(e.target.value)} placeholder="Your name" className="field" />
        <input value={userRole} onChange={(e) => setUserRole(e.target.value)} placeholder="Role or short descriptor" className="field" />
        <div className="flex justify-end">
          <button
            onClick={async () => {
              setSaving(true);
              const res = await apiFetch("/files/USER.md");
              let text = await res.text();
              if (text.match(/[Nn]ame:\s*.+/)) {
                text = text.replace(/[Nn]ame:\s*.+/, `Name: ${userName}`);
              } else {
                text = `# User Profile\n\nName: ${userName}\n` + text;
              }
              if (userRole) {
                if (text.match(/[Rr]ole:\s*.+/)) {
                  text = text.replace(/[Rr]ole:\s*.+/, `Role: ${userRole}`);
                } else {
                  text += `\nRole: ${userRole}\n`;
                }
              }
              await writeFile("USER.md", text);
              setSaving(false);
            }}
            disabled={saving}
            className="cta disabled:opacity-40"
          >
            {saving ? "Saving..." : "Save profile"}
          </button>
        </div>
      </div>

      <details className="disclosure">
        <summary>Identity files</summary>
        <div className="disclosure__body">
          <div className="settings-grid">
            <select value={selected} onChange={(e) => setSelected(e.target.value)} className="field">
              {files.map((item) => (
                <option key={item} value={item}>{item}</option>
              ))}
            </select>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              className="field is-mono resize-none"
              rows={14}
            />
            <div className="flex justify-end">
              <button
                onClick={async () => {
                  setSaving(true);
                  await writeFile(selected, content);
                  setSaving(false);
                }}
                disabled={saving}
                className="cta disabled:opacity-40"
              >
                {saving ? "Saving..." : "Save file"}
              </button>
            </div>
          </div>
        </div>
      </details>
    </div>
  );
}

export default function Settings() {
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const [connectedProviders, setConnectedProviders] = useState<Record<string, { model?: string }>>({});
  const [bannerMessage, setBannerMessage] = useState("");
  const [stoppingRun, setStoppingRun] = useState(false);

  const refreshStatus = async () => {
    const next = await getStatus();
    setStatus(next);
    try {
      const res = await apiFetch("/settings");
      const cfg = await res.json();
      if (cfg.providers) {
        const connected: Record<string, { model?: string }> = {};
        for (const [name, provider] of Object.entries(cfg.providers)) {
          connected[name] = { model: (provider as { model?: string }).model };
        }
        setConnectedProviders(connected);
      }
    } catch {
      // daemon unavailable
    }
  };

  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div>
          <div className="subtle-label">Settings</div>
          <h2 className="section-title">System, providers, and files</h2>
          <p className="section-meta">One scroll view. Advanced controls stay available, but no longer drive the structure.</p>
        </div>
      </div>

      <div className="settings-stack">
        <section className="settings-summary">
          <div className="settings-summary__item">
            <div className="subtle-label">Provider</div>
            <div className="mt-1 text-[15px] font-semibold">{status?.active_provider ?? "Not configured"}</div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>{status?.active_model ?? "No active model"}</div>
          </div>
          <div className="settings-summary__item">
            <div className="subtle-label">Permissions</div>
            <div className="mt-1 text-[15px] font-semibold">{status?.permission_level ?? "Unknown"}</div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>{status?.telegram_connected ? "Telegram connected" : "Telegram idle"}</div>
          </div>
          <div className="settings-summary__item">
            <div className="subtle-label">Current run</div>
            <div className="mt-1 text-[15px] font-semibold">{status?.current_run ?? "None"}</div>
            <div className="mt-2">
              <button
                onClick={async () => {
                  setStoppingRun(true);
                  try {
                    await stopRun();
                    setBannerMessage("Stop signal sent to the current run.");
                  } finally {
                    setStoppingRun(false);
                  }
                }}
                disabled={!status?.current_run || stoppingRun}
                className="secondary-cta disabled:opacity-40"
              >
                {stoppingRun ? "Stopping..." : "Stop run"}
              </button>
            </div>
          </div>
        </section>

        {bannerMessage && (
          <div className="settings-block">
            <div className="settings-block__body">
              <div className="text-[12px]">{bannerMessage}</div>
            </div>
          </div>
        )}

        <Section
          label="Models"
          title="Providers"
          body="CLI is the fastest start. OAuth keeps direct vendor accounts clean. OpenRouter stays available for one-key access."
        >
          <div className="row-list">
            <ProviderRow name="codex_cli" status={status} connected={"codex_cli" in connectedProviders} currentModel={connectedProviders["codex_cli"]?.model} onRefresh={refreshStatus} onMessage={setBannerMessage} />
            <ProviderRow name="claude_cli" status={status} connected={"claude_cli" in connectedProviders} currentModel={connectedProviders["claude_cli"]?.model} onRefresh={refreshStatus} onMessage={setBannerMessage} />
            <ProviderRow name="openai" status={status} connected={"openai" in connectedProviders} currentModel={connectedProviders["openai"]?.model} onRefresh={refreshStatus} onMessage={setBannerMessage} />
            <ProviderRow name="anthropic" status={status} connected={"anthropic" in connectedProviders} currentModel={connectedProviders["anthropic"]?.model} onRefresh={refreshStatus} onMessage={setBannerMessage} />
            <ProviderRow name="openrouter" status={status} connected={"openrouter" in connectedProviders} currentModel={connectedProviders["openrouter"]?.model} onRefresh={refreshStatus} onMessage={setBannerMessage} />
          </div>
        </Section>

        <Section
          label="Safety"
          title="Permissions"
          body="These levels are mutually exclusive. The current mode should be obvious at a glance."
        >
          <PermissionsSection />
        </Section>

        <Section
          label="Bridge"
          title="Telegram"
          body="Pair the bot, then allow only the usernames you trust."
        >
          <TelegramSection onMessage={setBannerMessage} />
        </Section>

        <Section
          label="Daemon"
          title="Background service"
          body="This answers the only question that matters here: does Homard stay alive when the window closes?"
        >
          <DaemonSection onMessage={setBannerMessage} />
        </Section>

        <Section
          label="Identity"
          title="Profile and source files"
          body="Keep profile data short. Raw file editing stays available, but out of the primary path."
        >
          <IdentitySection />
        </Section>
      </div>
    </div>
  );
}

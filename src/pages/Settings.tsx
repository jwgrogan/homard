import { useEffect, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-shell";
import {
  apiFetch,
  generatePairingCode,
  getSettings,
  getSettingsSnapshot,
  getStatus,
  readFile,
  saveProviderApiKey,
  setPermissions,
  setServerMode,
  startAuth,
  stopRun,
  updateSettings,
  writeFile,
  type DaemonDiagnostics,
  type DaemonStatus,
  type IdentityDiagnostics,
  type ProviderDiagnostics,
  type SettingsSnapshot,
  type TelegramDiagnostics,
} from "../lib/api";

const providerModels: Record<string, string[]> = {
  codex_cli: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  claude_cli: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openai: ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"],
  anthropic: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  openrouter: ["anthropic/claude-sonnet-4-6", "openai/gpt-5.4"],
};

const providerOrder = ["codex_cli", "claude_cli", "openai", "anthropic", "openrouter"] as const;

function Section({
  label,
  title,
  body,
  children,
}: {
  label: string;
  title: string;
  body: string;
  children: ReactNode;
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

function providerMeta(name: string) {
  switch (name) {
    case "codex_cli":
      return {
        label: "Codex CLI",
        meta: "Uses your local Codex login and shows the MCP inventory the CLI can see.",
      };
    case "claude_cli":
      return {
        label: "Claude CLI",
        meta: "Uses your local Claude login and reports the CLI's current MCP health.",
      };
    case "openai":
      return {
        label: "OpenAI",
        meta: "Connects through browser OAuth and uses the saved secure token.",
      };
    case "anthropic":
      return {
        label: "Anthropic",
        meta: "Connects through browser OAuth and uses the saved secure token.",
      };
    case "openrouter":
      return {
        label: "OpenRouter",
        meta: "Stores one API key securely and routes requests through OpenRouter.",
      };
    default:
      return { label: name, meta: "" };
  }
}

function upsertField(content: string, field: string, value: string, { frontmatter = false }: { frontmatter?: boolean } = {}) {
  const lines = content.length > 0 ? content.split("\n") : [];
  const matcher = new RegExp(`^${field.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*:`, "i");
  const nextLine = `${field}: ${value}`.trimEnd();

  if (!value.trim()) {
    return lines.filter((line) => !matcher.test(line)).join("\n").replace(/\n{3,}/g, "\n\n").trimEnd() + "\n";
  }

  const index = lines.findIndex((line) => matcher.test(line));
  if (index >= 0) {
    lines[index] = nextLine;
    return `${lines.join("\n").trimEnd()}\n`;
  }

  if (frontmatter || lines.length === 0) {
    return `${[...lines.filter(Boolean), nextLine].join("\n").trimEnd()}\n`;
  }

  return `${content.trimEnd()}\n${nextLine}\n`;
}

function formatPermission(value?: string | null) {
  switch (value) {
    case "supervised":
      return "Supervised";
    case "autonomous":
      return "Autonomous";
    case "locked":
      return "Locked";
    default:
      return "Unknown";
  }
}

function formatProviderTitle(key?: string | null) {
  if (!key) return "Not configured";
  return providerMeta(key).label;
}

function mcpPill(status: string) {
  if (status === "connected") {
    return { text: "Connected", bg: "var(--success-bg)", color: "var(--success)" };
  }
  if (status === "needs_auth") {
    return { text: "Needs auth", bg: "rgba(235, 179, 66, 0.12)", color: "#9A6C00" };
  }
  if (status === "failed") {
    return { text: "Failed", bg: "var(--error-bg)", color: "var(--error)" };
  }
  return { text: status.replace(/_/g, " "), bg: "rgba(22, 48, 75, 0.06)", color: "var(--ink-soft)" };
}

function ProviderRow({
  name,
  provider,
  onRefresh,
  onMessage,
}: {
  name: string;
  provider?: ProviderDiagnostics;
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const { label, meta } = providerMeta(name);
  const [connecting, setConnecting] = useState(false);
  const [model, setModel] = useState(provider?.model || providerModels[name]?.[0] || "");
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    setModel(provider?.model || providerModels[name]?.[0] || "");
  }, [name, provider?.model]);

  const isCli = name === "codex_cli" || name === "claude_cli";
  const isOAuth = name === "openai" || name === "anthropic";
  const isApiKey = name === "openrouter";
  const isActive = provider?.active ?? false;
  const connected = provider?.connected ?? false;
  const installed = provider?.installed;

  const buttonLabel = (() => {
    if (connecting) return "Working...";
    if (isActive) return "Active";
    if (isOAuth && !connected) return "Connect";
    if (isApiKey && apiKey.trim()) return "Save";
    if (isCli && !installed) return "Unavailable";
    if (isCli && !connected) return "Needs login";
    return "Use";
  })();

  const disabled =
    connecting ||
    (isCli && (!installed || !connected)) ||
    (isApiKey && !connected && !apiKey.trim());

  const handleConnect = async () => {
    setConnecting(true);
    try {
      if (isOAuth && !connected) {
        const { auth_url } = await startAuth(name);
        await open(auth_url);
        onMessage(`Finish ${label} sign-in in your browser. Refresh this page if the token does not appear automatically.`);
        window.setTimeout(() => {
          onRefresh();
        }, 2500);
        return;
      }

      if (isApiKey && apiKey.trim()) {
        const res = await saveProviderApiKey(name, apiKey.trim(), model);
        setApiKey("");
        onMessage(res.message || `${label} saved.`);
        await onRefresh();
        return;
      }

      if (isCli && !installed) {
        onMessage(`${label} is not installed on this machine.`);
        return;
      }

      if (isCli && !connected) {
        onMessage(`${label} is installed but not logged in yet. Finish login in your terminal, then refresh.`);
        return;
      }

      const cfg = await getSettings();
      const providers = { ...(cfg.providers as Record<string, unknown> | undefined) };

      if (isCli) {
        providers[name] = {
          kind: name,
          auth_type: "cli",
          model: model || providerModels[name]?.[0] || "",
        };
      } else if (isOAuth) {
        providers[name] = {
          ...(providers[name] as Record<string, unknown> | undefined),
          model: model || providerModels[name]?.[0] || "",
        };
      } else if (isApiKey) {
        providers[name] = {
          ...(providers[name] as Record<string, unknown> | undefined),
          model: model || providerModels[name]?.[0] || "",
        };
      }

      await updateSettings({
        ...cfg,
        providers,
        active_provider: name,
      });
      await onRefresh();
      onMessage(isActive ? `${label} updated.` : `${label} is now the active provider.`);
    } catch (error) {
      console.error(error);
      onMessage(error instanceof Error ? error.message : `${label} could not be configured.`);
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
          <span
            className="pill"
            style={{
              background: connected ? "var(--success-bg)" : "rgba(22, 48, 75, 0.06)",
              borderColor: "transparent",
              color: connected ? "var(--success)" : "var(--ink-soft)",
            }}
          >
            {provider?.auth_status ?? "Unknown"}
          </span>
        </div>
        <div className="mt-1 text-[12px]" style={{ color: "var(--ink-soft)" }}>{meta}</div>
        {provider?.auth_detail && (
          <div className="mt-2 text-[12px]" style={{ color: "var(--ink-soft)" }}>
            {provider.auth_detail}
          </div>
        )}
        {(provider?.version || provider?.binary_path) && (
          <div className="mt-2 flex flex-wrap gap-2 text-[11px]" style={{ color: "var(--ink-soft)" }}>
            {provider.version && <span className="pill">Version {provider.version}</span>}
            {provider.binary_path && <span className="pill">{provider.binary_path}</span>}
          </div>
        )}
        <div className="mt-3 flex gap-2">
          <select value={model} onChange={(event) => setModel(event.target.value)} className="field">
            {(providerModels[name] || []).map((item) => (
              <option key={item} value={item}>{item}</option>
            ))}
          </select>
          <button onClick={handleConnect} disabled={disabled} className="cta disabled:opacity-40">
            {buttonLabel}
          </button>
        </div>
        {isApiKey && (
          <input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            placeholder="OpenRouter API key"
            className="field is-mono mt-2"
          />
        )}
        {(provider?.mcp_servers?.length ?? 0) > 0 && (
          <div className="mt-3 grid gap-2">
            <div className="text-[12px] font-medium">Available MCPs</div>
            {provider?.mcp_servers?.map((server) => {
              const pill = mcpPill(server.status);
              return (
                <div
                  key={`${provider?.key ?? name}-${server.name}`}
                  className="rounded-[14px] border px-3 py-2"
                  style={{ borderColor: "var(--line)", background: "rgba(255,255,255,0.48)" }}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="min-w-0 text-[13px] font-medium">{server.name}</div>
                    <span className="pill" style={{ background: pill.bg, borderColor: "transparent", color: pill.color }}>
                      {pill.text}
                    </span>
                  </div>
                  <div className="mt-1 text-[11px] break-all" style={{ color: "var(--ink-soft)" }}>{server.target}</div>
                  {server.auth && <div className="mt-1 text-[11px]" style={{ color: "var(--ink-soft)" }}>{server.auth}</div>}
                </div>
              );
            })}
          </div>
        )}
        {isCli && provider?.installed && (provider?.mcp_servers?.length ?? 0) === 0 && (
          <p className="section-meta mt-3">No MCP servers were reported by this CLI.</p>
        )}
      </div>
    </div>
  );
}

function PermissionsSection({
  currentLevel,
  onRefresh,
  onMessage,
}: {
  currentLevel?: string | null;
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const [saving, setSaving] = useState<string | null>(null);

  const levels = [
    { id: "supervised", label: "Supervised", desc: "Approve dangerous actions before they run." },
    { id: "autonomous", label: "Autonomous", desc: "Run without prompts and notify only on outcomes." },
    { id: "locked", label: "Locked", desc: "Keep the assistant read-only." },
  ];

  return (
    <div className="row-list">
      {levels.map((item) => (
        <button
          key={item.id}
          onClick={async () => {
            setSaving(item.id);
            try {
              await setPermissions(item.id);
              await onRefresh();
              onMessage(`Permission level set to ${item.label.toLowerCase()}.`);
            } finally {
              setSaving(null);
            }
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
              background: currentLevel === item.id ? "var(--accent-soft)" : "rgba(22, 48, 75, 0.06)",
              borderColor: "transparent",
              color: currentLevel === item.id ? "var(--accent)" : "var(--ink-soft)",
            }}
          >
            {saving === item.id ? "Saving..." : currentLevel === item.id ? "Current" : "Select"}
          </span>
        </button>
      ))}
    </div>
  );
}

function TelegramSection({
  telegram,
  onRefresh,
  onMessage,
}: {
  telegram: TelegramDiagnostics;
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const [botToken, setBotToken] = useState("");
  const [saving, setSaving] = useState(false);
  const [newUsername, setNewUsername] = useState("");
  const [pairingCode, setPairingCode] = useState("");
  const [generatingPairCode, setGeneratingPairCode] = useState(false);
  const usernames = telegram.allowed_usernames || [];

  const persistUsernames = async (updated: string[]) => {
    const cfg = await getSettings();
    const next = Array.from(new Set(updated.map((item) => item.trim().replace(/^@/, "")).filter(Boolean)));
    await updateSettings({
      ...cfg,
      telegram: {
        ...(cfg.telegram as Record<string, unknown> | undefined),
        allowed_usernames: next,
      },
    });
    await onRefresh();
  };

  return (
    <div className="settings-grid">
      <div>
        <div className="text-[14px] font-medium">
          {telegram.connected ? (telegram.bot_name ? `@${telegram.bot_name}` : "Bot connected") : "Not connected"}
        </div>
        <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
          {telegram.status_label}
          {telegram.allowed_usernames.length > 0 && ` · Allowed: ${telegram.allowed_usernames.map((item) => `@${item}`).join(", ")}`}
        </div>
        <div className="mt-1 text-[12px]" style={{ color: "var(--ink-soft)" }}>
          {telegram.paired_chat_ids.length > 0
            ? `${telegram.paired_chat_ids.length} paired chats: ${telegram.paired_chat_ids.join(", ")}`
            : "No Telegram chats have paired yet."}
        </div>
      </div>

      {!telegram.connected && (
        <>
          <input
            type="password"
            value={botToken}
            onChange={(event) => setBotToken(event.target.value)}
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
                    await onRefresh();
                  }
                } catch (error) {
                  console.error(error);
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

      {telegram.connected && (
        <div className="rounded-[16px] border p-3" style={{ borderColor: "var(--line)", background: "rgba(255,255,255,0.48)" }}>
          <div className="flex items-center justify-between gap-2">
            <div>
              <div className="text-[13px] font-medium">Pairing code</div>
              <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
                Allowed usernames auto-pair on first `/start`. Use a code only when you need to pair manually with `/pair CODE`.
              </div>
            </div>
            <button
              onClick={async () => {
                setGeneratingPairCode(true);
                try {
                  setPairingCode(await generatePairingCode());
                  onMessage("Generated a Telegram pairing code.");
                } finally {
                  setGeneratingPairCode(false);
                }
              }}
              disabled={generatingPairCode}
              className="secondary-cta disabled:opacity-40"
            >
              {generatingPairCode ? "Generating..." : "Generate"}
            </button>
          </div>
          {pairingCode && (
            <div className="mt-3">
              <span className="pill" style={{ fontFamily: 'ui-monospace, "SF Mono", "Menlo", monospace' }}>
                {pairingCode}
              </span>
            </div>
          )}
        </div>
      )}

      <div className="settings-grid">
        <div className="text-[12px] font-medium">Allowed usernames</div>
        <div className="flex gap-2">
          <input
            value={newUsername}
            onChange={(event) => setNewUsername(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && newUsername.trim()) {
                persistUsernames([...usernames, newUsername]);
                setNewUsername("");
              }
            }}
            placeholder="@username"
            className="field"
          />
          <button
            onClick={() => {
              if (!newUsername.trim()) return;
              persistUsernames([...usernames, newUsername]);
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
          <p className="section-meta">Telegram messages are ignored until at least one username is allowed.</p>
        )}
      </div>
    </div>
  );
}

function DaemonSection({
  daemon,
  onRefresh,
  onMessage,
}: {
  daemon: DaemonDiagnostics;
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const [loading, setLoading] = useState(false);

  return (
    <div className="row-list">
      <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
        <div>
          <div className="text-[14px] font-medium">Server mode</div>
          <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
            {daemon.server_mode === "on" ? "Starts on boot and restarts after crashes." : "Runs only while you keep it open."}
          </div>
        </div>
        <button
          onClick={async () => {
            setLoading(true);
            try {
              const mode = daemon.server_mode === "on" ? "off" : "on";
              const res = await setServerMode(mode);
              onMessage(res.message || "Daemon mode updated.");
              await onRefresh();
            } finally {
              setLoading(false);
            }
          }}
          disabled={loading}
          className={daemon.server_mode === "on" ? "cta" : "secondary-cta"}
        >
          {loading ? "Working..." : daemon.server_mode === "on" ? "Turn off" : "Turn on"}
        </button>
      </div>

      <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
        <div>
          <div className="text-[14px] font-medium">Launch agent</div>
          <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
            launchd is {daemon.launchd_installed ? "installed" : "not installed"}.
          </div>
        </div>
        <span
          className="pill"
          style={{
            background: daemon.launchd_installed ? "var(--success-bg)" : "rgba(22, 48, 75, 0.06)",
            borderColor: "transparent",
            color: daemon.launchd_installed ? "var(--success)" : "var(--ink-soft)",
          }}
        >
          {daemon.launchd_installed ? "Installed" : "Missing"}
        </span>
      </div>

      <div className="row-item grid-cols-[minmax(0,1fr)_auto]">
        <div>
          <div className="text-[14px] font-medium">Live activity</div>
          <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
            {daemon.current_run ?? "No run in progress"}
          </div>
        </div>
        <span className="pill">
          {daemon.running_sessions} session{daemon.running_sessions === 1 ? "" : "s"}
        </span>
      </div>
    </div>
  );
}

function IdentitySection({
  identity,
  files,
  onRefresh,
  onMessage,
}: {
  identity: IdentityDiagnostics;
  files: string[];
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const [botName, setBotName] = useState(identity.assistant_name || "Homard");
  const [botEmoji, setBotEmoji] = useState(identity.assistant_emoji || "🦞");
  const [botTagline, setBotTagline] = useState(identity.assistant_tagline || "Your personal crustacean");
  const [userName, setUserName] = useState(identity.user_name || "");
  const [userRole, setUserRole] = useState(identity.user_role || "");
  const [selected, setSelected] = useState(files[0] || "IDENTITY.md");
  const [content, setContent] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setBotName(identity.assistant_name || "Homard");
    setBotEmoji(identity.assistant_emoji || "🦞");
    setBotTagline(identity.assistant_tagline || "Your personal crustacean");
    setUserName(identity.user_name || "");
    setUserRole(identity.user_role || "");
  }, [
    identity.assistant_emoji,
    identity.assistant_name,
    identity.assistant_tagline,
    identity.user_name,
    identity.user_role,
  ]);

  useEffect(() => {
    if (!files.includes(selected) && files.length > 0) {
      setSelected(files[0]);
    }
  }, [files, selected]);

  useEffect(() => {
    if (!selected) return;
    readFile(selected).then(setContent).catch(() => setContent(""));
  }, [selected]);

  return (
    <div className="settings-grid">
      <div className="settings-grid md:grid-cols-2">
        <div className="settings-grid">
          <div className="text-[12px] font-medium">Assistant</div>
          <input value={botName} onChange={(event) => setBotName(event.target.value)} placeholder="Name" className="field" />
          <input value={botEmoji} onChange={(event) => setBotEmoji(event.target.value)} placeholder="Emoji" className="field" />
          <input value={botTagline} onChange={(event) => setBotTagline(event.target.value)} placeholder="Tagline" className="field" />
          <div className="flex justify-end">
            <button
              onClick={async () => {
                setSaving(true);
                try {
                  let text = await readFile("IDENTITY.md");
                  text = upsertField(text, "name", botName, { frontmatter: true });
                  text = upsertField(text, "emoji", botEmoji, { frontmatter: true });
                  text = upsertField(text, "tagline", botTagline, { frontmatter: true });
                  await writeFile("IDENTITY.md", text);
                  await onRefresh();
                  onMessage("Assistant identity saved.");
                } finally {
                  setSaving(false);
                }
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
          <input value={userName} onChange={(event) => setUserName(event.target.value)} placeholder="Your name" className="field" />
          <input value={userRole} onChange={(event) => setUserRole(event.target.value)} placeholder="Role or short descriptor" className="field" />
          <div className="flex justify-end">
            <button
              onClick={async () => {
                setSaving(true);
                try {
                  let text = await readFile("USER.md");
                  if (!text.trim()) {
                    text = "# User Profile\n";
                  }
                  text = upsertField(text, "Name", userName);
                  text = upsertField(text, "Role", userRole);
                  await writeFile("USER.md", text);
                  await onRefresh();
                  onMessage("User profile saved.");
                } finally {
                  setSaving(false);
                }
              }}
              disabled={saving}
              className="cta disabled:opacity-40"
            >
              {saving ? "Saving..." : "Save profile"}
            </button>
          </div>
        </div>
      </div>

      <details className="disclosure">
        <summary>Identity files</summary>
        <div className="disclosure__body">
          <div className="settings-grid">
            <select value={selected} onChange={(event) => setSelected(event.target.value)} className="field">
              {files.map((item) => (
                <option key={item} value={item}>{item}</option>
              ))}
            </select>
            <textarea
              value={content}
              onChange={(event) => setContent(event.target.value)}
              className="field is-mono resize-none"
              rows={14}
            />
            <div className="flex justify-end">
              <button
                onClick={async () => {
                  setSaving(true);
                  try {
                    await writeFile(selected, content);
                    await onRefresh();
                    onMessage(`${selected} saved.`);
                  } finally {
                    setSaving(false);
                  }
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
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [loadingSnapshot, setLoadingSnapshot] = useState(true);
  const [bannerMessage, setBannerMessage] = useState("");
  const [stoppingRun, setStoppingRun] = useState(false);

  const refreshStatus = async () => {
    const next = await getStatus();
    setStatus(next);
  };

  const refreshSnapshot = async () => {
    setLoadingSnapshot(true);
    try {
      const next = await getSettingsSnapshot();
      setSnapshot(next);
    } finally {
      setLoadingSnapshot(false);
    }
  };

  const refreshAll = async () => {
    try {
      await Promise.all([refreshStatus(), refreshSnapshot()]);
    } catch (error) {
      console.error(error);
      setBannerMessage(error instanceof Error ? error.message : "Failed to refresh settings.");
    }
  };

  useEffect(() => {
    refreshAll();
    const interval = setInterval(refreshStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const overview = snapshot?.overview;
  const currentRun = status?.current_run ?? overview?.current_run ?? null;

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div>
          <div className="subtle-label">Settings</div>
          <h2 className="section-title">System, providers, and files</h2>
          <p className="section-meta">Everything here is sourced from current daemon state, not placeholders.</p>
        </div>
        <button onClick={() => refreshAll()} className="secondary-cta">
          Refresh
        </button>
      </div>

      <div className="settings-stack">
        <section className="settings-summary">
          <div className="settings-summary__item">
            <div className="subtle-label">Provider</div>
            <div className="mt-1 text-[15px] font-semibold">
              {formatProviderTitle(status?.active_provider ?? overview?.active_provider)}
            </div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
              {status?.active_model ?? overview?.active_model ?? "No active model"}
            </div>
          </div>

          <div className="settings-summary__item">
            <div className="subtle-label">Overview</div>
            <div className="mt-1 text-[15px] font-semibold">
              {overview ? `${overview.ready_provider_count} ready` : "Loading..."}
            </div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
              {overview
                ? `${overview.configured_provider_count} configured · ${formatPermission(status?.permission_level ?? overview.permission_level)} permissions`
                : "Fetching live diagnostics"}
            </div>
          </div>

          <div className="settings-summary__item">
            <div className="subtle-label">Telegram</div>
            <div className="mt-1 text-[15px] font-semibold">
              {overview ? (overview.telegram_connected ? "Connected" : "Idle") : "Loading..."}
            </div>
            <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>
              {overview?.telegram_label ?? "Not connected"}
            </div>
          </div>

          <div className="settings-summary__item">
            <div className="subtle-label">Current run</div>
            <div className="mt-1 text-[15px] font-semibold">{currentRun ?? "None"}</div>
            <div className="mt-2">
              <button
                onClick={async () => {
                  setStoppingRun(true);
                  try {
                    await stopRun();
                    await refreshStatus();
                    setBannerMessage("Stop signal sent to the current run.");
                  } finally {
                    setStoppingRun(false);
                  }
                }}
                disabled={!currentRun || stoppingRun}
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

        {loadingSnapshot && !snapshot ? (
          <div className="settings-block">
            <div className="settings-block__body">
              <div className="text-[12px]" style={{ color: "var(--ink-soft)" }}>Loading live settings snapshot…</div>
            </div>
          </div>
        ) : (
          <>
            <Section
              label="Models"
              title="Providers"
              body="Provider rows report the actual local install/auth state and the MCPs visible to each CLI."
            >
              <div className="row-list">
                {providerOrder.map((name) => (
                  <ProviderRow
                    key={name}
                    name={name}
                    provider={snapshot?.providers[name]}
                    onRefresh={refreshAll}
                    onMessage={setBannerMessage}
                  />
                ))}
              </div>
            </Section>

            <Section
              label="Safety"
              title="Permissions"
              body="These levels are mutually exclusive. The current mode is sourced from the daemon."
            >
              <PermissionsSection
                currentLevel={status?.permission_level ?? overview?.permission_level}
                onRefresh={refreshAll}
                onMessage={setBannerMessage}
              />
            </Section>

            {snapshot && (
              <Section
                label="Bridge"
                title="Telegram"
                body="Bot status, paired chats, and the allowed usernames come from the live daemon config."
              >
                <TelegramSection telegram={snapshot.telegram} onRefresh={refreshAll} onMessage={setBannerMessage} />
              </Section>
            )}

            {snapshot && (
              <Section
                label="Daemon"
                title="Background service"
                body="This answers the practical question: will Homard stay alive when the window closes?"
              >
                <DaemonSection daemon={snapshot.daemon} onRefresh={refreshAll} onMessage={setBannerMessage} />
              </Section>
            )}

            {snapshot && (
              <Section
                label="Identity"
                title="Profile and source files"
                body="The form updates the actual identity files, and the raw editor is still available for direct edits."
              >
                <IdentitySection
                  identity={snapshot.identity}
                  files={snapshot.files}
                  onRefresh={refreshAll}
                  onMessage={setBannerMessage}
                />
              </Section>
            )}
          </>
        )}
      </div>
    </div>
  );
}

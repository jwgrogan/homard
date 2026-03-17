import { useEffect, useState } from "react";
import { useSettingsStore } from "../lib/store";
import PermissionsPanel from "../components/settings/PermissionsPanel";
import McpServersPanel from "../components/settings/McpServersPanel";
import EnvVarsPanel from "../components/settings/EnvVarsPanel";
import AgentsBrowser from "../components/settings/AgentsBrowser";
import ProfilesPanel from "../components/settings/ProfilesPanel";
import { TelegramPanel } from "../components/settings/TelegramPanel";

const TABS = ["permissions", "mcp", "env", "agents", "profiles", "telegram"] as const;
type Tab = (typeof TABS)[number];

const TAB_LABELS: Record<Tab, string> = {
  permissions: "Permissions",
  mcp: "MCP Servers",
  env: "Env Vars",
  agents: "Agents",
  profiles: "Profiles",
  telegram: "Telegram",
};

export default function Settings() {
  const [tab, setTab] = useState<Tab>("permissions");
  const { fetchSettings, loading } = useSettingsStore();

  useEffect(() => {
    fetchSettings();
  }, []);

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold">Settings</h1>
        {loading && <span className="text-xs text-zinc-500">Refreshing…</span>}
      </div>

      <div className="flex gap-1 border-b border-zinc-700 mb-6">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm rounded-t transition-colors ${
              tab === t
                ? "bg-zinc-700 text-zinc-100 border-b-2 border-blue-500"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800"
            }`}
          >
            {TAB_LABELS[t]}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto">
        {tab === "permissions" && <PermissionsPanel />}
        {tab === "mcp" && <McpServersPanel />}
        {tab === "env" && <EnvVarsPanel />}
        {tab === "agents" && <AgentsBrowser />}
        {tab === "profiles" && <ProfilesPanel />}
        {tab === "telegram" && <TelegramPanel />}
      </div>
    </div>
  );
}

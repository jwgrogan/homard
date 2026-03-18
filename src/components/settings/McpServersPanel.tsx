import { useState, useEffect } from "react";
import { useSettingsStore } from "../../lib/store";
import type { McpServerConfig } from "../../lib/types";
import * as api from "../../lib/tauri";

type McpType = "http" | "stdio";

function extractCloudServices(allowList: string[], enabledMcpjsonServers?: string[]): string[] {
  const services = new Set<string>();
  for (const pattern of allowList) {
    const match = pattern.match(/^mcp__claude_ai_([^_]+(?:_[^_]+)*?)__/);
    if (match) {
      // Convert snake_case back to display name (e.g. Google_Calendar -> Google Calendar)
      services.add(match[1].replace(/_/g, " "));
    }
  }
  if (enabledMcpjsonServers) {
    for (const srv of enabledMcpjsonServers) {
      services.add(srv);
    }
  }
  return Array.from(services).sort();
}

export default function McpServersPanel() {
  const { settings } = useSettingsStore();

  // --- Managed MCP state ---
  const [managedServers, setManagedServers] = useState<Record<string, McpServerConfig>>({});
  const [managedLoading, setManagedLoading] = useState(false);
  const [managedError, setManagedError] = useState<string | null>(null);

  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [type, setType] = useState<McpType>("http");
  const [url, setUrl] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  async function fetchManaged() {
    setManagedLoading(true);
    setManagedError(null);
    try {
      const result = await api.listManagedMcps();
      setManagedServers(result ?? {});
    } catch (e) {
      setManagedError(String(e));
    } finally {
      setManagedLoading(false);
    }
  }

  useEffect(() => {
    fetchManaged();
  }, []);

  async function handleAdd() {
    const trimmedName = name.trim();
    if (!trimmedName) { setFormError("Name is required"); return; }
    if (trimmedName in managedServers) { setFormError("A server with this name already exists"); return; }

    let config: McpServerConfig;
    if (type === "http") {
      const trimmedUrl = url.trim();
      if (!trimmedUrl) { setFormError("URL is required"); return; }
      config = { type: "http", url: trimmedUrl };
    } else {
      const trimmedCmd = command.trim();
      if (!trimmedCmd) { setFormError("Command is required"); return; }
      const parsedArgs = args.split(",").map((a) => a.trim()).filter(Boolean);
      config = { type: "stdio", command: trimmedCmd, args: parsedArgs };
    }

    setAdding(true);
    setFormError(null);
    try {
      await api.addManagedMcp(trimmedName, config);
      setName(""); setUrl(""); setCommand(""); setArgs("");
      setShowForm(false);
      await fetchManaged();
    } catch (e) {
      setFormError(String(e));
    } finally {
      setAdding(false);
    }
  }

  async function handleRemove(serverName: string) {
    try {
      await api.removeManagedMcp(serverName);
      await fetchManaged();
    } catch (e) {
      setManagedError(String(e));
    }
  }

  const managedEntries = Object.entries(managedServers);
  const enabledPlugins = settings?.enabledPlugins ?? null;
  const cloudServices = extractCloudServices(
    settings?.permissions?.allow ?? [],
    settings?.enabledMcpjsonServers,
  );

  return (
    <div className="space-y-8">
      {/* Managed MCP Servers */}
      <div>
        <h2 className="text-sm font-semibold text-zinc-300 mb-1">Managed MCP Servers</h2>
        <p className="text-xs text-zinc-500 mb-3">
          Managed by arcctl — synced automatically to Claude's settings
        </p>

        {managedError && (
          <p className="text-xs text-red-400 mb-2">{managedError}</p>
        )}

        <div className="space-y-3 mb-4">
          {managedLoading ? (
            <p className="text-sm text-zinc-500">Loading…</p>
          ) : managedEntries.length === 0 ? (
            <p className="text-sm text-zinc-500">No managed MCP servers configured.</p>
          ) : (
            managedEntries.map(([serverName, cfg]) => {
              const isHttp = cfg.type === "http" || !!cfg.url;
              return (
                <div
                  key={serverName}
                  className="bg-zinc-800 border border-zinc-700 rounded p-3 flex items-start justify-between"
                >
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-zinc-100">{serverName}</span>
                      <span
                        className={`rounded-full px-2 py-0.5 text-xs ${
                          isHttp ? "bg-blue-900 text-blue-300" : "bg-purple-900 text-purple-300"
                        }`}
                      >
                        {isHttp ? "HTTP" : "stdio"}
                      </span>
                    </div>
                    {isHttp ? (
                      <p className="text-xs text-zinc-400 font-mono">{cfg.url}</p>
                    ) : (
                      <p className="text-xs text-zinc-400 font-mono">
                        {cfg.command}
                        {cfg.args && cfg.args.length > 0 ? " " + cfg.args.join(" ") : ""}
                      </p>
                    )}
                  </div>
                  <button
                    onClick={() => handleRemove(serverName)}
                    className="text-zinc-500 hover:text-red-400 text-xs px-2 py-0.5 rounded ml-4 shrink-0"
                  >
                    Remove
                  </button>
                </div>
              );
            })
          )}
        </div>

        {!showForm ? (
          <button
            onClick={() => setShowForm(true)}
            className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500"
          >
            + Add Server
          </button>
        ) : (
          <div className="bg-zinc-800 border border-zinc-700 rounded p-4 space-y-3">
            <h3 className="text-sm font-medium text-zinc-200">New MCP Server</h3>

            <div>
              <label className="block text-xs text-zinc-400 mb-1">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => { setName(e.target.value); setFormError(null); }}
                placeholder="my-server"
                className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
              />
            </div>

            <div>
              <label className="block text-xs text-zinc-400 mb-1">Type</label>
              <div className="flex gap-2">
                {(["http", "stdio"] as McpType[]).map((t) => (
                  <button
                    key={t}
                    onClick={() => setType(t)}
                    className={`px-3 py-1.5 rounded text-sm uppercase ${
                      type === t ? "bg-zinc-600 text-zinc-100" : "bg-zinc-700 text-zinc-400 hover:bg-zinc-600"
                    }`}
                  >
                    {t}
                  </button>
                ))}
              </div>
            </div>

            {type === "http" ? (
              <div>
                <label className="block text-xs text-zinc-400 mb-1">URL</label>
                <input
                  type="text"
                  value={url}
                  onChange={(e) => { setUrl(e.target.value); setFormError(null); }}
                  placeholder="https://example.com/mcp"
                  className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                />
              </div>
            ) : (
              <>
                <div>
                  <label className="block text-xs text-zinc-400 mb-1">Command</label>
                  <input
                    type="text"
                    value={command}
                    onChange={(e) => { setCommand(e.target.value); setFormError(null); }}
                    placeholder="npx"
                    className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                  />
                </div>
                <div>
                  <label className="block text-xs text-zinc-400 mb-1">Args (comma-separated)</label>
                  <input
                    type="text"
                    value={args}
                    onChange={(e) => { setArgs(e.target.value); setFormError(null); }}
                    placeholder="-y, @modelcontextprotocol/server-filesystem"
                    className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                  />
                </div>
              </>
            )}

            {formError && <p className="text-xs text-red-400">{formError}</p>}

            <div className="flex gap-2">
              <button
                onClick={handleAdd}
                disabled={adding}
                className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
              >
                {adding ? "Adding…" : "Add"}
              </button>
              <button
                onClick={() => { setShowForm(false); setFormError(null); }}
                className="px-3 py-1.5 rounded text-sm bg-zinc-700 hover:bg-zinc-600"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Enabled Plugins */}
      {enabledPlugins !== null && (
        <div>
          <h2 className="text-sm font-semibold text-zinc-300 mb-1">Enabled Plugins</h2>
          <p className="text-xs text-zinc-500 mb-3">Managed via Claude Code CLI</p>
          {Object.keys(enabledPlugins).length === 0 ? (
            <p className="text-sm text-zinc-500">No plugins configured.</p>
          ) : (
            <div className="space-y-2">
              {Object.entries(enabledPlugins).map(([pluginName, enabled]) => (
                <div
                  key={pluginName}
                  className="bg-zinc-800 border border-zinc-700 rounded px-3 py-2 flex items-center justify-between"
                >
                  <span className="text-sm text-zinc-200">{pluginName}</span>
                  <span
                    className={`rounded-full px-2 py-0.5 text-xs ${
                      enabled ? "bg-green-900 text-green-300" : "bg-zinc-700 text-zinc-400"
                    }`}
                  >
                    {enabled ? "enabled" : "disabled"}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Cloud-Connected Services */}
      {cloudServices.length > 0 && (
        <div>
          <h2 className="text-sm font-semibold text-zinc-300 mb-1">Cloud-Connected Services</h2>
          <p className="text-xs text-zinc-500 mb-3">Connected via claude.ai — read-only</p>
          <div className="flex flex-wrap gap-2">
            {cloudServices.map((service) => (
              <span
                key={service}
                className="rounded-full px-3 py-1 text-xs bg-indigo-900 text-indigo-300 border border-indigo-700"
              >
                {service}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

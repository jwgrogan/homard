import { useState } from "react";
import { useSettingsStore } from "../../lib/store";
import type { McpServerConfig } from "../../lib/types";

type McpType = "http" | "stdio";

export default function McpServersPanel() {
  const { settings, addMcpServer, removeMcpServer } = useSettingsStore();
  const servers = settings?.mcpServers ?? {};

  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [type, setType] = useState<McpType>("http");
  const [url, setUrl] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  async function handleAdd() {
    const trimmedName = name.trim();
    if (!trimmedName) { setError("Name is required"); return; }
    if (trimmedName in servers) { setError("A server with this name already exists"); return; }

    let config: McpServerConfig;
    if (type === "http") {
      const trimmedUrl = url.trim();
      if (!trimmedUrl) { setError("URL is required"); return; }
      config = { type: "http", url: trimmedUrl };
    } else {
      const trimmedCmd = command.trim();
      if (!trimmedCmd) { setError("Command is required"); return; }
      const parsedArgs = args.split(",").map((a) => a.trim()).filter(Boolean);
      config = { type: "stdio", command: trimmedCmd, args: parsedArgs };
    }

    setAdding(true);
    setError(null);
    try {
      await addMcpServer(trimmedName, config);
      setName(""); setUrl(""); setCommand(""); setArgs("");
      setShowForm(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setAdding(false);
    }
  }

  const entries = Object.entries(servers);

  return (
    <div>
      <div className="space-y-3 mb-4">
        {entries.length === 0 ? (
          <p className="text-sm text-zinc-500">No MCP servers configured.</p>
        ) : (
          entries.map(([serverName, cfg]) => {
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
                  onClick={() => removeMcpServer(serverName)}
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
              onChange={(e) => { setName(e.target.value); setError(null); }}
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
                onChange={(e) => { setUrl(e.target.value); setError(null); }}
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
                  onChange={(e) => { setCommand(e.target.value); setError(null); }}
                  placeholder="npx"
                  className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                />
              </div>
              <div>
                <label className="block text-xs text-zinc-400 mb-1">Args (comma-separated)</label>
                <input
                  type="text"
                  value={args}
                  onChange={(e) => { setArgs(e.target.value); setError(null); }}
                  placeholder="-y, @modelcontextprotocol/server-filesystem"
                  className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                />
              </div>
            </>
          )}

          {error && <p className="text-xs text-red-400">{error}</p>}

          <div className="flex gap-2">
            <button
              onClick={handleAdd}
              disabled={adding}
              className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
            >
              {adding ? "Adding…" : "Add"}
            </button>
            <button
              onClick={() => { setShowForm(false); setError(null); }}
              className="px-3 py-1.5 rounded text-sm bg-zinc-700 hover:bg-zinc-600"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

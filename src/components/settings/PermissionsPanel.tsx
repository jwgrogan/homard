import { useState } from "react";
import { useSettingsStore } from "../../lib/store";

function PermissionList({
  title,
  list,
  patterns,
  onAdd,
  onRemove,
}: {
  title: string;
  list: "allow" | "deny";
  patterns: string[];
  onAdd: (list: "allow" | "deny", pattern: string) => Promise<void>;
  onRemove: (list: "allow" | "deny", pattern: string) => Promise<void>;
}) {
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  async function handleAdd() {
    const trimmed = input.trim();
    if (!trimmed) {
      setError("Pattern cannot be empty");
      return;
    }
    if (patterns.includes(trimmed)) {
      setError("Pattern already exists");
      return;
    }
    setAdding(true);
    setError(null);
    try {
      await onAdd(list, trimmed);
      setInput("");
    } catch (e) {
      setError(String(e));
    } finally {
      setAdding(false);
    }
  }

  return (
    <div className="mb-6">
      <h3 className="text-sm font-medium text-zinc-300 mb-2">{title}</h3>
      <div className="bg-zinc-800 rounded border border-zinc-700">
        {patterns.length === 0 ? (
          <p className="px-3 py-2 text-sm text-zinc-500">No patterns configured</p>
        ) : (
          <ul className="divide-y divide-zinc-700">
            {patterns.map((p) => (
              <li key={p} className="flex items-center justify-between px-3 py-2">
                <span className="text-sm font-mono text-zinc-200">{p}</span>
                <button
                  onClick={() => onRemove(list, p)}
                  className="text-zinc-500 hover:text-red-400 text-xs px-2 py-0.5 rounded"
                  title="Remove"
                >
                  ✕
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="border-t border-zinc-700 px-3 py-2 flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => { setInput(e.target.value); setError(null); }}
            onKeyDown={(e) => e.key === "Enter" && handleAdd()}
            placeholder="Add pattern…"
            className="flex-1 bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-1.5 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
          />
          <button
            onClick={handleAdd}
            disabled={adding}
            className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
          >
            Add
          </button>
        </div>
        {error && <p className="px-3 pb-2 text-xs text-red-400">{error}</p>}
      </div>
    </div>
  );
}

export default function PermissionsPanel() {
  const { settings, addPermission, removePermission, setDefaultMode } = useSettingsStore();
  const perms = settings?.permissions;
  const isBypassed = settings?.defaultMode === "bypassPermissions";

  if (!perms) return <p className="text-zinc-400 text-sm">Loading…</p>;

  return (
    <div>
      <div className="mb-6 p-4 bg-zinc-800 rounded border border-zinc-700">
        <div className="flex items-center justify-between">
          <div>
            <span className="text-sm font-medium text-zinc-100">Bypass Permissions</span>
            <p className="text-xs text-zinc-400 mt-0.5">
              Allow all tool calls without permission checks. Use with caution.
            </p>
          </div>
          <button
            onClick={() => setDefaultMode(isBypassed ? null : "bypassPermissions")}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              isBypassed ? "bg-red-600" : "bg-zinc-600"
            }`}
          >
            <span
              className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                isBypassed ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
        </div>
        {isBypassed && (
          <p className="mt-2 text-xs text-red-400">
            Warning: All permission checks are disabled. Claude can execute any tool call without confirmation.
          </p>
        )}
      </div>

      <PermissionList
        title="Allowlist"
        list="allow"
        patterns={perms.allow}
        onAdd={addPermission}
        onRemove={removePermission}
      />

      <PermissionList
        title="Denylist"
        list="deny"
        patterns={perms.deny}
        onAdd={addPermission}
        onRemove={removePermission}
      />
    </div>
  );
}

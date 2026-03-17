import { useEffect, useState } from "react";
import { getAgents, getCommands } from "../../lib/tauri";
import type { AgentInfo, CommandInfo } from "../../lib/types";

function ScopeBadge({ scope }: { scope: "global" | "project" }) {
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-xs ${
        scope === "global" ? "bg-green-900 text-green-300" : "bg-blue-900 text-blue-300"
      }`}
    >
      {scope === "global" ? "Global" : "Project"}
    </span>
  );
}

function CollapsibleSection({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(true);

  return (
    <div className="mb-4">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-2 w-full text-left py-2 group"
      >
        <span className="text-sm font-medium text-zinc-200">{title}</span>
        <span className="text-xs text-zinc-500 bg-zinc-800 px-2 py-0.5 rounded-full">{count}</span>
        <span className="ml-auto text-zinc-500 group-hover:text-zinc-300 text-xs">
          {open ? "▲" : "▼"}
        </span>
      </button>
      {open && <div className="space-y-2">{children}</div>}
    </div>
  );
}

export default function AgentsBrowser() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [commands, setCommands] = useState<CommandInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    Promise.all([getAgents(), getCommands()])
      .then(([a, c]) => {
        if (!cancelled) {
          setAgents(a);
          setCommands(c);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => { cancelled = true; };
  }, []);

  if (loading) return <p className="text-sm text-zinc-400">Loading…</p>;
  if (error) return <p className="text-sm text-red-400">Error: {error}</p>;

  return (
    <div>
      <CollapsibleSection title="Agents" count={agents.length}>
        {agents.length === 0 ? (
          <p className="text-sm text-zinc-500 py-2">No agents found.</p>
        ) : (
          agents.map((agent) => (
            <div
              key={agent.path}
              className="bg-zinc-800 border border-zinc-700 rounded p-3 space-y-1"
            >
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-zinc-100">{agent.name}</span>
                <ScopeBadge scope={agent.scope} />
              </div>
              {agent.description && (
                <p className="text-xs text-zinc-400">{agent.description}</p>
              )}
              <div className="flex gap-3 text-xs text-zinc-500">
                {agent.model && <span>Model: <span className="text-zinc-400">{agent.model}</span></span>}
                {agent.tools.length > 0 && (
                  <span>
                    Tools: <span className="text-zinc-400">{agent.tools.length}</span>
                  </span>
                )}
              </div>
            </div>
          ))
        )}
      </CollapsibleSection>

      <CollapsibleSection title="Commands" count={commands.length}>
        {commands.length === 0 ? (
          <p className="text-sm text-zinc-500 py-2">No commands found.</p>
        ) : (
          commands.map((cmd) => (
            <div
              key={cmd.path}
              className="bg-zinc-800 border border-zinc-700 rounded p-3 space-y-1"
            >
              <div className="flex items-center gap-2">
                <span className="text-sm font-mono text-zinc-100">{cmd.name}</span>
                <ScopeBadge scope={cmd.scope} />
              </div>
              {cmd.description && (
                <p className="text-xs text-zinc-400">{cmd.description}</p>
              )}
            </div>
          ))
        )}
      </CollapsibleSection>
    </div>
  );
}

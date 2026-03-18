import type { AgentNode } from "../lib/types";

const STATUS_STYLES = {
  working: "bg-blue-500 animate-pulse",
  waiting: "bg-yellow-500",
  done: "bg-green-500",
  error: "bg-red-500",
};

function AgentNodeView({ node, depth = 0 }: { node: AgentNode; depth?: number }) {
  return (
    <div style={{ paddingLeft: `${depth * 20}px` }}>
      <div className="flex items-center gap-2 py-1">
        <span className={`w-2 h-2 rounded-full shrink-0 ${STATUS_STYLES[node.status]}`} />
        <span className="text-sm text-zinc-200 font-medium">{node.name}</span>
        {node.agent_type && (
          <span className="text-xs px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-400">
            {node.agent_type}
          </span>
        )}
        {node.files_touched.length > 0 && (
          <span className="text-xs text-zinc-500">{node.files_touched.length} files</span>
        )}
      </div>
      {node.current_activity && (
        <div style={{ paddingLeft: `${depth * 20 + 16}px` }} className="text-xs text-zinc-500 truncate">
          {node.current_activity}
        </div>
      )}
      {node.children.map((child) => (
        <AgentNodeView key={child.id} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}

export default function AgentTree({ root }: { root: AgentNode }) {
  return (
    <div className="bg-zinc-800/50 rounded-lg p-3">
      <AgentNodeView node={root} />
    </div>
  );
}

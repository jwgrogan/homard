import { useEffect, useState } from "react";
import { getActivity, type AgentRun } from "../lib/api";

function StatusDot({ status }: { status: string }) {
  const color =
    status === "running" ? "var(--coral)" :
    status === "complete" ? "#4CAF50" :
    status === "error" ? "#E53935" :
    "var(--navy-muted)";

  return (
    <span
      className={`inline-block w-2 h-2 rounded-full ${status === "running" ? "animate-pulse" : ""}`}
      style={{ background: color }}
    />
  );
}

function formatDuration(ms?: number): string {
  if (!ms) return "\u2014";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${s % 60}s`;
}

function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export default function Activity() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);

  useEffect(() => {
    getActivity().then(setRuns);
    const interval = setInterval(() => getActivity().then(setRuns), 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex flex-col h-full">
      <div
        className="px-4 py-3 border-b"
        style={{ borderColor: "var(--border)", background: "var(--sage)" }}
      >
        <span className="font-semibold text-sm" style={{ color: "var(--navy)" }}>
          Activity
        </span>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {runs.length === 0 ? (
          <p className="text-sm text-center mt-8" style={{ color: "var(--navy-muted)" }}>
            No activity yet
          </p>
        ) : (
          <div className="flex flex-col gap-2">
            {runs.map((run) => (
              <button
                key={run.id}
                onClick={() => setExpanded(expanded === run.id ? null : run.id)}
                className="w-full text-left px-3 py-2.5 rounded-xl transition-colors"
                style={{
                  background: "var(--cream-card)",
                  border: "1px solid var(--border)",
                }}
              >
                <div className="flex items-center gap-2">
                  <StatusDot status={run.status} />
                  <span className="text-sm font-medium flex-1 truncate" style={{ color: "var(--navy)" }}>
                    {run.channel}
                  </span>
                  <span className="text-xs" style={{ color: "var(--navy-muted)" }}>
                    {formatDuration(run.duration_ms)}
                  </span>
                  <span className="text-xs" style={{ color: "var(--navy-muted)" }}>
                    {formatTime(run.started_at)}
                  </span>
                </div>
                {expanded === run.id && (
                  <div className="mt-2 pt-2 text-xs" style={{ borderTop: "1px solid var(--border)", color: "var(--navy-muted)" }}>
                    <div>Trigger: {run.trigger}</div>
                    <div>Iterations: {run.iterations}</div>
                    {run.error_message && (
                      <div className="mt-1" style={{ color: "#E53935" }}>Error: {run.error_message}</div>
                    )}
                  </div>
                )}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { getActivity, getSessions, getCronHealth, killSession, type AgentRun, type CliSession, type CronHealthEntry } from "../lib/api";

function StatusDot({ status }: { status: string }) {
  const color =
    status === "running" ? "var(--coral)" :
    status === "complete" ? "var(--success)" :
    status === "error" ? "var(--error)" :
    "var(--navy-muted)";

  return (
    <span
      className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${status === "running" ? "animate-pulse" : ""}`}
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

function CronBar({ entries }: { entries: CronHealthEntry[] }) {
  return (
    <div
      className="flex items-center gap-3 px-4 py-1.5"
      style={{ borderBottom: "0.5px solid var(--border)", background: "rgba(232, 240, 236, 0.3)" }}
    >
      {entries.map(entry => {
        const rate = entry.total_runs > 0 ? Math.round((entry.successes / entry.total_runs) * 100) : 0;
        return (
          <div key={entry.name} className="flex items-center gap-1.5">
            <span className="text-[11px] font-medium" style={{ color: "var(--navy)" }}>{entry.name}</span>
            <span
              className="text-[10px] font-medium px-1 py-px rounded"
              style={{
                background: rate >= 90 ? "var(--success-bg)" : rate >= 50 ? "var(--warning-bg)" : "var(--error-bg)",
                color: rate >= 90 ? "var(--success)" : rate >= 50 ? "var(--warning)" : "var(--error)",
              }}
            >
              {rate}%
            </span>
            <span className="text-[10px]" style={{ color: "var(--navy-muted)" }}>
              {entry.total_runs} runs
            </span>
          </div>
        );
      })}
    </div>
  );
}

export default function Activity() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [sessions, setSessions] = useState<CliSession[]>([]);
  const [cronHealth, setCronHealth] = useState<CronHealthEntry[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);

  useEffect(() => {
    getActivity().then(setRuns);
    getSessions().then(setSessions);
    getCronHealth().then(setCronHealth);
    const interval = setInterval(() => {
      getActivity().then(setRuns);
      getSessions().then(setSessions);
      getCronHealth().then(setCronHealth);
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  const runningSessions = sessions.filter(s => s.status === "running");

  // Merge sessions and runs into a single flat list
  const allItems: Array<{ type: "session"; data: CliSession } | { type: "run"; data: AgentRun }> = [
    ...runningSessions.map(s => ({ type: "session" as const, data: s })),
    ...runs.map(r => ({ type: "run" as const, data: r })),
  ];

  return (
    <div className="flex flex-col h-full">
      {/* Cron health bar */}
      {cronHealth.length > 0 && <CronBar entries={cronHealth} />}

      {/* Flat list */}
      <div className="flex-1 overflow-y-auto">
        {allItems.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <p className="text-[13px]" style={{ color: "var(--navy-muted)" }}>No activity yet</p>
          </div>
        ) : (
          allItems.map(item => {
            if (item.type === "session") {
              const session = item.data;
              return (
                <div
                  key={`session-${session.id}`}
                  className="flex items-center gap-2 px-4 py-2"
                  style={{ borderBottom: "0.5px solid var(--border)" }}
                >
                  <StatusDot status={session.status} />
                  <span
                    className="text-[10px] font-medium px-1 py-px rounded shrink-0"
                    style={{ background: "var(--sage)", color: "var(--navy)" }}
                  >
                    {session.cli}
                  </span>
                  <span className="text-[13px] flex-1 truncate" style={{ color: "var(--navy)" }}>
                    {session.prompt.length > 50 ? session.prompt.slice(0, 50) + "..." : session.prompt}
                  </span>
                  <span className="text-[11px] shrink-0" style={{ color: "var(--navy-muted)" }}>
                    {session.directory.split("/").pop()}
                  </span>
                  {session.status === "running" && (
                    <button
                      onClick={() => killSession(session.id)}
                      className="text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0"
                      style={{ background: "var(--coral)", color: "white" }}
                    >
                      Kill
                    </button>
                  )}
                </div>
              );
            }

            const run = item.data;
            return (
              <button
                key={`run-${run.id}`}
                onClick={() => setExpanded(expanded === run.id ? null : run.id)}
                className="w-full text-left"
              >
                <div
                  className="flex items-center gap-2 px-4 py-2"
                  style={{ borderBottom: "0.5px solid var(--border)" }}
                >
                  <StatusDot status={run.status} />
                  <span className="text-[13px] font-medium flex-1 truncate" style={{ color: "var(--navy)" }}>
                    {run.channel}
                  </span>
                  <span className="text-[11px] shrink-0" style={{ color: "var(--navy-muted)" }}>
                    {formatDuration(run.duration_ms)}
                  </span>
                  <span className="text-[11px] shrink-0" style={{ color: "var(--navy-muted)" }}>
                    {formatTime(run.started_at)}
                  </span>
                </div>
                {expanded === run.id && (
                  <div
                    className="px-4 py-1.5 text-[11px]"
                    style={{ borderBottom: "0.5px solid var(--border)", background: "rgba(232, 240, 236, 0.25)", color: "var(--navy-muted)" }}
                  >
                    <span>Trigger: {run.trigger}</span>
                    <span className="mx-2">|</span>
                    <span>Iterations: {run.iterations}</span>
                    {run.error_message && (
                      <span style={{ color: "var(--error)" }}> | Error: {run.error_message}</span>
                    )}
                  </div>
                )}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}

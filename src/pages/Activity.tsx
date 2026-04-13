import { useEffect, useState } from "react";
import { getActivity, getSessions, getCronHealth, killSession, type AgentRun, type CliSession, type CronHealthEntry } from "../lib/api";

function StatusDot({ status }: { status: string }) {
  const color =
    status === "running" ? "var(--accent)" :
    status === "complete" ? "var(--success)" :
    status === "error" ? "var(--error)" :
    "var(--ink-soft)";

  return <span className={`inline-block h-2 w-2 rounded-full ${status === "running" ? "animate-pulse" : ""}`} style={{ background: color }} />;
}

function formatDuration(ms?: number): string {
  if (!ms) return "—";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${s % 60}s`;
}

function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function CronStrip({ entries }: { entries: CronHealthEntry[] }) {
  if (entries.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-2 px-4 pt-3">
      {entries.map((entry) => {
        const rate = entry.total_runs > 0 ? Math.round((entry.successes / entry.total_runs) * 100) : 0;
        const color =
          rate >= 90 ? { bg: "var(--success-bg)", text: "var(--success)" } :
          rate >= 50 ? { bg: "var(--warning-bg)", text: "var(--warning)" } :
          { bg: "var(--error-bg)", text: "var(--error)" };

        return (
          <span key={entry.name} className="pill" style={{ background: color.bg, color: color.text, borderColor: "transparent" }}>
            <span>{entry.name}</span>
            <span>{rate}%</span>
          </span>
        );
      })}
    </div>
  );
}

export default function Activity() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [sessions, setSessions] = useState<CliSession[]>([]);
  const [cronHealth, setCronHealth] = useState<CronHealthEntry[]>([]);

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

  const runningSessions = sessions.filter((s) => s.status === "running");
  const items: Array<{ type: "session"; data: CliSession } | { type: "run"; data: AgentRun }> = [
    ...runningSessions.map((s) => ({ type: "session" as const, data: s })),
    ...runs.map((r) => ({ type: "run" as const, data: r })),
  ];

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div>
          <div className="subtle-label">Activity</div>
          <h2 className="section-title">Runs and sessions</h2>
          <p className="section-meta">A compact ledger of what Homard is doing now and what just happened.</p>
        </div>
        <span className="pill">
          <span>{items.length}</span>
          <span>active or recent</span>
        </span>
      </div>

      <CronStrip entries={cronHealth} />

      <div className="row-list flex-1 overflow-y-auto">
        {items.length === 0 ? (
          <div className="empty-state">
            <p>No activity yet.</p>
          </div>
        ) : (
          items.map((item) => {
            if (item.type === "session") {
              const session = item.data;
              return (
                <div key={`session-${session.id}`} className="row-item grid-cols-[auto_minmax(0,1fr)_auto]">
                  <StatusDot status={session.status} />
                  <div className="min-w-0">
                    <div className="truncate text-[13px] font-medium">{session.prompt}</div>
                    <div className="truncate text-[11px]" style={{ color: "var(--ink-soft)" }}>
                      {String(session.cli).toUpperCase()} · {session.directory}
                    </div>
                  </div>
                  <button onClick={() => killSession(session.id)} className="danger-cta">
                    Kill
                  </button>
                </div>
              );
            }

            const run = item.data;
            return (
              <div key={`run-${run.id}`} className="row-item grid-cols-[auto_minmax(0,1fr)_auto]">
                <StatusDot status={run.status} />
                <div className="min-w-0">
                  <div className="truncate text-[13px] font-medium">{run.channel}</div>
                  <div className="truncate text-[11px]" style={{ color: "var(--ink-soft)" }}>
                    {run.trigger} · {formatDuration(run.duration_ms)} · {run.error_message ? run.error_message : `${run.iterations} iterations`}
                  </div>
                </div>
                <div className="text-[11px]" style={{ color: "var(--ink-soft)" }}>
                  {formatTime(run.started_at)}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

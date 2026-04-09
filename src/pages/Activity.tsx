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

function SessionCard({ session }: { session: CliSession }) {
  const handleKill = async () => {
    await killSession(session.id);
  };

  return (
    <div
      className="px-3 py-2.5 rounded-xl"
      style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
    >
      <div className="flex items-center gap-2">
        <StatusDot status={session.status} />
        <span
          className="text-xs px-1.5 py-0.5 rounded font-medium"
          style={{ background: "var(--sage)", color: "var(--navy)" }}
        >
          {session.cli}
        </span>
        <span className="text-sm flex-1 truncate" style={{ color: "var(--navy)" }}>
          {session.prompt.length > 60 ? session.prompt.slice(0, 60) + "..." : session.prompt}
        </span>
        {session.status === "running" && (
          <button
            onClick={handleKill}
            className="text-xs px-2 py-0.5 rounded"
            style={{ background: "var(--coral)", color: "white" }}
          >
            Kill
          </button>
        )}
      </div>
      <div className="text-xs mt-1 truncate" style={{ color: "var(--navy-muted)" }}>
        {session.directory}
      </div>
    </div>
  );
}

function CronHealthCard({ entry }: { entry: CronHealthEntry }) {
  const rate = entry.total_runs > 0 ? Math.round((entry.successes / entry.total_runs) * 100) : 0;
  const avgSecs = entry.avg_duration_ms ? Math.round(entry.avg_duration_ms / 1000) : 0;

  return (
    <div
      className="px-3 py-2.5 rounded-xl"
      style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
    >
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium" style={{ color: "var(--navy)" }}>{entry.name}</span>
        <span
          className="text-xs font-medium px-1.5 py-0.5 rounded"
          style={{
            background: rate >= 90 ? "var(--success-bg)" : rate >= 50 ? "var(--warning-bg)" : "var(--error-bg)",
            color: rate >= 90 ? "var(--success)" : rate >= 50 ? "var(--warning)" : "var(--error)",
          }}
        >
          {rate}% success
        </span>
      </div>
      <div className="flex gap-3 mt-1 text-xs" style={{ color: "var(--navy-muted)" }}>
        <span>{entry.total_runs} runs</span>
        <span>{entry.successes} ok</span>
        <span>{entry.failures} fail</span>
        {avgSecs > 0 && <span>avg {avgSecs}s</span>}
        {entry.last_run && <span>last: {new Date(entry.last_run).toLocaleString()}</span>}
      </div>
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

  const runningSessions = sessions.filter((s) => s.status === "running");

  return (
    <div className="flex-1 overflow-y-auto px-4 py-3">
      {/* Running Sessions */}
      {runningSessions.length > 0 && (
        <div className="mb-4">
          <h3 className="text-xs font-semibold uppercase tracking-wide mb-2" style={{ color: "var(--navy-muted)" }}>
            Running Sessions
          </h3>
          <div className="flex flex-col gap-2">
            {runningSessions.map((session) => (
              <SessionCard key={session.id} session={session} />
            ))}
          </div>
        </div>
      )}

      {/* Cron Health */}
      {cronHealth.length > 0 && (
        <div className="mb-4">
          <h3 className="text-xs font-semibold uppercase tracking-wide mb-2" style={{ color: "var(--navy-muted)" }}>
            Cron Health
          </h3>
          <div className="flex flex-col gap-2">
            {cronHealth.map((entry) => (
              <CronHealthCard key={entry.name} entry={entry} />
            ))}
          </div>
        </div>
      )}

      {/* Recent Runs */}
      <div>
        <h3 className="text-xs font-semibold uppercase tracking-wide mb-2" style={{ color: "var(--navy-muted)" }}>
          Recent Runs
        </h3>
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
                      <div className="mt-1" style={{ color: "var(--error)" }}>Error: {run.error_message}</div>
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

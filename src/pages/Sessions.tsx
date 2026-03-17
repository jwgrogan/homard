import { useEffect, useState } from "react";
import { useSessionsStore } from "../lib/store";
import type { Run, SessionInfo } from "../lib/types";

function formatDuration(startedAt: string): string {
  const start = new Date(startedAt).getTime();
  const now = Date.now();
  const seconds = Math.floor((now - start) / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

function formatDurationMs(ms: number | null): string {
  if (ms == null) return "—";
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString();
}

function StatusIcon({ status }: { status: Run["status"] }) {
  if (status === "complete") {
    return <span className="inline-block w-3 h-3 rounded-full bg-green-500" title="Complete" />;
  }
  if (status === "error") {
    return <span className="inline-block w-3 h-3 rounded-full bg-red-500" title="Error" />;
  }
  if (status === "killed") {
    return <span className="inline-block w-3 h-3 rounded-full bg-yellow-500" title="Killed" />;
  }
  // running
  return (
    <span
      className="inline-block w-3 h-3 rounded-full bg-blue-500 animate-pulse"
      title="Running"
    />
  );
}

const TRIGGER_COLORS: Record<Run["trigger"], string> = {
  manual: "bg-zinc-600 text-zinc-200",
  cron: "bg-blue-700 text-blue-100",
  telegram: "bg-purple-700 text-purple-100",
  email: "bg-green-700 text-green-100",
};

function TriggerBadge({ trigger }: { trigger: Run["trigger"] }) {
  return (
    <span
      className={`px-2 py-0.5 rounded-full text-xs font-medium ${TRIGGER_COLORS[trigger]}`}
    >
      {trigger}
    </span>
  );
}

function LiveSessionRow({ session }: { session: SessionInfo }) {
  const [, setTick] = useState(0);
  const { killSession } = useSessionsStore();

  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex items-center gap-4 p-3 rounded-lg bg-zinc-800 border border-zinc-700">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm text-zinc-300">{session.id.slice(0, 8)}</span>
          <span className="text-xs text-zinc-400">{session.agent ?? "ad-hoc"}</span>
        </div>
        <div className="flex items-center gap-3 mt-1 text-xs text-zinc-500">
          <span>Profile: {session.profile ?? "default"}</span>
          <span className="truncate max-w-xs" title={session.directory}>
            {session.directory}
          </span>
          <span className="text-zinc-400 font-medium">
            {formatDuration(session.started_at)}
          </span>
        </div>
      </div>
      <button
        onClick={() => killSession(session.id)}
        className="px-3 py-1 text-xs rounded bg-red-700 hover:bg-red-600 text-red-100 transition-colors shrink-0"
      >
        Kill
      </button>
    </div>
  );
}

const PAGE_SIZE = 20;

export default function Sessions() {
  const { liveSessions, runs, runsLoading, fetchLiveSessions, fetchRuns } = useSessionsStore();
  const [runsOffset, setRunsOffset] = useState(0);

  // Poll live sessions every 5 seconds
  useEffect(() => {
    fetchLiveSessions();
    const interval = setInterval(() => fetchLiveSessions(), 5000);
    return () => clearInterval(interval);
  }, []);

  // Fetch runs on mount and when offset changes
  useEffect(() => {
    fetchRuns(PAGE_SIZE, runsOffset);
  }, [runsOffset]);

  return (
    <div className="h-full flex flex-col gap-8 overflow-y-auto">
      {/* Running Sessions */}
      <section>
        <div className="flex items-center gap-2 mb-3">
          <h2 className="text-lg font-semibold">Running Sessions</h2>
          <span className="px-2 py-0.5 rounded-full text-xs bg-zinc-700 text-zinc-300">
            {liveSessions.length}
          </span>
        </div>
        {liveSessions.length === 0 ? (
          <p className="text-zinc-500 text-sm">No running sessions</p>
        ) : (
          <div className="flex flex-col gap-2">
            {liveSessions.map((session) => (
              <LiveSessionRow key={session.id} session={session} />
            ))}
          </div>
        )}
      </section>

      {/* Run History */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Run History</h2>
        {runsLoading && runs.length === 0 ? (
          <p className="text-zinc-500 text-sm">Loading…</p>
        ) : runs.length === 0 ? (
          <p className="text-zinc-500 text-sm">No runs yet</p>
        ) : (
          <>
            <div className="overflow-x-auto rounded-lg border border-zinc-700">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-zinc-700 bg-zinc-800 text-zinc-400 text-xs uppercase tracking-wide">
                    <th className="px-3 py-2 text-left">Status</th>
                    <th className="px-3 py-2 text-left">Agent</th>
                    <th className="px-3 py-2 text-left">Profile</th>
                    <th className="px-3 py-2 text-left">Trigger</th>
                    <th className="px-3 py-2 text-left">Started</th>
                    <th className="px-3 py-2 text-left">Duration</th>
                    <th className="px-3 py-2 text-left">Error</th>
                  </tr>
                </thead>
                <tbody>
                  {runs.map((run) => (
                    <tr
                      key={run.id}
                      className="border-b border-zinc-800 hover:bg-zinc-800/50 transition-colors"
                    >
                      <td className="px-3 py-2">
                        <StatusIcon status={run.status} />
                      </td>
                      <td className="px-3 py-2 text-zinc-300">{run.agent ?? "—"}</td>
                      <td className="px-3 py-2 text-zinc-400">{run.profile ?? "default"}</td>
                      <td className="px-3 py-2">
                        <TriggerBadge trigger={run.trigger} />
                      </td>
                      <td className="px-3 py-2 text-zinc-400 whitespace-nowrap">
                        {formatDate(run.started_at)}
                      </td>
                      <td className="px-3 py-2 text-zinc-400 whitespace-nowrap">
                        {formatDurationMs(run.duration_ms)}
                      </td>
                      <td className="px-3 py-2 text-red-400 text-xs max-w-xs truncate" title={run.error_message ?? undefined}>
                        {run.error_message ?? "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="mt-3 flex justify-center">
              <button
                onClick={() => setRunsOffset((o) => o + PAGE_SIZE)}
                disabled={runsLoading}
                className="px-4 py-2 text-sm rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors disabled:opacity-50"
              >
                {runsLoading ? "Loading…" : "Load more"}
              </button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

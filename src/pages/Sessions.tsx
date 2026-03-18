import { useEffect, useState } from "react";
import { useSessionsStore } from "../lib/store";
import type { Run, Session } from "../lib/types";
import { listSessionsFiltered } from "../lib/tauri";
import NewSessionModal from "../components/NewSessionModal";
import SessionDetail from "../components/SessionDetail";

// ── Helpers ──────────────────────────────────────────────────────────────────

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
  return new Date(iso).toLocaleString();
}

function shortenPath(p: string | null): string {
  if (!p) return "—";
  const parts = p.split("/");
  if (parts.length <= 3) return p;
  return `…/${parts.slice(-2).join("/")}`;
}

// ── Provider badge ────────────────────────────────────────────────────────────

function ProviderBadge({ provider }: { provider: string }) {
  const lower = provider.toLowerCase();
  if (lower === "claude") {
    return (
      <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-amber-700/60 text-amber-200">
        Claude
      </span>
    );
  }
  if (lower === "gemini") {
    return (
      <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-blue-700/60 text-blue-200">
        Gemini
      </span>
    );
  }
  return (
    <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-600 text-zinc-200">
      {provider}
    </span>
  );
}

// ── Status badge ──────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: Session["status"] }) {
  switch (status) {
    case "running":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-blue-700/40 text-blue-200">
          <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
          Running
        </span>
      );
    case "stopped":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-700 text-zinc-300">
          <span className="w-1.5 h-1.5 rounded-full bg-zinc-400" />
          Stopped
        </span>
      );
    case "error":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-red-700/40 text-red-200">
          <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
          Error
        </span>
      );
    case "killed":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-700/40 text-yellow-200">
          <span className="w-1.5 h-1.5 rounded-full bg-yellow-400" />
          Killed
        </span>
      );
    default:
      return (
        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-600 text-zinc-200">
          {status}
        </span>
      );
  }
}

// ── Running session card ──────────────────────────────────────────────────────

function RunningSessionCard({ session, onClick }: { session: Session; onClick?: () => void }) {
  const [, setTick] = useState(0);
  const { killSession } = useSessionsStore();

  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div
      className="flex flex-col gap-3 p-4 rounded-xl bg-zinc-800 border border-zinc-700 hover:border-zinc-600 transition-colors cursor-pointer"
      onClick={onClick}
    >
      {/* Top row */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 flex-wrap">
          <ProviderBadge provider={session.provider} />
          <span className="text-sm font-medium text-zinc-200">
            {session.profile_name ?? "default"}
          </span>
          <StatusBadge status={session.status} />
        </div>
        <span className="text-xs text-zinc-400 whitespace-nowrap font-mono">
          {session.id.slice(0, 8)}
        </span>
      </div>

      {/* Directory */}
      <div
        className="text-xs text-zinc-400 font-mono truncate"
        title={session.directory ?? undefined}
      >
        {shortenPath(session.directory)}
      </div>

      {/* Bottom row */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-3 text-xs text-zinc-500">
          <span>Started {formatDate(session.started_at)}</span>
          <span className="text-zinc-300 font-medium">
            {formatDuration(session.started_at)}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="px-3 py-1 text-xs rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              // Placeholder: open terminal
              console.log("Open terminal for session", session.id);
            }}
          >
            Open Terminal
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); killSession(session.id); }}
            className="px-3 py-1 text-xs rounded-lg bg-red-700/70 hover:bg-red-600 text-red-100 transition-colors"
          >
            Kill
          </button>
        </div>
      </div>
    </div>
  );
}

// ── History session card ──────────────────────────────────────────────────────

function HistorySessionCard({ session, onClick }: { session: Session; onClick?: () => void }) {
  return (
    <div
      className="flex items-center gap-3 px-4 py-3 rounded-xl bg-zinc-800 border border-zinc-700/50 hover:border-zinc-600 transition-colors cursor-pointer"
      onClick={onClick}
    >
      <StatusBadge status={session.status} />
      <ProviderBadge provider={session.provider} />
      <span className="text-sm text-zinc-300 min-w-0 truncate flex-1">
        {session.profile_name ?? "default"}
      </span>
      <span
        className="text-xs text-zinc-500 font-mono hidden sm:block truncate max-w-[180px]"
        title={session.directory ?? undefined}
      >
        {shortenPath(session.directory)}
      </span>
      <span className="text-xs text-zinc-400 whitespace-nowrap">
        {formatDurationMs(session.duration_ms)}
      </span>
      <span className="text-xs text-zinc-500 whitespace-nowrap hidden md:block">
        {formatDate(session.started_at)}
      </span>
    </div>
  );
}

// ── Run history card (from runs table) ───────────────────────────────────────

function RunStatusBadge({ status }: { status: Run["status"] }) {
  switch (status) {
    case "running":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-blue-700/40 text-blue-200">
          <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
          Running
        </span>
      );
    case "complete":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-green-700/40 text-green-200">
          <span className="w-1.5 h-1.5 rounded-full bg-green-400" />
          Complete
        </span>
      );
    case "error":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-red-700/40 text-red-200">
          <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
          Error
        </span>
      );
    case "killed":
      return (
        <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-700/40 text-yellow-200">
          <span className="w-1.5 h-1.5 rounded-full bg-yellow-400" />
          Killed
        </span>
      );
    default:
      return (
        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-600 text-zinc-200">
          {status}
        </span>
      );
  }
}

function RunCard({ run }: { run: Run }) {
  return (
    <div className="flex items-center gap-3 px-4 py-3 rounded-xl bg-zinc-800 border border-zinc-700/50 hover:border-zinc-600 transition-colors">
      <RunStatusBadge status={run.status} />
      <span className="text-sm text-zinc-300 min-w-0 truncate flex-1">
        {run.agent ?? run.profile ?? "ad-hoc"}
      </span>
      <span
        className="text-xs text-zinc-500 font-mono hidden sm:block truncate max-w-[180px]"
        title={run.directory ?? undefined}
      >
        {shortenPath(run.directory)}
      </span>
      <span className="text-xs text-zinc-400 whitespace-nowrap">
        {formatDurationMs(run.duration_ms)}
      </span>
      <span className="text-xs text-zinc-500 whitespace-nowrap hidden md:block">
        {formatDate(run.started_at)}
      </span>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

const PAGE_SIZE = 20;

export default function Sessions() {
  const { liveSessions, runs, runsLoading, fetchLiveSessions, fetchRuns } =
    useSessionsStore();
  const [runsOffset, setRunsOffset] = useState(0);
  const [modalOpen, setModalOpen] = useState(false);
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);

  // Filter state
  const [filterProvider, setFilterProvider] = useState<string | undefined>();
  const [filterDir, setFilterDir] = useState<string>("");
  const [filteredSessions, setFilteredSessions] = useState<Session[]>([]);

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

  // Fetch filtered sessions when filters change
  useEffect(() => {
    const dir = filterDir.trim() === "" ? undefined : filterDir.trim();
    listSessionsFiltered(dir, filterProvider, 50, 0)
      .then((sessions) => setFilteredSessions(sessions))
      .catch(console.error);
  }, [filterProvider, filterDir]);

  const runningSessions = liveSessions.filter((s) => s.status === "running");

  return (
    <>
      <NewSessionModal open={modalOpen} onClose={() => setModalOpen(false)} />

      <div className="h-full flex flex-col gap-8 overflow-y-auto">
        {/* Page header */}
        <div className="flex items-center justify-between">
          <h1 className="text-xl font-semibold text-zinc-100">Sessions</h1>
          <button
            onClick={() => setModalOpen(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
          >
            <span className="text-lg leading-none">+</span>
            New Session
          </button>
        </div>

        {/* Running Sessions */}
        <section>
          <div className="flex items-center gap-2 mb-3">
            <h2 className="text-base font-semibold text-zinc-200">
              Running Sessions
            </h2>
            <span className="px-2 py-0.5 rounded-full text-xs bg-zinc-700 text-zinc-300">
              {runningSessions.length}
            </span>
          </div>
          {runningSessions.length === 0 ? (
            <p className="text-zinc-500 text-sm">No running sessions</p>
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {runningSessions.map((session) => (
                <RunningSessionCard key={session.id} session={session} onClick={() => setSelectedSession(session)} />
              ))}
            </div>
          )}
        </section>

        {/* Session Detail Panel */}
        {selectedSession && (
          <SessionDetail
            session={selectedSession}
            onClose={() => setSelectedSession(null)}
          />
        )}

        {/* Session History (from sessions table, with filters) */}
        <section>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-base font-semibold text-zinc-200">
              Session History
            </h2>
          </div>

          {/* Filter bar */}
          <div className="flex items-center gap-3 mb-4 flex-wrap">
            {/* Provider toggle buttons */}
            <div className="flex items-center gap-1 bg-zinc-800 rounded-lg p-1">
              {[
                { label: "All", value: undefined },
                { label: "Claude", value: "claude" },
                { label: "Gemini", value: "gemini" },
              ].map(({ label, value }) => (
                <button
                  key={label}
                  onClick={() => setFilterProvider(value)}
                  className={`px-3 py-1 rounded-md text-xs font-medium transition-colors ${
                    filterProvider === value
                      ? "bg-zinc-600 text-zinc-100"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>

            {/* Directory filter input */}
            <input
              type="text"
              value={filterDir}
              onChange={(e) => setFilterDir(e.target.value)}
              placeholder="Filter by directory…"
              className="flex-1 min-w-[180px] max-w-[320px] px-3 py-1.5 rounded-lg bg-zinc-800 border border-zinc-700 text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500"
            />
          </div>

          {filteredSessions.length === 0 ? (
            <p className="text-zinc-500 text-sm">No sessions found</p>
          ) : (
            <div className="flex flex-col gap-2">
              {filteredSessions.map((session) => (
                <HistorySessionCard
                  key={session.id}
                  session={session}
                  onClick={() => setSelectedSession(session)}
                />
              ))}
            </div>
          )}
        </section>

        {/* Run History (from runs table) */}
        <section>
          <h2 className="text-base font-semibold text-zinc-200 mb-3">
            Run History
          </h2>
          {runsLoading && runs.length === 0 ? (
            <p className="text-zinc-500 text-sm">Loading…</p>
          ) : runs.length === 0 ? (
            <p className="text-zinc-500 text-sm">No runs yet</p>
          ) : (
            <>
              <div className="flex flex-col gap-2">
                {runs.map((run) => (
                  <RunCard key={run.id} run={run} />
                ))}
              </div>
              <div className="mt-4 flex justify-center">
                <button
                  onClick={() => setRunsOffset((o) => o + PAGE_SIZE)}
                  disabled={runsLoading}
                  className="px-4 py-2 text-sm rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors disabled:opacity-50"
                >
                  {runsLoading ? "Loading…" : "Load more"}
                </button>
              </div>
            </>
          )}
        </section>
      </div>
    </>
  );
}

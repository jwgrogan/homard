import { useEffect, useState } from "react";
import type { Schedule, Run } from "../../lib/types";
import { useSchedulerStore } from "../../lib/store";

interface Props {
  schedule: Schedule;
  onEdit: () => void;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString();
}

function formatDurationMs(ms: number | null): string {
  if (ms == null) return "—";
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remaining = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remaining}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${Math.floor(minutes % 60)}m`;
}

function humanCron(cron: string): string {
  if (cron === "0 * * * *") return "Every hour";
  const dayMatch = cron.match(/^(\d+) (\d+) \* \* \*$/);
  if (dayMatch) {
    const d = new Date();
    d.setHours(parseInt(dayMatch[2], 10), parseInt(dayMatch[1], 10), 0, 0);
    return `Every day at ${d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  }
  const weekdayMatch = cron.match(/^(\d+) (\d+) \* \* 1-5$/);
  if (weekdayMatch) {
    const d = new Date();
    d.setHours(parseInt(weekdayMatch[2], 10), parseInt(weekdayMatch[1], 10), 0, 0);
    return `Every weekday at ${d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  }
  const nHoursMatch = cron.match(/^0 \*\/(\d+) \* \* \*$/);
  if (nHoursMatch) return `Every ${nHoursMatch[1]} hours`;
  return cron;
}

function StatusDot({ status }: { status: Run["status"] }) {
  const colors: Record<Run["status"], string> = {
    complete: "bg-green-500",
    error: "bg-red-500",
    killed: "bg-yellow-500",
    running: "bg-blue-500 animate-pulse",
  };
  return <span className={`inline-block w-2.5 h-2.5 rounded-full ${colors[status]}`} />;
}

export default function ScheduleDetail({ schedule, onEdit }: Props) {
  const { toggleSchedule, deleteSchedule, fetchScheduleRuns, selectedScheduleRuns } =
    useSchedulerStore();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [toggling, setToggling] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    fetchScheduleRuns(schedule.id, 20, 0);
  }, [schedule.id]);

  async function handleToggle() {
    setToggling(true);
    try {
      await toggleSchedule(schedule.id, !schedule.enabled);
    } finally {
      setToggling(false);
    }
  }

  async function handleDelete() {
    setDeleting(true);
    try {
      await deleteSchedule(schedule.id);
    } finally {
      setDeleting(false);
      setConfirmDelete(false);
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold text-zinc-100">{schedule.name}</h2>
          <p className="text-xs text-zinc-500 font-mono mt-0.5">{schedule.id}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {/* Enabled toggle */}
          <button
            type="button"
            onClick={handleToggle}
            disabled={toggling}
            className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors focus:outline-none disabled:opacity-50 ${
              schedule.enabled ? "bg-green-600" : "bg-zinc-600"
            }`}
            title={schedule.enabled ? "Enabled — click to pause" : "Paused — click to enable"}
          >
            <span
              className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform ${
                schedule.enabled ? "translate-x-4" : "translate-x-1"
              }`}
            />
          </button>

          <button
            onClick={onEdit}
            className="px-3 py-1.5 text-xs rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-200 border border-zinc-600 transition-colors"
          >
            Edit
          </button>

          {confirmDelete ? (
            <div className="flex items-center gap-1">
              <button
                onClick={handleDelete}
                disabled={deleting}
                className="px-3 py-1.5 text-xs rounded bg-red-700 hover:bg-red-600 text-red-100 transition-colors disabled:opacity-50"
              >
                {deleting ? "Deleting…" : "Confirm"}
              </button>
              <button
                onClick={() => setConfirmDelete(false)}
                className="px-2 py-1.5 text-xs rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-300 transition-colors"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setConfirmDelete(true)}
              className="px-3 py-1.5 text-xs rounded bg-zinc-800 hover:bg-red-900/40 text-zinc-400 hover:text-red-400 border border-zinc-700 transition-colors"
            >
              Delete
            </button>
          )}
        </div>
      </div>

      {/* Config summary */}
      <div className="rounded-lg border border-zinc-700 bg-zinc-800/50 overflow-hidden">
        <table className="w-full text-sm">
          <tbody>
            {[
              ["Schedule", humanCron(schedule.schedule)],
              ["Agent", schedule.agent ?? "Raw Prompt"],
              ["Directory", schedule.directory],
              ["Profile", schedule.profile ?? "Default"],
              ["Timeout", `${schedule.timeout_minutes ?? 60} min`],
              ["Session Mode", schedule.session_mode === "fresh" ? "Fresh" : "Persistent"],
            ].map(([label, val]) => (
              <tr key={label} className="border-b border-zinc-700 last:border-0">
                <td className="px-4 py-2 text-xs text-zinc-500 font-medium w-28 shrink-0">
                  {label}
                </td>
                <td className="px-4 py-2 text-zinc-300 text-xs font-mono break-all">{val}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Status badge */}
      <div className="flex items-center gap-2">
        <span
          className={`px-2.5 py-1 rounded-full text-xs font-medium ${
            schedule.enabled
              ? "bg-green-900/40 text-green-300 border border-green-800"
              : "bg-zinc-700 text-zinc-400 border border-zinc-600"
          }`}
        >
          {schedule.enabled ? "Enabled" : "Paused"}
        </span>
        <span className="text-xs text-zinc-500">
          Delivery: {schedule.delivery.channels.join(", ")} on{" "}
          {schedule.delivery.on.join(", ")}
        </span>
      </div>

      {/* Run history */}
      <div>
        <h3 className="text-sm font-semibold text-zinc-300 mb-3">Run History</h3>
        {selectedScheduleRuns.length === 0 ? (
          <p className="text-zinc-500 text-sm">No runs yet for this schedule.</p>
        ) : (
          <div className="rounded-lg border border-zinc-700 overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-zinc-800 border-b border-zinc-700 text-zinc-400 text-xs uppercase tracking-wide">
                  <th className="px-3 py-2 text-left">Status</th>
                  <th className="px-3 py-2 text-left">Started</th>
                  <th className="px-3 py-2 text-left">Duration</th>
                  <th className="px-3 py-2 text-left">Error</th>
                </tr>
              </thead>
              <tbody>
                {selectedScheduleRuns.map((run) => (
                  <tr
                    key={run.id}
                    className="border-b border-zinc-800 last:border-0 hover:bg-zinc-800/50 transition-colors"
                  >
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <StatusDot status={run.status} />
                        <span className="text-xs text-zinc-300 capitalize">{run.status}</span>
                      </div>
                    </td>
                    <td className="px-3 py-2 text-xs text-zinc-400 whitespace-nowrap">
                      {formatDate(run.started_at)}
                    </td>
                    <td className="px-3 py-2 text-xs text-zinc-400 whitespace-nowrap">
                      {formatDurationMs(run.duration_ms)}
                    </td>
                    <td
                      className="px-3 py-2 text-xs text-red-400 max-w-xs truncate"
                      title={run.error_message ?? undefined}
                    >
                      {run.error_message ?? "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

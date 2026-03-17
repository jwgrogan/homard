import { useEffect, useState } from "react";
import { useSchedulerStore } from "../lib/store";
import type { Schedule } from "../lib/types";
import CreateScheduleForm from "../components/scheduler/CreateScheduleForm";
import ScheduleDetail from "../components/scheduler/ScheduleDetail";
import ImportSection from "../components/scheduler/ImportSection";

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
    return `Weekdays at ${d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  }
  const nHoursMatch = cron.match(/^0 \*\/(\d+) \* \* \*$/);
  if (nHoursMatch) return `Every ${nHoursMatch[1]}h`;
  return cron;
}

type RightPanel = "detail" | "create" | "edit";

export default function Scheduler() {
  const {
    schedules,
    loading,
    selectedScheduleId,
    fetchSchedules,
    selectSchedule,
    toggleSchedule,
  } = useSchedulerStore();

  const [panel, setPanel] = useState<RightPanel>("detail");
  const [togglingId, setTogglingId] = useState<string | null>(null);

  useEffect(() => {
    fetchSchedules();
  }, []);

  const selectedSchedule: Schedule | undefined = schedules.find(
    (s) => s.id === selectedScheduleId
  );

  function handleSelectSchedule(id: string) {
    selectSchedule(id);
    setPanel("detail");
  }

  function handleNewJob() {
    selectSchedule(null);
    setPanel("create");
  }

  function handleEdit() {
    setPanel("edit");
  }

  function handleFormDone() {
    setPanel(selectedScheduleId ? "detail" : "detail");
    // If we just created, select the newest schedule
    if (panel === "create") {
      // fetchSchedules already ran inside createSchedule; pick the first schedule if none selected
      fetchSchedules();
    }
  }

  async function handleToggleInline(e: React.MouseEvent, s: Schedule) {
    e.stopPropagation();
    setTogglingId(s.id);
    try {
      await toggleSchedule(s.id, !s.enabled);
    } finally {
      setTogglingId(null);
    }
  }

  return (
    <div className="h-full flex gap-0 overflow-hidden">
      {/* Left panel */}
      <div className="w-72 shrink-0 flex flex-col border-r border-zinc-700 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-700">
          <h1 className="text-base font-semibold text-zinc-100">Schedules</h1>
          <button
            onClick={handleNewJob}
            className="px-3 py-1 text-xs rounded bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors"
          >
            + New Job
          </button>
        </div>

        {/* Schedule list */}
        <div className="flex-1 overflow-y-auto">
          {loading && schedules.length === 0 ? (
            <p className="text-zinc-500 text-sm px-4 py-4">Loading…</p>
          ) : schedules.length === 0 ? (
            <div className="px-4 py-6 text-center">
              <p className="text-zinc-500 text-sm">No schedules yet.</p>
              <p className="text-zinc-600 text-xs mt-1">Click "+ New Job" to create one.</p>
            </div>
          ) : (
            <ul className="p-2 space-y-1">
              {schedules.map((s) => {
                const isSelected = s.id === selectedScheduleId;
                return (
                  <li key={s.id}>
                    <button
                      onClick={() => handleSelectSchedule(s.id)}
                      className={`w-full text-left px-3 py-2.5 rounded-lg border transition-colors ${
                        isSelected
                          ? "bg-zinc-700 border-zinc-600"
                          : "bg-zinc-800/50 border-zinc-700/50 hover:bg-zinc-800 hover:border-zinc-700"
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-sm font-medium text-zinc-100 truncate">
                          {s.name}
                        </span>
                        {/* Inline enabled toggle */}
                        <button
                          type="button"
                          onClick={(e) => handleToggleInline(e, s)}
                          disabled={togglingId === s.id}
                          className={`relative inline-flex h-4 w-7 items-center rounded-full transition-colors shrink-0 disabled:opacity-50 ${
                            s.enabled ? "bg-green-600" : "bg-zinc-600"
                          }`}
                        >
                          <span
                            className={`inline-block h-3 w-3 transform rounded-full bg-white shadow transition-transform ${
                              s.enabled ? "translate-x-3.5" : "translate-x-0.5"
                            }`}
                          />
                        </button>
                      </div>
                      <p className="text-xs text-zinc-400 mt-0.5">{humanCron(s.schedule)}</p>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* Import section */}
        <div className="p-3">
          <ImportSection />
        </div>
      </div>

      {/* Right panel */}
      <div className="flex-1 overflow-y-auto p-6">
        {panel === "create" ? (
          <CreateScheduleForm onDone={handleFormDone} />
        ) : panel === "edit" && selectedSchedule ? (
          <CreateScheduleForm schedule={selectedSchedule} onDone={handleFormDone} />
        ) : selectedSchedule ? (
          <ScheduleDetail schedule={selectedSchedule} onEdit={handleEdit} />
        ) : (
          <div className="flex flex-col items-center justify-center h-full text-center">
            <p className="text-zinc-400 text-sm">Select a schedule to view details</p>
            <p className="text-zinc-600 text-xs mt-1">
              or click "+ New Job" to create one
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

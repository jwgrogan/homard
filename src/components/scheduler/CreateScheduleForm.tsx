import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Schedule, AgentInfo, Profile } from "../../lib/types";
import { useSchedulerStore } from "../../lib/store";
import * as api from "../../lib/tauri";
import CronBuilder from "./CronBuilder";

interface Props {
  schedule?: Schedule;
  onDone: () => void;
}

function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/\s+/g, "-")
    .replace(/[^a-z0-9-]/g, "")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export default function CreateScheduleForm({ schedule, onDone }: Props) {
  const { createSchedule, updateSchedule } = useSchedulerStore();

  const [name, setName] = useState(schedule?.name ?? "");
  const [id, setId] = useState(schedule?.id ?? "");
  const [idManuallyEdited, setIdManuallyEdited] = useState(!!schedule);
  const [cron, setCron] = useState(schedule?.schedule ?? "0 7 * * *");
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [agentSelection, setAgentSelection] = useState<string>(schedule?.agent ?? "__raw__");
  const [prompt, setPrompt] = useState(schedule?.prompt ?? "");
  const [directory, setDirectory] = useState(schedule?.directory ?? "");
  const [profile, setProfile] = useState<string>(schedule?.profile ?? "");
  const [timeoutMinutes, setTimeoutMinutes] = useState<number>(schedule?.timeout_minutes ?? 60);
  const [sessionMode, setSessionMode] = useState<"fresh" | "persistent">(
    schedule?.session_mode ?? "fresh"
  );
  const [deliveryChannels, setDeliveryChannels] = useState<string[]>(
    schedule?.delivery.channels ?? ["log"]
  );
  const [deliverOn, setDeliverOn] = useState<string[]>(
    schedule?.delivery.on ?? ["complete", "error"]
  );
  const [retryMaxAttempts, setRetryMaxAttempts] = useState<number>(
    schedule?.retry.max_attempts ?? 3
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getAgents().then(setAgents).catch(() => {});
    api.listProfiles().then(setProfiles).catch(() => {});
  }, []);

  // Auto-slug ID from name unless user has manually edited it
  useEffect(() => {
    if (!idManuallyEdited) {
      setId(slugify(name));
    }
  }, [name, idManuallyEdited]);

  async function browseDirectory() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setDirectory(selected);
      }
    } catch {
      // dialog cancelled or unavailable
    }
  }

  function toggleChannel(ch: string) {
    setDeliveryChannels((prev) =>
      prev.includes(ch) ? prev.filter((c) => c !== ch) : [...prev, ch]
    );
  }

  function toggleDeliverOn(val: string) {
    setDeliverOn((prev) =>
      prev.includes(val) ? prev.filter((v) => v !== val) : [...prev, val]
    );
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) { setError("Name is required"); return; }
    if (!id.trim()) { setError("ID is required"); return; }
    if (!directory.trim()) { setError("Directory is required"); return; }

    const builtSchedule: Schedule = {
      id: id.trim(),
      name: name.trim(),
      schedule: cron,
      timezone: null,
      agent: agentSelection === "__raw__" ? null : agentSelection,
      prompt: prompt.trim() || null,
      directory: directory.trim(),
      profile: profile || null,
      timeout_minutes: timeoutMinutes,
      session_mode: sessionMode,
      last_session_id: schedule?.last_session_id ?? null,
      delivery: {
        channels: deliveryChannels,
        on: deliverOn,
      },
      retry: {
        max_attempts: retryMaxAttempts,
        backoff_seconds: schedule?.retry.backoff_seconds ?? [30, 60, 120],
      },
      enabled: schedule?.enabled ?? true,
    };

    setSubmitting(true);
    setError(null);
    try {
      if (schedule) {
        await updateSchedule(builtSchedule);
      } else {
        await createSchedule(builtSchedule);
      }
      onDone();
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  }

  const inputClass =
    "w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-100 placeholder-zinc-600 focus:outline-none focus:border-blue-500";
  const labelClass = "block text-xs font-medium text-zinc-400 mb-1";

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      <h2 className="text-base font-semibold text-zinc-100">
        {schedule ? "Edit Schedule" : "New Schedule"}
      </h2>

      {/* Name + ID row */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className={labelClass}>Name *</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Daily brief"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>ID *</label>
          <input
            type="text"
            value={id}
            onChange={(e) => {
              setId(e.target.value);
              setIdManuallyEdited(true);
            }}
            placeholder="daily-brief"
            className={`${inputClass} font-mono`}
          />
        </div>
      </div>

      {/* Schedule */}
      <div>
        <label className={labelClass}>Schedule</label>
        <div className="p-3 bg-zinc-800/50 border border-zinc-700 rounded">
          <CronBuilder value={cron} onChange={setCron} />
        </div>
      </div>

      {/* Agent */}
      <div>
        <label className={labelClass}>Agent</label>
        <select
          value={agentSelection}
          onChange={(e) => setAgentSelection(e.target.value)}
          className={inputClass}
        >
          <option value="__raw__">Raw Prompt</option>
          {agents.map((a) => (
            <option key={a.path} value={a.name}>
              {a.name} ({a.scope})
            </option>
          ))}
        </select>
      </div>

      {/* Prompt — shown always for "Raw Prompt", or as optional override */}
      <div>
        <label className={labelClass}>
          Prompt{agentSelection === "__raw__" ? " *" : " (override)"}
        </label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder={
            agentSelection === "__raw__"
              ? "Enter the prompt to run…"
              : "Optional: override agent's default prompt"
          }
          rows={4}
          className={`${inputClass} resize-none`}
        />
      </div>

      {/* Directory */}
      <div>
        <label className={labelClass}>Directory *</label>
        <div className="flex gap-2">
          <input
            type="text"
            value={directory}
            onChange={(e) => setDirectory(e.target.value)}
            placeholder="/path/to/project"
            className={`${inputClass} flex-1 font-mono text-xs`}
          />
          <button
            type="button"
            onClick={browseDirectory}
            className="px-3 py-2 text-xs rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-200 border border-zinc-600 transition-colors shrink-0"
          >
            Browse
          </button>
        </div>
      </div>

      {/* Profile + Timeout row */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className={labelClass}>Profile</label>
          <select
            value={profile}
            onChange={(e) => setProfile(e.target.value)}
            className={inputClass}
          >
            <option value="">Default</option>
            {profiles.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className={labelClass}>Timeout (minutes)</label>
          <input
            type="number"
            value={timeoutMinutes}
            onChange={(e) => setTimeoutMinutes(parseInt(e.target.value, 10) || 60)}
            min={1}
            max={1440}
            className={inputClass}
          />
        </div>
      </div>

      {/* Session Mode */}
      <div>
        <label className={labelClass}>Session Mode</label>
        <div className="flex gap-4">
          {(["fresh", "persistent"] as const).map((mode) => (
            <label key={mode} className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="session-mode"
                value={mode}
                checked={sessionMode === mode}
                onChange={() => setSessionMode(mode)}
                className="accent-blue-500"
              />
              <span className="text-sm text-zinc-200 capitalize">{mode}</span>
            </label>
          ))}
        </div>
      </div>

      {/* Delivery */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className={labelClass}>Delivery Channels</label>
          <div className="space-y-1.5">
            {[
              { value: "notification", label: "System Notification" },
              { value: "log", label: "Log Only" },
            ].map(({ value, label }) => (
              <label key={value} className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={deliveryChannels.includes(value)}
                  onChange={() => toggleChannel(value)}
                  className="accent-blue-500"
                />
                <span className="text-sm text-zinc-200">{label}</span>
              </label>
            ))}
          </div>
        </div>
        <div>
          <label className={labelClass}>Deliver On</label>
          <div className="space-y-1.5">
            {["complete", "error"].map((val) => (
              <label key={val} className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={deliverOn.includes(val)}
                  onChange={() => toggleDeliverOn(val)}
                  className="accent-blue-500"
                />
                <span className="text-sm text-zinc-200 capitalize">{val}</span>
              </label>
            ))}
          </div>
        </div>
      </div>

      {/* Retry */}
      <div>
        <label className={labelClass}>Retry Max Attempts</label>
        <input
          type="number"
          value={retryMaxAttempts}
          onChange={(e) => setRetryMaxAttempts(parseInt(e.target.value, 10) || 0)}
          min={0}
          max={10}
          className={`${inputClass} w-24`}
        />
      </div>

      {error && (
        <p className="text-sm text-red-400 bg-red-900/20 border border-red-800 rounded px-3 py-2">
          {error}
        </p>
      )}

      {/* Actions */}
      <div className="flex gap-3 pt-1">
        <button
          type="submit"
          disabled={submitting}
          className="px-4 py-2 text-sm rounded bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors disabled:opacity-50"
        >
          {submitting ? "Saving…" : schedule ? "Save Changes" : "Create Schedule"}
        </button>
        <button
          type="button"
          onClick={onDone}
          className="px-4 py-2 text-sm rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}

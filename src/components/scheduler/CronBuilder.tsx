import { useState, useEffect } from "react";

export interface CronBuilderProps {
  value: string;
  onChange: (cron: string) => void;
}

type Preset = "every-hour" | "every-day" | "every-weekday" | "every-n-hours" | "custom";

const PRESET_LABELS: Record<Preset, string> = {
  "every-hour": "Every hour",
  "every-day": "Every day at…",
  "every-weekday": "Every weekday at…",
  "every-n-hours": "Every N hours",
  "custom": "Custom",
};

function detectPreset(cron: string): Preset {
  if (cron === "0 * * * *") return "every-hour";
  if (/^\d+ \d+ \* \* \*$/.test(cron) && !cron.includes("/")) return "every-day";
  if (/^\d+ \d+ \* \* 1-5$/.test(cron)) return "every-weekday";
  if (/^0 \*\/\d+ \* \* \*$/.test(cron)) return "every-n-hours";
  return "custom";
}

function parseDayTime(cron: string): { hour: number; minute: number } {
  const parts = cron.split(" ");
  const minute = parseInt(parts[0], 10) || 0;
  const hour = parseInt(parts[1], 10) || 7;
  return { hour, minute };
}

function describePreset(preset: Preset, cron: string): string {
  if (preset === "every-hour") return "Every hour at :00";
  if (preset === "every-day") {
    const { hour, minute } = parseDayTime(cron);
    const d = new Date();
    d.setHours(hour, minute, 0, 0);
    return `Every day at ${d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  }
  if (preset === "every-weekday") {
    const { hour, minute } = parseDayTime(cron);
    const d = new Date();
    d.setHours(hour, minute, 0, 0);
    return `Every weekday (Mon–Fri) at ${d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  }
  if (preset === "every-n-hours") {
    const match = cron.match(/^0 \*\/(\d+) \* \* \*$/);
    const n = match ? parseInt(match[1], 10) : 1;
    return `Every ${n} hour${n !== 1 ? "s" : ""}`;
  }
  return `Custom: ${cron}`;
}

export default function CronBuilder({ value, onChange }: CronBuilderProps) {
  const [preset, setPreset] = useState<Preset>(() => detectPreset(value));
  const [hour, setHour] = useState<number>(() => parseDayTime(value).hour);
  const [minute, setMinute] = useState<number>(() => parseDayTime(value).minute);
  const [interval, setInterval_] = useState<number>(2);
  const [customCron, setCustomCron] = useState(value);

  // Sync external value changes (e.g. when editing existing schedule)
  useEffect(() => {
    const detected = detectPreset(value);
    setPreset(detected);
    if (detected === "every-day" || detected === "every-weekday") {
      const { hour: h, minute: m } = parseDayTime(value);
      setHour(h);
      setMinute(m);
    }
    if (detected === "every-n-hours") {
      const match = value.match(/^0 \*\/(\d+) \* \* \*$/);
      if (match) setInterval_(parseInt(match[1], 10));
    }
    if (detected === "custom") {
      setCustomCron(value);
    }
  }, [value]);

  function applyPreset(p: Preset) {
    setPreset(p);
    if (p === "every-hour") onChange("0 * * * *");
    else if (p === "every-day") onChange(`${minute} ${hour} * * *`);
    else if (p === "every-weekday") onChange(`${minute} ${hour} * * 1-5`);
    else if (p === "every-n-hours") onChange(`0 */${interval} * * *`);
    else onChange(customCron);
  }

  function handleTimeChange(h: number, m: number) {
    setHour(h);
    setMinute(m);
    if (preset === "every-day") onChange(`${m} ${h} * * *`);
    else if (preset === "every-weekday") onChange(`${m} ${h} * * 1-5`);
  }

  function handleIntervalChange(n: number) {
    setInterval_(n);
    onChange(`0 */${n} * * *`);
  }

  const description = describePreset(preset, value);

  return (
    <div className="space-y-3">
      {/* Preset pills */}
      <div className="flex flex-wrap gap-2">
        {(Object.keys(PRESET_LABELS) as Preset[]).map((p) => (
          <button
            key={p}
            type="button"
            onClick={() => applyPreset(p)}
            className={`px-3 py-1 rounded-full text-xs font-medium transition-colors border ${
              preset === p
                ? "bg-blue-600 border-blue-500 text-white"
                : "bg-zinc-800 border-zinc-600 text-zinc-300 hover:bg-zinc-700"
            }`}
          >
            {PRESET_LABELS[p]}
          </button>
        ))}
      </div>

      {/* Controls */}
      {(preset === "every-day" || preset === "every-weekday") && (
        <div className="flex items-center gap-2">
          <label className="text-xs text-zinc-400 w-10">Time</label>
          <select
            value={hour}
            onChange={(e) => handleTimeChange(parseInt(e.target.value, 10), minute)}
            className="bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-100"
          >
            {Array.from({ length: 24 }, (_, i) => (
              <option key={i} value={i}>
                {String(i).padStart(2, "0")}
              </option>
            ))}
          </select>
          <span className="text-zinc-400">:</span>
          <select
            value={minute}
            onChange={(e) => handleTimeChange(hour, parseInt(e.target.value, 10))}
            className="bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-100"
          >
            {[0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55].map((m) => (
              <option key={m} value={m}>
                {String(m).padStart(2, "0")}
              </option>
            ))}
          </select>
        </div>
      )}

      {preset === "every-n-hours" && (
        <div className="flex items-center gap-2">
          <label className="text-xs text-zinc-400 w-10">Every</label>
          <select
            value={interval}
            onChange={(e) => handleIntervalChange(parseInt(e.target.value, 10))}
            className="bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-100"
          >
            {[1, 2, 3, 4, 6, 8, 12].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
          <span className="text-xs text-zinc-400">hours</span>
        </div>
      )}

      {preset === "custom" && (
        <div className="flex items-center gap-2">
          <label className="text-xs text-zinc-400 w-10">Cron</label>
          <input
            type="text"
            value={customCron}
            onChange={(e) => {
              setCustomCron(e.target.value);
              onChange(e.target.value);
            }}
            placeholder="0 7 * * *"
            className="flex-1 bg-zinc-800 border border-zinc-600 rounded px-3 py-1 text-sm text-zinc-100 font-mono placeholder-zinc-600"
          />
        </div>
      )}

      {/* Human-readable description */}
      <p className="text-xs text-blue-400 font-medium">{description}</p>
    </div>
  );
}

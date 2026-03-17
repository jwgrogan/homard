import { useState } from "react";
import { useSchedulerStore } from "../../lib/store";

export default function ImportSection() {
  const { discoveredJobs, discoverJobs, importJob, selectSchedule } = useSchedulerStore();
  const [scanning, setScanning] = useState(false);
  const [importing, setImporting] = useState<string | null>(null);
  const [importedLabels, setImportedLabels] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [scanned, setScanned] = useState(false);

  async function handleScan() {
    setScanning(true);
    setError(null);
    try {
      await discoverJobs();
      setScanned(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setScanning(false);
    }
  }

  async function handleImport(label: string) {
    setImporting(label);
    setError(null);
    try {
      const schedule = await importJob(label);
      setImportedLabels((prev) => new Set([...prev, label]));
      selectSchedule(schedule.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(null);
    }
  }

  function commandPreview(args: string[]): string {
    return args.join(" ").slice(0, 60) + (args.join(" ").length > 60 ? "…" : "");
  }

  return (
    <div className="border-t border-zinc-700 pt-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-semibold text-zinc-400 uppercase tracking-wide">
          Import launchd Jobs
        </h3>
        <button
          onClick={handleScan}
          disabled={scanning}
          className="px-2.5 py-1 text-xs rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-200 border border-zinc-600 transition-colors disabled:opacity-50"
        >
          {scanning ? "Scanning…" : "Scan"}
        </button>
      </div>

      {error && (
        <p className="text-xs text-red-400 mb-2">{error}</p>
      )}

      {scanned && discoveredJobs.length === 0 && (
        <p className="text-xs text-zinc-500 text-center py-3">
          No existing Claude launchd jobs found
        </p>
      )}

      {discoveredJobs.length > 0 && (
        <div className="space-y-1.5">
          {discoveredJobs.map((job) => {
            const alreadyImported = importedLabels.has(job.label);
            return (
              <div
                key={job.label}
                className="flex items-start gap-2 p-2 rounded bg-zinc-800 border border-zinc-700"
              >
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium text-zinc-200 truncate">{job.label}</p>
                  {(job.hour != null || job.minute != null) && (
                    <p className="text-xs text-zinc-500">
                      {String(job.hour ?? 0).padStart(2, "0")}:
                      {String(job.minute ?? 0).padStart(2, "0")} daily
                    </p>
                  )}
                  {job.program_args.length > 0 && (
                    <p className="text-xs text-zinc-600 font-mono truncate mt-0.5">
                      {commandPreview(job.program_args)}
                    </p>
                  )}
                </div>
                <button
                  onClick={() => handleImport(job.label)}
                  disabled={importing === job.label || alreadyImported}
                  className={`px-2.5 py-1 text-xs rounded transition-colors shrink-0 ${
                    alreadyImported
                      ? "bg-green-900/40 text-green-400 border border-green-800 cursor-default"
                      : "bg-blue-700 hover:bg-blue-600 text-white disabled:opacity-50"
                  }`}
                >
                  {importing === job.label
                    ? "Importing…"
                    : alreadyImported
                    ? "Imported"
                    : "Import"}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

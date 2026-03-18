import { useEffect, useState } from "react";
import { runHealthCheck, detectClaudeSwitch } from "../lib/tauri";
import type { HealthStatus } from "../lib/types";

function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      className={`inline-block w-3 h-3 rounded-full ${ok ? "bg-green-500" : "bg-red-500"}`}
    />
  );
}

export default function Health() {
  const [status, setStatus] = useState<HealthStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claudeSwitchDetected, setClaudeSwitchDetected] = useState(false);

  useEffect(() => {
    runHealthCheck()
      .then(setStatus)
      .catch((e: unknown) =>
        setError(e instanceof Error ? e.message : String(e))
      );
  }, []);

  useEffect(() => {
    detectClaudeSwitch().then(setClaudeSwitchDetected);
  }, []);

  if (error) {
    return (
      <div className="text-red-400">
        <p className="font-semibold">Health check failed</p>
        <p className="text-sm mt-1">{error}</p>
      </div>
    );
  }

  if (!status) {
    return <p className="text-zinc-400">Checking system health...</p>;
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-zinc-100">System Health</h2>

      {claudeSwitchDetected && (
        <div className="bg-yellow-900/50 border border-yellow-700 rounded-lg p-4 mb-6">
          <h3 className="text-yellow-200 font-medium">claude-switch detected</h3>
          <p className="text-yellow-300/80 text-sm mt-1">
            arcctl now handles profile switching. You can import your existing profiles in Settings → Profiles,
            then remove claude-switch:
          </p>
          <code className="block mt-2 text-xs text-yellow-200 bg-yellow-900/50 rounded px-2 py-1">
            brew uninstall claude-switch
          </code>
        </div>
      )}

      <div className="bg-zinc-800 rounded-lg p-4 space-y-3">
        <div className="flex items-center gap-3">
          <StatusDot ok={status.claude_cli_installed} />
          <span className="text-zinc-200 text-sm">Claude CLI installed</span>
          {status.claude_cli_version && (
            <span className="ml-auto text-zinc-400 text-xs font-mono">
              {status.claude_cli_version}
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          <StatusDot ok={status.arcctl_dir_exists} />
          <span className="text-zinc-200 text-sm">~/.arcctl directory</span>
        </div>

        <div className="flex items-center gap-3">
          <StatusDot ok={status.active_profile !== null} />
          <span className="text-zinc-200 text-sm">Active profile</span>
          {status.active_profile && (
            <span className="ml-auto text-zinc-400 text-xs">
              {status.active_profile.name}
              {status.active_profile.email && ` (${status.active_profile.email})`}
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          <StatusDot ok={status.telegram_connected} />
          <span className="text-zinc-200 text-sm">Telegram connected</span>
        </div>

        <div className="flex items-center gap-3">
          <StatusDot ok={status.email_configured} />
          <span className="text-zinc-200 text-sm">Email configured</span>
        </div>
      </div>

      <p className="text-zinc-500 text-xs">
        Checked at {new Date(status.checked_at).toLocaleString()}
      </p>
    </div>
  );
}

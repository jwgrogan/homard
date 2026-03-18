import { useEffect, useState } from "react";
import { getEmailConfig, saveEmailConfig } from "../../lib/tauri";

export function EmailPanel() {
  const [enabled, setEnabled] = useState(false);
  const [botAddress, setBotAddress] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getEmailConfig()
      .then((cfg) => {
        setEnabled(cfg.enabled);
        setBotAddress(cfg.bot_address ?? "");
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const addr = botAddress.trim() || null;
      await saveEmailConfig(true, addr);
      setEnabled(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleDisable() {
    setSaving(true);
    try {
      await saveEmailConfig(false, null);
      setEnabled(false);
      setBotAddress("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (loading) return <div className="p-4 text-sm text-gray-400">Loading...</div>;

  return (
    <div className="space-y-6 p-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-white">Email Bridge</h2>
          <p className="text-xs text-gray-400 mt-0.5">
            Receive job results and trigger arcctl via email.
          </p>
        </div>
        {enabled && (
          <div className="flex items-center gap-2">
            <span className="h-2 w-2 rounded-full bg-green-400" />
            <span className="text-xs text-gray-400">Configured</span>
          </div>
        )}
      </div>

      {!enabled ? (
        <div className="space-y-4">
          <div className="rounded-lg border border-gray-700 bg-gray-800/50 p-4 space-y-3">
            <h3 className="text-xs font-medium text-gray-300">Setup</h3>
            <ol className="text-xs text-gray-400 space-y-1 list-decimal list-inside">
              <li>Set up an email address for arcctl to monitor</li>
              <li>Configure your email provider to forward to arcctl</li>
              <li>Enter the bot email address below</li>
            </ol>
            <div className="flex gap-2 mt-3">
              <input
                type="email"
                value={botAddress}
                onChange={(e) => setBotAddress(e.target.value)}
                placeholder="arcctl-bot@yourmail.com"
                className="flex-1 rounded bg-gray-900 border border-gray-700 px-3 py-1.5 text-xs text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                onKeyDown={(e) => e.key === "Enter" && handleSave()}
              />
              <button
                onClick={handleSave}
                disabled={saving || !botAddress.trim()}
                className="rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-50 px-3 py-1.5 text-xs text-white font-medium"
              >
                {saving ? "Saving..." : "Save & Enable"}
              </button>
            </div>
            {error && <p className="text-xs text-red-400">{error}</p>}
          </div>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="rounded-lg border border-gray-700 bg-gray-800/50 p-4 flex items-center justify-between">
            <div>
              <p className="text-xs font-medium text-white">{botAddress || "No address set"}</p>
              <p className="text-xs text-gray-400">Email bridge is active</p>
            </div>
            <button
              onClick={handleDisable}
              disabled={saving}
              className="rounded bg-gray-700 hover:bg-gray-600 px-3 py-1.5 text-xs font-medium text-gray-300"
            >
              Disable
            </button>
          </div>
          {error && <p className="text-xs text-red-400">{error}</p>}
        </div>
      )}
    </div>
  );
}

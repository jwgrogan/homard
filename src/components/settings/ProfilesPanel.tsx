import { useEffect, useState } from "react";
import { useProfilesStore } from "../../lib/store";

export default function ProfilesPanel() {
  const { profiles, loading, fetchProfiles, switchProfile, importProfile } = useProfilesStore();
  const [importName, setImportName] = useState("");
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [switching, setSwitching] = useState<string | null>(null);
  const [confirmSwitch, setConfirmSwitch] = useState<string | null>(null);

  useEffect(() => {
    fetchProfiles();
  }, []);

  async function handleImport() {
    const trimmed = importName.trim();
    if (!trimmed) { setImportError("Name is required"); return; }
    setImporting(true);
    setImportError(null);
    try {
      await importProfile(trimmed);
      setImportName("");
    } catch (e) {
      setImportError(String(e));
    } finally {
      setImporting(false);
    }
  }

  async function handleSwitch(name: string) {
    if (confirmSwitch !== name) {
      setConfirmSwitch(name);
      setTimeout(() => setConfirmSwitch(null), 3000);
      return;
    }
    setConfirmSwitch(null);
    setSwitching(name);
    try {
      await switchProfile(name);
    } finally {
      setSwitching(null);
    }
  }

  if (loading && profiles.length === 0) {
    return <p className="text-sm text-zinc-400">Loading profiles…</p>;
  }

  return (
    <div>
      <div className="space-y-3 mb-6">
        {profiles.length === 0 ? (
          <p className="text-sm text-zinc-500">No profiles found.</p>
        ) : (
          profiles.map((profile) => (
            <div
              key={profile.name}
              className="bg-zinc-800 border border-zinc-700 rounded p-3 flex items-center justify-between"
            >
              <div className="flex items-center gap-3">
                <div
                  className={`h-2.5 w-2.5 rounded-full ${
                    profile.is_active ? "bg-green-400" : "bg-zinc-600"
                  }`}
                  title={profile.is_active ? "Active" : "Inactive"}
                />
                <div>
                  <span className="text-sm font-medium text-zinc-100">{profile.name}</span>
                  {profile.email && (
                    <p className="text-xs text-zinc-400">{profile.email}</p>
                  )}
                </div>
                {profile.is_active && (
                  <span className="rounded-full px-2 py-0.5 text-xs bg-green-900 text-green-300">
                    Active
                  </span>
                )}
              </div>
              <button
                onClick={() => handleSwitch(profile.name)}
                disabled={profile.is_active || switching === profile.name}
                className={`px-3 py-1.5 rounded text-sm disabled:opacity-40 disabled:cursor-not-allowed ${
                  confirmSwitch === profile.name
                    ? "bg-yellow-600 hover:bg-yellow-500"
                    : "bg-zinc-700 hover:bg-zinc-600"
                }`}
              >
                {switching === profile.name
                  ? "Switching…"
                  : confirmSwitch === profile.name
                  ? "Confirm?"
                  : "Switch"}
              </button>
            </div>
          ))
        )}
      </div>

      <div className="bg-zinc-800 border border-zinc-700 rounded p-4 space-y-3">
        <h3 className="text-sm font-medium text-zinc-200">Import Current Profile</h3>
        <p className="text-xs text-zinc-400">
          Save the currently authenticated Claude CLI session as a named profile.
        </p>
        <div className="flex gap-2">
          <input
            type="text"
            value={importName}
            onChange={(e) => { setImportName(e.target.value); setImportError(null); }}
            onKeyDown={(e) => e.key === "Enter" && handleImport()}
            placeholder="Profile name…"
            className="flex-1 bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
          />
          <button
            onClick={handleImport}
            disabled={importing}
            className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
          >
            {importing ? "Importing…" : "Import"}
          </button>
        </div>
        {importError && <p className="text-xs text-red-400">{importError}</p>}
      </div>
    </div>
  );
}

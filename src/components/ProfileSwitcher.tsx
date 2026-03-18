import { useEffect, useRef, useState } from "react";
import { useProfilesStore } from "../lib/store";
import { checkAllProfileHealth } from "../lib/tauri";
import type { CredentialHealth, Profile } from "../lib/types";

const PROVIDER_LABELS: Record<string, string> = {
  claude: "Claude Code",
  gemini: "Gemini CLI",
};

function healthDotColor(h: CredentialHealth | undefined): string {
  switch (h) {
    case "valid":
      return "bg-green-500";
    case "expiring":
      return "bg-yellow-500";
    case "expired":
      return "bg-red-500";
    default:
      return "bg-zinc-600";
  }
}

export default function ProfileSwitcher() {
  const { profiles, fetchProfiles, switchProfile, importProfile } = useProfilesStore();
  const [open, setOpen] = useState(false);
  const [health, setHealth] = useState<Record<string, CredentialHealth>>({});
  const [showImportForm, setShowImportForm] = useState(false);
  const [importName, setImportName] = useState("");
  const [importError, setImportError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    fetchProfiles();
  }, []);

  // Fetch health when popover opens
  useEffect(() => {
    if (!open) return;
    checkAllProfileHealth().then((results) => {
      const map: Record<string, CredentialHealth> = {};
      for (const [name, status] of results) {
        map[name] = status;
      }
      setHealth(map);
    });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function handleClickOutside(e: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  const activeProfile = profiles.find((p) => p.is_active) ?? null;

  // Group profiles by provider
  const grouped = profiles.reduce<Record<string, Profile[]>>((acc, p) => {
    const key = p.provider;
    if (!acc[key]) acc[key] = [];
    acc[key].push(p);
    return acc;
  }, {});

  const handleSwitch = async (name: string) => {
    await switchProfile(name);
    setOpen(false);
    setShowImportForm(false);
  };

  const handleImport = async () => {
    const trimmed = importName.trim();
    if (!trimmed) { setImportError("Name is required"); return; }
    setImporting(true);
    setImportError(null);
    try {
      await importProfile(trimmed);
      setImportName("");
      setShowImportForm(false);
      setImportError(null);
    } catch (e) {
      setImportError(String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="pt-4 border-t border-zinc-700 relative" ref={popoverRef}>
      {open && (
        <div className="absolute bottom-full left-0 mb-2 w-56 bg-zinc-800 border border-zinc-700 rounded-lg shadow-lg z-50 overflow-hidden">
          {Object.entries(grouped).map(([provider, providerProfiles]) => (
            <div key={provider}>
              <div className="px-3 py-1.5 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
                {PROVIDER_LABELS[provider] ?? provider}
              </div>
              {providerProfiles.map((profile) => {
                const profileHealth = health[profile.name];
                const isExpired = profileHealth === "expired";
                return (
                  <button
                    key={profile.name}
                    onClick={() => handleSwitch(profile.name)}
                    className="w-full text-left px-3 py-2 hover:bg-zinc-700 flex items-center gap-2"
                  >
                    <span
                      className={`w-2 h-2 rounded-full shrink-0 ${
                        profile.is_active
                          ? healthDotColor(profileHealth)
                          : isExpired
                          ? "bg-red-500"
                          : "bg-zinc-600"
                      }`}
                    />
                    <div className="min-w-0">
                      <div className="text-sm text-zinc-100 truncate">{profile.name}</div>
                      {isExpired ? (
                        <div className="text-xs text-red-400 truncate">Re-auth needed</div>
                      ) : profile.email ? (
                        <div className="text-xs text-zinc-400 truncate">{profile.email}</div>
                      ) : null}
                    </div>
                  </button>
                );
              })}
            </div>
          ))}
          {profiles.length === 0 && (
            <div className="px-3 py-2 text-xs text-zinc-500">No profiles found</div>
          )}
          <div className="border-t border-zinc-700">
            {!showImportForm ? (
              <button
                onClick={() => setShowImportForm(true)}
                className="w-full text-left px-3 py-2 text-sm text-zinc-400 hover:bg-zinc-700 flex items-center gap-2"
              >
                <span className="text-base leading-none">+</span>
                <span>Add profile</span>
              </button>
            ) : (
              <div className="px-3 py-2 space-y-2">
                <input
                  type="text"
                  value={importName}
                  onChange={(e) => { setImportName(e.target.value); setImportError(null); }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleImport();
                    if (e.key === "Escape") { setShowImportForm(false); setImportError(null); }
                  }}
                  placeholder="Profile name…"
                  autoFocus
                  className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-2 py-1 text-xs placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
                />
                {importError && <p className="text-xs text-red-400">{importError}</p>}
                <div className="flex gap-1">
                  <button
                    onClick={handleImport}
                    disabled={importing}
                    className="px-2 py-1 rounded text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
                  >
                    {importing ? "..." : "Import"}
                  </button>
                  <button
                    onClick={() => { setShowImportForm(false); setImportError(null); }}
                    className="px-2 py-1 rounded text-xs bg-zinc-700 hover:bg-zinc-600"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      <button
        onClick={() => setOpen((v) => !v)}
        className="w-full text-left px-3 py-2 rounded text-xs text-zinc-400 hover:bg-zinc-800"
      >
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full shrink-0 ${
              activeProfile ? "bg-green-500" : "bg-zinc-600"
            }`}
          />
          <span className="truncate">{activeProfile?.name ?? "No profile"}</span>
        </div>
        {activeProfile?.email && (
          <div className="text-zinc-500 text-xs mt-0.5 truncate pl-4">
            {activeProfile.email}
          </div>
        )}
      </button>
    </div>
  );
}

import { useEffect, useRef, useState } from "react";
import { useProfilesStore } from "../lib/store";
import type { Profile } from "../lib/types";

const PROVIDER_LABELS: Record<string, string> = {
  claude: "Claude Code",
  gemini: "Gemini CLI",
};

export default function ProfileSwitcher() {
  const { profiles, fetchProfiles, switchProfile, importProfile } = useProfilesStore();
  const [open, setOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    fetchProfiles();
  }, []);

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
  };

  const handleImport = async () => {
    const name = prompt("Enter profile name to import:");
    if (!name) return;
    await importProfile(name);
    setOpen(false);
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
              {providerProfiles.map((profile) => (
                <button
                  key={profile.name}
                  onClick={() => handleSwitch(profile.name)}
                  className="w-full text-left px-3 py-2 hover:bg-zinc-700 flex items-center gap-2"
                >
                  <span
                    className={`w-2 h-2 rounded-full shrink-0 ${
                      profile.is_active ? "bg-green-500" : "bg-zinc-600"
                    }`}
                  />
                  <div className="min-w-0">
                    <div className="text-sm text-zinc-100 truncate">{profile.name}</div>
                    {profile.email && (
                      <div className="text-xs text-zinc-400 truncate">{profile.email}</div>
                    )}
                  </div>
                </button>
              ))}
            </div>
          ))}
          {profiles.length === 0 && (
            <div className="px-3 py-2 text-xs text-zinc-500">No profiles found</div>
          )}
          <div className="border-t border-zinc-700">
            <button
              onClick={handleImport}
              className="w-full text-left px-3 py-2 text-sm text-zinc-400 hover:bg-zinc-700 flex items-center gap-2"
            >
              <span className="text-base leading-none">+</span>
              <span>Add profile</span>
            </button>
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

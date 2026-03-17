import { useEffect, useState } from "react";
import Health from "./pages/Health";
import Settings from "./pages/Settings";
import Sessions from "./pages/Sessions";
import QuickPrompt from "./components/QuickPrompt";
import { useProfilesStore } from "./lib/store";

type Page = "health" | "sessions" | "settings" | "scheduler";

export default function App() {
  const [page, setPage] = useState<Page>("health");
  const { profiles, fetchProfiles } = useProfilesStore();

  useEffect(() => {
    fetchProfiles();
  }, []);

  const activeProfile = profiles.find((p) => p.is_active) ?? null;

  return (
    <>
      <div className="flex h-screen bg-zinc-900 text-zinc-100">
        <nav className="w-48 border-r border-zinc-700 p-4 flex flex-col">
          <div className="space-y-2 flex-1">
          {(["health", "sessions", "settings", "scheduler"] as Page[]).map((p) => (
            <button
              key={p}
              onClick={() => setPage(p)}
              className={`block w-full text-left px-3 py-2 rounded text-sm capitalize ${
                page === p ? "bg-zinc-700" : "hover:bg-zinc-800"
              }`}
            >
              {p}
            </button>
          ))}
          </div>
          <div className="pt-4 border-t border-zinc-700">
            <button
              onClick={() => setPage("settings")}
              className="w-full text-left px-3 py-2 rounded text-xs text-zinc-400 hover:bg-zinc-800"
            >
              <div className="flex items-center gap-2">
                <span
                  className={`w-2 h-2 rounded-full shrink-0 ${
                    activeProfile ? "bg-green-500" : "bg-zinc-600"
                  }`}
                />
                <span className="truncate">
                  {activeProfile?.name ?? "No profile"}
                </span>
              </div>
              {activeProfile?.email && (
                <div className="text-zinc-500 text-xs mt-0.5 truncate pl-4">
                  {activeProfile.email}
                </div>
              )}
            </button>
          </div>
        </nav>
        <main className="flex-1 p-6 overflow-hidden">
          {page === "health" ? (
            <Health />
          ) : page === "settings" ? (
            <Settings />
          ) : page === "sessions" ? (
            <Sessions />
          ) : (
            <>
              <h1 className="text-xl font-semibold capitalize">{page}</h1>
              <p className="text-zinc-400 mt-2">Coming soon.</p>
            </>
          )}
        </main>
      </div>
      <QuickPrompt />
    </>
  );
}

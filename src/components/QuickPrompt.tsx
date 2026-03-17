import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useProfilesStore, useSessionsStore } from "../lib/store";

const RECENT_DIRS_KEY = "arcctl:recent-dirs";
const MAX_RECENT = 5;

function getRecentDirs(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_DIRS_KEY) ?? "[]");
  } catch {
    return [];
  }
}

function addRecentDir(dir: string): void {
  const existing = getRecentDirs().filter((d) => d !== dir);
  const updated = [dir, ...existing].slice(0, MAX_RECENT);
  localStorage.setItem(RECENT_DIRS_KEY, JSON.stringify(updated));
}

export default function QuickPrompt() {
  const [visible, setVisible] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [directory, setDirectory] = useState("");
  const [selectedProfile, setSelectedProfile] = useState("");
  const [recentDirs, setRecentDirs] = useState<string[]>([]);

  const inputRef = useRef<HTMLTextAreaElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  const { profiles, fetchProfiles } = useProfilesStore();
  const { spawnSession } = useSessionsStore();

  // Listen for Tauri event from global shortcut
  useEffect(() => {
    const unlisten = listen("open-quick-prompt", () => setVisible((v) => !v));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Escape key closes overlay
  useEffect(() => {
    if (!visible) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setVisible(false);
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [visible]);

  // Autofocus and refresh recent dirs when opened
  useEffect(() => {
    if (visible) {
      setRecentDirs(getRecentDirs());
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [visible]);

  // Fetch profiles on mount
  useEffect(() => {
    fetchProfiles();
  }, []);

  // Default selected profile when profiles load
  useEffect(() => {
    if (profiles.length > 0 && !selectedProfile) {
      const active = profiles.find((p) => p.is_active);
      setSelectedProfile(active?.name ?? profiles[0].name);
    }
  }, [profiles]);

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (modalRef.current && !modalRef.current.contains(e.target as Node)) {
      setVisible(false);
    }
  };

  const handleBrowse = async () => {
    const selected = await openDialog({ directory: true });
    if (typeof selected === "string") {
      setDirectory(selected);
    }
  };

  const handleSubmit = async () => {
    if (!prompt.trim()) return;
    const dir = directory.trim() || ".";
    await spawnSession(prompt.trim(), dir, selectedProfile || undefined);
    if (dir !== ".") addRecentDir(dir);
    setPrompt("");
    setDirectory("");
    setVisible(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  if (!visible) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={handleBackdropClick}
    >
      <div
        ref={modalRef}
        className="w-full max-w-lg bg-zinc-800 border border-zinc-600 rounded-xl shadow-2xl p-6 flex flex-col gap-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-base font-semibold text-zinc-100">Quick Prompt</h2>

        {/* Prompt input */}
        <textarea
          ref={inputRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="What should Claude do?"
          rows={3}
          className="w-full rounded-lg bg-zinc-900 border border-zinc-600 text-zinc-100 placeholder-zinc-500 px-3 py-2 text-sm resize-none focus:outline-none focus:ring-2 focus:ring-blue-500"
        />

        {/* Profile + Directory row */}
        <div className="flex gap-2">
          <select
            value={selectedProfile}
            onChange={(e) => setSelectedProfile(e.target.value)}
            className="rounded-lg bg-zinc-900 border border-zinc-600 text-zinc-100 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 shrink-0"
          >
            {profiles.length === 0 && <option value="">default</option>}
            {profiles.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>

          <div className="flex flex-1 gap-1">
            <input
              type="text"
              value={directory}
              onChange={(e) => setDirectory(e.target.value)}
              placeholder="Directory (optional)"
              className="flex-1 rounded-lg bg-zinc-900 border border-zinc-600 text-zinc-100 placeholder-zinc-500 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button
              onClick={handleBrowse}
              className="px-3 py-2 rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 text-sm transition-colors shrink-0"
            >
              Browse
            </button>
          </div>
        </div>

        {/* Recent directories */}
        {recentDirs.length > 0 && (
          <div className="flex flex-col gap-1">
            <span className="text-xs text-zinc-500">Recent directories</span>
            <div className="flex flex-wrap gap-1">
              {recentDirs.map((dir) => (
                <button
                  key={dir}
                  onClick={() => setDirectory(dir)}
                  className="px-2 py-0.5 rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-300 text-xs font-mono transition-colors"
                  title={dir}
                >
                  {dir.length > 40 ? `\u2026${dir.slice(-38)}` : dir}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Footer */}
        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={() => setVisible(false)}
            className="px-4 py-2 text-sm rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={!prompt.trim()}
            className="px-4 py-2 text-sm rounded-lg bg-blue-600 hover:bg-blue-500 text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Launch
          </button>
        </div>
      </div>
    </div>
  );
}

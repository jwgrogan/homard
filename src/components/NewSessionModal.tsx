import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionsStore } from "../lib/store";
import { useProfilesStore } from "../lib/store";

export interface NewSessionModalProps {
  open: boolean;
  onClose: () => void;
}

export default function NewSessionModal({ open: isOpen, onClose }: NewSessionModalProps) {
  const { spawnSession } = useSessionsStore();
  const { profiles, fetchProfiles } = useProfilesStore();

  const [directory, setDirectory] = useState("");
  const [selectedProfileName, setSelectedProfileName] = useState("");
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const overlayRef = useRef<HTMLDivElement>(null);

  // Fetch profiles when modal opens
  useEffect(() => {
    if (isOpen) {
      fetchProfiles();
      setError(null);
    }
  }, [isOpen]);

  // Set default selected profile once profiles load
  useEffect(() => {
    if (profiles.length > 0 && !selectedProfileName) {
      const active = profiles.find((p) => p.is_active);
      setSelectedProfileName(active?.name ?? profiles[0].name);
    }
  }, [profiles]);

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select Working Directory",
    });
    if (typeof selected === "string") {
      setDirectory(selected);
    }
  };

  const handleStart = async () => {
    if (!directory.trim()) {
      setError("Please select a working directory.");
      return;
    }
    if (!selectedProfileName) {
      setError("Please select a profile.");
      return;
    }
    const profile = profiles.find((p) => p.name === selectedProfileName);
    if (!profile) {
      setError("Selected profile not found.");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      await spawnSession(
        directory.trim(),
        profile.provider,
        profile.name,
        undefined,
        prompt.trim() || undefined
      );
      // Reset form
      setDirectory("");
      setPrompt("");
      setSelectedProfileName("");
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleOverlayClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.target === overlayRef.current) {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={handleOverlayClick}
    >
      <div className="bg-zinc-900 border border-zinc-700 rounded-xl shadow-xl w-full max-w-md mx-4 p-6 flex flex-col gap-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-zinc-100">New Session</h2>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-zinc-100 transition-colors text-xl leading-none"
            aria-label="Close"
          >
            &times;
          </button>
        </div>

        {/* Directory */}
        <div className="flex flex-col gap-1.5">
          <label className="text-sm font-medium text-zinc-300">
            Working Directory
          </label>
          <div className="flex gap-2">
            <input
              type="text"
              value={directory}
              onChange={(e) => setDirectory(e.target.value)}
              placeholder="/path/to/project"
              className="flex-1 px-3 py-2 bg-zinc-800 border border-zinc-600 rounded-lg text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-blue-500"
            />
            <button
              onClick={handleBrowse}
              className="px-3 py-2 bg-zinc-700 hover:bg-zinc-600 border border-zinc-600 rounded-lg text-sm text-zinc-200 transition-colors shrink-0"
            >
              Browse
            </button>
          </div>
        </div>

        {/* Profile */}
        <div className="flex flex-col gap-1.5">
          <label className="text-sm font-medium text-zinc-300">
            Profile / Provider
          </label>
          <select
            value={selectedProfileName}
            onChange={(e) => setSelectedProfileName(e.target.value)}
            className="px-3 py-2 bg-zinc-800 border border-zinc-600 rounded-lg text-sm text-zinc-100 focus:outline-none focus:border-blue-500"
          >
            {profiles.length === 0 && (
              <option value="" disabled>
                Loading profiles…
              </option>
            )}
            {profiles.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name} ({p.provider})
              </option>
            ))}
          </select>
        </div>

        {/* Prompt */}
        <div className="flex flex-col gap-1.5">
          <label className="text-sm font-medium text-zinc-300">
            Initial Prompt{" "}
            <span className="text-zinc-500 font-normal">(optional)</span>
          </label>
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="What would you like to work on?"
            rows={3}
            className="px-3 py-2 bg-zinc-800 border border-zinc-600 rounded-lg text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-blue-500 resize-none"
          />
        </div>

        {/* Error */}
        {error && (
          <p className="text-sm text-red-400 bg-red-900/20 border border-red-800 rounded-lg px-3 py-2">
            {error}
          </p>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            disabled={loading}
            className="px-4 py-2 text-sm rounded-lg bg-zinc-700 hover:bg-zinc-600 text-zinc-200 transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleStart}
            disabled={loading}
            className="px-4 py-2 text-sm rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors disabled:opacity-50"
          >
            {loading ? "Starting…" : "Start Session"}
          </button>
        </div>
      </div>
    </div>
  );
}

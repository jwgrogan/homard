import { useState } from "react";
import { useSettingsStore } from "../../lib/store";

const KNOWN_ENV_VARS: Record<string, { description: string; default?: string }> = {
  "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": {
    description: "Enable Agent Teams for multi-agent orchestration",
    default: "1",
  },
  "CLAUDE_CODE_MAX_TURNS": {
    description: "Maximum number of turns per agent session",
    default: "50",
  },
  "CLAUDE_CODE_MAX_OUTPUT_TOKENS": {
    description: "Maximum output tokens per response",
  },
  "CLAUDE_CODE_USE_BEDROCK": {
    description: "Use AWS Bedrock as the model provider",
  },
  "CLAUDE_CODE_USE_VERTEX": {
    description: "Use GCP Vertex AI as the model provider",
  },
  "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": {
    description: "Disable telemetry and non-essential network calls",
  },
};

function EyeIcon({ visible }: { visible: boolean }) {
  if (visible) {
    return (
      <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
      </svg>
    );
  }
  return (
    <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
    </svg>
  );
}

export default function EnvVarsPanel() {
  const { settings, setEnvVar, removeEnvVar } = useSettingsStore();
  const env = settings?.env ?? {};

  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  function toggleReveal(key: string) {
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  const knownDescription =
    newKey in KNOWN_ENV_VARS ? KNOWN_ENV_VARS[newKey].description : null;

  const suggestions = Object.keys(KNOWN_ENV_VARS).filter(
    (k) => k.toLowerCase().includes(newKey.toLowerCase()) && !(k in env) && newKey.length > 0
  );

  async function handleAdd() {
    const trimmedKey = newKey.trim();
    const trimmedValue = newValue.trim();
    if (!trimmedKey) { setError("Key is required"); return; }
    if (!trimmedValue) { setError("Value is required"); return; }
    setAdding(true);
    setError(null);
    try {
      await setEnvVar(trimmedKey, trimmedValue);
      setNewKey(""); setNewValue("");
    } catch (e) {
      setError(String(e));
    } finally {
      setAdding(false);
    }
  }

  const entries = Object.entries(env);

  return (
    <div>
      <div className="bg-zinc-800 border border-zinc-700 rounded overflow-hidden mb-4">
        {entries.length === 0 ? (
          <p className="px-4 py-3 text-sm text-zinc-500">No environment variables configured.</p>
        ) : (
          <table className="w-full text-sm">
            <thead className="border-b border-zinc-700">
              <tr>
                <th className="text-left px-4 py-2 text-xs text-zinc-400 font-medium">Key</th>
                <th className="text-left px-4 py-2 text-xs text-zinc-400 font-medium">Value</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-700">
              {entries.map(([key, value]) => (
                <tr key={key}>
                  <td className="px-4 py-2 font-mono text-zinc-200">{key}</td>
                  <td className="px-4 py-2 font-mono text-zinc-300">
                    <div className="flex items-center gap-2">
                      <span>{revealed.has(key) ? value : "••••••••"}</span>
                      <button
                        onClick={() => toggleReveal(key)}
                        className="text-zinc-500 hover:text-zinc-300"
                        title={revealed.has(key) ? "Hide" : "Reveal"}
                      >
                        <EyeIcon visible={revealed.has(key)} />
                      </button>
                    </div>
                  </td>
                  <td className="px-4 py-2 text-right">
                    <button
                      onClick={() => removeEnvVar(key)}
                      className="text-zinc-500 hover:text-red-400 text-xs px-2 py-0.5 rounded"
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="bg-zinc-800 border border-zinc-700 rounded p-4 space-y-3">
        <h3 className="text-sm font-medium text-zinc-200">Add Variable</h3>
        <div className="flex gap-2 relative">
          <div className="flex-1 relative">
            <input
              type="text"
              value={newKey}
              onChange={(e) => { setNewKey(e.target.value); setError(null); setShowSuggestions(true); }}
              onFocus={() => setShowSuggestions(true)}
              onBlur={() => setTimeout(() => setShowSuggestions(false), 150)}
              placeholder="KEY"
              className="w-full bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm font-mono placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
            />
            {showSuggestions && suggestions.length > 0 && (
              <ul className="absolute z-10 top-full mt-1 left-0 right-0 bg-zinc-800 border border-zinc-600 rounded shadow-lg">
                {suggestions.map((s) => (
                  <li
                    key={s}
                    onMouseDown={() => {
                      setNewKey(s);
                      const defaultVal = KNOWN_ENV_VARS[s].default;
                      if (defaultVal && !newValue) setNewValue(defaultVal);
                      setShowSuggestions(false);
                    }}
                    className="px-3 py-2 text-xs cursor-pointer hover:bg-zinc-700"
                  >
                    <span className="font-mono text-zinc-200">{s}</span>
                    <p className="text-zinc-500 mt-0.5">{KNOWN_ENV_VARS[s].description}</p>
                  </li>
                ))}
              </ul>
            )}
          </div>
          <input
            type="text"
            value={newValue}
            onChange={(e) => { setNewValue(e.target.value); setError(null); }}
            onKeyDown={(e) => e.key === "Enter" && handleAdd()}
            placeholder="value"
            className="flex-1 bg-zinc-900 border border-zinc-600 text-zinc-100 rounded px-3 py-2 text-sm placeholder-zinc-500 focus:outline-none focus:border-zinc-400"
          />
          <button
            onClick={handleAdd}
            disabled={adding}
            className="px-3 py-1.5 rounded text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50"
          >
            Add
          </button>
        </div>
        {knownDescription && (
          <p className="text-xs text-zinc-400">{knownDescription}</p>
        )}
        {error && <p className="text-xs text-red-400">{error}</p>}
      </div>
    </div>
  );
}

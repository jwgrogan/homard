import { useState } from "react";

type Page = "health" | "sessions" | "settings" | "scheduler";

export default function App() {
  const [page, setPage] = useState<Page>("health");

  return (
    <div className="flex h-screen bg-zinc-900 text-zinc-100">
      <nav className="w-48 border-r border-zinc-700 p-4 space-y-2">
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
      </nav>
      <main className="flex-1 p-6">
        <h1 className="text-xl font-semibold capitalize">{page}</h1>
        <p className="text-zinc-400 mt-2">Coming soon.</p>
      </main>
    </div>
  );
}

import { useState, useEffect } from "react";
import Chat from "./pages/Chat";
import Activity from "./pages/Activity";
import Settings from "./pages/Settings";
import { apiFetch } from "./lib/api";

type Tab = "chat" | "activity" | "settings";

const icons: Record<Tab, string> = {
  chat: "💬",
  activity: "📊",
  settings: "⚙️",
};

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [daemonOnline, setDaemonOnline] = useState(true);
  const [wide, setWide] = useState(window.innerWidth > 600);

  useEffect(() => {
    const check = () => apiFetch("/status")
      .then(r => { setDaemonOnline(r.ok); })
      .catch(() => setDaemonOnline(false));
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const onResize = () => setWide(window.innerWidth > 600);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const res = await apiFetch("/files/USER.md");
        const text = await res.text();
        const nameMatch = text.match(/[Nn]ame:\s*(.+)/);
        const name = nameMatch ? nameMatch[1].trim().split(" ")[0] : null;
        document.title = name && name.length > 1
          ? `Homard — ${name}'s personal crustacean 🦞`
          : "Homard — your personal crustacean 🦞";
      } catch { /* keep default */ }
    })();
  }, []);

  const tabs: { id: Tab; label: string }[] = [
    { id: "chat", label: "Chat" },
    { id: "activity", label: "Activity" },
    { id: "settings", label: "Settings" },
  ];

  if (wide) {
    // Wide layout: sidebar + content
    return (
      <div className="flex h-screen">
        {/* Sidebar */}
        <div
          className="w-48 shrink-0 flex flex-col border-r"
          style={{ borderColor: "var(--border)", background: "var(--sage)" }}
        >
          {/* Sidebar nav */}
          <div className="flex flex-col gap-0.5 p-2 mt-6">
            {tabs.map(({ id, label }) => (
              <button
                key={id}
                onClick={() => setTab(id)}
                className="flex items-center gap-2 px-2.5 py-1.5 rounded-md text-[12px] font-medium transition-all text-left"
                style={{
                  background: tab === id ? "rgba(255,255,255,0.7)" : "transparent",
                  color: tab === id ? "var(--navy)" : "var(--navy-muted)",
                }}
              >
                <span className="text-[14px]">{icons[id]}</span>
                {label}
              </button>
            ))}
          </div>

          {/* Status at bottom */}
          <div className="mt-auto p-2">
            {!daemonOnline && (
              <div className="text-[10px] px-2 py-1 rounded" style={{ background: "var(--error-bg)", color: "var(--error)" }}>
                Daemon offline
              </div>
            )}
            <div className="flex items-center gap-1.5 px-2 py-1 text-[10px]" style={{ color: "var(--navy-muted)" }}>
              <span className="w-1.5 h-1.5 rounded-full" style={{ background: daemonOnline ? "var(--success)" : "var(--error)" }} />
              {daemonOnline ? "Connected" : "Offline"}
            </div>
          </div>
        </div>

        {/* Content */}
        <main className="flex-1 overflow-hidden">
          {tab === "chat" && <Chat />}
          {tab === "activity" && <Activity />}
          {tab === "settings" && <Settings />}
        </main>
      </div>
    );
  }

  // Narrow layout: segmented control + content (phone-like)
  return (
    <div className="flex flex-col h-screen">
      {!daemonOnline && (
        <div className="px-3 py-1 text-[11px] text-center" style={{ background: "var(--error-bg)", color: "var(--error)" }}>
          Daemon offline
        </div>
      )}

      {/* Segmented control */}
      <div
        className="px-3 py-1.5 flex items-center justify-center border-b shrink-0"
        style={{ borderColor: "var(--border)" }}
      >
        <div
          className="flex gap-0.5 p-0.5 rounded-md"
          style={{ background: "rgba(27, 45, 79, 0.06)" }}
        >
          {tabs.map(({ id, label }) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className="flex items-center gap-1 px-2.5 py-1 rounded text-[11px] font-medium transition-all"
              style={{
                background: tab === id ? "white" : "transparent",
                color: tab === id ? "var(--navy)" : "var(--navy-muted)",
                boxShadow: tab === id ? "0 1px 3px rgba(0,0,0,0.08), 0 0 0 0.5px rgba(0,0,0,0.04)" : "none",
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <span
          className="w-2 h-2 rounded-full ml-2"
          style={{ background: daemonOnline ? "var(--success)" : "var(--error)" }}
        />
      </div>

      <main className="flex-1 overflow-hidden">
        {tab === "chat" && <Chat />}
        {tab === "activity" && <Activity />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}

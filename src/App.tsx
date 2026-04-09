import { useState, useEffect } from "react";
import Chat from "./pages/Chat";
import Activity from "./pages/Activity";
import Settings from "./pages/Settings";

type Tab = "chat" | "activity" | "settings";

const ChatIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
  </svg>
);

const ActivityIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
  </svg>
);

const SettingsIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
  </svg>
);

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [daemonOnline, setDaemonOnline] = useState(true);

  // Poll daemon status
  useEffect(() => {
    const check = () => fetch("http://localhost:17700/status")
      .then(() => setDaemonOnline(true))
      .catch(() => setDaemonOnline(false));
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--cream)" }}>
      {/* Daemon offline banner */}
      {!daemonOnline && (
        <div className="px-3 py-1.5 text-xs text-center" style={{ background: "var(--error-bg)", color: "var(--error)" }}>
          Daemon offline — run <code className="font-mono">homard serve</code>
        </div>
      )}

      {/* Top nav bar with segmented control */}
      <div
        className="px-4 py-2.5 flex items-center gap-3 border-b"
        style={{ borderColor: "var(--border)", background: "var(--sage)", WebkitAppRegion: "drag" } as React.CSSProperties}
      >
        {/* Brand mark */}
        <span className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold shrink-0" style={{ background: "var(--coral)", color: "white" }}>H</span>

        {/* Segmented control */}
        <div className="flex rounded-lg overflow-hidden" style={{ background: "var(--cream-card)", border: "1px solid var(--border)", WebkitAppRegion: "no-drag" } as React.CSSProperties}>
          {([
            { id: "chat" as Tab, label: "Chat", Icon: ChatIcon },
            { id: "activity" as Tab, label: "Activity", Icon: ActivityIcon },
            { id: "settings" as Tab, label: "Settings", Icon: SettingsIcon },
          ]).map(({ id, label, Icon }) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium transition-all"
              style={{
                background: tab === id ? "var(--navy)" : "transparent",
                color: tab === id ? "white" : "var(--navy-muted)",
              }}
            >
              <Icon />
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Main content */}
      <main className="flex-1 overflow-hidden">
        {tab === "chat" && <Chat />}
        {tab === "activity" && <Activity />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}

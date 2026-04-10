import { useState, useEffect } from "react";
import Chat from "./pages/Chat";
import Activity from "./pages/Activity";
import Settings from "./pages/Settings";
import { apiFetch } from "./lib/api";

type Tab = "chat" | "activity" | "settings";

const ChatIcon = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
  </svg>
);

const ActivityIcon = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
  </svg>
);

const SettingsIcon = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
  </svg>
);

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [daemonOnline, setDaemonOnline] = useState(true);

  useEffect(() => {
    const check = () => apiFetch("/status")
      .then(r => { setDaemonOnline(r.ok); })
      .catch(() => setDaemonOnline(false));
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  const tabs = [
    { id: "chat" as Tab, label: "Chat", Icon: ChatIcon },
    { id: "activity" as Tab, label: "Activity", Icon: ActivityIcon },
    { id: "settings" as Tab, label: "Settings", Icon: SettingsIcon },
  ];

  return (
    <div className="flex flex-col h-screen">
      {/* Segmented control below native title bar */}
      <div
        className="px-3 py-1.5 flex items-center justify-center border-b shrink-0"
        style={{ borderColor: "var(--border)" }}
      >
        {/* Segmented control */}
        <div
          className="flex gap-0.5 p-0.5 rounded-md"
          style={{ background: "rgba(27, 45, 79, 0.06)" }}
        >
          {tabs.map(({ id, label, Icon }) => (
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
              <Icon />
              {label}
            </button>
          ))}
        </div>

        {/* Status dot */}
        <span
          className="w-2 h-2 rounded-full ml-2"
          style={{ background: daemonOnline ? "var(--success)" : "var(--error)" }}
        />
      </div>

      {/* Offline banner */}
      {!daemonOnline && (
        <div className="px-3 py-1 text-[11px] text-center" style={{ background: "var(--error-bg)", color: "var(--error)" }}>
          Daemon offline — <code className="font-mono">homard serve</code>
        </div>
      )}

      {/* Content */}
      <main className="flex-1 overflow-hidden">
        {tab === "chat" && <Chat />}
        {tab === "activity" && <Activity />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}

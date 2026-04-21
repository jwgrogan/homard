import { useEffect, useState } from "react";
import Chat from "./pages/Chat";
import Activity from "./pages/Activity";
import Settings from "./pages/Settings";
import { apiFetch } from "./lib/api";

type Tab = "chat" | "activity" | "settings";

const tabs: { id: Tab; label: string; description: string }[] = [
  { id: "chat", label: "Chat", description: "Talk to Homard" },
  { id: "activity", label: "Activity", description: "Runs and sessions" },
  { id: "settings", label: "Settings", description: "Providers and system" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [daemonOnline, setDaemonOnline] = useState(true);
  const [userName, setUserName] = useState<string | null>(null);

  useEffect(() => {
    const check = () =>
      apiFetch("/status")
        .then((r) => setDaemonOnline(r.ok))
        .catch(() => setDaemonOnline(false));
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const res = await apiFetch("/files/USER.md");
        const text = await res.text();
        const nameMatch = text.match(/[Nn]ame:\s*(.+)/);
        const name = nameMatch ? nameMatch[1].trim().split(" ")[0] : null;
        setUserName(name);
      } catch {
        setUserName(null);
      }
      document.title = "Homard - your personal crustacean 🦞";
    })();
  }, []);

  const activeTab = tabs.find((item) => item.id === tab)!;

  return (
    <div className="tray-shell">
      <header className="tray-header">
        <div className="tray-header__copy">
          <div className="topbar__eyebrow">Homard</div>
          <h1 className="tray-header__title">Homard - your personal crustacean 🦞</h1>
          <p className="tray-header__subtitle">
            {userName ? `${userName} · ${activeTab.description}` : activeTab.description}
          </p>
        </div>
        <span className={`status-chip ${daemonOnline ? "is-online" : "is-offline"}`}>
          <span className={`status-dot ${daemonOnline ? "is-online" : "is-offline"}`} />
          <span>{daemonOnline ? "Live" : "Offline"}</span>
        </span>
      </header>

      <nav className="segmented page-tabs" aria-label="Primary">
        {tabs.map((item) => (
          <button key={item.id} onClick={() => setTab(item.id)} className={tab === item.id ? "is-active" : ""}>
            {item.label}
          </button>
        ))}
      </nav>

      <main className="tray-page">
        {tab === "chat" && <Chat />}
        {tab === "activity" && <Activity />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}

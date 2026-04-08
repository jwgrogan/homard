import { useState } from "react";
import Chat from "./pages/Chat";
import Activity from "./pages/Activity";
import Settings from "./pages/Settings";

type Tab = "chat" | "activity" | "settings";

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--cream)" }}>
      {/* Main content */}
      <main className="flex-1 overflow-hidden">
        {tab === "chat" && <Chat />}
        {tab === "activity" && <Activity />}
        {tab === "settings" && <Settings />}
      </main>

      {/* Bottom tab bar */}
      <nav
        className="flex items-center justify-around py-2 px-4 border-t"
        style={{ borderColor: "var(--border)", background: "var(--sage)" }}
      >
        {([
          { id: "chat" as Tab, label: "Chat", icon: "\u{1F4AC}" },
          { id: "activity" as Tab, label: "Activity", icon: "\u{1F4CB}" },
          { id: "settings" as Tab, label: "Settings", icon: "\u2699\uFE0F" },
        ]).map(({ id, label, icon }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className="flex flex-col items-center gap-0.5 px-4 py-1 rounded-lg transition-colors text-xs"
            style={{
              color: tab === id ? "var(--coral)" : "var(--navy-muted)",
              fontWeight: tab === id ? 600 : 400,
            }}
          >
            <span className="text-base">{icon}</span>
            {label}
          </button>
        ))}
      </nav>
    </div>
  );
}

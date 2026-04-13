import { useEffect, useRef, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { sendChat, getStatus, apiFetch, type ChatMessage } from "../lib/api";

function MessageItem({
  msg,
  botLabel,
  userInitial,
}: {
  msg: ChatMessage;
  botLabel: string;
  userInitial: string;
}) {
  const isUser = msg.role === "user";
  const isSystem = msg.role === "system";
  if (isSystem) return null;

  return (
    <div className={`flex flex-col gap-1 ${isUser ? "items-end" : "items-start"}`}>
      <div className="pill" style={{ background: isUser ? "rgba(22, 48, 75, 0.06)" : "var(--accent-soft)" }}>
        <span>{isUser ? userInitial : botLabel}</span>
        <span>{isUser ? "You" : "Homard"}</span>
      </div>
      <div
        className="max-w-[92%] rounded-[18px] border px-4 py-3 text-[13px] leading-relaxed"
        style={{
          background: isUser ? "rgba(22, 48, 75, 0.05)" : "rgba(255, 255, 255, 0.76)",
          borderColor: "var(--line)",
        }}
      >
        {isUser ? (
          <div>{msg.content}</div>
        ) : (
          <div
            className="prose max-w-none"
            dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(marked.parse(msg.content, { breaks: true }) as string) }}
          />
        )}
        {msg.tool_calls && msg.tool_calls.length > 0 && (
          <div className="mt-2 text-[10px]" style={{ color: "var(--ink-soft)" }}>
            {msg.tool_calls.map((tc) => tc.name).join(", ")}
          </div>
        )}
      </div>
    </div>
  );
}

export default function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasProvider, setHasProvider] = useState<boolean | null>(null);
  const [channel, setChannel] = useState("chat");
  const [channels, setChannels] = useState<string[]>(["chat"]);
  const [botLabel, setBotLabel] = useState("HM");
  const [userInitial, setUserInitial] = useState("Y");
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    getStatus().then((s) => setHasProvider(s?.active_provider != null));
    apiFetch("/files/IDENTITY.md")
      .then((r) => r.text())
      .then((text) => {
        const nameMatch = text.match(/name:\s*(.+)/);
        const value = nameMatch ? nameMatch[1].trim().slice(0, 2).toUpperCase() : "HM";
        setBotLabel(value);
      })
      .catch(() => {});
    apiFetch("/files/USER.md")
      .then((r) => r.text())
      .then((text) => {
        const nameMatch = text.match(/[Nn]ame:\s*(.+)/);
        if (nameMatch) {
          const first = nameMatch[1].trim()[0];
          if (first) setUserInitial(first.toUpperCase());
        }
      })
      .catch(() => {});
    apiFetch("/conversations")
      .then((r) => r.json())
      .then((list: string[]) => {
        if (list.length > 0) setChannels(list);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    apiFetch(`/conversations/${channel}?limit=50`)
      .then((r) => r.json())
      .then((msgs: ChatMessage[]) => {
        setMessages(Array.isArray(msgs) ? msgs : []);
      })
      .catch(() => setMessages([]));
  }, [channel]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  useEffect(() => {
    const poll = setInterval(() => {
      apiFetch(`/conversations/${channel}?limit=50`)
        .then((r) => r.json())
        .then((msgs: ChatMessage[]) => {
          if (Array.isArray(msgs) && msgs.length > messages.length) {
            setMessages(msgs);
          }
        })
        .catch(() => {});
    }, 3000);
    return () => clearInterval(poll);
  }, [channel, messages.length]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    const userMsg: ChatMessage = { role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const { response } = await sendChat(text, channel);
      setMessages((prev) => [...prev, { role: "assistant", content: response }]);
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: `Error: ${e instanceof Error ? e.message : "Connection failed"}`,
        },
      ]);
    } finally {
      setLoading(false);
      inputRef.current?.focus();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const telegramChannels = channels.filter((c) => c.startsWith("telegram_"));
  const allChannels = ["chat", ...telegramChannels];

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div>
          <div className="subtle-label">Conversation</div>
          <h2 className="section-title">{channel === "chat" ? "Local thread" : "Telegram relay"}</h2>
          <p className="section-meta">
            {hasProvider === false ? "Connect a provider in Settings before sending a message." : "One thread, one input, no extra chrome."}
          </p>
        </div>
      </div>

      {allChannels.length > 1 && (
        <div className="px-4 pt-3">
          <div className="segmented">
            {allChannels.map((item, index) => (
              <button key={item} onClick={() => setChannel(item)} className={channel === item ? "is-active" : ""}>
                {item === "chat" ? "Chat" : `Telegram ${index}`}
              </button>
            ))}
          </div>
        </div>
      )}

      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 py-4">
        <div className="flex flex-col gap-4">
          {messages.length === 0 && !loading && (
            <div className="empty-state">
              <p>{hasProvider === false ? "Connect a model provider in Settings to start chatting." : "No messages yet."}</p>
            </div>
          )}
          {messages.map((msg, i) => (
            <MessageItem key={`${msg.role}-${msg.timestamp || i}`} msg={msg} botLabel={botLabel} userInitial={userInitial} />
          ))}
          {loading && (
            <div className="flex flex-col gap-1 items-start">
              <div className="pill" style={{ background: "var(--accent-soft)" }}>
                <span>{botLabel}</span>
                <span>Homard</span>
              </div>
              <div className="rounded-[18px] border px-4 py-3" style={{ background: "rgba(255,255,255,0.76)", borderColor: "var(--line)" }}>
                <div className="flex gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--ink-soft)", animationDelay: "0ms" }} />
                  <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--ink-soft)", animationDelay: "150ms" }} />
                  <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--ink-soft)", animationDelay: "300ms" }} />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="border-t px-4 py-4" style={{ borderColor: "var(--line)" }}>
        <div className="flex flex-col gap-3">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Message Homard"
            rows={3}
            className="field resize-none"
            style={{ minHeight: "92px", maxHeight: "180px" }}
          />
          <div className="flex items-center justify-between gap-3">
            <p className="section-meta">Enter sends. Shift+Enter adds a line break.</p>
            <button onClick={handleSend} disabled={!input.trim() || loading} className="cta disabled:opacity-40">
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

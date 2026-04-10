import { useEffect, useRef, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { sendChat, getStatus, apiFetch, type ChatMessage } from "../lib/api";

function Avatar({ emoji, isUser }: { emoji: string; isUser: boolean }) {
  return (
    <div
      className="w-7 h-7 rounded-full flex items-center justify-center text-[14px] shrink-0"
      style={{
        background: isUser ? "var(--sage)" : "var(--coral)",
      }}
    >
      {emoji}
    </div>
  );
}

function MessageBubble({ msg, botEmoji, userInitial }: { msg: ChatMessage; botEmoji: string; userInitial: string }) {
  const isUser = msg.role === "user";
  const isSystem = msg.role === "system";
  if (isSystem) return null;

  return (
    <div className={`flex gap-2 ${isUser ? "flex-row-reverse" : "flex-row"} mb-2 items-end`}>
      <Avatar emoji={isUser ? userInitial : botEmoji} isUser={isUser} />
      <div
        className="max-w-[75%] px-3 py-2 text-[13px] leading-relaxed"
        style={{
          background: isUser ? "var(--sage)" : "var(--cream-card)",
          color: "var(--navy)",
          borderRadius: isUser ? "16px 16px 4px 16px" : "16px 16px 16px 4px",
          border: isUser ? "none" : "0.5px solid var(--border)",
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
          <div className="mt-1 text-[10px] opacity-60">
            {msg.tool_calls.map(tc => tc.name).join(", ")}
          </div>
        )}
      </div>
    </div>
  );
}

function ChannelPill({ label, active, count, onClick }: { label: string; active: boolean; count?: number; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium transition-all"
      style={{
        background: active ? "var(--coral)" : "rgba(27, 45, 79, 0.06)",
        color: active ? "white" : "var(--navy-muted)",
      }}
    >
      {label}
      {count !== undefined && count > 0 && (
        <span className="text-[9px] opacity-70">({count})</span>
      )}
    </button>
  );
}

export default function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasProvider, setHasProvider] = useState<boolean | null>(null);
  const [channel, setChannel] = useState("chat");
  const [channels, setChannels] = useState<string[]>(["chat"]);
  const [botEmoji, setBotEmoji] = useState("🦞");
  const [userInitial, setUserInitial] = useState("Y");
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Load identity + channels + messages
  useEffect(() => {
    getStatus().then(s => setHasProvider(s?.active_provider != null));
    // Load bot emoji from IDENTITY.md
    apiFetch("/files/IDENTITY.md").then(r => r.text()).then(text => {
      const emojiMatch = text.match(/emoji:\s*(.+)/);
      if (emojiMatch) setBotEmoji(emojiMatch[1].trim());
    }).catch(() => {});
    // Load user initial from USER.md
    apiFetch("/files/USER.md").then(r => r.text()).then(text => {
      const nameMatch = text.match(/[Nn]ame:\s*(.+)/);
      if (nameMatch) {
        const first = nameMatch[1].trim()[0];
        if (first) setUserInitial(first.toUpperCase());
      }
    }).catch(() => {});
    // Discover channels from conversations
    apiFetch("/conversations").then(r => r.json()).then((list: string[]) => {
      if (list.length > 0) setChannels(list);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    apiFetch(`/conversations/${channel}?limit=50`).then(r => r.json()).then((msgs: ChatMessage[]) => {
      setMessages(Array.isArray(msgs) ? msgs : []);
    }).catch(() => setMessages([]));
  }, [channel]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  // Poll for new messages (Telegram messages appear here too)
  useEffect(() => {
    const poll = setInterval(() => {
      apiFetch(`/conversations/${channel}?limit=50`).then(r => r.json()).then((msgs: ChatMessage[]) => {
        if (Array.isArray(msgs) && msgs.length > messages.length) {
          setMessages(msgs);
        }
      }).catch(() => {});
    }, 3000);
    return () => clearInterval(poll);
  }, [channel, messages.length]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    const userMsg: ChatMessage = { role: "user", content: text };
    setMessages(prev => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const { response } = await sendChat(text, channel);
      setMessages(prev => [...prev, { role: "assistant", content: response }]);
    } catch (e) {
      setMessages(prev => [...prev, {
        role: "assistant",
        content: `Error: ${e instanceof Error ? e.message : "Connection failed"}`,
      }]);
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

  const telegramChannels = channels.filter(c => c.startsWith("telegram_"));
  const hasMultipleChannels = channels.length > 1;

  return (
    <div className="flex flex-col h-full">
      {/* Channel selector — only show if there are Telegram channels */}
      {hasMultipleChannels && (
        <div className="flex gap-1 px-3 py-1.5 overflow-x-auto" style={{ borderBottom: "0.5px solid var(--border)" }}>
          <ChannelPill label="Chat" active={channel === "chat"} onClick={() => setChannel("chat")} />
          {telegramChannels.map(c => (
            <ChannelPill
              key={c}
              label={`Telegram`}
              active={channel === c}
              onClick={() => setChannel(c)}
            />
          ))}
        </div>
      )}

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-2">
        {messages.length === 0 && !loading && (
          <div className="px-2 py-4">
            <p className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
              {hasProvider === false ? "No provider configured. Go to Settings → Providers." : "No messages yet."}
            </p>
          </div>
        )}
        {messages.map((msg, i) => (
          <MessageBubble key={`${msg.role}-${msg.timestamp || i}`} msg={msg} botEmoji={botEmoji} userInitial={userInitial} />
        ))}
        {loading && (
          <div className="flex gap-2 items-end mb-2">
            <Avatar emoji={botEmoji} isUser={false} />
            <div
              className="px-3 py-2 text-[13px]"
              style={{
                background: "var(--cream-card)",
                border: "0.5px solid var(--border)",
                borderRadius: "16px 16px 16px 4px",
              }}
            >
              <div className="flex gap-1">
                <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "0ms" }} />
                <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "150ms" }} />
                <span className="w-1.5 h-1.5 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "300ms" }} />
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Input */}
      <div className="px-3 py-2 border-t" style={{ borderColor: "var(--border)" }}>
        <div className="flex items-end gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Message..."
            rows={1}
            className="flex-1 text-[13px] resize-none outline-none px-3 py-1.5 rounded-2xl"
            style={{
              background: "white",
              color: "var(--navy)",
              border: "0.5px solid var(--border)",
              maxHeight: "72px",
            }}
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || loading}
            className="p-1.5 rounded-full transition-all disabled:opacity-20 shrink-0"
            style={{ background: "var(--coral)", color: "white" }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
              <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/>
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}

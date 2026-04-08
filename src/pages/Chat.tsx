import { useEffect, useRef, useState } from "react";
import { sendChat, getConversation, type ChatMessage } from "../lib/api";

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";
  const isSystem = msg.role === "system";
  if (isSystem) return null;

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"} mb-3`}>
      <div
        className="max-w-[85%] px-3 py-2 rounded-2xl text-sm leading-relaxed"
        style={{
          background: isUser ? "var(--coral)" : "var(--cream-card)",
          color: isUser ? "white" : "var(--navy)",
          border: isUser ? "none" : "1px solid var(--border)",
          borderRadius: isUser ? "18px 18px 4px 18px" : "18px 18px 18px 4px",
        }}
      >
        <div className="whitespace-pre-wrap">{msg.content}</div>
        {msg.tool_calls && msg.tool_calls.length > 0 && (
          <div className="mt-1 text-xs opacity-70">
            {"\u{1F527}"} {msg.tool_calls.map((tc) => tc.name).join(", ")}
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
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    getConversation("chat", 30).then(setMessages);
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    const userMsg: ChatMessage = { role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const { response } = await sendChat(text);
      const assistantMsg: ChatMessage = { role: "assistant", content: response };
      setMessages((prev) => [...prev, assistantMsg]);
    } catch (e) {
      const errMsg: ChatMessage = {
        role: "assistant",
        content: `Error: ${e instanceof Error ? e.message : "Connection failed. Is the daemon running?"}`,
      };
      setMessages((prev) => [...prev, errMsg]);
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

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div
        className="px-4 py-3 border-b flex items-center gap-2"
        style={{ borderColor: "var(--border)", background: "var(--sage)" }}
      >
        <span className="text-lg">{"\u{1F99E}"}</span>
        <span className="font-semibold text-sm" style={{ color: "var(--navy)" }}>
          Homard
        </span>
        {loading && (
          <span className="text-xs ml-auto" style={{ color: "var(--navy-muted)" }}>
            thinking...
          </span>
        )}
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 py-3">
        {messages.length === 0 && !loading && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-center">
            <span className="text-4xl">{"\u{1F99E}"}</span>
            <p className="text-sm" style={{ color: "var(--navy-muted)" }}>
              Hey there. What can I help with?
            </p>
          </div>
        )}
        {messages.map((msg, i) => (
          <MessageBubble key={`${msg.role}-${msg.timestamp || i}`} msg={msg} />
        ))}
        {loading && (
          <div className="flex justify-start mb-3">
            <div
              className="px-3 py-2 rounded-2xl text-sm"
              style={{
                background: "var(--cream-card)",
                border: "1px solid var(--border)",
                borderRadius: "18px 18px 18px 4px",
              }}
            >
              <span className="animate-pulse" style={{ color: "var(--navy-muted)" }}>
                {"\u00B7\u00B7\u00B7"}
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Input */}
      <div className="px-3 pb-3 pt-1">
        <div
          className="flex items-end gap-2 rounded-2xl px-3 py-2"
          style={{ background: "var(--cream-card)", border: "1px solid var(--border)" }}
        >
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Message Homard..."
            rows={1}
            className="flex-1 bg-transparent text-sm resize-none outline-none placeholder:text-[var(--navy-muted)]"
            style={{ color: "var(--navy)", maxHeight: "80px" }}
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || loading}
            className="p-1.5 rounded-full transition-colors disabled:opacity-30"
            style={{ background: "var(--coral)", color: "white" }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}

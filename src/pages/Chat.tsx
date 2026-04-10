import { useEffect, useRef, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { sendChat, getConversation, getStatus, type ChatMessage } from "../lib/api";

function Message({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";
  const isSystem = msg.role === "system";
  if (isSystem) return null;

  return (
    <div
      className="px-4 py-2.5"
      style={{
        background: isUser ? "transparent" : "rgba(232, 240, 236, 0.4)",
        borderBottom: "0.5px solid var(--border)",
      }}
    >
      {/* Role label */}
      <div className="flex items-center gap-1.5 mb-1">
        <span
          className="w-4 h-4 rounded flex items-center justify-center text-[9px] font-bold"
          style={{
            background: isUser ? "var(--navy)" : "var(--coral)",
            color: "white",
          }}
        >
          {isUser ? "Y" : "H"}
        </span>
        <span className="text-[11px] font-medium" style={{ color: "var(--navy-muted)" }}>
          {isUser ? "You" : "Homard"}
        </span>
        {msg.tool_calls && msg.tool_calls.length > 0 && (
          <span className="text-[10px] ml-auto" style={{ color: "var(--navy-muted)" }}>
            tools: {msg.tool_calls.map(tc => tc.name).join(", ")}
          </span>
        )}
      </div>
      {/* Content */}
      {isUser ? (
        <div className="text-[13px] leading-relaxed" style={{ color: "var(--navy)" }}>
          {msg.content}
        </div>
      ) : (
        <div
          className="prose max-w-none"
          dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(marked.parse(msg.content, { breaks: true }) as string) }}
        />
      )}
    </div>
  );
}

export default function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasProvider, setHasProvider] = useState<boolean | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    getConversation("chat", 30).then(setMessages);
    getStatus().then(s => setHasProvider(s?.active_provider != null));
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
    setMessages(prev => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const { response } = await sendChat(text);
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

  return (
    <div className="flex flex-col h-full">
      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        {messages.length === 0 && !loading && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-center px-8">
            {hasProvider === false ? (
              <>
                <p className="text-[13px] font-medium" style={{ color: "var(--navy)" }}>
                  No provider configured
                </p>
                <p className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
                  Go to Settings &rarr; Providers
                </p>
              </>
            ) : (
              <p className="text-[13px]" style={{ color: "var(--navy-muted)" }}>
                Start a conversation
              </p>
            )}
          </div>
        )}
        {messages.map((msg, i) => (
          <Message key={`${msg.role}-${msg.timestamp || i}`} msg={msg} />
        ))}
        {loading && (
          <div className="px-4 py-2.5" style={{ background: "rgba(232, 240, 236, 0.4)", borderBottom: "0.5px solid var(--border)" }}>
            <div className="flex items-center gap-1.5 mb-1">
              <span className="w-4 h-4 rounded flex items-center justify-center text-[9px] font-bold" style={{ background: "var(--coral)", color: "white" }}>H</span>
              <span className="text-[11px] font-medium" style={{ color: "var(--navy-muted)" }}>Homard</span>
            </div>
            <span className="text-[13px] animate-pulse" style={{ color: "var(--navy-muted)" }}>Thinking...</span>
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
            className="flex-1 text-[13px] resize-none outline-none px-2.5 py-1.5 rounded-md"
            style={{
              background: "var(--cream-card)",
              color: "var(--navy)",
              border: "0.5px solid var(--border)",
              maxHeight: "80px",
            }}
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || loading}
            className="p-1.5 rounded-md transition-all disabled:opacity-25"
            style={{ background: "var(--coral)", color: "white" }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <path d="M5 12h14M12 5l7 7-7 7"/>
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}

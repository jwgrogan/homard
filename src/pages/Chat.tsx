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
      <div className="flex items-center gap-1.5 mb-0.5">
        <span className="text-[11px] font-semibold" style={{ color: isUser ? "var(--navy)" : "var(--coral)" }}>
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
          <div className="px-4 py-6">
            <p className="text-[11px]" style={{ color: "var(--navy-muted)" }}>
              {hasProvider === false ? "No provider configured. Go to Settings → Providers." : "No messages yet."}
            </p>
          </div>
        )}
        {messages.map((msg, i) => (
          <Message key={`${msg.role}-${msg.timestamp || i}`} msg={msg} />
        ))}
        {loading && (
          <div className="px-4 py-2.5" style={{ background: "rgba(232, 240, 236, 0.4)", borderBottom: "0.5px solid var(--border)" }}>
            <span className="text-[11px] font-semibold" style={{ color: "var(--coral)" }}>Homard</span>
            <div className="mt-0.5 flex gap-1">
              <span className="w-1 h-1 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "0ms" }} />
              <span className="w-1 h-1 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "150ms" }} />
              <span className="w-1 h-1 rounded-full animate-bounce" style={{ background: "var(--navy-muted)", animationDelay: "300ms" }} />
            </div>
          </div>
        )}
      </div>

      {/* Input */}
      <div className="px-3 py-1.5 border-t" style={{ borderColor: "var(--border)", background: "rgba(232, 240, 236, 0.3)" }}>
        <textarea
          ref={inputRef}
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Message — press Return to send"
          rows={1}
          className="w-full text-[13px] resize-none outline-none px-2 py-1.5 rounded"
          style={{
            background: "white",
            color: "var(--navy)",
            border: "0.5px solid var(--border)",
            maxHeight: "72px",
          }}
        />
      </div>
    </div>
  );
}

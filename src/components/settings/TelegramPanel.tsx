import { useEffect, useState } from "react";
import {
  addPairedChat,
  removePairedChat,
  saveTelegramToken,
  startTelegramPolling,
  stopTelegramPolling,
  verifyTelegramToken,
} from "../../lib/tauri";
import { useTelegramStore } from "../../lib/store";

export function TelegramPanel() {
  const { status, loading, pairingCode, fetchStatus, refreshPairingCode } =
    useTelegramStore();
  const [tokenInput, setTokenInput] = useState("");
  const [verifying, setVerifying] = useState(false);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [newChatId, setNewChatId] = useState("");

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  async function handleVerifyAndSave() {
    if (!tokenInput.trim()) return;
    setVerifying(true);
    setVerifyError(null);
    try {
      await verifyTelegramToken(tokenInput.trim());
      await saveTelegramToken(tokenInput.trim());
      setTokenInput("");
      await fetchStatus();
    } catch (e) {
      setVerifyError(String(e));
    } finally {
      setVerifying(false);
    }
  }

  async function handleTogglePolling() {
    if (!status) return;
    try {
      if (status.is_polling) {
        await stopTelegramPolling();
      } else {
        await startTelegramPolling();
      }
      await fetchStatus();
    } catch (e) {
      console.error("Toggle polling error:", e);
    }
  }

  async function handleAddChatId() {
    if (!newChatId.trim()) return;
    try {
      await addPairedChat(newChatId.trim());
      setNewChatId("");
      await fetchStatus();
    } catch (e) {
      console.error("Add chat error:", e);
    }
  }

  async function handleRemoveChatId(chatId: string) {
    try {
      await removePairedChat(chatId);
      await fetchStatus();
    } catch (e) {
      console.error("Remove chat error:", e);
    }
  }

  if (loading && !status) {
    return <div className="p-4 text-sm text-gray-400">Loading...</div>;
  }

  const isConfigured = status?.enabled && status?.bot_username;

  return (
    <div className="space-y-6 p-4">
      {/* Status header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-white">Telegram Bridge</h2>
          <p className="text-xs text-gray-400 mt-0.5">
            Receive job results and control arcctl from your phone.
          </p>
        </div>
        {isConfigured && (
          <div className="flex items-center gap-2">
            <span
              className={`h-2 w-2 rounded-full ${status?.is_polling ? "bg-green-400" : "bg-gray-500"}`}
            />
            <span className="text-xs text-gray-400">
              {status?.is_polling ? "Polling" : "Idle"}
            </span>
          </div>
        )}
      </div>

      {/* Onboarding — not yet configured */}
      {!isConfigured && (
        <div className="space-y-4">
          <div className="rounded-lg border border-gray-700 bg-gray-800/50 p-4 space-y-3">
            <h3 className="text-xs font-medium text-gray-300">Setup</h3>
            <ol className="text-xs text-gray-400 space-y-1 list-decimal list-inside">
              <li>Open Telegram and message <code className="text-blue-400">@BotFather</code></li>
              <li>Send <code className="text-blue-400">/newbot</code> and follow the prompts</li>
              <li>Copy the bot token BotFather gives you</li>
              <li>Paste it below and click Verify</li>
            </ol>
            <div className="flex gap-2 mt-3">
              <input
                type="password"
                value={tokenInput}
                onChange={(e) => setTokenInput(e.target.value)}
                placeholder="123456:ABC-DEF1234..."
                className="flex-1 rounded bg-gray-900 border border-gray-700 px-3 py-1.5 text-xs text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                onKeyDown={(e) => e.key === "Enter" && handleVerifyAndSave()}
              />
              <button
                onClick={handleVerifyAndSave}
                disabled={verifying || !tokenInput.trim()}
                className="rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-50 px-3 py-1.5 text-xs text-white font-medium"
              >
                {verifying ? "Verifying..." : "Verify & Save"}
              </button>
            </div>
            {verifyError && (
              <p className="text-xs text-red-400">{verifyError}</p>
            )}
          </div>
        </div>
      )}

      {/* Configured state */}
      {isConfigured && (
        <>
          {/* Bot info + polling toggle */}
          <div className="rounded-lg border border-gray-700 bg-gray-800/50 p-4 flex items-center justify-between">
            <div>
              <p className="text-xs font-medium text-white">
                @{status?.bot_username}
              </p>
              <p className="text-xs text-gray-400">
                {status?.is_polling
                  ? "Polling for inbound messages"
                  : "Not polling — start to receive commands"}
              </p>
            </div>
            <button
              onClick={handleTogglePolling}
              className={`rounded px-3 py-1.5 text-xs font-medium ${
                status?.is_polling
                  ? "bg-gray-700 hover:bg-gray-600 text-gray-300"
                  : "bg-green-700 hover:bg-green-600 text-white"
              }`}
            >
              {status?.is_polling ? "Stop Polling" : "Start Polling"}
            </button>
          </div>

          {/* Pairing section */}
          <div className="space-y-3">
            <h3 className="text-xs font-medium text-gray-300">Paired Chats</h3>
            <p className="text-xs text-gray-400">
              Only paired Telegram chats can control arcctl. To pair, generate a
              code and send <code className="text-blue-400">/pair {"<code>"}</code> to your bot.
            </p>

            {/* Generate pairing code */}
            <div className="flex items-center gap-3">
              <button
                onClick={refreshPairingCode}
                className="rounded bg-gray-700 hover:bg-gray-600 px-3 py-1.5 text-xs text-white"
              >
                Generate Pairing Code
              </button>
              {pairingCode && (
                <div className="flex items-center gap-2">
                  <code className="rounded bg-gray-900 border border-gray-700 px-3 py-1.5 text-sm font-mono text-green-400 tracking-widest">
                    {pairingCode}
                  </code>
                  <span className="text-xs text-gray-500">expires in 10 min</span>
                </div>
              )}
            </div>

            {/* Manual chat_id entry */}
            <div className="flex gap-2">
              <input
                type="text"
                value={newChatId}
                onChange={(e) => setNewChatId(e.target.value)}
                placeholder="Chat ID (e.g. 123456789)"
                className="flex-1 rounded bg-gray-900 border border-gray-700 px-3 py-1.5 text-xs text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                onKeyDown={(e) => e.key === "Enter" && handleAddChatId()}
              />
              <button
                onClick={handleAddChatId}
                disabled={!newChatId.trim()}
                className="rounded bg-gray-700 hover:bg-gray-600 disabled:opacity-50 px-3 py-1.5 text-xs text-white"
              >
                Add
              </button>
            </div>

            {/* Paired chat list */}
            {status?.paired_chat_ids.length === 0 ? (
              <p className="text-xs text-gray-500 italic">No paired chats yet.</p>
            ) : (
              <ul className="space-y-1">
                {status?.paired_chat_ids.map((chatId) => (
                  <li
                    key={chatId}
                    className="flex items-center justify-between rounded bg-gray-800 px-3 py-2"
                  >
                    <span className="text-xs font-mono text-gray-300">{chatId}</span>
                    <button
                      onClick={() => handleRemoveChatId(chatId)}
                      className="text-xs text-red-400 hover:text-red-300"
                    >
                      Unpair
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </div>
  );
}

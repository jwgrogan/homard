import { TelegramPanel } from "../components/settings/TelegramPanel";

export default function Assistant() {
  return (
    <div className="h-full flex flex-col">
      <div className="mb-6">
        <h1 className="text-xl font-semibold">Assistant</h1>
        <p className="text-sm text-zinc-400 mt-1">Always-on communication bridges</p>
      </div>

      <div className="flex-1 overflow-y-auto space-y-8">
        {/* Telegram bridge */}
        <section>
          <TelegramPanel />
        </section>

        {/* Future channels */}
        <section className="rounded-lg border border-zinc-700 bg-zinc-800/50 p-4">
          <h2 className="text-sm font-semibold text-white mb-1">Email</h2>
          <p className="text-xs text-zinc-400">Coming soon — email-triggered runs and result delivery.</p>
        </section>

        <section className="rounded-lg border border-zinc-700 bg-zinc-800/50 p-4">
          <h2 className="text-sm font-semibold text-white mb-1">Other Channels</h2>
          <p className="text-xs text-zinc-400">Coming soon — Slack, SMS, and more.</p>
        </section>
      </div>
    </div>
  );
}

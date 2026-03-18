import { useEffect, useState } from "react";
import type { Session, SessionTree } from "../lib/types";
import { getSessionTree, resumeSession, forkSession } from "../lib/tauri";
import AgentTree from "./AgentTree";

interface SessionDetailProps {
  session: Session;
  onClose: () => void;
}

export default function SessionDetail({ session, onClose }: SessionDetailProps) {
  const [tree, setTree] = useState<SessionTree | null>(null);

  const handleResume = async () => {
    await resumeSession(session.id);
  };

  const handleFork = async () => {
    await forkSession(session.id);
  };

  useEffect(() => {
    const fetchTree = async () => {
      const t = await getSessionTree(session.id);
      setTree(t);
    };
    fetchTree();

    if (session.status === "running") {
      const interval = setInterval(fetchTree, 3000);
      return () => clearInterval(interval);
    }
  }, [session.id, session.status]);

  return (
    <div className="bg-zinc-800 border border-zinc-700 rounded-lg p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-zinc-100">Session Detail</h3>
        <button onClick={onClose} className="text-zinc-400 hover:text-zinc-200 text-sm">Close</button>
      </div>

      <div className="grid grid-cols-2 gap-2 text-sm">
        <div><span className="text-zinc-500">Provider:</span> <span className="text-zinc-200">{session.provider}</span></div>
        <div><span className="text-zinc-500">Profile:</span> <span className="text-zinc-200">{session.profile_name ?? "default"}</span></div>
        <div><span className="text-zinc-500">Directory:</span> <span className="text-zinc-200 truncate">{session.directory ?? "—"}</span></div>
        <div><span className="text-zinc-500">Status:</span> <span className="text-zinc-200">{session.status}</span></div>
        {session.cli_session_id && (
          <div className="col-span-2"><span className="text-zinc-500">CLI Session:</span> <span className="text-zinc-300 font-mono text-xs">{session.cli_session_id}</span></div>
        )}
      </div>

      {session.status !== "running" && session.cli_session_id && (
        <div className="flex gap-2">
          <button onClick={handleResume} className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white">Resume</button>
          <button onClick={handleFork} className="px-3 py-1.5 text-sm bg-zinc-600 hover:bg-zinc-500 rounded text-white">Fork</button>
        </div>
      )}

      {tree ? (
        <div>
          <h4 className="text-sm font-medium text-zinc-400 mb-2">Agent Team</h4>
          <AgentTree root={tree.root} />
        </div>
      ) : (
        <p className="text-zinc-500 text-sm">No agent tree data available</p>
      )}
    </div>
  );
}

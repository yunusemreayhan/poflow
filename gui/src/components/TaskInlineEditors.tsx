import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface Props {
  taskId: number;
  depth: number;
  show: boolean;
  onClose: () => void;
}

export function InlineTimeReport({ taskId, depth, show, onClose, onLogged }: Props & { onLogged: (h: number) => void }) {
  const [hours, setHours] = useState("");
  const [desc, setDesc] = useState("");
  const [history, setHistory] = useState<{ id: number; hours: number; username: string; created_at: string; note: string | null }[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  const loadHistory = () => {
    apiCall<{ id: number; hours: number; username: string; created_at: string; note: string | null }[]>("GET", `/api/tasks/${taskId}/time`)
      .then(h => h && setHistory(h)).catch(() => {});
  };

  const submit = async () => {
    const h = parseFloat(hours);
    if (!h || h <= 0) return;
    await apiCall("POST", `/api/tasks/${taskId}/time`, { hours: h, description: desc || undefined });
    onLogged(h);
    setHours(""); setDesc("");
    if (showHistory) loadHistory();
  };

  return (
    <AnimatePresence>
      {show && (
        <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
          className="overflow-hidden" style={{ marginLeft: depth * 24 + 48 }}>
          <div className="flex gap-2 items-center py-2 px-4">
            <input type="number" step="0.25" min="0.25" value={hours} onChange={e => setHours(e.target.value)}
              placeholder="Hours" className="w-20 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-work)]" autoFocus />
            <input value={desc} onChange={e => setDesc(e.target.value)} onKeyDown={e => e.key === "Enter" && submit()}
              placeholder="Description (optional)" className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-work)]" />
            <button onClick={submit} className="px-3 py-2 rounded-lg bg-[var(--color-accent)] text-white text-xs">Log</button>
            <button onClick={() => { setShowHistory(!showHistory); if (!showHistory) loadHistory(); }} className="text-[10px] text-white/30 hover:text-white/50">
              {showHistory ? "Hide" : "History"}
            </button>
            <button onClick={onClose} className="text-white/30 text-xs" aria-label="Close">✕</button>
          </div>
          {showHistory && history.length > 0 && (
            <div className="px-4 pb-2 space-y-0.5">
              {history.slice(0, 10).map(h => (
                <div key={h.id} className="text-[10px] text-white/30 flex gap-2">
                  <span>{h.created_at.slice(0, 10)}</span>
                  <span className="text-white/50">{h.hours.toFixed(1)}h</span>
                  <span>{h.username}</span>
                  {h.note && <span className="text-white/20 truncate">{h.note}</span>}
                </div>
              ))}
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function InlineComment({ taskId, depth, show, onClose }: Props) {
  const [content, setContent] = useState("");

  const submit = async () => {
    if (!content.trim()) return;
    await apiCall("POST", `/api/tasks/${taskId}/comments`, { content: content.trim() });
    useStore.getState().toast("Comment added");
    setContent(""); onClose();
  };

  return (
    <AnimatePresence>
      {show && (
        <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
          className="overflow-hidden" style={{ marginLeft: depth * 24 + 48 }}>
          <div className="flex gap-2 items-center py-2 px-4">
            <input value={content} onChange={e => setContent(e.target.value)} onKeyDown={e => e.key === "Enter" && submit()}
              placeholder="Add a comment..." className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-work)]" autoFocus />
            <button onClick={submit} className="px-3 py-2 rounded-lg bg-[var(--color-accent)] text-white text-xs">Post</button>
            <button onClick={onClose} className="text-white/30 text-xs" aria-label="Close">✕</button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function InlineAddSubtask({ parentId, depth, show, onClose }: { parentId: number; depth: number; show: boolean; onClose: () => void }) {
  const [title, setTitle] = useState("");
  const { createTask } = useStore();

  const submit = () => {
    if (!title.trim()) return;
    createTask(title.trim(), parentId);
    setTitle(""); onClose();
  };

  return (
    <AnimatePresence>
      {show && (
        <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
          className="overflow-hidden" style={{ marginLeft: (depth + 1) * 24 + 24 }}>
          <div className="flex gap-2 items-center py-2">
            <input value={title} onChange={e => setTitle(e.target.value)} onKeyDown={e => { if (e.key === "Enter") submit(); if (e.key === "Escape") onClose(); }}
              placeholder="New subtask..." className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-work)]" autoFocus />
            <button onClick={submit} className="px-3 py-2 rounded-lg bg-[var(--color-accent)] text-white text-xs">Add</button>
            <button onClick={onClose} className="text-white/30 text-xs" aria-label="Close">✕</button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

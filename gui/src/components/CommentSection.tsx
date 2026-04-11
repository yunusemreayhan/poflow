import { useState, useEffect, useCallback } from "react";
import { Plus, Trash2 } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface Comment { id: number; user: string; content: string; created_at: string; session_id: number | null; }

export default function CommentSection({ taskId, sessionId }: { taskId: number; sessionId?: number }) {
  const { addComment, username: currentUser, role } = useStore();
  const [comments, setComments] = useState<Comment[]>([]);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const c = await apiCall<Comment[]>("GET", `/api/tasks/${taskId}/comments`);
    if (c) setComments(sessionId ? c.filter((x) => x.session_id === sessionId) : c);
    setLoading(false);
  }, [taskId, sessionId]);

  useEffect(() => { load(); }, [load]);

  const handleAdd = async () => {
    if (!text.trim()) return;
    const content = text.trim();
    const optimistic = { id: -Date.now(), content, user: currentUser || "you", created_at: new Date().toISOString(), task_id: taskId, user_id: 0, session_id: sessionId ?? null };
    setComments(prev => [...prev, optimistic as any]);
    setText("");
    await addComment(taskId, content, sessionId);
    load();
  };

  return (
    <div className="space-y-2">
      {loading ? <div className="text-xs text-white/30">Loading...</div> : comments.map((c) => (
        <div key={c.id} className="flex gap-2 items-start group">
          <div className="flex-1 text-xs text-white/60">
            <span className="text-white/30 mr-2">{c.created_at.slice(0, 16).replace("T", " ")}</span>
            <span className="text-[var(--color-accent)]/60 mr-2">@{c.user}</span>
            {c.content}
          </div>
          {(c.user === currentUser || role === "root") && <button onClick={async () => { await apiCall("DELETE", `/api/comments/${c.id}`); load(); }}
            aria-label={`Delete comment by ${c.user}`}
            className="opacity-0 group-hover:opacity-100 text-white/20 hover:text-[var(--color-danger)] transition-all shrink-0">
            <Trash2 size={12} />
          </button>}
        </div>
      ))}
      <div className="flex gap-2">
        <input value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          placeholder="Add comment..." className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-accent)]" />
        <button onClick={handleAdd} className="w-7 h-7 flex items-center justify-center rounded-lg bg-[var(--color-accent)] text-white shrink-0"><Plus size={12} /></button>
      </div>
    </div>
  );
}

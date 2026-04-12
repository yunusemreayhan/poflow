import { useState, useEffect, useCallback } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface Recurrence {
  task_id: number;
  pattern: string;
  next_due: string;
  last_created: string | null;
}

const PATTERNS = ["daily", "weekly", "biweekly", "monthly"];

export function TaskRecurrence({ taskId }: { taskId: number }) {
  const [rec, setRec] = useState<Recurrence | null>(null);
  const [editing, setEditing] = useState(false);
  const [pattern, setPattern] = useState("daily");
  const [nextDue, setNextDue] = useState("");

  const load = useCallback(() => apiCall<Recurrence>("GET", `/api/tasks/${taskId}/recurrence`)
    .then(r => { setRec(r); setPattern(r.pattern); setNextDue(r.next_due); })
    .catch(() => setRec(null)), [taskId]);
  useEffect(load, [load]);

  const save = async () => {
    await apiCall("PUT", `/api/tasks/${taskId}/recurrence`, { pattern, next_due: nextDue });
    setEditing(false); load();
    useStore.getState().toast("Recurrence set");
  };

  const remove = async () => {
    await apiCall("DELETE", `/api/tasks/${taskId}/recurrence`);
    setRec(null); setEditing(false);
    useStore.getState().toast("Recurrence removed");
  };

  if (!editing && !rec) {
    return (
      <button onClick={() => { setEditing(true); setNextDue(new Date().toISOString().slice(0, 10)); }}
        className="text-xs text-[var(--color-accent)] hover:underline">🔄 Add recurrence</button>
    );
  }

  if (!editing && rec) {
    return (
      <div className="flex items-center gap-2 text-xs text-[var(--color-dim)]">
        <span>🔄 {rec.pattern} — next: {rec.next_due}</span>
        <button onClick={() => setEditing(true)} className="text-[var(--color-accent)]">edit</button>
        <button onClick={remove} className="text-[var(--color-danger)]">remove</button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <select value={pattern} onChange={e => setPattern(e.target.value)}
        className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]"
        aria-label="Recurrence pattern">
        {PATTERNS.map(p => <option key={p} value={p}>{p}</option>)}
      </select>
      <input type="date" value={nextDue} onChange={e => setNextDue(e.target.value)} aria-label="Next due date"
        className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]" />
      <button onClick={save} className="text-xs px-2 py-1 rounded bg-[var(--color-accent)] text-white">Save</button>
      <button onClick={() => setEditing(false)} className="text-xs text-[var(--color-dim)]">Cancel</button>
    </div>
  );
}

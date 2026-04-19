import { useState, useEffect } from "react";
import { apiCall, type Task } from "../store/api";
import { useStore } from "../store/store";
import { useT } from "../i18n";

export function TaskDependencies({ taskId, allTasks }: { taskId: number; allTasks: Task[] }) {
  const t = useT();
  const [deps, setDeps] = useState<number[]>([]);

  const load = () => apiCall<number[]>("GET", `/api/tasks/${taskId}/dependencies`).then(setDeps).catch(e => console.error(e));
  useEffect(() => { void load(); }, [taskId]);

  const add = async (depId: number) => {
    await apiCall("POST", `/api/tasks/${taskId}/dependencies`, { depends_on: depId });
    load();
    useStore.getState().toast("Dependency added");
  };

  const remove = async (depId: number) => {
    await apiCall("DELETE", `/api/tasks/${taskId}/dependencies/${depId}`);
    load();
    useStore.getState().toast("Dependency removed");
  };

  const available = allTasks.filter(t => t.id !== taskId && !deps.includes(t.id));
  const [depSearch, setDepSearch] = useState("");

  return (
    <div className="space-y-1">
      <span className="text-xs text-[var(--color-dim)]">{t.dependsOn}</span>
      <div className="flex flex-wrap gap-1">
        {deps.map(id => {
          const t = allTasks.find(t => t.id === id);
          return (
            <span key={id} className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-white/5 text-[var(--color-text)]">
              {t ? t.title : `#${id}`}
              <button onClick={() => remove(id)} className="opacity-50 hover:opacity-100" aria-label={`Remove dependency on ${t?.title || id}`}>×</button>
            </span>
          );
        })}
        {deps.length === 0 && <span className="text-xs text-[var(--color-dim)]">{t.none}</span>}
      </div>
      {available.length > 0 && (
        <div className="flex gap-1 items-center">
          <input value={depSearch} onChange={e => setDepSearch(e.target.value)} placeholder="Search tasks..."
            className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)] outline-none w-24 placeholder-white/20" />
          <select onChange={e => { if (e.target.value) { add(Number(e.target.value)); setDepSearch(""); } e.target.value = ""; }}
            className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]"
            defaultValue="" aria-label="Add dependency">
            <option value="" disabled>{t.addDependency}</option>
            {available.filter(t => !depSearch || t.title.toLowerCase().includes(depSearch.toLowerCase())).slice(0, 30).map(t => <option key={t.id} value={t.id}>{t.title}</option>)}
          </select>
        </div>
      )}
    </div>
  );
}

import { useState, useEffect } from "react";
import { apiCall, type Task } from "../store/api";
import { useStore } from "../store/store";
import { useT } from "../i18n";

export function TaskDependencies({ taskId, allTasks }: { taskId: number; allTasks: Task[] }) {
  const t = useT();
  const [deps, setDeps] = useState<number[]>([]);

  const load = () => apiCall<number[]>("GET", `/api/tasks/${taskId}/dependencies`).then(setDeps).catch(() => {});
  useEffect(load, [taskId]);

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
        <select onChange={e => { if (e.target.value) add(Number(e.target.value)); e.target.value = ""; }} aria-label="Add dependency"
          className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]"
          defaultValue="" aria-label="Add dependency">
          <option value="" disabled>{t.addDependency}</option>
          {available.slice(0, 20).map(t => <option key={t.id} value={t.id}>{t.title}</option>)}
        </select>
      )}
    </div>
  );
}

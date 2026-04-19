import { useState, useEffect, useCallback } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { useT } from "../i18n";

interface Label {
  id: number;
  name: string;
  color: string;
}

export function LabelManager() {
  const t = useT();
  const [labels, setLabels] = useState<Label[]>([]);
  const [name, setName] = useState("");
  const [color, setColor] = useState("#6366f1");

  const load = () => apiCall<Label[]>("GET", "/api/labels").then(setLabels).catch(e => console.error(e));
  useEffect(() => { load(); }, []);

  const create = async () => {
    if (!name.trim()) return;
    await apiCall("POST", "/api/labels", { name: name.trim(), color });
    setName(""); load();
    useStore.getState().toast("Label created");
  };

  const remove = async (id: number) => {
    useStore.getState().showConfirm("Delete this label?", async () => {
      await apiCall("DELETE", `/api/labels/${id}`);
      load();
      useStore.getState().toast("Label deleted");
    });
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-[var(--color-text)]">{t.labels}</h3>
      <div className="flex gap-2 items-center">
        <input value={name} onChange={e => setName(e.target.value)} placeholder={t.labelName}
          className="flex-1 px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-sm text-[var(--color-text)]"
          onKeyDown={e => e.key === "Enter" && create()} />
        <input type="color" value={color} onChange={e => setColor(e.target.value)} className="w-8 h-8 rounded cursor-pointer" aria-label="Label color" />
        <button onClick={create} className="px-3 py-1 rounded text-xs bg-[var(--color-accent)] text-white">{t.addLabel}</button>
      </div>
      <div className="flex flex-wrap gap-2">
        {labels.map(l => (
          <span key={l.id} className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs text-white"
            style={{ backgroundColor: l.color }}>
            {l.name}
            <button onClick={() => remove(l.id)} className="opacity-50 hover:opacity-100" aria-label={`Delete label ${l.name}`}>×</button>
          </span>
        ))}
      </div>
    </div>
  );
}

/** Inline label picker for task detail view */
export function TaskLabelPicker({ taskId }: { taskId: number }) {
  const t = useT();
  const [allLabels, setAllLabels] = useState<Label[]>([]);
  const [taskLabels, setTaskLabels] = useState<Label[]>([]);

  const load = useCallback(() => {
    apiCall<Label[]>("GET", "/api/labels").then(setAllLabels).catch(e => console.error(e));
    apiCall<Label[]>("GET", `/api/tasks/${taskId}/labels`).then(setTaskLabels).catch(e => console.error(e));
  }, [taskId]);
  useEffect(load, [load]);

  const toggle = async (label: Label) => {
    const has = taskLabels.some(l => l.id === label.id);
    if (has) {
      await apiCall("DELETE", `/api/tasks/${taskId}/labels/${label.id}`);
    } else {
      await apiCall("PUT", `/api/tasks/${taskId}/labels/${label.id}`);
    }
    load();
    useStore.getState().loadTasks();
  };

  return (
    <div className="flex flex-wrap gap-1">
      {allLabels.map(l => {
        const active = taskLabels.some(tl => tl.id === l.id);
        return (
          <button key={l.id} onClick={() => toggle(l)}
            className={`px-2 py-0.5 rounded-full text-xs transition-all ${active ? "text-white ring-2 ring-white/30" : "text-white/60"}`}
            style={{ backgroundColor: active ? l.color : `${l.color}40` }}
            aria-pressed={active} aria-label={`Label ${l.name}`}>
            {l.name}
          </button>
        );
      })}
      {allLabels.length === 0 && <span className="text-xs text-[var(--color-dim)]">{t.noLabels} — {t.createInSettings}</span>}
    </div>
  );
}

import { useState, useEffect } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import Select from "./Select";

interface Template { id: number; name: string; data: string; created_at: string }

export function TemplateManager() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [priority, setPriority] = useState(3);
  const [estimated, setEstimated] = useState(1);

  const load = () => apiCall<Template[]>("GET", "/api/templates").then(setTemplates).catch(() => {});
  useEffect(() => { void load(); }, []);

  const create = async () => {
    if (!name.trim()) return;
    const data = { title, priority, estimated };
    await apiCall("POST", "/api/templates", { name: name.trim(), data });
    setName(""); setTitle(""); load();
  };

  const del = (id: number) => {
    useStore.getState().showConfirm("Delete this template?", async () => {
      await apiCall("DELETE", `/api/templates/${id}`);
      load();
    });
  };

  const apply = async (t: Template) => {
    // BL10: Use instantiate endpoint for variable resolution ({{today}}, {{username}})
    await apiCall("POST", `/api/templates/${t.id}/instantiate`);
    useStore.getState().loadTasks();
    useStore.getState().toast(`Created task from "${t.name}"`);
  };

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Templates</h3>
      <div className="space-y-2 mb-2">
        <input value={name} onChange={e => setName(e.target.value)} placeholder="Template name"
          className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-[var(--color-text)] outline-none" />
        <input value={title} onChange={e => setTitle(e.target.value)} placeholder="Task title prefix (e.g. 'Bug: ')"
          className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-[var(--color-text)] outline-none" />
        <div className="flex gap-2">
          <label className="text-xs text-[var(--color-dim)] flex items-center gap-1">Priority
            <Select value={String(priority)} onChange={v => setPriority(Number(v))} ariaLabel="Priority"
              options={[1,2,3,4,5].map(p => ({value:String(p),label:String(p)}))} />
          </label>
          <label className="text-xs text-[var(--color-dim)] flex items-center gap-1">Est.
            <input type="number" min={0} value={estimated} onChange={e => setEstimated(Number(e.target.value))}
              className="w-12 bg-white/5 border border-white/10 rounded px-1 py-0.5 text-xs text-[var(--color-text)]" />
          </label>
          <button onClick={create} className="px-3 py-1 rounded text-xs bg-[var(--color-accent)] text-white ml-auto">Add</button>
        </div>
      </div>
      {templates.map(t => (
        <div key={t.id} className="flex items-center gap-2 text-xs py-1 group">
          <span className="flex-1 text-[var(--color-text)]">{t.name}</span>
          <button onClick={() => apply(t)} className="text-[var(--color-accent)] hover:underline">Use</button>
          <button onClick={() => del(t.id)} className="text-white/30 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
        </div>
      ))}
      {templates.length === 0 && <div className="text-xs text-[var(--color-dim)]">No templates yet</div>}
    </div>
  );
}

interface Webhook { id: number; url: string; events: string; active: number; created_at: string }

export function WebhookManager() {
  const [hooks, setHooks] = useState<Webhook[]>([]);
  const [url, setUrl] = useState("");
  const [events, setEvents] = useState("*");

  const load = () => apiCall<Webhook[]>("GET", "/api/webhooks").then(setHooks).catch(() => {});
  useEffect(() => { void load(); }, []);

  const create = async () => {
    if (!url.trim()) return;
    await apiCall("POST", "/api/webhooks", { url: url.trim(), events });
    setUrl(""); load();
  };

  const del = (id: number) => {
    useStore.getState().showConfirm("Delete this webhook?", async () => {
      await apiCall("DELETE", `/api/webhooks/${id}`);
      load();
    });
  };

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Webhooks</h3>
      <div className="flex gap-2 mb-2">
        <input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com/hook"
          className="flex-1 bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-[var(--color-text)] outline-none" />
        <Select value={events} onChange={v => setEvents(v)}
          options={[{value:"*",label:"All events"},{value:"task.created",label:"task.created"},{value:"task.updated",label:"task.updated"},{value:"task.deleted",label:"task.deleted"},{value:"sprint.created",label:"sprint.created"},{value:"sprint.started",label:"sprint.started"},{value:"sprint.completed",label:"sprint.completed"}]} />
        <button onClick={create} className="px-3 py-1 rounded text-xs bg-[var(--color-accent)] text-white">Add</button>
      </div>
      {hooks.map(h => (
        <div key={h.id} className="flex items-center gap-2 text-xs py-1 group">
          <span className="flex-1 truncate text-[var(--color-text)]">{h.url}</span>
          <span className="text-[var(--color-dim)]">{h.events}</span>
          <button onClick={() => del(h.id)} className="text-white/30 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
        </div>
      ))}
      {hooks.length === 0 && <div className="text-xs text-[var(--color-dim)]">No webhooks configured</div>}
    </div>
  );
}

export function CsvImport() {
  const [result, setResult] = useState<string | null>(null);

  const processFile = async (file: File) => {
    const text = await file.text();
    try {
      const resp = await apiCall<{ created: number; errors: string[] }>("POST", "/api/import/tasks", { csv: text });
      setResult(`Imported ${resp.created} tasks${resp.errors?.length ? ` (${resp.errors.length} errors)` : ""}`);
      useStore.getState().loadTasks();
    } catch {
      setResult("Import failed");
    }
  };

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) await processFile(file);
    e.target.value = "";
  };

  const [dragging, setDragging] = useState(false);

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Import Tasks (CSV)</h3>
      <div className="text-xs text-[var(--color-dim)] mb-2">CSV format: title,priority,estimated,project</div>
      <div className={`border-2 border-dashed rounded-lg p-4 text-center transition-colors ${dragging ? "border-[var(--color-accent)] bg-[var(--color-accent)]/5" : "border-white/10"}`}
        onDragOver={e => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={async e => { e.preventDefault(); setDragging(false); const file = e.dataTransfer.files[0]; if (file && (file.name.endsWith('.csv') || file.type === 'text/csv')) await processFile(file); else if (file) setResult("Only .csv files are accepted"); }}>
        <label className="cursor-pointer text-xs text-white/40 hover:text-white/60" tabIndex={0} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (e.currentTarget.querySelector("input") as HTMLInputElement)?.click(); } }}>
          {dragging ? "Drop CSV here" : "Drag CSV here or click to browse"}
          <input type="file" accept=".csv" onChange={handleFile} className="hidden" />
        </label>
      </div>
      {result && <div className="text-xs text-[var(--color-accent)] mt-2">{result}</div>}
    </div>
  );
}

// --- Trash View ---
import type { Task } from "../store/api";

export function TrashView() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const load = () => { apiCall<Task[]>("GET", "/api/tasks/trash").then(setTasks).catch(() => {}).finally(() => setLoading(false)); };
  useEffect(() => { void load(); }, []);

  const restore = async (id: number) => {
    await apiCall("POST", `/api/tasks/${id}/restore`);
    setTasks(prev => prev.filter(t => t.id !== id));
    useStore.getState().loadTasks();
  };

  const purge = (id: number) => {
    useStore.getState().showConfirm("Permanently delete this task? This cannot be undone.", async () => {
      await apiCall("DELETE", `/api/tasks/${id}/permanent`);
      setTasks(prev => prev.filter(t => t.id !== id));
    });
  };

  if (loading) return <div className="text-xs text-white/40">Loading...</div>;
  if (tasks.length === 0) return <div className="text-xs text-white/40 mt-2">Trash is empty</div>;

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Trash ({tasks.length})</h3>
      <div className="space-y-1 max-h-64 overflow-y-auto">
        {tasks.map(t => (
          <div key={t.id} className="flex items-center justify-between py-1.5 px-2 rounded bg-white/5 text-xs">
            <span className="text-white/60 truncate flex-1">{t.title}</span>
            <span className="text-white/30 text-[10px] mx-2">{t.deleted_at?.slice(0, 10)}</span>
            <button onClick={() => restore(t.id)} className="text-[var(--color-accent)] hover:underline text-[10px] mr-2">Restore</button>
            <button onClick={() => purge(t.id)} className="text-[var(--color-danger)] hover:underline text-[10px]">Delete</button>
          </div>
        ))}
      </div>
    </div>
  );
}

// F12: Notification preferences per event type
interface NotifPref { event_type: string; enabled: boolean }
const EVENT_LABELS: Record<string, string> = {
  task_assigned: "Task assigned to me",
  task_completed: "Task completed",
  comment_added: "New comment",
  sprint_started: "Sprint started",
  sprint_completed: "Sprint completed",
  time_logged: "Time logged on my task",
};

export function NotificationPrefs() {
  const [prefs, setPrefs] = useState<NotifPref[]>([]);
  useEffect(() => { apiCall<NotifPref[]>("GET", "/api/profile/notifications").then(p => p && setPrefs(p)).catch(() => {}); }, []);

  const toggle = async (eventType: string) => {
    const updated = prefs.map(p => p.event_type === eventType ? { ...p, enabled: !p.enabled } : p);
    setPrefs(updated);
    await apiCall("PUT", "/api/profile/notifications", updated);
  };

  if (prefs.length === 0) return null;
  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Notification Preferences</h3>
      <div className="space-y-1">
        {prefs.map(p => (
          <label key={p.event_type} className="flex items-center gap-2 text-xs text-[var(--color-dim)] cursor-pointer">
            <input type="checkbox" checked={p.enabled} onChange={() => toggle(p.event_type)} className="accent-[var(--color-accent)]" />
            {EVENT_LABELS[p.event_type] || p.event_type}
          </label>
        ))}
      </div>
    </div>
  );
}

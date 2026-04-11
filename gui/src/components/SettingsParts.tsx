import { useState, useEffect } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface Template { id: number; name: string; data: string; created_at: string }

export function TemplateManager() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [priority, setPriority] = useState(3);
  const [estimated, setEstimated] = useState(1);

  const load = () => apiCall<Template[]>("GET", "/api/templates").then(setTemplates).catch(() => {});
  useEffect(load, []);

  const create = async () => {
    if (!name.trim()) return;
    const data = JSON.stringify({ title, priority, estimated });
    await apiCall("POST", "/api/templates", { name: name.trim(), data });
    setName(""); setTitle(""); load();
  };

  const del = async (id: number) => {
    await apiCall("DELETE", `/api/templates/${id}`);
    load();
  };

  const apply = async (t: Template) => {
    try {
      const parsed = JSON.parse(t.data);
      await apiCall("POST", "/api/tasks", parsed);
      useStore.getState().toast(`Created task from template "${t.name}"`);
      useStore.getState().loadTasks();
    } catch { useStore.getState().toast("Invalid template data", "error"); }
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
            <select value={priority} onChange={e => setPriority(Number(e.target.value))} aria-label="Priority"
              className="bg-white/5 border border-white/10 rounded px-1 py-0.5 text-xs text-[var(--color-text)]">
              {[1,2,3,4,5].map(p => <option key={p} value={p}>{p}</option>)}
            </select>
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
          <button onClick={() => del(t.id)} className="text-white/20 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
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
  useEffect(load, []);

  const create = async () => {
    if (!url.trim()) return;
    await apiCall("POST", "/api/webhooks", { url: url.trim(), events });
    setUrl(""); load();
  };

  const del = async (id: number) => {
    await apiCall("DELETE", `/api/webhooks/${id}`);
    load();
  };

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-[var(--color-text)] mb-2">Webhooks</h3>
      <div className="flex gap-2 mb-2">
        <input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com/hook"
          className="flex-1 bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-[var(--color-text)] outline-none" />
        <input value={events} onChange={e => setEvents(e.target.value)} placeholder="* or task.created"
          className="w-32 bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-[var(--color-text)] outline-none" />
        <button onClick={create} className="px-3 py-1 rounded text-xs bg-[var(--color-accent)] text-white">Add</button>
      </div>
      {hooks.map(h => (
        <div key={h.id} className="flex items-center gap-2 text-xs py-1 group">
          <span className="flex-1 truncate text-[var(--color-text)]">{h.url}</span>
          <span className="text-[var(--color-dim)]">{h.events}</span>
          <button onClick={() => del(h.id)} className="text-white/20 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
        </div>
      ))}
      {hooks.length === 0 && <div className="text-xs text-[var(--color-dim)]">No webhooks configured</div>}
    </div>
  );
}

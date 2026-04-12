import { Paperclip } from "lucide-react";
import { useStore } from "../store/store";
import { useState, useEffect } from "react";
import { apiCall } from "../store/api";

interface AuditEntry { id: number; user_id: number; username: string; action: string; entity_type: string; entity_id: number | null; detail: string | null; created_at: string }

export function TaskActivityFeed({ taskId }: { taskId: number }) {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (show) apiCall<AuditEntry[]>("GET", `/api/audit?entity_type=task&entity_id=${taskId}&per_page=50`).then(e => setEntries(e || [])).catch(() => {});
  }, [show, taskId]);

  const icon = (a: string) => a === "create" ? "🆕" : a === "update" ? "✏️" : a === "delete" ? "🗑" : "📋";

  return (
    <div className="mt-2">
      <button onClick={() => setShow(!show)} className="text-xs text-white/30 hover:text-white/50 flex items-center gap-1">
        📋 {show ? "Hide" : "Show"} activity
      </button>
      {show && (
        <div className="mt-1 space-y-1 max-h-48 overflow-y-auto">
          {entries.map(e => (
            <div key={e.id} className="flex items-center gap-2 text-[11px] text-white/40 py-0.5">
              <span>{icon(e.action)}</span>
              <span className="text-white/60">{e.username}</span>
              <span>{e.action}</span>
              {e.detail && <span className="truncate text-white/25 max-w-40" title={e.detail}>{e.detail}</span>}
              <span className="ml-auto text-white/20 shrink-0">{e.created_at.slice(5, 16)}</span>
            </div>
          ))}
          {entries.length === 0 && <div className="text-xs text-white/20 py-2">No activity recorded</div>}
        </div>
      )}
    </div>
  );
}

interface Attachment {
  id: number;
  task_id: number;
  filename: string;
  mime_type: string;
  size_bytes: number;
  created_at: string;
}

export function TaskAttachments({ taskId }: { taskId: number }) {
  const [atts, setAtts] = useState<Attachment[]>([]);
  const [uploading, setUploading] = useState(false);

  const load = () => apiCall<Attachment[]>("GET", `/api/tasks/${taskId}/attachments`).then(setAtts).catch(() => {});
  useEffect(load, [taskId]);

  const upload = async (file: File) => {
    setUploading(true);
    try {
      const { serverUrl, token } = useStore.getState();
      const buf = await file.arrayBuffer();
      const resp = await fetch(`${serverUrl}/api/tasks/${taskId}/attachments`, {
        method: "POST",
        headers: {
          "content-type": file.type || "application/octet-stream",
          "x-filename": file.name,
          "x-requested-with": "PomodoroGUI",
          "authorization": `Bearer ${token}`,
        },
        body: buf,
      });
      if (resp.ok) load();
    } catch { /* ignore */ }
    setUploading(false);
  };

  const del = async (id: number) => {
    await apiCall("DELETE", `/api/attachments/${id}`);
    setAtts(a => a.filter(x => x.id !== id));
  };

  const download = async (id: number, filename: string) => {
    const { serverUrl, token } = useStore.getState();
    const resp = await fetch(`${serverUrl}/api/attachments/${id}/download`, {
      headers: { "authorization": `Bearer ${token}` },
    });
    if (resp.ok) {
      const blob = await resp.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url; a.download = filename; a.click();
      URL.revokeObjectURL(url);
    }
  };

  const fmt = (bytes: number) => bytes < 1024 ? `${bytes}B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(1)}KB` : `${(bytes / 1048576).toFixed(1)}MB`;

  return (
    <div className="mb-3">
      <div className="flex items-center gap-2 mb-1">
        <Paperclip size={12} className="text-white/30" />
        <span className="text-xs text-[var(--color-dim)]">Attachments ({atts.length})</span>
        <label className="text-xs text-[var(--color-accent)] cursor-pointer hover:underline">
          {uploading ? "Uploading..." : "+ Add"}
          <input type="file" className="hidden" onChange={e => { if (e.target.files?.[0]) upload(e.target.files[0]); e.target.value = ""; }} />
        </label>
      </div>
      {atts.map(a => (
        <div key={a.id} className="py-0.5 group">
          <div className="flex items-center gap-2 text-xs text-white/60">
            <span className="truncate flex-1">{a.filename}</span>
            <span className="text-white/20">{fmt(a.size_bytes)}</span>
            <button onClick={() => download(a.id, a.filename)}
              className="text-[var(--color-accent)] hover:underline">↓</button>
            <button onClick={() => del(a.id)} className="text-white/20 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
          </div>
          {/* F7: Inline preview for images */}
          {a.mime_type.startsWith("image/") && (
            <img src={`${useStore.getState().serverUrl}/api/attachments/${a.id}/download`}
              alt={a.filename} className="mt-1 max-h-32 rounded border border-white/10 object-contain"
              loading="lazy" />
          )}
        </div>
      ))}
    </div>
  );
}

// F3: Task time tracking chart — shows daily focus time for a task
import type { Session } from "../store/api";

export function TaskTimeChart({ taskId }: { taskId: number }) {
  const [sessions, setSessions] = useState<Session[]>([]);
  useEffect(() => {
    apiCall<Session[]>("GET", `/api/tasks/${taskId}/sessions`).then(s => s && setSessions(s)).catch(() => {});
  }, [taskId]);

  if (sessions.length === 0) return null;

  // Aggregate by date
  const byDate: Record<string, number> = {};
  sessions.forEach(s => {
    if (!s.started_at || !s.duration_s) return;
    const date = s.started_at.slice(0, 10);
    byDate[date] = (byDate[date] || 0) + s.duration_s;
  });
  const data = Object.entries(byDate).sort(([a], [b]) => a.localeCompare(b)).slice(-14);
  if (data.length === 0) return null;
  const maxSecs = Math.max(...data.map(([, v]) => v), 1);

  return (
    <div className="mt-2">
      <div className="text-[10px] text-white/40 mb-1">Focus time (last 14 days)</div>
      <div className="flex items-end gap-0.5 h-12">
        {data.map(([date, secs]) => (
          <div key={date} className="flex-1 flex flex-col items-center" title={`${date}: ${(secs / 3600).toFixed(1)}h`}>
            <div className="w-full bg-[var(--color-accent)]/30 rounded-t" style={{ height: `${(secs / maxSecs) * 100}%`, minHeight: 2 }} />
          </div>
        ))}
      </div>
      <div className="flex justify-between text-[8px] text-white/20 mt-0.5">
        <span>{data[0][0].slice(5)}</span>
        <span>{data[data.length - 1][0].slice(5)}</span>
      </div>
    </div>
  );
}

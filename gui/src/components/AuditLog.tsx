import { useState, useEffect } from "react";
import { apiCall } from "../store/api";
import { useT } from "../i18n";

interface AuditEntry {
  id: number;
  user_id: number;
  username?: string;
  action: string;
  entity_type: string;
  entity_id: number | null;
  detail: string | null;
  created_at: string;
}

export default function AuditLog() {
  const t = useT();
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    const q = filter ? `?entity_type=${filter}` : "";
    apiCall<AuditEntry[]>("GET", `/api/audit${q}`).then(setEntries).catch(() => {});
  }, [filter]);

  const actionColor: Record<string, string> = {
    create: "text-green-400", update: "text-blue-400", delete: "text-red-400", register: "text-purple-400",
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <h2 className="text-sm font-semibold text-[var(--color-text)]">{t.auditLog}</h2>
        <select value={filter} onChange={e => setFilter(e.target.value)}
          className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]"
          aria-label="Filter by entity type">
          <option value="">All</option>
          <option value="task">Tasks</option>
          <option value="user">Users</option>
          <option value="sprint">Sprints</option>
          <option value="room">Rooms</option>
        </select>
      </div>
      <div className="space-y-1 max-h-96 overflow-y-auto" role="table" aria-label="Audit log entries">
        <div className="sr-only" role="row"><span role="columnheader">Time</span><span role="columnheader">Action</span><span role="columnheader">Entity</span><span role="columnheader">Detail</span></div>
        {entries.map(e => (
          <div key={e.id} className="flex items-center gap-2 text-xs py-1 border-b border-white/5" role="row">
            <span className="text-[var(--color-dim)] w-32 shrink-0">{new Date(e.created_at).toLocaleString()}</span>
            <span className={`w-14 ${actionColor[e.action] || "text-[var(--color-text)]"}`}>{e.action}</span>
            <span className="text-[var(--color-text)]">{e.entity_type}{e.entity_id ? ` #${e.entity_id}` : ""}</span>
            {e.detail && <span className="text-[var(--color-dim)] truncate">{e.detail}</span>}
          </div>
        ))}
        {entries.length === 0 && <p className="text-xs text-[var(--color-dim)]">No audit entries</p>}
      </div>
    </div>
  );
}

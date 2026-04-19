import { useState, useEffect } from "react";
import { apiCall } from "../store/api";
import { useT } from "../i18n";
import Select from "./Select";

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
  const [page, setPage] = useState(1);
  const perPage = 50;

  useEffect(() => {
    const q = filter ? `&entity_type=${filter}` : "";
    apiCall<AuditEntry[]>("GET", `/api/audit?page=${page}&per_page=${perPage}${q}`).then(setEntries).catch(e => console.error(e));
  }, [filter, page]);

  const actionColor: Record<string, string> = {
    create: "text-green-400", update: "text-blue-400", delete: "text-red-400", register: "text-purple-400",
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <h2 className="text-sm font-semibold text-[var(--color-text)]">{t.auditLog}</h2>
        <Select value={filter} onChange={v => { setFilter(v); setPage(1); }} ariaLabel="Filter by entity type"
          options={[{value:"",label:t.filterAll},{value:"task",label:t.filterTasks},{value:"user",label:t.filterUsers},{value:"sprint",label:t.filterSprints},{value:"room",label:t.filterRooms}]} />
      </div>
      <div className="space-y-1 max-h-96 overflow-y-auto" role="table" aria-label="Audit log entries">
        <div className="sr-only" role="row"><span role="columnheader">Time</span><span role="columnheader">Action</span><span role="columnheader">Entity</span><span role="columnheader">Detail</span></div>
        {entries.map(e => (
          <div key={e.id} className="flex items-center gap-2 text-xs py-1 border-b border-white/5" role="row">
            <span role="cell" className="text-[var(--color-dim)] w-32 shrink-0">{new Date(e.created_at).toLocaleString()}</span>
            <span role="cell" className={`w-14 ${actionColor[e.action] || "text-[var(--color-text)]"}`}>{e.action}</span>
            <span role="cell" className="text-[var(--color-text)]">{e.entity_type}{e.entity_id ? ` #${e.entity_id}` : ""}</span>
            {e.detail && <span role="cell" className="text-[var(--color-dim)] truncate">{e.detail}</span>}
          </div>
        ))}
        {entries.length === 0 && <p className="text-xs text-[var(--color-dim)]">No audit entries</p>}
      </div>
      {/* UX4: Pagination */}
      <div className="flex items-center justify-center gap-2 text-xs">
        <button disabled={page <= 1} onClick={() => setPage(p => p - 1)} className="px-2 py-1 rounded bg-white/5 text-white/40 disabled:opacity-30">← Prev</button>
        <span className="text-white/30">Page {page}</span>
        <button disabled={entries.length < perPage} onClick={() => setPage(p => p + 1)} className="px-2 py-1 rounded bg-white/5 text-white/40 disabled:opacity-30">Next →</button>
      </div>
    </div>
  );
}

import { useState, useEffect, useCallback } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { matchSearch } from "../utils";
import { useT } from "../i18n";
import type { EpicGroup, EpicGroupDetail } from "../store/api";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

export default function EpicBurndown() {
  const t = useT();
  const [groups, setGroups] = useState<EpicGroup[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [detail, setDetail] = useState<EpicGroupDetail | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [open, setOpen] = useState(false);
  const [epicSearch, setEpicSearch] = useState("");
  const tasks = useStore(s => s.tasks);

  const loadGroups = useCallback(async () => {
    const d = await apiCall<EpicGroup[]>("GET", "/api/epics");
    if (d) setGroups(d);
  }, []);

  const loadDetail = useCallback(async (id: number) => {
    const d = await apiCall<EpicGroupDetail>("GET", `/api/epics/${id}`);
    if (d) setDetail(d);
  }, []);

  useEffect(() => { if (open) loadGroups(); }, [open, loadGroups]);
  useEffect(() => { if (selected) loadDetail(selected); }, [selected, loadDetail]);

  const create = async () => {
    if (!name.trim()) return;
    const g = await apiCall<EpicGroup>("POST", "/api/epics", { name: name.trim() });
    if (g) { setName(""); setCreating(false); loadGroups(); setSelected(g.id); }
  };

  const addTask = async (taskId: number) => {
    if (!selected) return;
    await apiCall("POST", `/api/epics/${selected}/tasks`, { task_ids: [taskId] });
    loadDetail(selected);
  };

  const removeTask = async (taskId: number) => {
    if (!selected) return;
    await apiCall("DELETE", `/api/epics/${selected}/tasks/${taskId}`);
    loadDetail(selected);
  };

  const snapshot = async () => {
    if (!selected) return;
    await apiCall("POST", `/api/epics/${selected}/snapshot`);
    loadDetail(selected);
  };

  const del = async (id: number) => {
    useStore.getState().showConfirm("Delete this epic group?", async () => {
      await apiCall("DELETE", `/api/epics/${id}`);
      if (selected === id) { setSelected(null); setDetail(null); }
      loadGroups();
    });
  };

  if (!open) return (
    <button onClick={() => setOpen(true)} className="w-full text-left text-xs text-white/30 hover:text-white/50 py-1 px-2">
      ▶ {t.epicBurndown}
    </button>
  );

  const rootTasks = tasks.filter(t => t.parent_id === null);
  const epicTaskIds = new Set(detail?.task_ids ?? []);

  const chartData = (detail?.snapshots ?? []).map(s => ({
    date: s.date.slice(5),
    remaining: s.total_points - s.done_points,
    done: s.done_points,
  }));

  return (
    <div className="bg-[var(--color-surface)] p-3 rounded-lg border border-white/5 space-y-3">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-white/60 flex-1">{t.epicBurndown}</span>
        <button onClick={() => setCreating(true)} className="text-xs text-[var(--color-accent)]">+ New</button>
        <button onClick={() => setOpen(false)} className="text-xs text-white/30 hover:text-white/50">▼ Hide</button>
      </div>

      <div className="flex gap-1 flex-wrap">
        {groups.map(g => (
          <button key={g.id} onClick={() => setSelected(g.id)}
            className={`px-2 py-0.5 rounded text-xs transition-colors ${selected === g.id ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/50 hover:text-white/70"}`}>
            {g.name}
            <button onClick={e => { e.stopPropagation(); del(g.id); }} className="ml-1 text-white/30 hover:text-red-400" aria-label={`Delete ${g.name}`}>×</button>
          </button>
        ))}
      </div>

      {creating && (
        <div className="flex gap-2">
          <input placeholder="Epic group name" value={name} onChange={e => setName(e.target.value)}
            onKeyDown={e => e.key === "Enter" && create()}
            className="flex-1 bg-transparent border-b border-white/20 text-white text-xs outline-none pb-1" autoFocus />
          <button onClick={create} className="text-xs text-[var(--color-accent)]">Create</button>
          <button onClick={() => setCreating(false)} className="text-xs text-white/30">Cancel</button>
        </div>
      )}

      {selected && detail && (
        <>
          <div className="space-y-1">
            <div className="text-xs text-white/40">Root tasks in group:</div>
            {detail.task_ids.map(tid => {
              const t = tasks.find(tk => tk.id === tid);
              return t ? (
                <div key={tid} className="flex items-center gap-2 text-xs text-white/70">
                  <span className="flex-1 truncate">{t.title}</span>
                  <button onClick={() => removeTask(tid)} className="text-red-400/50 hover:text-red-400">✕</button>
                </div>
              ) : null;
            })}
            {detail.task_ids.length === 0 && <div className="text-xs text-white/20">No root tasks added yet</div>}
            <div className="text-xs text-white/40 mt-2">Add root tasks:</div>
            <input placeholder="Search root tasks..." value={epicSearch} onChange={e => setEpicSearch(e.target.value)}
              className="w-full bg-transparent border-b border-white/10 text-white text-xs outline-none pb-1 mt-1" />
            <div className="max-h-24 overflow-y-auto space-y-0.5">
              {rootTasks.filter(t => !epicTaskIds.has(t.id) && (!epicSearch || matchSearch(t.title, epicSearch))).slice(0, 20).map(t => (
                <button key={t.id} onClick={() => addTask(t.id)}
                  className="w-full text-left text-xs text-white/50 hover:text-green-400 truncate py-0.5">
                  + {t.title}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button onClick={snapshot} className="text-xs px-2 py-0.5 rounded bg-white/5 text-white/50 hover:text-white/70">📸 Snapshot now</button>
            <span className="text-[10px] text-white/20">{detail.snapshots.length} data points</span>
          </div>

          {chartData.length > 0 && (
            <>
              <div className="h-32">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={chartData}>
                    <XAxis dataKey="date" tick={{ fontSize: 10, fill: "var(--color-chart-axis)" }} />
                    <YAxis tick={{ fontSize: 10, fill: "var(--color-chart-axis)" }} width={30} />
                    <Tooltip contentStyle={{ background: "var(--color-chart-tooltip-bg)", border: "1px solid var(--color-chart-tooltip-border)", borderRadius: 8, fontSize: 12, color: "var(--color-chart-tooltip-text)" }} />
                    <Area type="monotone" dataKey="remaining" stroke="var(--color-accent)" fill="var(--color-accent)" fillOpacity={0.15} name="Remaining" />
                    <Area type="monotone" dataKey="done" stroke="#22c55e" fill="#22c55e" fillOpacity={0.1} name="Done" />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
              <table className="sr-only">
                <caption>Epic burndown data</caption>
                <thead><tr><th>Date</th><th>Remaining</th><th>Done</th></tr></thead>
                <tbody>{chartData.map(d => <tr key={d.date}><td>{d.date}</td><td>{d.remaining}</td><td>{d.done}</td></tr>)}</tbody>
              </table>
              {(() => {
                const last = detail.snapshots[detail.snapshots.length - 1];
                const pct = last.total_points > 0 ? Math.round((last.done_points / last.total_points) * 100) : 0;
                return (
                  <div className="flex gap-4 text-xs text-white/40">
                    <span>{last.done_points}/{last.total_points} pts ({pct}%)</span>
                    <span>{last.done_tasks}/{last.total_tasks} tasks</span>
                    <span>{last.done_hours.toFixed(1)}/{last.total_hours.toFixed(1)} hrs</span>
                  </div>
                );
              })()}
            </>
          )}
        </>
      )}
    </div>
  );
}

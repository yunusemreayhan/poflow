import { useState, useCallback, useEffect } from "react";
import { apiCall } from "../store/api";
import type { Task, BurnEntry, BurnSummaryEntry, SprintDailyStat } from "../store/api";
import Select from "./Select";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

export function BurnsView({ sprintId, sprintName, tasks }: { sprintId: number; sprintName?: string; tasks: Task[] }) {
  const [burns, setBurns] = useState<BurnEntry[]>([]);
  const [summary, setSummary] = useState<BurnSummaryEntry[]>([]);
  const [taskId, setTaskId] = useState<number>(tasks[0]?.id || 0);
  useEffect(() => { if (tasks.length && !taskId) setTaskId(tasks[0].id); }, [tasks]);
  const [points, setPoints] = useState("");
  const [hours, setHours] = useState("");
  const [note, setNote] = useState("");
  const [view, setView] = useState<"log" | "summary">("log");

  const load = useCallback(async () => {
    const b = await apiCall<BurnEntry[]>("GET", `/api/sprints/${sprintId}/burns`);
    if (b) setBurns(b);
    const s = await apiCall<BurnSummaryEntry[]>("GET", `/api/sprints/${sprintId}/burn-summary`);
    if (s) setSummary(s);
  }, [sprintId]);

  useEffect(() => { load(); }, [load]);

  const submit = async () => {
    if (!taskId || taskId <= 0 || (!points && !hours)) return;
    await apiCall("POST", `/api/sprints/${sprintId}/burn`, {
      task_id: taskId,
      points: points ? parseFloat(points) : 0,
      hours: hours ? parseFloat(hours) : 0,
      note: note || undefined,
    });
    setPoints(""); setHours(""); setNote("");
    load();
  };

  const cancel = async (id: number) => {
    await apiCall("DELETE", `/api/sprints/${sprintId}/burns/${id}`);
    load();
  };

  const taskMap = Object.fromEntries(tasks.map(t => [t.id, t.title]));
  const byDate: Record<string, BurnSummaryEntry[]> = {};
  summary.forEach(s => { (byDate[s.date] ||= []).push(s); });

  return (
    <div className="space-y-3">
      <div className="bg-[var(--color-surface)] p-3 rounded-lg border border-white/5 space-y-2">
        <div className="text-xs text-white/50 font-medium">Log Burn{sprintName ? ` — ${sprintName}` : ""}</div>
        <Select value={String(taskId)} onChange={v => setTaskId(Number(v))} className="w-full text-xs"
          options={tasks.map(t => ({value:String(t.id),label:t.title}))} />
        <div className="flex gap-2">
          <input type="number" placeholder="Points" value={points} onChange={e => setPoints(e.target.value)}
            className="flex-1 bg-transparent border border-white/10 text-white text-xs rounded p-1.5 outline-none" />
          <input type="number" placeholder="Hours" value={hours} onChange={e => setHours(e.target.value)}
            className="flex-1 bg-transparent border border-white/10 text-white text-xs rounded p-1.5 outline-none" />
        </div>
        <div className="flex gap-2">
          <input placeholder="Note (optional)" value={note} onChange={e => setNote(e.target.value)}
            onKeyDown={e => e.key === "Enter" && submit()}
            className="flex-1 bg-transparent border border-white/10 text-white/70 text-xs rounded p-1.5 outline-none" />
          <button onClick={submit} className="px-3 py-1.5 bg-[var(--color-accent)] text-white text-xs rounded">Burn</button>
        </div>
      </div>

      <div className="flex gap-1">
        <button onClick={() => setView("log")} className={`text-xs px-2 py-1 rounded ${view === "log" ? "bg-white/10 text-white" : "text-white/40"}`}>Log</button>
        <button onClick={() => setView("summary")} className={`text-xs px-2 py-1 rounded ${view === "summary" ? "bg-white/10 text-white" : "text-white/40"}`}>Per-User Summary</button>
      </div>

      {view === "log" && (
        <div className="space-y-1 max-h-80 overflow-y-auto">
          {burns.map(b => (
            <div key={b.id} className={`flex items-center gap-2 p-2 rounded text-xs ${b.cancelled ? "opacity-30 line-through" : "bg-[var(--color-surface)]"} border border-white/5`}>
              <span className="text-white/70 font-medium w-16 shrink-0">👤 {b.username}</span>
              <span className="text-white/50 truncate flex-1" title={taskMap[b.task_id]}>{taskMap[b.task_id] || `#${b.task_id}`}</span>
              {b.points > 0 && <span className="text-[var(--color-accent)]">{b.points}pt</span>}
              {b.hours > 0 && <span className="text-blue-400">{b.hours}h</span>}
              {b.note && <span className="text-white/30 truncate max-w-24" title={b.note}>💬{b.note}</span>}
              <span className="text-white/20 shrink-0">{b.created_at.slice(5, 16)}</span>
              {b.cancelled ? (
                <span className="text-red-400/50 text-[10px] shrink-0">cancelled by {b.cancelled_by}</span>
              ) : (
                <button onClick={() => cancel(b.id)} className="text-white/20 hover:text-red-400 shrink-0" title="Cancel this burn">✕</button>
              )}
            </div>
          ))}
          {burns.length === 0 && <div className="text-white/20 text-xs text-center py-4">No burns logged yet</div>}
        </div>
      )}

      {view === "summary" && (
        <div className="space-y-2">
          {Object.entries(byDate).sort(([a], [b]) => b.localeCompare(a)).map(([date, entries]) => (
            <div key={date} className="bg-[var(--color-surface)] p-2 rounded border border-white/5">
              <div className="text-[10px] text-white/30 mb-1">{date}</div>
              {entries.map(e => (
                <div key={e.username} className="flex items-center gap-2 text-xs py-0.5">
                  <span className="text-white/70 w-20">👤 {e.username}</span>
                  {e.points > 0 && <span className="text-[var(--color-accent)]">{e.points} pts</span>}
                  {e.hours > 0 && <span className="text-blue-400">{e.hours}h</span>}
                  <span className="text-white/20">({e.count} entries)</span>
                </div>
              ))}
              <div className="flex gap-3 text-[10px] text-white/20 mt-1 border-t border-white/5 pt-1">
                <span>Total: {entries.reduce((s, e) => s + e.points, 0)} pts</span>
                <span>{entries.reduce((s, e) => s + e.hours, 0)}h</span>
              </div>
            </div>
          ))}
          {summary.length === 0 && <div className="text-white/20 text-xs text-center py-4">No active burns</div>}
        </div>
      )}
    </div>
  );
}

export function BurndownView({ stats }: { stats: SprintDailyStat[] }) {
  const [metric, setMetric] = useState<"points" | "hours" | "tasks">("points");
  if (stats.length === 0) return <div className="text-white/30 text-sm text-center py-8">No snapshots yet. Start the sprint and take a snapshot.</div>;

  const data = stats.map(s => ({
    date: s.date.slice(5),
    remaining: metric === "points" ? s.total_points - s.done_points
      : metric === "hours" ? s.total_hours - s.done_hours
      : s.total_tasks - s.done_tasks,
    ideal: 0,
  }));
  // Compute ideal line
  if (data.length > 1) {
    const start = data[0].remaining;
    const step = start / (data.length - 1);
    data.forEach((d, i) => { d.ideal = Math.max(0, start - step * i); });
  }

  const exportCsv = () => {
    const header = "date,remaining_" + metric + ",ideal\n";
    const rows = data.map(d => `${d.date},${d.remaining.toFixed(2)},${d.ideal.toFixed(2)}`).join("\n");
    const blob = new Blob([header + rows], { type: "text/csv" });
    const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = `burndown_${metric}.csv`; a.click();
  };

  return (
    <div className="space-y-2">
      <div className="flex gap-1 items-center">
        {(["points", "hours", "tasks"] as const).map(m => (
          <button key={m} onClick={() => setMetric(m)}
            className={`text-xs px-2 py-1 rounded ${metric === m ? "bg-white/10 text-white" : "text-white/40"}`}>{m}</button>
        ))}
        <button onClick={exportCsv} className="ml-auto text-xs text-white/30 hover:text-white/60" title="Export CSV">📥 CSV</button>
      </div>
      <ResponsiveContainer width="100%" height={200}>
        <AreaChart data={data}>
          <XAxis dataKey="date" tick={{ fill: "rgba(255,255,255,0.3)", fontSize: 10 }} axisLine={false} tickLine={false} />
          <YAxis tick={{ fill: "rgba(255,255,255,0.3)", fontSize: 10 }} axisLine={false} tickLine={false} width={30} />
          <Tooltip contentStyle={{ background: "#1A1A2E", border: "1px solid rgba(255,255,255,0.1)", borderRadius: 8, color: "#fff", fontSize: 12 }} />
          <Area type="monotone" dataKey="ideal" stroke="rgba(255,255,255,0.15)" fill="none" strokeDasharray="4 4" />
          <Area type="monotone" dataKey="remaining" stroke="#7C3AED" fill="rgba(124,58,237,0.15)" strokeWidth={2} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

export function VelocityChart() {
  const [data, setData] = useState<{ sprint: string; points: number; hours: number }[]>([]);

  useEffect(() => {
    apiCall<{ sprint_name: string; total_points: number; total_hours: number }[]>("GET", "/api/sprints/velocity")
      .then(v => {
        if (v) setData(v.map(d => ({ sprint: d.sprint_name, points: d.total_points, hours: d.total_hours })));
      }).catch(() => {});
  }, []);

  if (data.length < 2) return null;

  return (
    <div className="bg-[var(--color-surface)] p-3 rounded-lg border border-white/5">
      <div className="text-xs text-white/50 font-medium mb-2">Velocity Trend</div>
      <ResponsiveContainer width="100%" height={120}>
        <AreaChart data={data}>
          <XAxis dataKey="sprint" tick={{ fill: "rgba(255,255,255,0.3)", fontSize: 9 }} axisLine={false} tickLine={false} />
          <YAxis tick={{ fill: "rgba(255,255,255,0.3)", fontSize: 9 }} axisLine={false} tickLine={false} width={25} />
          <Tooltip contentStyle={{ background: "#1A1A2E", border: "1px solid rgba(255,255,255,0.1)", borderRadius: 8, color: "#fff", fontSize: 11 }} />
          <Area type="monotone" dataKey="points" stroke="#7C3AED" fill="rgba(124,58,237,0.15)" strokeWidth={2} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

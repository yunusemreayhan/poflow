import { useStore } from "../store/store";
import { useMemo, useState, useEffect } from "react";

export default function Dashboard() {
  const { tasks, stats, sprints } = useStore();

  // B2: Recompute today every minute to handle midnight rollover
  const [today, setToday] = useState(() => new Date().toISOString().slice(0, 10));
  useEffect(() => { const id = setInterval(() => setToday(new Date().toISOString().slice(0, 10)), 60000); return () => clearInterval(id); }, []);
  const todayStats = stats.find(s => s.date === today);
  const activeSprint = sprints.find(s => s.status === "active");
  const overdue = useMemo(() => tasks.filter(t => t.due_date && t.due_date < today && t.status !== "completed" && t.status !== "archived"), [tasks, today]);
  const recentlyUpdated = useMemo(() => [...tasks].sort((a, b) => b.updated_at.localeCompare(a.updated_at)).slice(0, 5), [tasks]);
  const activeCount = tasks.filter(t => t.status === "active").length;
  const completedToday = tasks.filter(t => t.status === "completed" && t.updated_at.startsWith(today)).length;

  return (
    <div className="space-y-4 p-1">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <Stat label="Focus today" value={todayStats ? `${Math.round(todayStats.total_focus_s / 60)}m` : "0m"} />
        <Stat label="Sessions" value={String(todayStats?.completed ?? 0)} />
        <Stat label="Active tasks" value={String(activeCount)} />
        <Stat label="Completed today" value={String(completedToday)} />
      </div>

      {activeSprint && (
        <div className="glass p-3 rounded-lg">
          <div className="text-xs text-white/40 mb-1">Active Sprint</div>
          <div className="text-sm text-white/80 font-medium">{activeSprint.name}</div>
          {activeSprint.end_date && <div className="text-[10px] text-white/30 mt-1">Ends {activeSprint.end_date}</div>}
        </div>
      )}

      {overdue.length > 0 && (
        <div className="glass p-3 rounded-lg border border-red-500/20">
          <div className="text-xs text-red-400 mb-2">⚠ Overdue ({overdue.length})</div>
          {overdue.slice(0, 5).map(t => (
            <div key={t.id} className="text-xs text-white/60 truncate">• {t.title} <span className="text-red-400/60">({t.due_date})</span></div>
          ))}
        </div>
      )}

      <div className="glass p-3 rounded-lg">
        <div className="text-xs text-white/40 mb-2">Recently Updated</div>
        {recentlyUpdated.map(t => (
          <div key={t.id} className="text-xs text-white/60 truncate flex justify-between">
            <span>• {t.title}</span>
            <span className="text-white/20 ml-2 shrink-0">{t.updated_at.slice(5, 16)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="glass p-3 rounded-lg text-center">
      <div className="text-lg font-bold text-white/80">{value}</div>
      <div className="text-[10px] text-white/30">{label}</div>
    </div>
  );
}

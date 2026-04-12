import { useStore } from "../store/store";
import { useMemo, useState, useEffect } from "react";
import { apiCall } from "../store/api";

export default function Dashboard() {
  const { tasks, stats, sprints } = useStore();
  const [activity, setActivity] = useState<{ action: string; entity_type: string; detail: string | null; created_at: string }[]>([]);

  // B2: Recompute today every minute to handle midnight rollover
  const [today, setToday] = useState(() => new Date().toISOString().slice(0, 10));
  useEffect(() => { const id = setInterval(() => setToday(new Date().toISOString().slice(0, 10)), 60000); return () => clearInterval(id); }, []);
  useEffect(() => { apiCall<typeof activity>("GET", "/api/audit?limit=10").then(d => d && setActivity(d)).catch(() => {}); }, []);
  const todayStats = stats.find(s => s.date === today);
  const activeSprint = sprints.find(s => s.status === "active");
  const overdue = useMemo(() => tasks.filter(t => t.due_date && t.due_date < today && t.status !== "completed" && t.status !== "archived"), [tasks, today]);
  const recentlyUpdated = useMemo(() => [...tasks].sort((a, b) => b.updated_at.localeCompare(a.updated_at)).slice(0, 5), [tasks]);
  const activeCount = useMemo(() => tasks.filter(t => t.status === "active").length, [tasks]);
  const completedToday = useMemo(() => tasks.filter(t => t.status === "completed" && t.updated_at.startsWith(today)).length, [tasks, today]);

  return (
    <div className="space-y-4 p-1">
      <div className="flex justify-between items-center">
        <dl className="grid grid-cols-2 md:grid-cols-4 gap-3 flex-1">
        <Stat label="Focus today" value={todayStats ? `${Math.round(todayStats.total_focus_s / 60)}m` : "0m"} />
        <Stat label="Sessions" value={String(todayStats?.completed ?? 0)} />
        <Stat label="Active tasks" value={String(activeCount)} />
        <Stat label="Completed today" value={String(completedToday)} />
      </dl>
        <button onClick={() => {
          const md = `# Dashboard ${today}\n- Focus: ${todayStats ? Math.round(todayStats.total_focus_s / 60) : 0}m\n- Sessions: ${todayStats?.completed ?? 0}\n- Active: ${activeCount}\n- Completed today: ${completedToday}\n${overdue.length ? `\n## Overdue (${overdue.length})\n${overdue.map(t => `- ${t.title} (${t.due_date})`).join("\n")}` : ""}`;
          navigator.clipboard.writeText(md);
        }} className="shrink-0 text-[10px] text-white/30 hover:text-white/60 px-2" title="Copy as Markdown">📋</button>
      </div>

      {/* U4: Weekly focus sparkline */}
      {stats.length > 1 && (() => {
        const last7 = stats.slice(-7);
        const max = Math.max(...last7.map(s => s.total_focus_s), 1);
        return (
          <div className="glass p-3 rounded-lg">
            <div className="text-xs text-white/40 mb-2">Last {last7.length} days</div>
            <div className="flex items-end gap-1 h-8">
              {last7.map(s => (
                <div key={s.date} className="flex-1 bg-[var(--color-accent)]/30 rounded-t" title={`${s.date}: ${Math.round(s.total_focus_s / 60)}m`}
                  style={{ height: `${(s.total_focus_s / max) * 100}%`, minHeight: s.total_focus_s > 0 ? 2 : 0 }} />
              ))}
            </div>
          </div>
        );
      })()}

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

      {activity.length > 0 && (
        <div className="glass p-3 rounded-lg">
          <div className="text-xs text-white/40 mb-2">Activity Timeline</div>
          {activity.map((a, i) => (
            <div key={i} className="text-xs text-white/50 truncate flex justify-between">
              <span>{a.action} {a.entity_type}{a.detail ? `: ${a.detail}` : ""}</span>
              <span className="text-white/20 ml-2 shrink-0">{a.created_at.slice(5, 16)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="glass p-3 rounded-lg text-center">
      <dd className="text-lg font-bold text-white/80">{value}</dd>
      <dt className="text-[10px] text-white/30">{label}</dt>
    </div>
  );
}

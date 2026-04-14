import { useStore } from "../store/store";
import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from "recharts";
import { Users } from "lucide-react";
import { useT } from "../i18n";
import { formatDateTime } from "../utils";
import Select from "./Select";

function HeatmapCell({ count, max, date }: { count: number; max: number; date: string }) {
  const intensity = max > 0 ? count / max : 0;
  const bg = count === 0
    ? "rgba(255,255,255,0.03)"
    : `rgba(124, 58, 237, ${0.2 + intensity * 0.8})`;
  return (
    <div
      tabIndex={0}
      role="gridcell"
      aria-label={`${date}: ${count} sessions`}
      title={`${date}: ${count} sessions`}
      className="w-4 h-4 rounded-sm cursor-pointer transition-transform hover:scale-125"
      style={{ background: bg }}
    />
  );
}

export default function History() {
  const t = useT();
  const { stats, loadStats, history, loadHistory } = useStore();
  const [userFilter, setUserFilter] = useState<string>("all");
  const [showCount, setShowCount] = useState(20);
  // U4: Date range filter
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");

  useEffect(() => {
    loadStats();
    loadHistory();
  }, []);

  // Unique users from history
  const users = useMemo(() => {
    const set = new Set(history.map((s) => s.user).filter(Boolean));
    return Array.from(set).sort();
  }, [history]);

  const filteredHistory = useMemo(() => {
    let h = userFilter === "all" ? history : history.filter((s) => s.user === userFilter);
    if (dateFrom) h = h.filter(s => s.started_at >= dateFrom);
    if (dateTo) h = h.filter(s => s.started_at <= dateTo + "T23:59:59");
    return h;
  }, [history, userFilter, dateFrom, dateTo]);

  // Rebuild stats from filtered history for per-user view
  const filteredStats = useMemo(() => {
    if (userFilter === "all") return stats;
    const map = new Map<string, { date: string; completed: number; interrupted: number; total_focus_s: number }>();
    for (const s of filteredHistory) {
      if (s.session_type !== "work") continue;
      const date = s.started_at?.slice(0, 10) ?? "";
      if (!date) continue;
      const entry = map.get(date) ?? { date, completed: 0, interrupted: 0, total_focus_s: 0 };
      if (s.status === "completed") entry.completed += 1;
      if (s.status === "interrupted") entry.interrupted += 1;
      entry.total_focus_s += s.duration_s ?? 0;
      map.set(date, entry);
    }
    return Array.from(map.values());
  }, [filteredHistory, stats, userFilter]);

  const heatmapData = useMemo(() => {
    const map = new Map(filteredStats.map((s) => [s.date, s]));
    const days: { date: string; count: number }[] = [];
    const now = new Date();
    for (let i = 364; i >= 0; i--) {
      const d = new Date(now);
      d.setDate(d.getDate() - i);
      const key = d.toISOString().slice(0, 10);
      const stat = map.get(key);
      days.push({ date: key, count: stat?.completed ?? 0 });
    }
    return days;
  }, [filteredStats]);

  const maxCount = Math.max(1, ...heatmapData.map((d) => d.count));

  const weeklyData = useMemo(() => {
    const dayNames = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    return heatmapData.slice(-7).map((d) => ({
      ...d,
      day: dayNames[new Date(d.date).getDay()],
      hours: filteredStats.find((s) => s.date === d.date)?.total_focus_s ?? 0,
    }));
  }, [heatmapData, filteredStats]);

  const totalSessions = filteredStats.reduce((a, s) => a + s.completed, 0);
  const totalFocusHours = Math.round(filteredStats.reduce((a, s) => a + s.total_focus_s, 0) / 3600);
  const streak = useMemo(() => {
    let count = 0;
    for (let i = heatmapData.length - 1; i >= 0; i--) {
      if (heatmapData[i].count > 0) count++;
      else break;
    }
    return count;
  }, [heatmapData]);

  return (
    <div className="flex flex-col gap-3 md:gap-6 p-3 md:p-8 h-full overflow-y-auto">
      {/* User filter */}
      <div className="flex items-center gap-3">
        <Users size={16} className="text-white/40" />
        <Select value={userFilter} onChange={setUserFilter} className="w-40"
          options={[{value:"all",label:"All users"}, ...users.map(u => ({value:u,label:u}))]} />
        {userFilter !== "all" && (
          <span className="text-xs text-white/40">Showing data for @{userFilter}</span>
        )}
        <input type="date" value={dateFrom} onChange={e => setDateFrom(e.target.value)}
          className="bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white/60 outline-none" />
        <span className="text-white/20 text-xs">to</span>
        <input type="date" value={dateTo} onChange={e => setDateTo(e.target.value)}
          className="bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white/60 outline-none" />
        {(dateFrom || dateTo) && <button onClick={() => { setDateFrom(""); setDateTo(""); }} className="text-xs text-white/30 hover:text-white/60">✕</button>}
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-3 gap-4">
        {[
          { label: t.totalSessions, value: totalSessions, icon: "🍅" },
          { label: t.focusHours, value: totalFocusHours, icon: "⏱️" },
          { label: t.currentStreak, value: `${streak}d`, icon: "🔥" },
        ].map((card) => (
          <motion.div
            key={card.label}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            className="glass p-5 text-center"
          >
            <div className="text-2xl mb-1" role="img" aria-label={card.label}>{card.icon}</div>
            <div className="text-2xl font-bold text-white">{card.value}</div>
            <div className="text-xs text-white/40 mt-1">{card.label}</div>
          </motion.div>
        ))}
      </div>

      {/* BL11: Focus time report */}
      {filteredStats.length > 7 && (() => {
        const last7 = filteredStats.slice(-7);
        const last30 = filteredStats.slice(-30);
        const weekFocus = last7.reduce((a, s) => a + s.total_focus_s, 0) / 3600;
        const monthFocus = last30.reduce((a, s) => a + s.total_focus_s, 0) / 3600;
        const avgDaily = last30.length > 0 ? monthFocus / last30.length : 0;
        const activeDays = last30.filter(s => s.total_focus_s > 0).length;
        const bestDay = last30.reduce((best, s) => s.total_focus_s > best.total_focus_s ? s : best, last30[0]);
        return (
          <div className="glass p-4">
            <div className="text-xs text-white/40 mb-2 font-medium">Focus Report</div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-center">
              <div><div className="text-sm text-white font-semibold">{weekFocus.toFixed(1)}h</div><div className="text-[10px] text-white/30">This week</div></div>
              <div><div className="text-sm text-white font-semibold">{monthFocus.toFixed(1)}h</div><div className="text-[10px] text-white/30">Last 30 days</div></div>
              <div><div className="text-sm text-white font-semibold">{avgDaily.toFixed(1)}h</div><div className="text-[10px] text-white/30">Avg daily</div></div>
              <div><div className="text-sm text-white font-semibold">{activeDays}/{last30.length}d</div><div className="text-[10px] text-white/30">Active days</div></div>
            </div>
            {bestDay && bestDay.total_focus_s > 0 && (
              <div className="text-[10px] text-white/20 mt-2 text-center">Best day: {bestDay.date} ({(bestDay.total_focus_s / 3600).toFixed(1)}h)</div>
            )}
            {/* BL13: Break compliance */}
            {(() => {
              const workSessions = filteredHistory.filter(s => s.session_type === "work" && s.status === "completed").length;
              const breakSessions = filteredHistory.filter(s => (s.session_type === "short_break" || s.session_type === "long_break") && s.status === "completed").length;
              if (workSessions === 0) return null;
              const ratio = breakSessions / workSessions;
              const color = ratio >= 0.8 ? "text-green-400" : ratio >= 0.5 ? "text-yellow-400" : "text-red-400";
              return (
                <div className="text-[10px] text-white/20 mt-1 text-center">
                  Break compliance: <span className={color}>{Math.round(ratio * 100)}%</span> ({breakSessions} breaks / {workSessions} work sessions)
                </div>
              );
            })()}
          </div>
        );
      })()}

      {/* BL21: Week-over-week comparison */}
      {filteredStats.length > 14 && (() => {
        const thisWeek = filteredStats.slice(-7);
        const lastWeek = filteredStats.slice(-14, -7);
        const twFocus = thisWeek.reduce((a, s) => a + s.total_focus_s, 0) / 3600;
        const lwFocus = lastWeek.reduce((a, s) => a + s.total_focus_s, 0) / 3600;
        const twSessions = thisWeek.reduce((a, s) => a + s.completed, 0);
        const lwSessions = lastWeek.reduce((a, s) => a + s.completed, 0);
        const fDelta = lwFocus > 0 ? ((twFocus - lwFocus) / lwFocus * 100) : 0;
        const sDelta = lwSessions > 0 ? ((twSessions - lwSessions) / lwSessions * 100) : 0;
        const arrow = (d: number) => d > 0 ? `↑${Math.abs(d).toFixed(0)}%` : d < 0 ? `↓${Math.abs(d).toFixed(0)}%` : "—";
        const color = (d: number) => d > 0 ? "text-green-400" : d < 0 ? "text-red-400" : "text-white/30";
        return (
          <div className="glass p-3">
            <div className="flex justify-between items-center mb-1">
              <div className="text-xs text-white/40 font-medium">This Week vs Last Week</div>
              {/* BL20: Export weekly report */}
              <button onClick={() => {
                const csv = "date,focus_hours,sessions\n" + filteredStats.slice(-7).map(s => `${s.date},${(s.total_focus_s / 3600).toFixed(2)},${s.completed}`).join("\n");
                const blob = new Blob([csv], { type: "text/csv" });
                const a = document.createElement("a"); const u = URL.createObjectURL(blob); a.href = u; a.download = "weekly_report.csv"; a.click(); URL.revokeObjectURL(u);
              }} className="text-[10px] text-white/30 hover:text-white/50">📥 CSV</button>
            </div>
            <div className="grid grid-cols-2 gap-3 text-center text-xs">
              <div>Focus: {twFocus.toFixed(1)}h <span className={color(fDelta)}>{arrow(fDelta)}</span></div>
              <div>Sessions: {twSessions} <span className={color(sDelta)}>{arrow(sDelta)}</span></div>
            </div>
          </div>
        );
      })()}

      {/* Heatmap */}
      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-4">Activity (Last 365 Days)</h3>
        <div className="flex flex-wrap gap-[3px] overflow-x-auto max-w-full" role="grid" aria-label={t.activityHeatmap}
          onKeyDown={(e) => {
            const cells = Array.from(e.currentTarget.querySelectorAll<HTMLElement>("[role=gridcell]"));
            const idx = cells.indexOf(e.target as HTMLElement);
            if (idx < 0) return;
            let next = -1;
            if (e.key === "ArrowRight") next = Math.min(idx + 1, cells.length - 1);
            else if (e.key === "ArrowLeft") next = Math.max(idx - 1, 0);
            else if (e.key === "ArrowDown") next = Math.min(idx + 7, cells.length - 1);
            else if (e.key === "ArrowUp") next = Math.max(idx - 7, 0);
            if (next >= 0) { e.preventDefault(); cells[next].focus(); }
          }}
        >
          {heatmapData.map((d) => (
            <HeatmapCell key={d.date} count={d.count} max={maxCount} date={d.date} />
          ))}
        </div>
        <div className="flex items-center gap-1.5 mt-2 text-[10px] text-white/30 justify-end">
          <span>Less</span>
          {[0, 0.25, 0.5, 0.75, 1].map((intensity, i) => (
            <div key={i} className="w-2.5 h-2.5 rounded-sm" style={{ background: `rgba(124, 58, 237, ${intensity * 0.8 + 0.1})` }} />
          ))}
          <span>More</span>
        </div>
      </div>

      {/* Weekly bar chart */}
      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-4">This Week</h3>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={weeklyData}>
            <XAxis dataKey="day" tick={{ fill: "rgba(255,255,255,0.4)", fontSize: 12 }} axisLine={false} tickLine={false} />
            <YAxis hide />
            <Tooltip
              contentStyle={{ background: "var(--color-chart-tooltip-bg)", border: "1px solid var(--color-chart-tooltip-border)", borderRadius: 8, color: "var(--color-chart-tooltip-text)" }}
              formatter={(v: unknown) => [`${v} sessions`, "Completed"]}
            />
            <Bar dataKey="count" radius={[6, 6, 0, 0]}>
              {weeklyData.map((_, i) => (
                <Cell key={i} fill={i === weeklyData.length - 1 ? "#7C3AED" : "rgba(124,58,237,0.4)"} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
        <table className="sr-only" aria-label={t.thisWeek}>
          <thead><tr><th>Day</th><th>Sessions</th></tr></thead>
          <tbody>{weeklyData.map(d => <tr key={d.day}><td>{d.day}</td><td>{d.count}</td></tr>)}</tbody>
        </table>
      </div>

      {/* Recent sessions */}
      <div className="glass p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-semibold text-white/60">{t.recentSessions}</h3>
          <button onClick={() => {
            const esc = (s: string) => { const v = s.replace(/"/g, '""'); return /[,"\n\r]/.test(v) ? `"${v}"` : v; };
            const csv = ["date,type,status,user,task,duration_min",
              ...filteredHistory.map(s => `${s.started_at?.slice(0,10)},${s.session_type},${s.status},${esc(s.user)},${esc((s.task_path || []).join(" > "))},${s.duration_s ? Math.round(s.duration_s / 60) : 0}`)
            ].join("\n");
            const blob = new Blob([csv], { type: "text/csv" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `sessions-${new Date().toISOString().slice(0,10)}.csv`;
            a.click();
            URL.revokeObjectURL(url);
          }} className="text-xs text-[var(--color-accent)] hover:underline">{t.exportCsv}</button>
        </div>
        <div className="space-y-3 max-h-64 overflow-y-auto">
          {filteredHistory.slice(0, showCount).map((s) => (
            <div key={s.id} className="flex items-center gap-3 text-xs text-white/60 py-1">
              <div className={`w-2.5 h-2.5 rounded-full shrink-0 ${
                s.status === "completed" ? "bg-[var(--color-success)]" :
                s.status === "interrupted" ? "bg-[var(--color-warning)]" : "bg-white/20"
              }`} />
              <span className="shrink-0">{s.session_type.replace("_", " ")}</span>
              <span className="shrink-0 text-white/30">@{s.user}</span>
              {s.task_path && s.task_path.length > 0 && (
                <span className="flex-1 truncate text-white/40">
                  {s.task_path.join(" › ")}
                </span>
              )}
              {(!s.task_path || s.task_path.length === 0) && <span className="flex-1" />}
              <span className="shrink-0">{s.duration_s ? `${Math.round(s.duration_s / 60)}m` : "-"}</span>
              <span className="text-white/30 shrink-0">{formatDateTime(s.started_at)}</span>
            </div>
          ))}
          {filteredHistory.length === 0 && (
            <div className="text-center text-white/20 py-6">No sessions yet</div>
          )}
          {filteredHistory.length > showCount && (
            <button onClick={() => setShowCount(c => c + 20)} className="w-full text-center text-xs text-[var(--color-accent)] hover:underline py-2">
              Show more ({filteredHistory.length - showCount} remaining)
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

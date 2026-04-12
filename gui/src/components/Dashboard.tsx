import { useStore } from "../store/store";
import { useMemo, useState, useEffect } from "react";
import { apiCall } from "../store/api";
import type { SprintBoard } from "../store/api";

export default function Dashboard() {
  const { tasks, stats, sprints, loadSprints } = useStore();
  const [activity, setActivity] = useState<{ action: string; entity_type: string; detail: string | null; created_at: string }[]>([]);

  // B2: Recompute today every minute to handle midnight rollover
  const [today, setToday] = useState(() => new Date().toISOString().slice(0, 10));
  useEffect(() => { const id = setInterval(() => setToday(new Date().toISOString().slice(0, 10)), 60000); return () => clearInterval(id); }, []);
  // B7: Load sprints for Dashboard widgets
  useEffect(() => { loadSprints(); }, [loadSprints]);
  useEffect(() => { apiCall<typeof activity>("GET", "/api/audit?limit=10").then(d => d && setActivity(d)).catch(() => {}); }, []);
  // B6: Refresh activity when tasks change
  useEffect(() => {
    const handler = () => apiCall<typeof activity>("GET", "/api/audit?limit=10").then(d => d && setActivity(d)).catch(() => {});
    window.addEventListener("sse-sprints", handler);
    return () => window.removeEventListener("sse-sprints", handler);
  }, []);
  const todayStats = stats.find(s => s.date === today);
  const activeSprint = sprints.find(s => s.status === "active");
  const overdue = useMemo(() => tasks.filter(t => t.due_date && t.due_date < today && t.status !== "completed" && t.status !== "archived").sort((a, b) => (a.due_date || "").localeCompare(b.due_date || "")), [tasks, today]);
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

      {activeSprint && <SprintProgress sprintId={activeSprint.id} name={activeSprint.name} endDate={activeSprint.end_date} />}

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

      {/* F12: Active timers from other users */}
      <ActiveTimers />

      {/* F2: Focus heatmap — year view */}
      <FocusHeatmap stats={stats} />

      {/* F5: Productivity trends — weekly comparison */}
      <ProductivityTrends stats={stats} />

      {/* F8: Focus score */}
      <FocusScore />

      {/* F22: Achievements */}
      <Achievements />

      {/* BL3: Daily standup view */}
      <StandupView today={today} tasks={tasks} />

      {/* BL9: Team workload view */}
      <WorkloadView tasks={tasks} sprints={sprints} />

      {/* BL19: Project stats */}
      <ProjectStats tasks={tasks} />
    </div>
  );
}

function ActiveTimers() {
  const [timers, setTimers] = useState<{ username: string; phase: string; task: string | null; elapsed_s: number; duration_s: number }[]>([]);
  const username = useStore(s => s.username);
  useEffect(() => {
    const load = () => apiCall<typeof timers>("GET", "/api/timer/active").then(d => d && setTimers(d.filter(t => t.username !== username))).catch(() => {});
    load();
    const id = setInterval(load, 15000);
    return () => clearInterval(id);
  }, [username]);
  if (timers.length === 0) return null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Team Activity</div>
      {timers.map((t, i) => (
        <div key={i} className="text-xs text-white/50 flex justify-between">
          <span>🍅 {t.username}{t.task ? ` — ${t.task}` : ""}</span>
          <span className="text-white/20">{Math.floor((t.duration_s - t.elapsed_s) / 60)}m left</span>
        </div>
      ))}
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

// BL1: Sprint progress widget — shows board status to all team members
function SprintProgress({ sprintId, name, endDate }: { sprintId: number; name: string; endDate: string | null }) {
  const [board, setBoard] = useState<SprintBoard | null>(null);
  useEffect(() => { apiCall<SprintBoard>("GET", `/api/sprints/${sprintId}/board`).then(setBoard).catch(() => {}); }, [sprintId]);
  const total = board ? board.todo.length + board.in_progress.length + board.blocked.length + board.done.length : 0;
  const pct = board && total > 0 ? Math.round((board.done.length / total) * 100) : 0;
  const daysLeft = endDate ? Math.max(0, Math.ceil((new Date(endDate).getTime() - Date.now()) / 86400000)) : null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="flex justify-between items-center mb-2">
        <div>
          <div className="text-xs text-white/40">Active Sprint</div>
          <div className="text-sm text-white/80 font-medium">{name}</div>
        </div>
        {daysLeft !== null && <div className="text-xs text-white/30">{daysLeft}d left</div>}
      </div>
      {board && total > 0 && (<>
        <div className="flex items-center gap-2 mb-1">
          <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
            <div className="h-full bg-green-500 rounded-full transition-all" style={{ width: `${pct}%` }} />
          </div>
          <span className="text-[10px] text-white/40">{pct}%</span>
        </div>
        <div className="flex gap-3 text-[10px] text-white/30">
          <span>📋 {board.todo.length} todo</span>
          <span>🔨 {board.in_progress.length} wip</span>
          {board.blocked.length > 0 && <span>🚫 {board.blocked.length} blocked</span>}
          <span>✅ {board.done.length} done</span>
        </div>
        {board.in_progress.length > 0 && (
          <div className="mt-2 space-y-0.5">
            <div className="text-[10px] text-white/30">In Progress:</div>
            {board.in_progress.slice(0, 3).map(t => (
              <div key={t.id} className="text-xs text-white/50 truncate">• {t.title} <span className="text-white/20">({t.user})</span></div>
            ))}
            {board.in_progress.length > 3 && <div className="text-[10px] text-white/20">+{board.in_progress.length - 3} more</div>}
          </div>
        )}
      </>)}
    </div>
  );
}

// BL3: Daily standup view — yesterday done, today planned, blockers
function StandupView({ today, tasks }: { today: string; tasks: import("../store/api").Task[] }) {
  const yesterday = useMemo(() => {
    const d = new Date(today); d.setDate(d.getDate() - 1); return d.toISOString().slice(0, 10);
  }, [today]);
  const byUser = useMemo(() => {
    const map: Record<string, { done: string[]; wip: string[]; blocked: string[] }> = {};
    for (const t of tasks) {
      if (!map[t.user]) map[t.user] = { done: [], wip: [], blocked: [] };
      // B6: Use both yesterday and today for "done" — updated_at is approximate but status must be completed
      if (t.status === "completed" && (t.updated_at.startsWith(yesterday) || t.updated_at.startsWith(today))) map[t.user].done.push(t.title);
      if (t.status === "in_progress" || t.status === "active") map[t.user].wip.push(t.title);
      if (t.status === "blocked") map[t.user].blocked.push(t.title);
    }
    return Object.entries(map).filter(([, v]) => v.done.length + v.wip.length + v.blocked.length > 0);
  }, [tasks, yesterday, today]);
  if (byUser.length === 0) return null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="flex justify-between items-center mb-2">
        <div className="text-xs text-white/40">Daily Standup</div>
        <button onClick={() => {
          const md = byUser.map(([user, { done, wip, blocked }]) =>
            `**@${user}**\n${done.length ? `✅ Done: ${done.join(", ")}\n` : ""}${wip.length ? `🔨 Working: ${wip.join(", ")}\n` : ""}${blocked.length ? `🚫 Blocked: ${blocked.join(", ")}\n` : ""}`
          ).join("\n");
          navigator.clipboard.writeText(md);
        }} className="text-[10px] text-white/30 hover:text-white/50" title="Copy standup">📋</button>
      </div>
      {byUser.map(([user, { done, wip, blocked }]) => (
        <div key={user} className="mb-2 last:mb-0">
          <div className="text-xs text-[var(--color-accent)]/70 font-medium">@{user}</div>
          {done.length > 0 && <div className="text-[10px] text-white/40 ml-2">✅ Done: {done.slice(0, 3).join(", ")}{done.length > 3 ? ` +${done.length - 3}` : ""}</div>}
          {wip.length > 0 && <div className="text-[10px] text-white/40 ml-2">🔨 Working: {wip.slice(0, 3).join(", ")}{wip.length > 3 ? ` +${wip.length - 3}` : ""}</div>}
          {blocked.length > 0 && <div className="text-[10px] text-red-400/60 ml-2">🚫 Blocked: {blocked.join(", ")}</div>}
        </div>
      ))}
    </div>
  );
}

// BL9: Team workload — hours/points per user in active sprints
function WorkloadView({ tasks, sprints }: { tasks: import("../store/api").Task[]; sprints: import("../store/api").Sprint[] }) {
  const taskSprintsMap = useStore(s => s.taskSprintsMap);
  const activeSprintIds = useMemo(() => new Set(sprints.filter(s => s.status === "active").map(s => s.id)), [sprints]);
  const workload = useMemo(() => {
    if (activeSprintIds.size === 0) return [];
    const map: Record<string, { hours: number; points: number; tasks: number }> = {};
    for (const t of tasks) {
      const inActive = (taskSprintsMap.get(t.id) || []).some(ts => activeSprintIds.has(ts.sprint_id));
      if (!inActive || t.status === "completed" || t.status === "done") continue;
      if (!map[t.user]) map[t.user] = { hours: 0, points: 0, tasks: 0 };
      map[t.user].hours += t.estimated_hours;
      map[t.user].points += t.remaining_points;
      map[t.user].tasks++;
    }
    return Object.entries(map).sort((a, b) => b[1].hours - a[1].hours);
  }, [tasks, taskSprintsMap, activeSprintIds]);
  if (workload.length === 0) return null;
  const maxHrs = Math.max(...workload.map(([, v]) => v.hours), 1);
  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Team Workload (active sprints)</div>
      {workload.map(([user, { hours, points, tasks: count }]) => (
        <div key={user} className="flex items-center gap-2 py-0.5">
          <span className="text-xs text-white/60 w-20 truncate">@{user}</span>
          <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
            <div className="h-full bg-[var(--color-accent)] rounded-full" style={{ width: `${(hours / maxHrs) * 100}%` }} />
          </div>
          <span className="text-[10px] text-white/30 w-24 text-right">{hours.toFixed(1)}h · {points}pt · {count}t</span>
        </div>
      ))}
    </div>
  );
}

// BL19: Project-level stats
function ProjectStats({ tasks }: { tasks: import("../store/api").Task[] }) {
  const projects = useMemo(() => {
    const map: Record<string, { total: number; done: number; hours: number }> = {};
    for (const t of tasks) {
      const p = t.project || "(no project)";
      if (!map[p]) map[p] = { total: 0, done: 0, hours: 0 };
      map[p].total++;
      if (t.status === "completed" || t.status === "done") map[p].done++;
      map[p].hours += t.estimated_hours;
    }
    return Object.entries(map).filter(([, v]) => v.total > 1).sort((a, b) => b[1].total - a[1].total).slice(0, 8);
  }, [tasks]);
  if (projects.length === 0) return null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Projects</div>
      {projects.map(([name, { total, done, hours }]) => (
        <div key={name} className="flex items-center gap-2 py-0.5">
          <span className="text-xs text-white/50 w-28 truncate">{name}</span>
          <div className="flex-1 h-1.5 bg-white/5 rounded-full overflow-hidden">
            <div className="h-full bg-[var(--color-accent)] rounded-full" style={{ width: `${total > 0 ? (done / total) * 100 : 0}%` }} />
          </div>
          <span className="text-[10px] text-white/30 w-20 text-right">{done}/{total} · {hours.toFixed(0)}h</span>
        </div>
      ))}
    </div>
  );
}

// F2: Focus heatmap — GitHub-style year view
function FocusHeatmap({ stats }: { stats: import("../store/api").DayStat[] }) {
  const data = useMemo(() => {
    const map = new Map<string, number>();
    for (const s of stats) map.set(s.date, Math.round(s.total_focus_s / 60));
    // Build 52 weeks of data ending today
    const today = new Date();
    const weeks: { date: string; min: number }[][] = [];
    let week: { date: string; min: number }[] = [];
    for (let i = 364; i >= 0; i--) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
      const dow = (d.getDay() + 6) % 7; // Mon=0
      if (dow === 0 && week.length > 0) { weeks.push(week); week = []; }
      week.push({ date: key, min: map.get(key) || 0 });
    }
    if (week.length > 0) weeks.push(week);
    return weeks;
  }, [stats]);

  const max = useMemo(() => Math.max(...data.flat().map(d => d.min), 1), [data]);
  const total = useMemo(() => data.flat().reduce((s, d) => s + d.min, 0), [data]);
  if (stats.length < 7) return null;

  return (
    <div className="glass p-3 rounded-lg">
      <div className="flex justify-between items-center mb-2">
        <div className="text-xs text-white/40">Focus Heatmap</div>
        <div className="text-[10px] text-white/20">{Math.round(total / 60)}h total</div>
      </div>
      <div className="flex gap-[2px] overflow-x-auto">
        {data.map((week, wi) => (
          <div key={wi} className="flex flex-col gap-[2px]">
            {week.map(d => {
              const intensity = d.min > 0 ? 0.2 + (d.min / max) * 0.8 : 0;
              return (
                <div key={d.date} title={`${d.date}: ${d.min}m`}
                  className="w-[10px] h-[10px] rounded-[2px]"
                  style={{ background: d.min === 0 ? "rgba(255,255,255,0.03)" : `rgba(124, 58, 237, ${intensity})` }} />
              );
            })}
          </div>
        ))}
      </div>
      <div className="flex items-center gap-1 mt-2 justify-end">
        <span className="text-[9px] text-white/20">Less</span>
        {[0, 0.2, 0.4, 0.6, 0.8, 1].map(v => (
          <div key={v} className="w-[10px] h-[10px] rounded-[2px]"
            style={{ background: v === 0 ? "rgba(255,255,255,0.03)" : `rgba(124, 58, 237, ${0.2 + v * 0.8})` }} />
        ))}
        <span className="text-[9px] text-white/20">More</span>
      </div>
    </div>
  );
}

// F5: Productivity trends — weekly comparison
function ProductivityTrends({ stats }: { stats: import("../store/api").DayStat[] }) {
  const weeks = useMemo(() => {
    if (stats.length < 7) return [];
    // Group stats into weeks (Mon-Sun)
    const sorted = [...stats].sort((a, b) => a.date.localeCompare(b.date));
    const result: { label: string; focusH: number; sessions: number; interrupted: number }[] = [];
    let weekStart = "";
    let acc = { focusH: 0, sessions: 0, interrupted: 0 };
    for (const s of sorted) {
      const d = new Date(s.date);
      const dow = (d.getDay() + 6) % 7;
      const monday = new Date(d);
      monday.setDate(d.getDate() - dow);
      const key = `${monday.getMonth() + 1}/${monday.getDate()}`;
      if (key !== weekStart) {
        if (weekStart) result.push({ label: weekStart, ...acc });
        weekStart = key;
        acc = { focusH: 0, sessions: 0, interrupted: 0 };
      }
      acc.focusH += s.total_focus_s / 3600;
      acc.sessions += s.completed;
      acc.interrupted += s.interrupted;
    }
    if (weekStart) result.push({ label: weekStart, ...acc });
    return result.slice(-8); // last 8 weeks
  }, [stats]);

  if (weeks.length < 2) return null;
  const curr = weeks[weeks.length - 1];
  const prev = weeks[weeks.length - 2];
  const delta = (v: number, p: number) => p === 0 ? "" : v >= p ? `↑${Math.round(((v - p) / p) * 100)}%` : `↓${Math.round(((p - v) / p) * 100)}%`;
  const maxH = Math.max(...weeks.map(w => w.focusH), 1);

  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Weekly Trends</div>
      <div className="grid grid-cols-3 gap-2 mb-3">
        <div className="text-center">
          <div className="text-sm font-bold text-white/80">{curr.focusH.toFixed(1)}h</div>
          <div className="text-[10px] text-white/30">Focus {delta(curr.focusH, prev.focusH)}</div>
        </div>
        <div className="text-center">
          <div className="text-sm font-bold text-white/80">{curr.sessions}</div>
          <div className="text-[10px] text-white/30">Sessions {delta(curr.sessions, prev.sessions)}</div>
        </div>
        <div className="text-center">
          <div className="text-sm font-bold text-white/80">{curr.sessions > 0 ? Math.round((1 - curr.interrupted / (curr.sessions + curr.interrupted)) * 100) : 0}%</div>
          <div className="text-[10px] text-white/30">Completion rate</div>
        </div>
      </div>
      <div className="flex items-end gap-1 h-16">
        {weeks.map((w, i) => (
          <div key={w.label} className="flex-1 flex flex-col items-center gap-0.5">
            <div className="w-full bg-[var(--color-accent)]/30 rounded-t transition-all"
              style={{ height: `${(w.focusH / maxH) * 100}%`, minHeight: w.focusH > 0 ? 2 : 0 }}
              title={`${w.label}: ${w.focusH.toFixed(1)}h, ${w.sessions} sessions`} />
            <span className={`text-[8px] ${i === weeks.length - 1 ? "text-[var(--color-accent)]" : "text-white/20"}`}>{w.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// F8: Focus score widget
function FocusScore() {
  const [data, setData] = useState<{ score: number; streak_days: number; components: Record<string, number> } | null>(null);
  useEffect(() => { apiCall<typeof data>("GET", "/api/analytics/focus-score").then(setData).catch(() => {}); }, []);
  if (!data || data.score === 0) return null;
  const color = data.score >= 80 ? "#10B981" : data.score >= 50 ? "#F59E0B" : "#EF4444";
  return (
    <div className="glass p-3 rounded-lg">
      <div className="flex items-center gap-3">
        <div className="relative w-14 h-14">
          <svg viewBox="0 0 36 36" className="w-full h-full -rotate-90">
            <circle cx="18" cy="18" r="15.5" fill="none" stroke="rgba(255,255,255,0.05)" strokeWidth="3" />
            <circle cx="18" cy="18" r="15.5" fill="none" stroke={color} strokeWidth="3"
              strokeDasharray={`${data.score} ${100 - data.score}`} strokeLinecap="round" />
          </svg>
          <div className="absolute inset-0 flex items-center justify-center text-sm font-bold" style={{ color }}>{Math.round(data.score)}</div>
        </div>
        <div className="flex-1">
          <div className="text-xs text-white/40">Focus Score</div>
          <div className="text-[10px] text-white/20 mt-0.5">
            🔥 {data.streak_days}d streak · Goal {data.components.goal_achievement}/30 · Consistency {data.components.consistency}/30
          </div>
        </div>
      </div>
    </div>
  );
}

// F22: Achievements widget
function Achievements() {
  const [achievements, setAchievements] = useState<{ type: string; description: string; unlocked: boolean; unlocked_at: string | null }[]>([]);
  useEffect(() => {
    // Check for new achievements, then load all
    apiCall("POST", "/api/achievements/check").catch(() => {});
    apiCall<typeof achievements>("GET", "/api/achievements").then(d => d && setAchievements(d)).catch(() => {});
  }, []);
  const unlocked = achievements.filter(a => a.unlocked);
  if (achievements.length === 0) return null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Achievements ({unlocked.length}/{achievements.length})</div>
      <div className="flex flex-wrap gap-2">
        {achievements.map(a => (
          <div key={a.type} title={a.description + (a.unlocked_at ? ` — ${a.unlocked_at.slice(0, 10)}` : "")}
            className={`text-[10px] px-2 py-1 rounded-full ${a.unlocked ? "bg-[var(--color-accent)]/20 text-[var(--color-accent)]" : "bg-white/5 text-white/15"}`}>
            {a.unlocked ? "🏆" : "🔒"} {a.description}
          </div>
        ))}
      </div>
    </div>
  );
}

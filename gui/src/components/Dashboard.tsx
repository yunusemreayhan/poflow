import { useStore } from "../store/store";
import { useMemo, useState, useEffect } from "react";
import { apiCall } from "../store/api";
import type { SprintBoard } from "../store/api";

export default function Dashboard() {
  const { tasks, stats, sprints } = useStore();
  const [activity, setActivity] = useState<{ action: string; entity_type: string; detail: string | null; created_at: string }[]>([]);

  // B2: Recompute today every minute to handle midnight rollover
  const [today, setToday] = useState(() => new Date().toISOString().slice(0, 10));
  useEffect(() => { const id = setInterval(() => setToday(new Date().toISOString().slice(0, 10)), 60000); return () => clearInterval(id); }, []);
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

      {/* BL3: Daily standup view */}
      <StandupView today={today} tasks={tasks} />
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
  const pct = total > 0 ? Math.round((board!.done.length / total) * 100) : 0;
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
      if (t.status === "completed" && t.updated_at.startsWith(yesterday)) map[t.user].done.push(t.title);
      if (t.status === "in_progress" || t.status === "active") map[t.user].wip.push(t.title);
      if (t.status === "blocked") map[t.user].blocked.push(t.title);
    }
    return Object.entries(map).filter(([, v]) => v.done.length + v.wip.length + v.blocked.length > 0);
  }, [tasks, yesterday]);
  if (byUser.length === 0) return null;
  return (
    <div className="glass p-3 rounded-lg">
      <div className="text-xs text-white/40 mb-2">Daily Standup</div>
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

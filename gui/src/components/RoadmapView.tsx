import { useEffect, useState } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface EpicGroup { id: number; name: string; created_at: string }
interface EpicDetail { group: EpicGroup; task_ids: number[]; snapshots: { date: string; total_tasks: number; done_tasks: number; total_points: number; done_points: number }[] }

const COLORS = ["#7c3aed", "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#ec4899", "#06b6d4", "#8b5cf6"];

export default function RoadmapView() {
  const tasks = useStore(s => s.tasks);
  const [epics, setEpics] = useState<EpicDetail[]>([]);

  useEffect(() => {
    apiCall<EpicGroup[]>("GET", "/api/epics").then(async (groups) => {
      const details = await Promise.all(groups.map(g => apiCall<EpicDetail>("GET", `/api/epics/${g.id}`)));
      setEpics(details);
    }).catch(e => console.error(e));
  }, []);

  // For each epic, compute progress and date range from its tasks
  const epicRows = epics.map((e, idx) => {
    const epicTasks = tasks.filter(t => e.task_ids.includes(t.id));
    const total = epicTasks.length;
    const done = epicTasks.filter(t => t.status === "completed" || t.status === "done").length;
    const pct = total > 0 ? Math.round((done / total) * 100) : 0;
    const dates = epicTasks.filter(t => t.due_date).map(t => t.due_date!).sort();
    const minDate = dates[0] || e.group.created_at.slice(0, 10);
    const maxDate = dates[dates.length - 1] || minDate;
    const totalPts = epicTasks.reduce((s, t) => s + t.remaining_points, 0);
    const donePts = epicTasks.filter(t => t.status === "completed" || t.status === "done").reduce((s, t) => s + t.remaining_points, 0);
    return { ...e, total, done, pct, minDate, maxDate, totalPts, donePts, color: COLORS[idx % COLORS.length] };
  });

  return (
    <div className="flex flex-col gap-3 md:gap-5 p-3 md:p-8 h-full overflow-y-auto">
      <div className="flex items-center gap-3">
        <span className="font-medium text-white/80 text-sm">Roadmap</span>
        <span className="text-xs text-white/30">{epics.length} epics</span>
      </div>

      {epicRows.length === 0 ? (
        <div className="flex-1 flex items-center justify-center text-white/30 text-sm">
          No epics yet. Create epics in the Tasks view to see them here.
        </div>
      ) : (
        <div className="space-y-4">
          {epicRows.map((e) => (
            <div key={e.group.id} className="glass p-4 rounded-xl">
              <div className="flex items-center gap-3 mb-3">
                <div className="w-3 h-3 rounded-full shrink-0" style={{ background: e.color }} />
                <span className="font-medium text-white/90 text-sm">{e.group.name}</span>
                <span className="text-xs text-white/30 ml-auto">{e.done}/{e.total} tasks</span>
                <span className="text-xs font-semibold" style={{ color: e.color }}>{e.pct}%</span>
              </div>

              {/* Progress bar */}
              <div className="h-6 bg-white/5 rounded-full overflow-hidden mb-2 relative">
                <div className="h-full rounded-full transition-all duration-500" style={{ width: `${e.pct}%`, background: e.color, opacity: 0.8 }} />
                {e.pct > 10 && <span className="absolute inset-0 flex items-center justify-center text-[10px] text-white/70 font-medium">{e.pct}%</span>}
              </div>

              {/* Date range + points */}
              <div className="flex items-center gap-4 text-[10px] text-white/40">
                <span>{e.minDate} → {e.maxDate}</span>
                {e.totalPts > 0 && <span>{e.donePts}/{e.totalPts} pts</span>}
              </div>

              {/* Task status breakdown */}
              {e.total > 0 && (
                <div className="flex gap-0.5 mt-2 h-1.5 rounded-full overflow-hidden">
                  {(() => {
                    const epicTasks = tasks.filter(t => e.task_ids.includes(t.id));
                    const statuses = ["completed", "done", "in_progress", "blocked", "backlog"];
                    const colors: Record<string, string> = { completed: "#22c55e", done: "#22c55e", in_progress: "#f59e0b", blocked: "#ef4444", backlog: "#6b7280" };
                    return statuses.map(s => {
                      const count = epicTasks.filter(t => t.status === s).length;
                      if (count === 0) return null;
                      return <div key={s} className="h-full" style={{ width: `${(count / e.total) * 100}%`, background: colors[s] ?? "#6b7280" }} />;
                    });
                  })()}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut } from "lucide-react";
import { useStore } from "../store/store";
import { apiCall } from "../store/api";

interface Dep { task_id: number; depends_on: number }

const STATUS_COLORS: Record<string, string> = {
  backlog: "#6b7280", active: "#3b82f6", in_progress: "#f59e0b",
  blocked: "#ef4444", completed: "#22c55e", done: "#22c55e",
};

const ROW_H = 32;
const LABEL_W = 200;
const DAY_W_OPTIONS = [12, 20, 32, 48];

export default function GanttChart() {
  const tasks = useStore(s => s.tasks);
  const [deps, setDeps] = useState<Dep[]>([]);
  const [zoomIdx, setZoomIdx] = useState(1);
  const [offsetWeeks, setOffsetWeeks] = useState(0);

  useEffect(() => { apiCall<Dep[]>("GET", "/api/dependencies").then(setDeps).catch(e => console.error(e)); }, []);

  const dayW = DAY_W_OPTIONS[zoomIdx] ?? 20;

  // Filter tasks with due dates, sort by due_date
  const dated = useMemo(() => {
    const d = tasks.filter(t => t.due_date && !t.deleted_at).sort((a, b) => (a.due_date ?? "").localeCompare(b.due_date ?? ""));
    return d;
  }, [tasks]);

  // Timeline range: 12 weeks centered on today + offset
  const today = new Date();
  const startDate = new Date(today);
  startDate.setDate(startDate.getDate() - 14 + offsetWeeks * 7);
  startDate.setDate(startDate.getDate() - startDate.getDay() + 1); // Monday
  const totalDays = 84; // 12 weeks
  const endDate = new Date(startDate);
  endDate.setDate(endDate.getDate() + totalDays);

  const dateToX = (d: string) => {
    const dt = new Date(d + "T00:00:00");
    const diff = (dt.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24);
    return Math.round(diff * dayW);
  };

  const todayX = dateToX(today.toISOString().slice(0, 10));

  // Build week labels
  const weeks = useMemo(() => {
    const w: { label: string; x: number }[] = [];
    const d = new Date(startDate);
    for (let i = 0; i < totalDays; i += 7) {
      w.push({ label: d.toLocaleDateString(undefined, { month: "short", day: "numeric" }), x: i * dayW });
      d.setDate(d.getDate() + 7);
    }
    return w;
  }, [startDate.getTime(), dayW]);

  // Task ID → row index
  const taskRowMap = new Map<number, number>();
  dated.forEach((t, i) => taskRowMap.set(t.id, i));

  const chartW = totalDays * dayW;
  

  return (
    <div className="flex flex-col gap-3 md:gap-5 p-3 md:p-8 h-full overflow-hidden">
      {/* Controls */}
      <div className="flex items-center gap-3 text-xs text-white/60">
        <span className="font-medium text-white/80">Gantt</span>
        <button onClick={() => setOffsetWeeks(w => w - 4)} className="p-1 hover:text-white" title="Earlier"><ChevronLeft size={16} /></button>
        <button onClick={() => setOffsetWeeks(0)} className="px-2 py-0.5 rounded bg-white/5 hover:bg-white/10">Today</button>
        <button onClick={() => setOffsetWeeks(w => w + 4)} className="p-1 hover:text-white" title="Later"><ChevronRight size={16} /></button>
        <div className="flex-1" />
        <button onClick={() => setZoomIdx(z => Math.max(0, z - 1))} className="p-1 hover:text-white" title="Zoom out" disabled={zoomIdx === 0}><ZoomOut size={16} /></button>
        <button onClick={() => setZoomIdx(z => Math.min(DAY_W_OPTIONS.length - 1, z + 1))} className="p-1 hover:text-white" title="Zoom in" disabled={zoomIdx === DAY_W_OPTIONS.length - 1}><ZoomIn size={16} /></button>
        <span className="text-white/30">{dated.length} tasks with dates</span>
      </div>

      {dated.length === 0 ? (
        <div className="flex-1 flex items-center justify-center text-white/30 text-sm">
          No tasks with due dates. Set due dates on tasks to see them here.
        </div>
      ) : (
        <div className="flex-1 overflow-auto border border-white/5 rounded-lg">
          <div className="flex" style={{ minWidth: LABEL_W + chartW }}>
            {/* Left: task labels */}
            <div className="shrink-0 border-r border-white/10 bg-[var(--color-bg)]" style={{ width: LABEL_W, position: "sticky", left: 0, zIndex: 10 }}>
              <div className="h-10 border-b border-white/10 flex items-center px-3 text-[10px] text-white/40 font-medium">Task</div>
              {dated.map((t) => (
                <div key={t.id} className="flex items-center px-3 gap-1.5 border-b border-white/5 text-xs text-white/70 truncate" style={{ height: ROW_H }}>
                  <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: STATUS_COLORS[t.status] ?? "#6366f1" }} />
                  <span className="truncate">{t.title}</span>
                </div>
              ))}
            </div>

            {/* Right: timeline */}
            <div className="relative" style={{ width: chartW }}>
              {/* Week headers */}
              <div className="h-10 border-b border-white/10 flex items-end relative">
                {weeks.map((w, i) => (
                  <div key={i} className="absolute text-[10px] text-white/30 pb-1 pl-1 border-l border-white/5" style={{ left: w.x }}>{w.label}</div>
                ))}
              </div>

              {/* Grid + bars */}
              <svg width={chartW} height={dated.length * ROW_H} className="block">
                {/* Weekend shading */}
                {Array.from({ length: totalDays }, (_, i) => {
                  const d = new Date(startDate);
                  d.setDate(d.getDate() + i);
                  const dow = d.getDay();
                  if (dow === 0 || dow === 6) return <rect key={i} x={i * dayW} y={0} width={dayW} height={dated.length * ROW_H} fill="rgba(255,255,255,0.02)" />;
                  return null;
                })}

                {/* Today line */}
                {todayX >= 0 && todayX <= chartW && (
                  <line x1={todayX} y1={0} x2={todayX} y2={dated.length * ROW_H} stroke="#7c3aed" strokeWidth={2} strokeDasharray="4,4" opacity={0.6} />
                )}

                {/* Task bars */}
                {dated.map((t, i) => {
                  const dueX = dateToX(t.due_date!);
                  // Bar: estimate duration before due date, or 1 day minimum
                  const estDays = Math.max(1, Math.ceil((t.estimated_hours || 1) / 8));
                  const barStart = dueX - estDays * dayW;
                  const barW = Math.max(dayW, estDays * dayW);
                  const y = i * ROW_H + 6;
                  const color = STATUS_COLORS[t.status] ?? "#6366f1";
                  const isOverdue = t.due_date! < today.toISOString().slice(0, 10) && t.status !== "completed" && t.status !== "done";
                  return (
                    <g key={t.id}>
                      <rect x={barStart} y={y} width={barW} height={ROW_H - 12} rx={4} fill={color} opacity={0.7} />
                      {isOverdue && <rect x={barStart} y={y} width={barW} height={ROW_H - 12} rx={4} fill="none" stroke="#ef4444" strokeWidth={2} strokeDasharray="3,2" />}
                      {/* Due date diamond */}
                      <polygon points={`${dueX},${y - 2} ${dueX + 5},${y + 4} ${dueX},${y + 10} ${dueX - 5},${y + 4}`} fill={isOverdue ? "#ef4444" : "#fff"} opacity={0.8} />
                    </g>
                  );
                })}

                {/* Dependency arrows */}
                {deps.map((dep, i) => {
                  const fromRow = taskRowMap.get(dep.depends_on);
                  const toRow = taskRowMap.get(dep.task_id);
                  if (fromRow === undefined || toRow === undefined) return null;
                  const fromTask = dated[fromRow];
                  const toTask = dated[toRow];
                  if (!fromTask?.due_date || !toTask?.due_date) return null;
                  const x1 = dateToX(fromTask.due_date);
                  const y1 = fromRow * ROW_H + ROW_H / 2;
                  const x2 = dateToX(toTask.due_date) - Math.max(1, Math.ceil((toTask.estimated_hours || 1) / 8)) * dayW;
                  const y2 = toRow * ROW_H + ROW_H / 2;
                  const midX = x1 + 10;
                  return (
                    <g key={`dep-${i}`}>
                      <path d={`M${x1},${y1} L${midX},${y1} L${midX},${y2} L${x2},${y2}`} fill="none" stroke="rgba(124,58,237,0.5)" strokeWidth={1.5} />
                      <polygon points={`${x2},${y2 - 3} ${x2 - 6},${y2} ${x2},${y2 + 3}`} fill="rgba(124,58,237,0.7)" />
                    </g>
                  );
                })}
              </svg>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

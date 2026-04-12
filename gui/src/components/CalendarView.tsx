import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useStore } from "../store/store";
import type { Task } from "../store/api";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

export default function CalendarView() {
  const tasks = useStore(s => s.tasks);
  const stats = useStore(s => s.stats);
  const loadStats = useStore(s => s.loadStats);
  useEffect(() => { loadStats(); }, []);
  const [offset, setOffset] = useState(0); // months from current
  const [selected, setSelected] = useState<string | null>(null);

  const today = new Date();
  const viewDate = new Date(today.getFullYear(), today.getMonth() + offset, 1);
  const year = viewDate.getFullYear();
  const month = viewDate.getMonth();
  const monthName = viewDate.toLocaleString(undefined, { month: "long", year: "numeric" });

  // Build calendar grid
  const cells = useMemo(() => {
    const first = new Date(year, month, 1);
    const startDay = (first.getDay() + 6) % 7; // Monday=0
    const daysInMonth = new Date(year, month + 1, 0).getDate();
    const grid: { date: string; day: number; inMonth: boolean }[] = [];
    // Previous month padding
    const prevDays = new Date(year, month, 0).getDate();
    for (let i = startDay - 1; i >= 0; i--) {
      const d = prevDays - i;
      const dt = new Date(year, month - 1, d);
      grid.push({ date: fmt(dt), day: d, inMonth: false });
    }
    for (let d = 1; d <= daysInMonth; d++) {
      grid.push({ date: fmt(new Date(year, month, d)), day: d, inMonth: true });
    }
    // Next month padding to fill 6 rows
    const remaining = 42 - grid.length;
    for (let d = 1; d <= remaining; d++) {
      const dt = new Date(year, month + 1, d);
      grid.push({ date: fmt(dt), day: d, inMonth: false });
    }
    return grid;
  }, [year, month]);

  // Tasks by due_date
  const tasksByDate = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const t of tasks) {
      if (t.due_date && t.status !== "archived" && !t.deleted_at) {
        const list = map.get(t.due_date) || [];
        list.push(t);
        map.set(t.due_date, list);
      }
    }
    return map;
  }, [tasks]);

  // Focus minutes by date
  const focusByDate = useMemo(() => {
    const map = new Map<string, number>();
    for (const s of stats) map.set(s.date, s.total_focus_s);
    return map;
  }, [stats]);

  const todayStr = fmt(today);
  const selectedTasks = selected ? tasksByDate.get(selected) || [] : [];

  return (
    <div className="p-4 space-y-3">
      <div className="flex items-center gap-3">
        <button onClick={() => setOffset(o => o - 1)} className="p-1 text-white/50 hover:text-white" aria-label="Previous month"><ChevronLeft size={18} /></button>
        <h2 className="text-lg font-semibold text-white flex-1 text-center">{monthName}</h2>
        <button onClick={() => setOffset(o => o + 1)} className="p-1 text-white/50 hover:text-white" aria-label="Next month"><ChevronRight size={18} /></button>
        {offset !== 0 && <button onClick={() => setOffset(0)} className="text-xs text-[var(--color-accent)]">Today</button>}
      </div>

      {/* Day headers */}
      <div className="grid grid-cols-7 gap-1">
        {DAYS.map(d => <div key={d} className="text-center text-[10px] text-white/30 font-medium py-1">{d}</div>)}
      </div>

      {/* Calendar grid */}
      <div className="grid grid-cols-7 gap-1" role="grid" aria-label="Calendar"
        onKeyDown={(e) => {
          // V29-19: Arrow key navigation
          const target = e.target as HTMLElement;
          const idx = Array.from(target.parentElement?.children || []).indexOf(target);
          if (idx < 0) return;
          let next = -1;
          if (e.key === "ArrowRight") next = idx + 1;
          else if (e.key === "ArrowLeft") next = idx - 1;
          else if (e.key === "ArrowDown") next = idx + 7;
          else if (e.key === "ArrowUp") next = idx - 7;
          if (next >= 0 && next < cells.length) {
            (target.parentElement?.children[next] as HTMLElement)?.focus();
            e.preventDefault();
          }
        }}>
        {cells.map((cell, i) => {
          const dueTasks = tasksByDate.get(cell.date) || [];
          const focusMin = Math.round((focusByDate.get(cell.date) || 0) / 60);
          const isToday = cell.date === todayStr;
          const isSelected = cell.date === selected;
          const overdue = dueTasks.some(t => t.status !== "completed" && t.status !== "done" && cell.date < todayStr);
          return (
            <button key={i} onClick={() => setSelected(isSelected ? null : cell.date)}
              className={`relative p-1 rounded-lg text-left min-h-[60px] md:min-h-[72px] transition-all ${
                !cell.inMonth ? "opacity-30" :
                isSelected ? "bg-[var(--color-accent)]/20 ring-1 ring-[var(--color-accent)]" :
                isToday ? "bg-white/10 ring-1 ring-white/20" : "bg-white/[0.02] hover:bg-white/5"
              }`}>
              <div className={`text-xs font-medium ${isToday ? "text-[var(--color-accent)]" : "text-white/60"}`}>{cell.day}</div>
              {dueTasks.length > 0 && (
                <div className="mt-0.5 space-y-0.5">
                  {dueTasks.slice(0, 2).map(t => (
                    <div key={t.id} className={`text-[8px] md:text-[9px] truncate px-0.5 rounded ${
                      t.status === "completed" || t.status === "done" ? "text-green-400/60 line-through" :
                      overdue ? "text-red-400/80" : "text-white/50"
                    }`}>• {t.title}</div>
                  ))}
                  {dueTasks.length > 2 && <div className="text-[8px] text-white/20">+{dueTasks.length - 2}</div>}
                </div>
              )}
              {focusMin > 0 && <div className="absolute bottom-0.5 right-1 text-[8px] text-[var(--color-accent)]/40">{focusMin}m</div>}
            </button>
          );
        })}
      </div>

      {/* Selected day detail */}
      {selected && (
        <div className="glass p-3 rounded-lg space-y-1">
          <div className="text-xs text-white/40">{selected} — {selectedTasks.length} task{selectedTasks.length !== 1 ? "s" : ""} due</div>
          {selectedTasks.length === 0 && <div className="text-xs text-white/20">No tasks due this day</div>}
          {selectedTasks.map(t => (
            <div key={t.id} className={`text-xs flex items-center gap-2 ${t.status === "completed" ? "text-green-400/60 line-through" : "text-white/70"}`}>
              <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: t.priority >= 4 ? "#EF4444" : t.priority >= 3 ? "#F59E0B" : "#10B981" }} />
              <span className="truncate flex-1">{t.title}</span>
              <span className="text-white/20 shrink-0">{t.status}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function fmt(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

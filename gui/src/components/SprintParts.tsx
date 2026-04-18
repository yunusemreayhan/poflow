import { useState, useEffect, useCallback, useMemo } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { matchSearch } from "../utils";
import type { SprintBoard, SprintDetail, Task } from "../store/api";
import TaskList from "./TaskList";
import { useT } from "../i18n";

export function BoardView({ board, reload, wipLimit: wipLimitProp }: { board: SprintBoard; reload: () => void; wipLimit?: number }) {
  const tl = useT();
  const changeStatus = useCallback(async (taskId: number, status: string) => {
    await apiCall("PUT", `/api/tasks/${taskId}`, { status });
    reload();
  }, [reload]);
  // U2: Touch drag state
  const [touchDrag, setTouchDrag] = useState<{ id: number; startX: number } | null>(null);
  // B7: Use store's taskLabelsMap instead of N+1 API calls
  const taskLabels = useStore(s => s.taskLabelsMap);
  // BL7: Detect blocked tasks (unresolved dependencies)
  const [blockedBy, setBlockedBy] = useState<Record<number, string[]>>({});
  useEffect(() => {
    // P2: Only fetch dependencies if there are non-done tasks that could be blocked
    const nonDone = [...board.todo, ...board.in_progress, ...board.blocked];
    if (nonDone.length === 0) { setBlockedBy({}); return; }
    apiCall<{ task_id: number; depends_on: number }[]>("GET", "/api/dependencies").then(deps => {
      if (!deps) return;
      const allTasks = [...board.todo, ...board.in_progress, ...board.blocked, ...board.done];
      const doneIds = new Set(board.done.map(t => t.id));
      const taskMap = new Map(allTasks.map(t => [t.id, t.title]));
      const result: Record<number, string[]> = {};
      for (const d of deps) {
        if (taskMap.has(d.task_id) && !doneIds.has(d.depends_on)) {
          const depTitle = taskMap.get(d.depends_on) || `#${d.depends_on}`;
          (result[d.task_id] ||= []).push(depTitle);
        }
      }
      setBlockedBy(result);
    }).catch(() => {});
  }, [board]);

  const WIP_LIMIT = wipLimitProp ?? 5;
  const Column = useCallback(({ title, tasks, color, status }: { title: string; tasks: Task[]; color: string; status: string }) => (
    <div className="flex-1 min-w-[200px] md:min-w-0 rounded-lg border-2 border-transparent transition-colors shrink-0 md:shrink" role="list" aria-label={`${title} tasks`}
      onDragOver={e => { e.preventDefault(); e.currentTarget.style.borderColor = "var(--color-accent)"; e.currentTarget.style.background = "rgba(124,58,237,0.05)"; }}
      onDragLeave={e => { e.currentTarget.style.borderColor = "transparent"; e.currentTarget.style.background = ""; }}
      onDrop={e => { e.currentTarget.style.borderColor = "transparent"; e.currentTarget.style.background = ""; const id = Number(e.dataTransfer.getData("text/plain")); if (id) changeStatus(id, status); }}>
      <div className={`text-xs font-medium mb-2 ${status === "in_progress" && tasks.length > WIP_LIMIT ? "text-red-400" : color}`}>
        {title} ({tasks.length}){status === "in_progress" && tasks.length > WIP_LIMIT && <span className="ml-1 text-red-400/70">⚠ WIP &gt; {WIP_LIMIT}</span>}
      </div>
      <div className="space-y-1.5 min-h-[40px] rounded p-1 transition-colors">
        {tasks.map(t => (
          <div key={t.id} draggable tabIndex={0} role="listitem"
            onKeyDown={e => {
              const statusOrder = ["backlog", "in_progress", "blocked", "completed"];
              const curIdx = statusOrder.indexOf(status);
              if (e.key === "ArrowRight" && curIdx < statusOrder.length - 1) { e.preventDefault(); changeStatus(t.id, statusOrder[curIdx + 1]); }
              else if (e.key === "ArrowLeft" && curIdx > 0) { e.preventDefault(); changeStatus(t.id, statusOrder[curIdx - 1]); }
            }}
            onDragStart={e => { e.dataTransfer.setData("text/plain", String(t.id)); (e.target as HTMLElement).style.opacity = "0.4"; }}
            onDragEnd={e => { (e.target as HTMLElement).style.opacity = "1"; }}
            onTouchStart={e => setTouchDrag({ id: t.id, startX: e.touches[0].clientX })}
            onTouchEnd={e => {
              if (!touchDrag || touchDrag.id !== t.id) return;
              const dx = e.changedTouches[0].clientX - touchDrag.startX;
              const statusOrder = ["backlog", "in_progress", "blocked", "completed"];
              const curIdx = statusOrder.indexOf(status);
              if (dx > 60 && curIdx < statusOrder.length - 1) changeStatus(t.id, statusOrder[curIdx + 1]);
              else if (dx < -60 && curIdx > 0) changeStatus(t.id, statusOrder[curIdx - 1]);
              setTouchDrag(null);
            }}
            className="bg-[var(--color-surface)] p-2 rounded border border-white/5 group cursor-grab active:cursor-grabbing">
            <div className="text-xs text-white/90 truncate">{t.title}</div>
            {taskLabels.get(t.id) && <div className="flex gap-0.5 mt-0.5 flex-wrap">{taskLabels.get(t.id)!.map(l => <span key={l.name} className="text-[8px] px-1 rounded" style={{ background: l.color + "30", color: l.color }}>{l.name}</span>)}</div>}
            {blockedBy[t.id] && <div className="text-[9px] text-red-400/70 mt-0.5 truncate" title={`Blocked by: ${blockedBy[t.id].join(", ")}`}>🚫 {blockedBy[t.id].length} dep{blockedBy[t.id].length > 1 ? "s" : ""} unresolved</div>}
            <div className="text-[10px] text-white/30 flex gap-1 mt-1">
              {t.estimated_hours > 0 && <span>{t.estimated_hours}h</span>}
              {t.remaining_points > 0 && <span>{t.remaining_points}pt</span>}
              <span>👤{t.user}</span>
            </div>
            <div className="hidden group-hover:flex gap-1 mt-1">
              {title !== tl.todo && <button onClick={() => changeStatus(t.id, "backlog")} className="text-[10px] text-white/30 hover:text-white">{tl.moveToTodo}</button>}
              {title !== tl.inProgress && <button onClick={() => changeStatus(t.id, "in_progress")} className="text-[10px] text-white/30 hover:text-yellow-400">→{tl.wip}</button>}
              {title !== tl.blocked && <button onClick={() => changeStatus(t.id, "blocked")} className="text-[10px] text-white/30 hover:text-red-400">→{tl.blocked}</button>}
              {title !== tl.done && <button onClick={() => changeStatus(t.id, "completed")} className="text-[10px] text-white/30 hover:text-green-400">→{tl.done}</button>}
            </div>
          </div>
        ))}
      </div>
    </div>
  ), [changeStatus, touchDrag, taskLabels, blockedBy, wipLimitProp]);

  const total = board.todo.length + board.in_progress.length + board.blocked.length + board.done.length;
  const pct = total > 0 ? Math.round((board.done.length / total) * 100) : 0;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3 text-xs text-white/50">
        <span>{total} tasks</span>
        <div className="flex-1 h-1.5 bg-white/5 rounded-full overflow-hidden">
          <div className="h-full bg-green-500 rounded-full transition-all" style={{ width: `${pct}%` }} />
        </div>
        <span>{pct}% done</span>
      </div>
      <div className="flex gap-3 overflow-x-auto min-w-0 pb-2">
      <Column title={tl.todo} tasks={board.todo} color="text-white/60" status="backlog" />
      <Column title={tl.inProgress} tasks={board.in_progress} color="text-yellow-400" status="in_progress" />
      {board.blocked.length > 0 && <Column title={tl.blocked} tasks={board.blocked} color="text-red-400" status="blocked" />}
      <Column title={tl.done} tasks={board.done} color="text-green-400" status="completed" />
      </div>
    </div>
  );
}

export function BacklogView({ sprintId, taskIds, reload, capacityHours, tasks: sprintTasks }: { sprintId: number; taskIds: Set<number>; reload: () => void; capacityHours?: number | null; tasks?: Task[] }) {
  const leafOnly = useStore(s => s.config?.leaf_only_mode ?? false);
  const tasks = useStore(s => s.tasks);
  const taskMap = useMemo(() => new Map(tasks.map(t => [t.id, t])), [tasks]);
  const [rootIds, setRootIds] = useState<number[]>([]);
  const [scopeIds, setScopeIds] = useState<Set<number> | null>(null);
  const [rootSearch, setRootSearch] = useState("");

  const loadRoots = useCallback(async () => {
    const r = await apiCall<number[]>("GET", `/api/sprints/${sprintId}/roots`);
    if (r) {
      setRootIds(r);
      if (r.length > 0) {
        const scope = await apiCall<number[]>("GET", `/api/sprints/${sprintId}/scope`);
        if (scope) setScopeIds(new Set(scope));
      } else {
        setScopeIds(null);
      }
    }
  }, [sprintId]);

  useEffect(() => { loadRoots(); }, [loadRoots]);

  const addRoot = async (taskId: number) => {
    await apiCall("POST", `/api/sprints/${sprintId}/roots`, { task_ids: [taskId] });
    loadRoots();
  };
  const removeRoot = async (taskId: number) => {
    await apiCall("DELETE", `/api/sprints/${sprintId}/roots/${taskId}`);
    loadRoots();
  };

  const addTask = async (taskId: number) => {
    await apiCall("POST", `/api/sprints/${sprintId}/tasks`, { task_ids: [taskId] });
    reload();
  };
  const removeTask = async (taskId: number) => {
    await apiCall("DELETE", `/api/sprints/${sprintId}/tasks/${taskId}`);
    reload();
  };

  const rootTasks = tasks.filter(t => t.parent_id === null);
  const rootIdSet = new Set(rootIds);

  return (
    <div className="space-y-4">
      {/* Root task scoping */}
      <div className="space-y-1">
        <div className="text-xs text-white/50 font-medium">Sprint Scope (root tasks)</div>
        {rootIds.map(rid => {
          const t = taskMap.get(rid);
          return t ? (
            <div key={rid} className="flex items-center gap-2 text-xs text-white/70">
              <span className="flex-1 truncate">{t.title}</span>
              <button onClick={() => removeRoot(rid)} className="text-red-400/50 hover:text-red-400">✕</button>
            </div>
          ) : null;
        })}
        {rootIds.length === 0 && <div className="text-xs text-white/20">No scope — showing all tasks</div>}
        <input placeholder="Search root tasks to scope..." value={rootSearch} onChange={e => setRootSearch(e.target.value)}
          className="w-full bg-transparent border-b border-white/10 text-white text-xs outline-none pb-1 mt-1" />
        {rootSearch && (
          <div className="max-h-20 overflow-y-auto space-y-0.5">
            {rootTasks.filter(t => !rootIdSet.has(t.id) && matchSearch(t.title, rootSearch)).slice(0, 15).map(t => (
              <button key={t.id} onClick={() => { addRoot(t.id); setRootSearch(""); }}
                className="w-full text-left text-xs text-white/50 hover:text-green-400 truncate py-0.5">
                + {t.title}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Sprint tasks — drop zone for drag-and-drop from available tasks */}
      <div
        onDragOver={e => { e.preventDefault(); e.currentTarget.style.borderColor = "var(--color-accent)"; }}
        onDragLeave={e => { e.currentTarget.style.borderColor = "transparent"; }}
        onDrop={e => { e.currentTarget.style.borderColor = "transparent"; const id = Number(e.dataTransfer.getData("text/plain")); if (id && !taskIds.has(id)) addTask(id); }}
        className="border-2 border-transparent rounded-lg transition-colors"
      >
        <div className="text-xs text-white/50 mb-1 font-medium">
          Sprint Tasks (click ✕ to remove, or drag from below)
          {/* BL18: Capacity indicator */}
          {sprintTasks && sprintTasks.length > 0 && (() => {
            const totalHrs = sprintTasks.reduce((s, t) => s + t.estimated_hours, 0);
            const totalPts = sprintTasks.reduce((s, t) => s + t.remaining_points, 0);
            const overCapacity = capacityHours && totalHrs > capacityHours;
            return (
              <span className={`ml-2 ${overCapacity ? "text-red-400" : "text-white/30"}`}>
                ({totalHrs.toFixed(1)}h{capacityHours ? ` / ${capacityHours}h` : ""} · {totalPts}pt)
                {overCapacity && <span role="alert"> ⚠ over capacity</span>}
              </span>
            );
          })()}
        </div>
        <div className="space-y-1 max-h-[50vh] overflow-y-auto">
          {/* BL19: Unestimated task warning */}
          {sprintTasks && (() => {
            const unest = sprintTasks.filter(t => t.estimated_hours === 0 && t.remaining_points === 0 && t.status !== "completed");
            return unest.length > 0 ? (
              <div className="text-[10px] text-amber-400/70 mb-1">⚠ {unest.length} task{unest.length > 1 ? "s" : ""} without estimates</div>
            ) : null;
          })()}
          {[...taskIds].length === 0 && <div className="text-xs text-white/20 py-4 text-center">📋 No tasks yet — add tasks from the task list</div>}
          <TaskList selectMode onSelect={removeTask} selectedTaskId={null} votedTaskIds={taskIds}
            selectLabel="✕" selectClassName="text-red-400 hover:text-red-300" filterIds={taskIds} />
        </div>
      </div>

      {/* Available tasks — scoped to root descendants if set */}
      <div>
        <div className="text-xs text-white/50 mb-1 font-medium">Available Tasks (click + to add){leafOnly ? " — leaf only" : ""}{scopeIds ? " — scoped" : ""}</div>
        <TaskList selectMode onSelect={addTask} selectedTaskId={null} votedTaskIds={new Set()}
          selectLabel="+" selectClassName="text-green-400 hover:text-green-300" excludeIds={taskIds}
          leafOnly={leafOnly} filterIds={scopeIds ?? undefined} />
      </div>
    </div>
  );
}

export function SummaryView({ detail }: { detail: SprintDetail }) {
  const tasks = detail.tasks;
  const done = tasks.filter(t => t.status === "completed" || t.status === "done");
  const totalPts = tasks.reduce((s, t) => s + t.remaining_points, 0);
  const donePts = done.reduce((s, t) => s + t.remaining_points, 0);
  const totalHrs = tasks.reduce((s, t) => s + t.estimated_hours, 0);
  const doneHrs = done.reduce((s, t) => s + t.estimated_hours, 0);

  // Per-user breakdown
  const byUser: Record<string, { total: number; done: number }> = {};
  tasks.forEach(t => {
    if (!byUser[t.user]) byUser[t.user] = { total: 0, done: 0 };
    byUser[t.user].total++;
    if (t.status === "completed" || t.status === "done") byUser[t.user].done++;
  });

  const s = detail.sprint;
  const days = s.start_date && s.end_date
    ? Math.max(1, Math.ceil((new Date(s.end_date).getTime() - new Date(s.start_date).getTime()) / 86400000))
    : null;

  const Stat = ({ label, value, sub }: { label: string; value: string; sub?: string }) => (
    <div className="bg-[var(--color-surface)] p-3 rounded-lg border border-white/5">
      <div className="text-[10px] text-white/40 uppercase">{label}</div>
      <div className="text-lg text-white font-semibold">{value}</div>
      {sub && <div className="text-[10px] text-white/30">{sub}</div>}
    </div>
  );

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-3 gap-2">
        <Stat label="Tasks" value={`${done.length}/${tasks.length}`} sub={`${tasks.length - done.length} remaining`} />
        <Stat label="Points" value={`${donePts}/${totalPts}`} sub={`${totalPts - donePts} remaining`} />
        <Stat label="Hours" value={`${doneHrs.toFixed(1)}/${totalHrs.toFixed(1)}`} sub={`${(totalHrs - doneHrs).toFixed(1)} remaining`} />
      </div>
      {days && (
        <div className="grid grid-cols-2 gap-2">
          <Stat label="Sprint Duration" value={`${days} days`} />
          <Stat label="Velocity" value={`${(donePts / days).toFixed(1)} pts/day`} sub={`${(doneHrs / days).toFixed(1)} hrs/day`} />
        </div>
      )}
      <div>
        <div className="text-xs text-white/50 mb-1 font-medium">Team Breakdown</div>
        {Object.entries(byUser).map(([user, { total: uTotal, done: uDone }]) => (
          <div key={user} className="flex items-center gap-2 py-1">
            <span className="text-xs text-white/70 w-24 truncate">👤 {user}</span>
            <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
              <div className="h-full bg-[var(--color-accent)] rounded-full" style={{ width: `${uTotal > 0 ? (uDone / uTotal) * 100 : 0}%` }} />
            </div>
            <span className="text-[10px] text-white/40">{uDone}/{uTotal}</span>
          </div>
        ))}
      </div>
      {/* BL12: Estimation accuracy — estimate vs actual */}
      {done.filter(t => t.estimated > 0 && t.actual > 0).length > 0 && (
        <div>
          <div className="text-xs text-white/50 mb-1 font-medium">Estimation Accuracy</div>
          {done.filter(t => t.estimated > 0 && t.actual > 0).map(t => {
            const ratio = t.actual / t.estimated;
            const color = ratio <= 1.1 ? "text-green-400" : ratio <= 1.5 ? "text-yellow-400" : "text-red-400";
            return (
              <div key={t.id} className="flex items-center gap-2 py-0.5 text-xs">
                <span className="text-white/50 flex-1 truncate">{t.title}</span>
                <span className="text-white/30">{t.estimated}→{t.actual}</span>
                <span className={`${color} w-12 text-right`}>{ratio <= 1 ? "✓" : `+${Math.round((ratio - 1) * 100)}%`}</span>
              </div>
            );
          })}
        </div>
      )}
      {s.status === "completed" && (
        <div className="bg-[var(--color-surface)] p-3 rounded-lg border border-white/5 space-y-2">
          <div className="text-xs text-white/50 font-medium">Retrospective</div>
          <div className="grid grid-cols-2 gap-2 text-xs">
            <div>
              <div className="text-green-400/60 mb-1">✅ Done ({done.length})</div>
              {done.slice(0, 5).map(t => <div key={t.id} className="text-white/50 truncate">• {t.title}</div>)}
              {done.length > 5 && <div className="text-white/30">+{done.length - 5} more</div>}
            </div>
            <div>
              <div className="text-amber-400/60 mb-1">⏳ Carried Over ({tasks.length - done.length})</div>
              {tasks.filter(t => t.status !== "completed").slice(0, 5).map(t => <div key={t.id} className="text-white/50 truncate">• {t.title}</div>)}
            </div>
          </div>
          {totalPts > 0 && <div className="text-[10px] text-white/30">Completion: {((donePts / totalPts) * 100).toFixed(0)}%</div>}
        </div>
      )}
    </div>
  );
}

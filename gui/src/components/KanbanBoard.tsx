import { useMemo, useState, useCallback } from "react";
import { useStore } from "../store/store";
import type { Task } from "../store/api";
import Select from "./Select";

const COLUMNS = [
  { id: "backlog", label: "Backlog", color: "#6C7A89" },
  { id: "active", label: "Active", color: "#F59E0B" },
  { id: "in_progress", label: "In Progress", color: "#3B82F6" },
  { id: "blocked", label: "Blocked", color: "#EF4444" },
  { id: "completed", label: "Done", color: "#10B981" },
] as const;


export default function KanbanBoard() {
  const { tasks, updateTask, teamScope } = useStore();
  const [dragId, setDragId] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<string | null>(null);
  const [groupBy, setGroupBy] = useState<"none" | "project" | "user">("none");

  const leafOnly = useStore(s => s.config?.leaf_only_mode ?? false);

  const filtered = useMemo(() => {
    let t = tasks.filter(t => t.status !== "archived" && !t.deleted_at);
    if (teamScope) t = t.filter(task => teamScope.has(task.id));
    if (leafOnly) {
      const parentIds = new Set(t.map(tk => tk.parent_id).filter(Boolean));
      t = t.filter(task => !parentIds.has(task.id));
    }
    return t;
  }, [tasks, teamScope, leafOnly]);

  // Build ancestor path map: task.id -> ["grandparent title", "parent title"]
  const ancestorMap = useMemo(() => {
    const byId = new Map(tasks.map(t => [t.id, t]));
    const map = new Map<number, string[]>();
    for (const t of filtered) {
      const path: string[] = [];
      let cur = t.parent_id ? byId.get(t.parent_id) : undefined;
      while (cur) {
        path.unshift(cur.title);
        cur = cur.parent_id ? byId.get(cur.parent_id) : undefined;
      }
      if (path.length) map.set(t.id, path);
    }
    return map;
  }, [tasks, filtered]);

  const columns = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const col of COLUMNS) map.set(col.id, []);
    for (const t of filtered) {
      const col = t.status === "done" ? "completed" : t.status === "estimated" ? "backlog" : t.status;
      const list = map.get(col);
      if (list) list.push(t);
      else map.get("backlog")!.push(t);
    }
    return map;
  }, [filtered]);

  const onDragStart = useCallback((e: React.DragEvent, taskId: number) => {
    setDragId(taskId);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(taskId));
  }, []);

  const onDrop = useCallback(async (e: React.DragEvent, colId: string) => {
    e.preventDefault();
    setDragOver(null);
    const taskId = dragId || Number(e.dataTransfer.getData("text/plain"));
    if (!taskId) return;
    setDragId(null);
    const task = tasks.find(t => t.id === taskId);
    if (!task || task.status === colId || (task.status === "done" && colId === "completed") || (task.status === "estimated" && colId === "backlog")) return;
    await updateTask(taskId, { status: colId });
  }, [dragId, tasks, updateTask]);

  return (
    <div className="p-3 md:p-8 h-full flex flex-col gap-3 md:gap-5 overflow-hidden">
      <div className="glass p-4 flex items-center gap-3">
        <h2 className="text-lg font-semibold text-white flex-1">Kanban Board</h2>
        <Select value={groupBy} onChange={v => setGroupBy(v as typeof groupBy)}
          options={[{value:"none",label:"No grouping"},{value:"project",label:"By project"},{value:"user",label:"By assignee"}]} />
      </div>

      <div className="flex-1 overflow-x-auto">
        <div className="flex gap-4 min-w-max h-full">
          {COLUMNS.map(col => {
            const colTasks = columns.get(col.id) || [];
            const isOver = dragOver === col.id;
            return (
              <div key={col.id}
                className={`w-60 md:w-72 flex flex-col rounded-xl bg-white/[0.03] border transition-colors ${
                  isOver ? "border-[var(--color-accent)]/50 bg-[var(--color-accent)]/5" : "border-white/5"
                }`}
                onDragOver={e => { e.preventDefault(); setDragOver(col.id); }}
                onDragLeave={() => setDragOver(null)}
                onDrop={e => onDrop(e, col.id)}>
                <div className="flex items-center gap-2 px-4 py-3 border-b border-white/5">
                  <span className="w-2 h-2 rounded-full" style={{ background: col.color }} />
                  <span className="text-xs font-medium text-white/70">{col.label}</span>
                  <span className="text-[10px] text-white/30 ml-auto">{colTasks.length}</span>
                </div>
                <div className="flex-1 overflow-y-auto p-3 space-y-2" role="list">
                  {colTasks.length === 0 && <div className="text-[10px] text-white/15 text-center py-4">Drop tasks here</div>}
                  {groupBy === "none"
                    ? colTasks.map(t => <KanbanCard key={t.id} task={t} onDragStart={onDragStart} ancestors={ancestorMap.get(t.id)} />)
                    : <GroupedCards tasks={colTasks} groupBy={groupBy} onDragStart={onDragStart} ancestorMap={ancestorMap} />
                  }
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function KanbanCard({ task, onDragStart, ancestors }: { task: Task; onDragStart: (e: React.DragEvent, id: number) => void; ancestors?: string[] }) {
  const labels = useStore(s => s.taskLabelsMap.get(task.id));
  const updateTask = useStore(s => s.updateTask);
  const nextStatus = task.status === "backlog" ? "in_progress" : task.status === "in_progress" ? "completed" : task.status === "active" ? "in_progress" : null;
  return (
    <div draggable onDragStart={e => onDragStart(e, task.id)} tabIndex={0}
      onKeyDown={e => { if (e.key === "Enter" && nextStatus) { e.preventDefault(); updateTask(task.id, { status: nextStatus }); } }}
      role="listitem" aria-label={`${task.title}, ${task.status}${nextStatus ? `. Press Enter to move to ${nextStatus}` : ""}`}
      className="bg-[var(--color-surface)] p-3 rounded-xl border border-white/5 cursor-grab active:cursor-grabbing hover:border-white/10 focus:ring-2 focus:ring-[var(--color-accent)] focus:outline-none transition-colors">
      {ancestors && ancestors.length > 0 && (
        <div className="text-[9px] text-white/20 leading-tight mb-1 truncate" title={ancestors.join(" › ")}>
          {ancestors.join(" › ")}
        </div>
      )}
      <div className="text-xs text-white/80 leading-tight">{task.title}</div>
      <div className="flex items-center gap-1.5 mt-1 flex-wrap">
        {task.project && <span className="text-[9px] bg-white/5 px-1 py-0.5 rounded text-white/30">{task.project}</span>}
        <span className="text-[9px] text-white/20">@{task.user}</span>
        {task.priority >= 4 && <span className="text-[9px] text-red-400">P{task.priority}</span>}
        {task.due_date && <span className="text-[9px] text-white/20">{task.due_date.slice(5)}</span>}
        {labels?.map(l => <span key={l.name} className="text-[8px] px-1 rounded" style={{ background: l.color + "30", color: l.color }}>{l.name}</span>)}
      </div>
    </div>
  );
}

function GroupedCards({ tasks, groupBy, onDragStart, ancestorMap }: { tasks: Task[]; groupBy: "project" | "user"; onDragStart: (e: React.DragEvent, id: number) => void; ancestorMap: Map<number, string[]> }) {
  const groups = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const t of tasks) {
      const key = groupBy === "project" ? (t.project || "(none)") : t.user;
      const list = map.get(key) || [];
      list.push(t);
      map.set(key, list);
    }
    return Array.from(map.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [tasks, groupBy]);

  return (<>
    {groups.map(([name, items]) => (
      <div key={name}>
        <div className="text-[9px] text-white/20 font-medium px-1 py-0.5 sticky top-0 bg-white/[0.02]">{name} ({items.length})</div>
        {items.map(t => <KanbanCard key={t.id} task={t} onDragStart={onDragStart} ancestors={ancestorMap.get(t.id)} />)}
      </div>
    ))}
  </>);
}

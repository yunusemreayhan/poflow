import { useMemo, useState, useCallback } from "react";
import { useStore } from "../store/store";
import { apiCall } from "../store/api";
import type { Task } from "../store/api";

const COLUMNS = [
  { id: "backlog", label: "Backlog", color: "#6C7A89" },
  { id: "active", label: "Active", color: "#F59E0B" },
  { id: "in_progress", label: "In Progress", color: "#3B82F6" },
  { id: "blocked", label: "Blocked", color: "#EF4444" },
  { id: "completed", label: "Done", color: "#10B981" },
] as const;

type ColId = typeof COLUMNS[number]["id"];

export default function KanbanBoard() {
  const { tasks, updateTask, teamScope } = useStore();
  const [dragId, setDragId] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<string | null>(null);
  const [groupBy, setGroupBy] = useState<"none" | "project" | "user">("none");

  const filtered = useMemo(() => {
    let t = tasks.filter(t => t.status !== "archived" && !t.deleted_at);
    if (teamScope) t = t.filter(task => teamScope.has(task.id));
    return t;
  }, [tasks, teamScope]);

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
    <div className="p-4 space-y-3 h-full flex flex-col">
      <div className="flex items-center gap-2">
        <h2 className="text-lg font-semibold text-white flex-1">Kanban Board</h2>
        <select value={groupBy} onChange={e => setGroupBy(e.target.value as typeof groupBy)}
          className="bg-white/5 border border-white/10 text-white/70 text-xs rounded px-2 py-1 outline-none">
          <option value="none">No grouping</option>
          <option value="project">By project</option>
          <option value="user">By assignee</option>
        </select>
      </div>

      <div className="flex-1 overflow-x-auto">
        <div className="flex gap-3 min-w-max h-full">
          {COLUMNS.map(col => {
            const colTasks = columns.get(col.id) || [];
            const isOver = dragOver === col.id;
            return (
              <div key={col.id}
                className={`w-56 md:w-64 flex flex-col rounded-lg bg-white/[0.02] border transition-colors ${
                  isOver ? "border-[var(--color-accent)]/50 bg-[var(--color-accent)]/5" : "border-white/5"
                }`}
                onDragOver={e => { e.preventDefault(); setDragOver(col.id); }}
                onDragLeave={() => setDragOver(null)}
                onDrop={e => onDrop(e, col.id)}>
                <div className="flex items-center gap-2 px-3 py-2 border-b border-white/5">
                  <span className="w-2 h-2 rounded-full" style={{ background: col.color }} />
                  <span className="text-xs font-medium text-white/70">{col.label}</span>
                  <span className="text-[10px] text-white/30 ml-auto">{colTasks.length}</span>
                </div>
                <div className="flex-1 overflow-y-auto p-2 space-y-1.5">
                  {colTasks.length === 0 && <div className="text-[10px] text-white/15 text-center py-4">Drop tasks here</div>}
                  {groupBy === "none"
                    ? colTasks.map(t => <KanbanCard key={t.id} task={t} onDragStart={onDragStart} />)
                    : <GroupedCards tasks={colTasks} groupBy={groupBy} onDragStart={onDragStart} />
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

function KanbanCard({ task, onDragStart }: { task: Task; onDragStart: (e: React.DragEvent, id: number) => void }) {
  const labels = useStore(s => s.taskLabelsMap.get(task.id));
  return (
    <div draggable onDragStart={e => onDragStart(e, task.id)}
      className="bg-[var(--color-surface)] p-2 rounded-lg border border-white/5 cursor-grab active:cursor-grabbing hover:border-white/10 transition-colors">
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

function GroupedCards({ tasks, groupBy, onDragStart }: { tasks: Task[]; groupBy: "project" | "user"; onDragStart: (e: React.DragEvent, id: number) => void }) {
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
        {items.map(t => <KanbanCard key={t.id} task={t} onDragStart={onDragStart} />)}
      </div>
    ))}
  </>);
}

import { useState, useEffect, useCallback } from "react";
import { Plus, Trash2, Play, CheckCircle, ArrowLeft } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { matchSearch } from "../utils";
import { useSseDebounce } from "../hooks/useSseDebounce";
import type { Sprint, SprintDetail, SprintBoard, SprintDailyStat, Task, BurnEntry, BurnSummaryEntry } from "../store/api";
import TaskList from "./TaskList";
import Select from "./Select";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";
import EpicBurndown from "./EpicBurndown";
import { BurnsView, BurndownView, VelocityChart } from "./SprintViews";

export default function Sprints() {
  const [sprints, setSprints] = useState<Sprint[]>([]);
  const [filter, setFilter] = useState<string>("all");
  const [selected, setSelected] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState({ name: "", project: "", goal: "", start_date: "", end_date: "" });
  const [createRoots, setCreateRoots] = useState<number[]>([]);
  const [createRootSearch, setCreateRootSearch] = useState("");
  const allTasks = useStore(s => s.tasks);
  const [loading, setLoading] = useState(true);
  const rootTasks = allTasks.filter(t => t.parent_id === null);

  const load = useCallback(async () => {
    const params = filter !== "all" ? `?status=${filter}` : "";
    const data = await apiCall<Sprint[]>("GET", `/api/sprints${params}`);
    if (data) setSprints(data);
    setLoading(false);
  }, [filter]);

  useEffect(() => { load(); }, [load]);
  useSseDebounce("sse-sprints", load);

  const create = async () => {
    if (!form.name.trim()) return;
    if (form.start_date && form.end_date && form.end_date < form.start_date) {
      useStore.getState().toast("End date must be after start date", "error");
      return;
    }
    const body: Record<string, unknown> = { name: form.name.trim() };
    if (form.project) body.project = form.project;
    if (form.goal) body.goal = form.goal;
    if (form.start_date) body.start_date = form.start_date;
    if (form.end_date) body.end_date = form.end_date;
    const sprint = await apiCall<Sprint>("POST", "/api/sprints", body);
    if (sprint && createRoots.length > 0) {
      await apiCall("POST", `/api/sprints/${sprint.id}/roots`, { task_ids: createRoots });
    }
    setCreating(false);
    setForm({ name: "", project: "", goal: "", start_date: "", end_date: "" });
    setCreateRoots([]);
    setCreateRootSearch("");
    load();
  };

  const del = async (id: number) => {
    await apiCall("DELETE", `/api/sprints/${id}`);
    load();
  };

  if (selected) return <SprintView id={selected} onBack={() => { setSelected(null); load(); }} />;

  return (
    <div className="p-4 space-y-3">
      <div className="flex items-center gap-2">
        <h2 className="text-lg font-semibold text-white flex-1">Sprints</h2>
        <Select value={filter} onChange={setFilter} className="w-32 text-xs"
          options={[{value:"all",label:"All"},{value:"planning",label:"Planning"},{value:"active",label:"Active"},{value:"completed",label:"Completed"}]} />
        <button onClick={() => setCreating(true)} className="p-1.5 rounded bg-[var(--color-accent)] text-white"><Plus size={14} /></button>
      </div>

      {creating && (
        <div className="bg-[var(--color-surface)] p-3 rounded-lg space-y-2 border border-white/10">
          <input placeholder="Sprint name" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })}
            onKeyDown={e => e.key === "Enter" && create()}
            className="w-full bg-transparent border-b border-white/20 text-white text-sm outline-none pb-1" autoFocus />
          <div className="flex gap-2">
            <input placeholder="Project" value={form.project} onChange={e => setForm({ ...form, project: e.target.value })}
              className="flex-1 bg-transparent border-b border-white/10 text-white/70 text-xs outline-none pb-1" />
            <input type="date" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })}
              className="bg-transparent border-b border-white/10 text-white/70 text-xs outline-none pb-1" />
            <input type="date" value={form.end_date} onChange={e => setForm({ ...form, end_date: e.target.value })}
              className="bg-transparent border-b border-white/10 text-white/70 text-xs outline-none pb-1" />
          </div>
          <textarea placeholder="Sprint goal" value={form.goal} onChange={e => setForm({ ...form, goal: e.target.value })}
            className="w-full bg-transparent border border-white/10 text-white/70 text-xs rounded p-1 outline-none" rows={2} />
          {/* Root task scope */}
          <div className="space-y-1">
            <div className="text-xs text-white/40">Scope to root tasks (optional):</div>
            {createRoots.map(rid => {
              const t = allTasks.find(tk => tk.id === rid);
              return t ? (
                <div key={rid} className="flex items-center gap-2 text-xs text-white/70">
                  <span className="flex-1 truncate">{t.title}</span>
                  <button onClick={() => setCreateRoots(createRoots.filter(r => r !== rid))} className="text-red-400/50 hover:text-red-400">✕</button>
                </div>
              ) : null;
            })}
            <input placeholder="Search root tasks..." value={createRootSearch} onChange={e => setCreateRootSearch(e.target.value)}
              className="w-full bg-transparent border-b border-white/10 text-white text-xs outline-none pb-1" />
            {createRootSearch && (
              <div className="max-h-20 overflow-y-auto space-y-0.5">
                {rootTasks.filter(t => !createRoots.includes(t.id) && matchSearch(t.title, createRootSearch)).slice(0, 15).map(t => (
                  <button key={t.id} onClick={() => { setCreateRoots([...createRoots, t.id]); setCreateRootSearch(""); }}
                    className="w-full text-left text-xs text-white/50 hover:text-green-400 truncate py-0.5">
                    + {t.title}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="flex gap-2">
            <button onClick={create} className="px-3 py-1 bg-[var(--color-accent)] text-white text-xs rounded">Create</button>
            <button onClick={() => setCreating(false)} className="px-3 py-1 text-white/50 text-xs">Cancel</button>
          </div>
        </div>
      )}

      <EpicBurndown />

      {sprints.map(s => (
        <div key={s.id} role="button" tabIndex={0} className="bg-[var(--color-surface)] p-3 rounded-lg flex items-center gap-3 cursor-pointer hover:bg-white/5 border border-white/5"
          onClick={() => setSelected(s.id)} onKeyDown={e => e.key === "Enter" && setSelected(s.id)}>
          <div className="flex-1 min-w-0">
            <div className="text-sm text-white font-medium truncate">{s.name}</div>
            <div className="text-xs text-white/40 flex gap-2 mt-0.5">
              {s.project && <span className="bg-white/5 px-1.5 py-0.5 rounded">{s.project}</span>}
              <span className={`px-1.5 py-0.5 rounded ${
                s.status === "active" ? "bg-green-500/20 text-green-400" :
                s.status === "completed" ? "bg-blue-500/20 text-blue-400" :
                "bg-white/5"
              }`}>{s.status}</span>
              {s.start_date && <span>{s.start_date} → {s.end_date || "?"}</span>}
            </div>
          </div>
          <button onClick={e => { e.stopPropagation(); useStore.getState().showConfirm(`Delete sprint "${s.name}"?`, () => del(s.id)); }} className="p-1 text-white/20 hover:text-red-400"><Trash2 size={14} /></button>
        </div>
      ))}
      {loading && sprints.length === 0 && <div className="text-center py-12 text-white/20 text-sm">Loading sprints...</div>}
      {!loading && sprints.length === 0 && <div className="text-center py-12"><div className="text-4xl mb-2">🏃</div><div className="text-white/30 text-sm">No sprints yet</div><div className="text-white/20 text-xs mt-1">Create one to start tracking progress</div></div>}

      {/* Velocity chart for completed sprints */}
      {sprints.filter(s => s.status === "completed").length >= 2 && (
        <VelocityChart />
      )}
    </div>
  );
}

function SprintView({ id, onBack }: { id: number; onBack: () => void }) {
  const [detail, setDetail] = useState<SprintDetail | null>(null);
  const [board, setBoard] = useState<SprintBoard | null>(null);
  const [tab, setTab] = useState<"board" | "backlog" | "burns" | "burndown" | "summary">("board");
  const [rootIds, setRootIds] = useState<number[]>([]);
  const allTasks = useStore(s => s.tasks);

  const load = useCallback(async () => {
    const d = await apiCall<SprintDetail>("GET", `/api/sprints/${id}`);
    if (d) setDetail(d);
    const b = await apiCall<SprintBoard>("GET", `/api/sprints/${id}/board`);
    if (b) setBoard(b);
    const r = await apiCall<number[]>("GET", `/api/sprints/${id}/roots`);
    if (r) setRootIds(r);
  }, [id]);

  useEffect(() => { load(); }, [load]);
  useSseDebounce("sse-sprints", load);

  const start = () => useStore.getState().showConfirm("Start this sprint?", async () => { await apiCall("POST", `/api/sprints/${id}/start`); load(); });
  const complete = () => useStore.getState().showConfirm("Complete this sprint?", async () => { await apiCall("POST", `/api/sprints/${id}/complete`); load(); });
  const snapshot = async () => { await apiCall("POST", `/api/sprints/${id}/snapshot`); load(); };

  if (!detail) return <div className="p-4 text-white/30">Loading...</div>;
  const s = detail.sprint;
  const taskIds = new Set(detail.tasks.map(t => t.id));

  return (
    <div className="p-4 space-y-3">
      <div className="flex items-center gap-2">
        <button onClick={onBack} className="p-1 text-white/50 hover:text-white"><ArrowLeft size={16} /></button>
        <div className="flex-1">
          <div className="text-lg font-semibold text-white">{s.name}</div>
          {s.goal && <div className="text-xs text-white/40">{s.goal}</div>}
        </div>
        <span className={`text-xs px-2 py-0.5 rounded ${
          s.status === "active" ? "bg-green-500/20 text-green-400" :
          s.status === "completed" ? "bg-blue-500/20 text-blue-400" : "bg-white/10 text-white/50"
        }`}>{s.status}</span>
        {s.status === "planning" && <button onClick={start} className="flex items-center gap-1 px-2 py-1 bg-green-600 text-white text-xs rounded"><Play size={12} />Start</button>}
        {s.status === "active" && <button onClick={complete} className="flex items-center gap-1 px-2 py-1 bg-blue-600 text-white text-xs rounded"><CheckCircle size={12} />Complete</button>}
        {s.status === "active" && <button onClick={snapshot} className="px-2 py-1 bg-white/10 text-white/60 text-xs rounded">📸 Snapshot</button>}
      </div>

      {s.start_date && <div className="text-xs text-white/30">{s.start_date} → {s.end_date || "?"}</div>}

      {/* Retro notes */}
      {(s.status === "completed" || s.retro_notes) && (
        <div className="space-y-1">
          <div className="text-xs text-white/30">Retro Notes</div>
          <textarea
            key={s.retro_notes || ""}
            defaultValue={s.retro_notes || ""}
            onBlur={e => {
              const val = e.target.value.trim() || null;
              if (val !== (s.retro_notes || null)) {
                apiCall("PUT", `/api/sprints/${id}`, { retro_notes: val }).then(() => load());
              }
            }}
            placeholder="Add retrospective notes..."
            className="w-full bg-white/5 border border-white/10 text-xs text-white/70 rounded p-2 outline-none focus:border-[var(--color-accent)] resize-none"
            rows={3}
          />
        </div>
      )}

      {rootIds.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          <span className="text-[10px] text-white/30">Scope:</span>
          {rootIds.map(rid => {
            const t = allTasks.find(tk => tk.id === rid);
            return t ? <span key={rid} className="text-[10px] bg-white/5 px-1.5 py-0.5 rounded text-white/50">{t.title}</span> : null;
          })}
        </div>
      )}

      <div className="flex gap-1 bg-[var(--color-surface)] rounded-lg p-0.5">
        {(["board", "backlog", "burns", "burndown", "summary"] as const).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={`flex-1 text-xs py-1.5 rounded ${tab === t ? "bg-[var(--color-accent)] text-white" : "text-white/50"}`}>
            {t[0].toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      {tab === "board" && board && <BoardView board={board} reload={load} />}
      {tab === "backlog" && <BacklogView sprintId={id} taskIds={taskIds} reload={load} />}
      {tab === "burns" && <BurnsView sprintId={id} sprintName={detail.sprint.name} tasks={detail.tasks} />}
      {tab === "burndown" && <BurndownView stats={detail.stats} />}
      {tab === "summary" && <SummaryView detail={detail} />}
    </div>
  );
}

function BoardView({ board, reload }: { board: SprintBoard; reload: () => void }) {
  const changeStatus = async (taskId: number, status: string) => {
    await apiCall("PUT", `/api/tasks/${taskId}`, { status });
    reload();
  };

  const Column = ({ title, tasks, color, status }: { title: string; tasks: Task[]; color: string; status: string }) => (
    <div className="flex-1 min-w-0" role="list" aria-label={`${title} tasks`}
      onDragOver={e => { e.preventDefault(); e.currentTarget.classList.add("bg-white/5"); }}
      onDragLeave={e => e.currentTarget.classList.remove("bg-white/5")}
      onDrop={e => { e.currentTarget.classList.remove("bg-white/5"); const id = Number(e.dataTransfer.getData("text/plain")); if (id) changeStatus(id, status); }}>
      <div className={`text-xs font-medium mb-2 ${color}`}>{title} ({tasks.length})</div>
      <div className="space-y-1.5 min-h-[40px] rounded p-1 transition-colors">
        {tasks.map(t => (
          <div key={t.id} draggable onDragStart={e => { e.dataTransfer.setData("text/plain", String(t.id)); (e.target as HTMLElement).style.opacity = "0.4"; }}
            onDragEnd={e => { (e.target as HTMLElement).style.opacity = "1"; }}
            className="bg-[var(--color-surface)] p-2 rounded border border-white/5 group cursor-grab active:cursor-grabbing">
            <div className="text-xs text-white/90 truncate">{t.title}</div>
            <div className="text-[10px] text-white/30 flex gap-1 mt-1">
              {t.estimated_hours > 0 && <span>{t.estimated_hours}h</span>}
              {t.remaining_points > 0 && <span>{t.remaining_points}pt</span>}
              <span>👤{t.user}</span>
            </div>
            <div className="hidden group-hover:flex gap-1 mt-1">
              {title !== "Todo" && <button onClick={() => changeStatus(t.id, "backlog")} className="text-[10px] text-white/30 hover:text-white">→Todo</button>}
              {title !== "In Progress" && <button onClick={() => changeStatus(t.id, "in_progress")} className="text-[10px] text-white/30 hover:text-yellow-400">→WIP</button>}
              {title !== "Done" && <button onClick={() => changeStatus(t.id, "completed")} className="text-[10px] text-white/30 hover:text-green-400">→Done</button>}
            </div>
          </div>
        ))}
      </div>
    </div>
  );

  const total = board.todo.length + board.in_progress.length + board.done.length;
  const pct = total > 0 ? Math.round((board.done.length / total) * 100) : 0;

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3 text-xs text-white/50">
        <span>{total} tasks</span>
        <div className="flex-1 h-1.5 bg-white/5 rounded-full overflow-hidden">
          <div className="h-full bg-green-500 rounded-full transition-all" style={{ width: `${pct}%` }} />
        </div>
        <span>{pct}% done</span>
      </div>
      <div className="flex gap-3">
      <Column title="Todo" tasks={board.todo} color="text-white/60" status="backlog" />
      <Column title="In Progress" tasks={board.in_progress} color="text-yellow-400" status="in_progress" />
      <Column title="Done" tasks={board.done} color="text-green-400" status="completed" />
      </div>
    </div>
  );
}

function BacklogView({ sprintId, taskIds, reload }: { sprintId: number; taskIds: Set<number>; reload: () => void }) {
  const leafOnly = useStore(s => s.config?.leaf_only_mode ?? false);
  const tasks = useStore(s => s.tasks);
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
    <div className="space-y-3">
      {/* Root task scoping */}
      <div className="space-y-1">
        <div className="text-xs text-white/50 font-medium">Sprint Scope (root tasks)</div>
        {rootIds.map(rid => {
          const t = tasks.find(tk => tk.id === rid);
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

      {/* Sprint tasks */}
      <div>
        <div className="text-xs text-white/50 mb-1 font-medium">Sprint Tasks (click ✕ to remove)</div>
        <div className="space-y-1 max-h-[50vh] overflow-y-auto">
          {[...taskIds].length === 0 && <div className="text-xs text-white/20 py-2">No tasks in sprint</div>}
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

function SummaryView({ detail }: { detail: SprintDetail }) {
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
    <div className="space-y-3">
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
        {Object.entries(byUser).map(([user, { total, done }]) => (
          <div key={user} className="flex items-center gap-2 py-1">
            <span className="text-xs text-white/70 w-24 truncate">👤 {user}</span>
            <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
              <div className="h-full bg-[var(--color-accent)] rounded-full" style={{ width: `${total > 0 ? (done / total) * 100 : 0}%` }} />
            </div>
            <span className="text-[10px] text-white/40">{done}/{total}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

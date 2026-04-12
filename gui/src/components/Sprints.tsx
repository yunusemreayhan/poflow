import { useState, useEffect, useCallback } from "react";
import { Plus, Trash2, Play, CheckCircle, ArrowLeft } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { matchSearch } from "../utils";
import { useT } from "../i18n";
import { useSseDebounce } from "../hooks/useSseDebounce";
import type { Sprint, SprintDetail, SprintBoard, SprintDailyStat, Task, BurnEntry, BurnSummaryEntry } from "../store/api";
import TaskList from "./TaskList";
import Select from "./Select";
import EpicBurndown from "./EpicBurndown";
import { BurnsView, BurndownView, VelocityChart } from "./SprintViews";
import { BoardView, BacklogView, SummaryView } from "./SprintParts";

export default function Sprints() {
  const t = useT();
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
            <label className="flex flex-col text-[10px] text-white/30">Start
              <input type="date" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })}
                className="bg-transparent border-b border-white/10 text-white/70 text-xs outline-none pb-1" />
            </label>
            <label className="flex flex-col text-[10px] text-white/30">End
              <input type="date" value={form.end_date} onChange={e => setForm({ ...form, end_date: e.target.value })}
                className="bg-transparent border-b border-white/10 text-white/70 text-xs outline-none pb-1" />
            </label>
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
      {!loading && sprints.length === 0 && <div className="text-center py-12"><div className="text-4xl mb-2">🏃</div><div className="text-white/30 text-sm">{t.noSprintsYet}</div><div className="text-white/20 text-xs mt-1">Create one to start tracking progress</div></div>}

      {/* Velocity chart for completed sprints */}
      {sprints.filter(s => s.status === "completed").length >= 2 && (
        <VelocityChart />
      )}
    </div>
  );
}

function SprintView({ id, onBack }: { id: number; onBack: () => void }) {
  const t = useT();
  const [detail, setDetail] = useState<SprintDetail | null>(null);
  const [board, setBoard] = useState<SprintBoard | null>(null);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<"board" | "backlog" | "burns" | "burndown" | "summary">("board");
  const [rootIds, setRootIds] = useState<number[]>([]);
  const allTasks = useStore(s => s.tasks);

  const load = useCallback(async () => {
    const [d, b, r] = await Promise.all([
      apiCall<SprintDetail>("GET", `/api/sprints/${id}`),
      apiCall<SprintBoard>("GET", `/api/sprints/${id}/board`),
      apiCall<number[]>("GET", `/api/sprints/${id}/roots`),
    ]);
    if (d) setDetail(d);
    if (b) setBoard(b);
    if (r) setRootIds(r);
    setLoading(false);
  }, [id]);

  useEffect(() => { load(); }, [load]);
  useSseDebounce("sse-sprints", load);

  const start = () => useStore.getState().showConfirm(t.startThisSprint, async () => { await apiCall("POST", `/api/sprints/${id}/start`); load(); }, t.start);
  const complete = () => useStore.getState().showConfirm(t.completeThisSprint, async () => { await apiCall("POST", `/api/sprints/${id}/complete`); load(); }, t.completed);
  const snapshot = async () => { await apiCall("POST", `/api/sprints/${id}/snapshot`); load(); };

  if (loading || !detail) return (
    <div className="p-4 space-y-3 animate-pulse">
      <div className="h-6 bg-white/5 rounded w-48" />
      <div className="h-4 bg-white/5 rounded w-32" />
      <div className="h-32 bg-white/5 rounded" />
    </div>
  );
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
        <button onClick={() => {
          const tasks = detail?.tasks || [];
          const done = tasks.filter(t => t.status === "completed" || t.status === "done");
          const md = [
            `# Sprint Report: ${s.name}`,
            s.goal ? `**Goal:** ${s.goal}` : "",
            `**Status:** ${s.status} | **Period:** ${s.start_date || "?"} → ${s.end_date || "?"}`,
            `\n## Summary`, `- Tasks: ${done.length}/${tasks.length} completed`,
            `- Points: ${done.reduce((a, t) => a + t.remaining_points, 0)}/${tasks.reduce((a, t) => a + t.remaining_points, 0)}`,
            `- Hours: ${done.reduce((a, t) => a + t.estimated_hours, 0).toFixed(1)}/${tasks.reduce((a, t) => a + t.estimated_hours, 0).toFixed(1)}`,
            `\n## Tasks`, ...tasks.map(t => `- [${t.status === "completed" ? "x" : " "}] ${t.title} (${t.remaining_points}pt, ${t.estimated_hours}h) — ${t.user}`),
            s.retro_notes ? `\n## Retrospective\n${s.retro_notes}` : "",
          ].filter(Boolean).join("\n");
          const blob = new Blob([md], { type: "text/markdown" });
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a"); a.href = url; a.download = `sprint_${s.name.replace(/\s+/g, "_")}.md`; a.click();
          URL.revokeObjectURL(url);
        }} className="px-2 py-1 bg-white/10 text-white/60 text-xs rounded">📄 Export</button>
      </div>

      {s.start_date && <div className="text-xs text-white/30">{s.start_date} → {s.end_date || "?"}</div>}

      {/* Retro notes */}
      {(s.status === "completed" || s.retro_notes) && (
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <div className="text-xs text-white/30">Retro Notes</div>
            {!s.retro_notes && (
              <button onClick={() => {
                const template = "## What went well\n- \n\n## What to improve\n- \n\n## Action items\n- [ ] ";
                apiCall("PUT", `/api/sprints/${id}`, { retro_notes: template }).then(() => load());
              }} className="text-[10px] text-[var(--color-accent)] hover:underline">Use template</button>
            )}
          </div>
          <textarea
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

      <div className="flex gap-1 bg-[var(--color-surface)] rounded-lg p-0.5" role="tablist">
        {(["board", "backlog", "burns", "burndown", "summary"] as const).map(t => (
          <button key={t} onClick={() => setTab(t)} role="tab" aria-selected={tab === t}
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

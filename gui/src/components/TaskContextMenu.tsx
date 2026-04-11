import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { useStore } from "../store/store";
import { apiCall } from "../store/api";
import type { Task, TaskSprintInfo, Config } from "../store/api";
import type { TreeNode } from "../tree";

import { PRIORITY_COLORS } from "../constants";

interface CtxMenuProps {
  pos: { x: number; y: number };
  task: Task;
  node: TreeNode;
  isOwner: boolean;
  assignees: string[];
  ctxSprints: { id: number; name: string; status: string }[];
  ctxUsers: string[];
  ctxBurnUsers: string[];
  taskSprints: TaskSprintInfo[];
  config: Config | null;
  onClose: () => void;
  updateTask: (id: number, fields: Record<string, unknown>) => void;
  start: (id: number) => void;
  setAssignees: (fn: (prev: string[]) => string[]) => void;
  setEditingTitle: (v: boolean) => void;
  setTitleDraft: (v: string) => void;
  setEditingDesc: (v: boolean) => void;
  setDescDraft: (v: string) => void;
  handleDelete: () => void;
  setTimeReporting: (v: boolean) => void;
  setCommenting: (v: boolean) => void;
  setAdding: (v: boolean) => void;
  onView: (id: number) => void;
}

export default function TaskContextMenu(p: CtxMenuProps) {
  const [ctxSub, setCtxSub] = useState<string | null>(null);
  const { task: t, pos, node, isOwner, assignees, ctxSprints, ctxUsers, ctxBurnUsers, taskSprints, config } = p;
  const close = p.onClose;

  return (
    <>
      <div className="fixed inset-0 z-40" onClick={close} onKeyDown={e => e.key === "Escape" && close()} />
      <div role="menu" aria-label="Task actions" className="fixed z-50 bg-[var(--color-surface)] border border-white/10 rounded-lg shadow-xl py-1 min-w-52 text-xs max-h-[80vh] overflow-y-auto"
        style={{ left: Math.min(pos.x, window.innerWidth - 260), top: Math.min(pos.y, window.innerHeight - 400) }}
        onKeyDown={e => { if (e.key === "Escape") close(); }}>

        <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider" role="presentation">Status</div>
        {([["backlog","Todo","○"],["active","WIP","▶"],["completed","Done","✓"],["archived","Archive","📦"]] as const).map(([s,label,icon]) => (
          <button key={s} role="menuitem" disabled={t.status === s} onClick={() => { p.updateTask(t.id, { status: s }); close(); }}
            className={`w-full text-left px-3 py-1.5 flex items-center gap-2 ${t.status === s ? "text-white/20" : "text-white/60 hover:bg-white/5"}`}>
            {icon} {label}
            {t.status === s && <span className="ml-auto text-white/20">current</span>}
          </button>
        ))}

        <div className="border-t border-white/5 my-1" />
        <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Priority</div>
        <div className="flex gap-1 px-3 py-1">
          {[1,2,3,4,5].map(pr => (
            <button key={pr} onClick={() => { p.updateTask(t.id, { priority: pr }); close(); }}
              className={`w-6 h-6 rounded-full border-2 transition-all ${t.priority === pr ? "scale-125" : "opacity-50 hover:opacity-100"}`}
              style={{ borderColor: PRIORITY_COLORS[pr], background: t.priority === pr ? PRIORITY_COLORS[pr] : "transparent" }}
              title={`Priority ${pr}`} />
          ))}
        </div>

        <div className="border-t border-white/5 my-1" />

        {/* Sprints submenu */}
        <div className="relative" onMouseEnter={() => setCtxSub("sprints")} onMouseLeave={() => setCtxSub(null)}>
          <div className="px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center justify-between cursor-default">
            🏃 Sprints <ChevronRight size={12} className="text-white/30" />
          </div>
          {ctxSub === "sprints" && (
            <div className="absolute top-0 bg-[var(--color-surface)] border border-white/10 rounded-lg shadow-xl py-1 min-w-48 z-50"
              style={pos.x > window.innerWidth - 520 ? { right: "100%" } : { left: "100%" }}>
              {taskSprints.filter(ts => ts.task_id === t.id).map(ts => (
                <button key={`rm-${ts.sprint_id}`} onClick={async () => {
                  await apiCall("DELETE", `/api/sprints/${ts.sprint_id}/tasks/${t.id}`);
                  useStore.getState().loadTasks(); close();
                }} className="w-full text-left px-3 py-1.5 text-red-400/70 hover:bg-white/5 flex items-center gap-2">
                  ✕ Remove from {ts.sprint_name}
                </button>
              ))}
              {ctxSprints.filter(s => !taskSprints.some(ts => ts.task_id === t.id && ts.sprint_id === s.id)).map(s => (
                <button key={`add-${s.id}`} onClick={async () => {
                  await apiCall("POST", `/api/sprints/${s.id}/tasks`, { task_ids: [t.id] });
                  useStore.getState().loadTasks(); close();
                }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
                  + {s.name} <span className="text-white/20">({s.status})</span>
                </button>
              ))}
              {ctxSprints.length === 0 && taskSprints.filter(ts => ts.task_id === t.id).length === 0 && (
                <div className="px-3 py-1.5 text-white/20">No sprints available</div>
              )}
            </div>
          )}
        </div>

        {/* Assign submenu */}
        <div className="relative" onMouseEnter={() => setCtxSub("assign")} onMouseLeave={() => setCtxSub(null)}>
          <div className="px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center justify-between cursor-default">
            👤 Assignees <ChevronRight size={12} className="text-white/30" />
          </div>
          {ctxSub === "assign" && (
            <div className="absolute top-0 bg-[var(--color-surface)] border border-white/10 rounded-lg shadow-xl py-1 min-w-48 z-50 max-h-64 overflow-y-auto"
              style={pos.x > window.innerWidth - 520 ? { right: "100%" } : { left: "100%" }}>
              {assignees.length > 0 && (
                <>
                  <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Assigned</div>
                  {assignees.map(a => {
                    const hasBurns = ctxBurnUsers.includes(a);
                    return (
                      <button key={`rm-${a}`} disabled={hasBurns} onClick={async () => {
                        await apiCall("DELETE", `/api/tasks/${t.id}/assignees/${a}`);
                        p.setAssignees(prev => prev.filter(x => x !== a)); close();
                      }} className={`w-full text-left px-3 py-1.5 flex items-center gap-2 ${hasBurns ? "text-white/20 cursor-not-allowed" : "text-red-400/70 hover:bg-white/5"}`}>
                        ✕ {a} {hasBurns && <span className="ml-auto text-[10px] text-white/15">has burns</span>}
                      </button>
                    );
                  })}
                </>
              )}
              {ctxUsers.filter(u => !assignees.includes(u)).length > 0 && (
                <>
                  <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Add</div>
                  {ctxUsers.filter(u => !assignees.includes(u)).map(u => (
                    <button key={`add-${u}`} onClick={async () => {
                      await apiCall("POST", `/api/tasks/${t.id}/assignees`, { username: u });
                      p.setAssignees(prev => [...prev, u]); close();
                    }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
                      + {u}
                    </button>
                  ))}
                </>
              )}
            </div>
          )}
        </div>

        <div className="border-t border-white/5 my-1" />

        <button onClick={async () => {
          const allTasks = useStore.getState().tasks;
          const siblings = allTasks.filter((s: Task) => s.parent_id === t.parent_id).sort((a: Task, b: Task) => a.sort_order - b.sort_order);
          const idx = siblings.findIndex((s: Task) => s.id === t.id);
          if (idx > 0) { const prev = siblings[idx - 1]; await Promise.all([p.updateTask(t.id, { sort_order: prev.sort_order }), p.updateTask(prev.id, { sort_order: t.sort_order })]); }
          close();
        }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">↑ Move up</button>
        <button onClick={async () => {
          const allTasks = useStore.getState().tasks;
          const siblings = allTasks.filter((s: Task) => s.parent_id === t.parent_id).sort((a: Task, b: Task) => a.sort_order - b.sort_order);
          const idx = siblings.findIndex((s: Task) => s.id === t.id);
          if (idx < siblings.length - 1) { const next = siblings[idx + 1]; await Promise.all([p.updateTask(t.id, { sort_order: next.sort_order }), p.updateTask(next.id, { sort_order: t.sort_order })]); }
          close();
        }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">↓ Move down</button>

        <div className="border-t border-white/5 my-1" />
        <button onClick={() => { p.start(t.id); close(); }}
          className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2"
          disabled={t.status === "completed" || (config?.leaf_only_mode && node.children.length > 0)}>▶ Start timer</button>
        <button onClick={() => { p.setTimeReporting(true); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">🕐 Log time</button>
        <button onClick={() => { p.setCommenting(true); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">💬 Comment</button>
        <button onClick={() => { p.setAdding(true); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">＋ Add subtask</button>
        <button onClick={() => { p.onView(t.id); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">👁 View details</button>

        {isOwner && (
          <>
            <div className="border-t border-white/5 my-1" />
            <button onClick={() => { p.setEditingTitle(true); p.setTitleDraft(t.title); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">✏️ Rename</button>
            <button onClick={() => { p.setEditingDesc(true); p.setDescDraft(t.description || ""); close(); }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">📝 Edit description</button>
            <button onClick={() => { p.handleDelete(); close(); }} className="w-full text-left px-3 py-1.5 text-red-400/70 hover:bg-red-500/10 flex items-center gap-2">🗑 Delete</button>
          </>
        )}
      </div>
    </>
  );
}

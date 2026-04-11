import { motion, AnimatePresence } from "framer-motion";
import { Plus, Trash2, Play, CheckCircle, Circle, ChevronRight, FolderOpen, Folder, MessageSquare, Eye, FileText, Clock } from "lucide-react";
import { useStore } from "../store/store";
import { useState, useMemo, useCallback, useEffect } from "react";
import { buildTree, countDescendants } from "../tree";
import { matchSearch } from "../utils";
import type { TreeNode } from "../tree";
import { apiCall } from "../store/api";
import type { Task } from "../store/api";
import TaskDetailView, { CommentSection } from "./TaskDetailView";
import { InlineTimeReport, InlineComment, InlineAddSubtask } from "./TaskInlineEditors";

const PRIORITY_COLORS = ["", "#10B981", "#4ECDC4", "#F59E0B", "#FF6B6B", "#EF4444"];

function TaskNode({ node, depth, onView, selectMode, onSelect, selectedTaskId, votedTaskIds, selectLabel, selectClassName }: {
  node: TreeNode; depth: number; onView: (id: number) => void;
  selectMode?: boolean; onSelect?: (id: number) => void; selectedTaskId?: number | null; votedTaskIds?: Set<number>;
  selectLabel?: string; selectClassName?: string;
}) {
  const { engine, createTask, updateTask, deleteTask, start, username: currentUser, role, taskSprints, burnTotals, allAssignees, config, tasks } = useStore();
  const [expanded, setExpanded] = useState(true);
  const [adding, setAdding] = useState(false);
  const [commenting, setCommenting] = useState(false);
  const [editingDesc, setEditingDesc] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [timeReporting, setTimeReporting] = useState(false);
  const [assignees, setAssignees] = useState<string[]>([]);
  const [totalHours, setTotalHours] = useState(0);
  const [newTitle, setNewTitle] = useState("");
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);
  const [ctxSub, setCtxSub] = useState<string | null>(null);
  const [ctxSprints, setCtxSprints] = useState<{ id: number; name: string; status: string }[]>([]);
  const [ctxUsers, setCtxUsers] = useState<string[]>([]);
  const [ctxBurnUsers, setCtxBurnUsers] = useState<string[]>([]);
  const [dropZone, setDropZone] = useState<"above" | "on" | "below" | null>(null);

  const t = node.task;

  // Use batch data from store instead of per-task API calls
  const storeAssignees = allAssignees.get(t.id) || [];
  const storeBurnTotal = burnTotals.get(t.id);
  const totalHoursVal = storeBurnTotal?.total_hours ?? 0;

  useEffect(() => { setAssignees(storeAssignees); }, [storeAssignees.join(",")]);
  useEffect(() => { setTotalHours(totalHoursVal); }, [totalHoursVal]);

  const hasChildren = node.children.length > 0;
  const isProject = depth === 0;
  const descendantCount = countDescendants(node);
  const doneCount = node.children.filter((c) => c.task.status === "completed").length;
  const isOwner = t.user === currentUser || role === "root";

  const isActive = engine?.current_task_id === t.id && engine?.status !== "Idle";

  const remainingPct = t.estimated_hours > 0
    ? Math.max(0, Math.round((1 - totalHours / t.estimated_hours) * 100))
    : (t.remaining_points > 0 && t.estimated > 0 ? Math.round((t.remaining_points / t.estimated) * 100) : null);

  const handleDelete = useCallback(async () => {
    try {
      await deleteTask(t.id);
    } catch (e) {
      alert(String(e));
    }
  }, [t.id, deleteTask]);

  const handleAdd = useCallback(() => {
    if (!newTitle.trim()) return;
    createTask(newTitle.trim(), t.id);
    setNewTitle("");
    setAdding(false);
    setExpanded(true);
  }, [newTitle, t.id, createTask]);

  return (
    <div>
      {dropZone === "above" && <div className="h-0.5 bg-[var(--color-accent)] rounded mx-2" />}
      <motion.div
        layout
        initial={{ opacity: 0, x: -10 }}
        animate={{ opacity: 1, x: 0 }}
        draggable={!selectMode}
        onDragStart={(e: any) => {
          e.dataTransfer?.setData("text/plain", String(t.id));
          e.dataTransfer.effectAllowed = "move";
          // Reduce opacity of dragged element
          setTimeout(() => { if (e.target) e.target.style.opacity = "0.4"; }, 0);
        }}
        onDragEnd={(e: any) => { if (e.target) e.target.style.opacity = "1"; }}
        onDragOver={(e: any) => {
          e.preventDefault();
          const rect = e.currentTarget.getBoundingClientRect();
          const y = e.clientY - rect.top;
          const zone = y < rect.height * 0.25 ? "above" : y > rect.height * 0.75 ? "below" : "on";
          setDropZone(zone);
        }}
        onDragLeave={() => setDropZone(null)}
        onDrop={async (e: any) => {
          e.preventDefault(); e.stopPropagation(); setDropZone(null);
          const dragId = Number(e.dataTransfer?.getData("text/plain"));
          if (!dragId || dragId === t.id) return;
          // Prevent dropping parent onto its own descendant
          const isDescendant = (parentId: number | null, childId: number): boolean => {
            let pid = parentId;
            while (pid) { if (pid === childId) return true; const p = tasks.find(tk => tk.id === pid); pid = p?.parent_id ?? null; }
            return false;
          };
          if (isDescendant(t.parent_id, dragId) || t.id === dragId) return;
          if (dropZone === "on") {
            await updateTask(dragId, { parent_id: t.id, sort_order: 0 });
          } else {
            const newParent = t.parent_id;
            const siblings = tasks.filter(s => s.parent_id === newParent && s.id !== dragId).sort((a, b) => a.sort_order - b.sort_order);
            const idx = siblings.findIndex(s => s.id === t.id);
            const insertAt = dropZone === "above" ? idx : idx + 1;
            const before = insertAt > 0 ? siblings[insertAt - 1].sort_order : 0;
            const after = insertAt < siblings.length ? siblings[insertAt].sort_order : before + 2;
            const newOrder = Math.floor((before + after) / 2);
            await updateTask(dragId, { parent_id: newParent, sort_order: newOrder === before ? after + 1 : newOrder });
          }
        }}
        onContextMenu={async (e) => {
          e.preventDefault();
          const [sprints, planning, users, burnUsers] = await Promise.all([
            apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=active").catch(() => []),
            apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=planning").catch(() => []),
            apiCall<string[]>("GET", "/api/users").catch(() => []),
            apiCall<string[]>("GET", `/api/tasks/${t.id}/burn-users`).catch(() => []),
          ]);
          setCtxSprints([...sprints, ...planning]);
          setCtxUsers(users);
          setCtxBurnUsers(burnUsers);
          setCtxSub(null);
          setCtxMenu({ x: e.clientX, y: e.clientY });
        }}
        className={`flex items-center gap-3 group transition-all rounded-xl ${
          isProject ? "glass p-4" : "px-4 py-3 hover:bg-white/5"
        } ${engine?.current_task_id === t.id ? "ring-1 ring-[var(--color-work)]" : ""} ${dropZone === "on" ? "ring-1 ring-[var(--color-accent)]" : ""}`}
        style={{ marginLeft: depth > 0 ? depth * 24 : 0 }}
      >
        {/* Expand/collapse */}
        <button
          onClick={() => setExpanded(!expanded)}
          className={`w-6 h-6 flex items-center justify-center rounded transition-all shrink-0 ${
            hasChildren ? "text-white/40 hover:text-white" : "text-transparent"
          }`}
        >
          <ChevronRight
            size={14}
            className={`transition-transform ${expanded ? "rotate-90" : ""}`}
          />
        </button>

        {/* Status toggle — owner only */}
        <button
          onClick={() => isOwner && updateTask(t.id, { status: t.status === "completed" ? "backlog" : "completed" })}
          className={`shrink-0 transition-colors ${isOwner ? "text-white/40 hover:text-white" : "text-white/20 cursor-default"}`}
        >
          {t.status === "completed" ? (
            <CheckCircle size={18} className="text-[var(--color-success)]" />
          ) : (
            <Circle size={18} />
          )}
        </button>

        {/* Icon */}
        {isProject && (
          <span className="shrink-0 text-white/40">
            {expanded ? <FolderOpen size={16} /> : <Folder size={16} />}
          </span>
        )}

        {/* Priority dot */}
        <div
          className="w-2 h-2 rounded-full shrink-0"
          style={{ background: PRIORITY_COLORS[t.priority] ?? "#6C7A89" }}
        />

        {/* Status badge */}
        <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${
          t.status === "completed" ? "bg-[var(--color-success)]/20 text-[var(--color-success)]"
          : t.status === "active" ? "bg-[var(--color-work)]/20 text-[var(--color-work)]"
          : "bg-white/5 text-white/25"
        }`}>
          {t.status === "completed" ? "Done" : t.status === "active" ? "WIP" : "Todo"}
        </span>

        {/* Title + meta */}
        <div className="flex-1 min-w-0">
          {editingTitle && isOwner ? (
            <input value={titleDraft} onChange={e => setTitleDraft(e.target.value)} autoFocus
              onKeyDown={e => {
                if (e.key === "Enter" && titleDraft.trim()) { updateTask(t.id, { title: titleDraft.trim() }); setEditingTitle(false); }
                if (e.key === "Escape") setEditingTitle(false);
              }}
              onBlur={() => { if (titleDraft.trim()) { updateTask(t.id, { title: titleDraft.trim() }); } setEditingTitle(false); }}
              className={`text-sm w-full bg-transparent border-b border-[var(--color-accent)] outline-none ${isProject ? "font-semibold" : ""} text-white`} />
          ) : (
            <div className={`text-sm truncate ${isProject ? "font-semibold" : ""} ${t.status === "completed" ? "line-through text-white/30" : "text-white/90"}`}
              onDoubleClick={() => { if (isOwner) { setTitleDraft(t.title); setEditingTitle(true); } }}>
              {t.title}
            </div>
          )}
          {t.description && !editingDesc && (
            <div className="text-xs text-white/40 mt-0.5 truncate cursor-pointer hover:text-white/60"
              onClick={() => { setEditingDesc(true); setDescDraft(t.description || ""); }}>
              {t.description}
            </div>
          )}
          <div className="flex gap-2 text-xs text-white/30 mt-0.5 flex-wrap">
            {t.project && <span className="bg-white/5 px-1.5 py-0.5 rounded">{t.project}</span>}
            {taskSprints.filter(ts => ts.task_id === t.id).map(ts => (
              <span key={ts.sprint_id} className={`px-1.5 py-0.5 rounded text-[10px] ${
                ts.sprint_status === "active" ? "bg-green-500/20 text-green-400" : "bg-green-500/10 text-green-400/40"
              }`}>🏃 {ts.sprint_name}</span>
            ))}
            <span className="bg-white/5 px-1.5 py-0.5 rounded">👤 {t.user}</span>
            {assignees.length > 0 && assignees.filter(a => a !== t.user).map(a => (
              <span key={a} className="bg-white/5 px-1.5 py-0.5 rounded text-white/20">{a}</span>
            ))}
            {descendantCount > 0 && (
              <span>{doneCount}/{node.children.length} done</span>
            )}
            <span>{t.actual}/{t.estimated}🍅</span>
            {totalHours > 0 && <span><Clock size={10} className="inline" /> {totalHours.toFixed(1)}h{t.estimated_hours > 0 ? `/${t.estimated_hours}h` : ""}</span>}
            {t.due_date && (() => {
              const due = new Date(t.due_date);
              const now = new Date();
              const daysLeft = Math.ceil((due.getTime() - now.getTime()) / 86400000);
              const overdue = daysLeft < 0 && t.status !== "completed";
              const soon = daysLeft >= 0 && daysLeft <= 3 && t.status !== "completed";
              return <span className={`${overdue ? "text-[var(--color-danger)] font-semibold" : soon ? "text-[var(--color-warning)]" : "text-white/30"}`}>
                📅 {t.due_date}{overdue ? ` (${-daysLeft}d overdue)` : soon ? ` (${daysLeft}d left)` : ""}
              </span>;
            })()}
            {remainingPct !== null && (
              <span className={`${remainingPct > 50 ? "text-[var(--color-success)]" : remainingPct > 20 ? "text-[var(--color-warning)]" : "text-[var(--color-danger)]"}`}>
                {remainingPct}% left
              </span>
            )}
          </div>
        </div>

        {/* Select mode: vote badge + select button for leaf tasks */}
        {selectMode && (
          <div className="flex items-center gap-2 shrink-0">
            {votedTaskIds?.has(t.id) && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--color-success)]/10 text-[var(--color-success)]">✓ estimated</span>
            )}
            {selectedTaskId === t.id && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--color-accent)]/20 text-[var(--color-accent)]">voting</span>
            )}
            {onSelect && (
              <button onClick={(e) => { e.stopPropagation(); onSelect(t.id); }}
                className={selectClassName || `px-2 py-1 rounded-lg text-xs font-semibold transition-all ${
                  selectedTaskId === t.id ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                }`}>
                {selectLabel || (selectedTaskId === t.id ? "voting" : votedTaskIds?.has(t.id) ? "re-vote" : "vote")}
              </button>
            )}
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
          <button onClick={() => setCommenting(!commenting)}
            className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all"
            title="Comment">
            <MessageSquare size={14} />
          </button>
          <button onClick={() => setTimeReporting(!timeReporting)}
            className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-work)] hover:bg-white/5 transition-all"
            title="Log time">
            <Clock size={14} />
          </button>
          <button onClick={() => { setEditingDesc(!editingDesc); setDescDraft(t.description || ""); }}
            className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all ${isOwner ? "text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5" : "hidden"}`}
            title="Edit description">
            <FileText size={14} />
          </button>
          <button onClick={() => onView(t.id)}
            className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all"
            title="View & Export">
            <Eye size={14} />
          </button>
          <button
            onClick={() => setAdding(!adding)}
            className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all"
            title="Add subtask"
          >
            <Plus size={14} />
          </button>
          {t.status !== "completed" && (!config?.leaf_only_mode || node.children.length === 0) && (
            <button
              onClick={() => start(t.id)}
              className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-work)] hover:bg-white/5 transition-all"
              title="Focus on this"
            >
              <Play size={14} />
            </button>
          )}
          <button
            onClick={handleDelete}
            className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all ${isOwner ? "text-white/30 hover:text-[var(--color-danger)] hover:bg-white/5" : "hidden"}`}
            title={isActive ? "Stop timer first" : "Delete"}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </motion.div>

      {/* Inline add child */}
      <AnimatePresence>
        {adding && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden"
            style={{ marginLeft: (depth + 1) * 24 + 24 }}
          >
            <div className="flex gap-2 items-center py-2 px-4">
              <input
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleAdd();
                  if (e.key === "Escape") { setAdding(false); setNewTitle(""); }
                }}
                placeholder={`Add subtask to "${t.title}"...`}
                className="flex-1 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-accent)]"
                autoFocus
              />
              <button
                onClick={handleAdd}
                className="w-8 h-8 flex items-center justify-center rounded-lg bg-[var(--color-accent)] text-white shrink-0"
              >
                <Plus size={14} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Inline description editor */}
      <AnimatePresence>
        {editingDesc && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden"
            style={{ marginLeft: depth * 24 + 48 }}
          >
            <div className="flex gap-2 items-start py-2 px-4">
              <textarea
                value={descDraft}
                onChange={(e) => setDescDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    updateTask(t.id, { description: descDraft || null });
                    setEditingDesc(false);
                  }
                  if (e.key === "Escape") setEditingDesc(false);
                }}
                placeholder="Add description... (Enter to save, Esc to cancel)"
                className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-accent)] resize-none min-h-[60px]"
                autoFocus
                rows={3}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Context menu */}
      {ctxMenu && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setCtxMenu(null)} />
          <div className="fixed z-50 bg-[#1a1a2e] border border-white/10 rounded-lg shadow-xl py-1 min-w-52 text-xs"
            style={{ left: Math.min(ctxMenu.x, window.innerWidth - 260), top: Math.min(ctxMenu.y, window.innerHeight - 400) }}>

            {/* --- Status --- */}
            <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Status</div>
            {([["backlog","Todo","○"],["active","WIP","▶"],["completed","Done","✓"]] as const).map(([s,label,icon]) => (
              <button key={s} disabled={t.status === s} onClick={() => { updateTask(t.id, { status: s }); setCtxMenu(null); }}
                className={`w-full text-left px-3 py-1.5 flex items-center gap-2 ${t.status === s ? "text-white/20" : "text-white/60 hover:bg-white/5"}`}>
                {icon} {label}
                {t.status === s && <span className="ml-auto text-white/20">current</span>}
              </button>
            ))}

            <div className="border-t border-white/5 my-1" />

            {/* --- Priority --- */}
            <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Priority</div>
            <div className="flex gap-1 px-3 py-1">
              {[1,2,3,4,5].map(p => (
                <button key={p} onClick={() => { updateTask(t.id, { priority: p }); setCtxMenu(null); }}
                  className={`w-6 h-6 rounded-full border-2 transition-all ${t.priority === p ? "scale-125" : "opacity-50 hover:opacity-100"}`}
                  style={{ borderColor: PRIORITY_COLORS[p], background: t.priority === p ? PRIORITY_COLORS[p] : "transparent" }}
                  title={`Priority ${p}`} />
              ))}
            </div>

            <div className="border-t border-white/5 my-1" />

            {/* --- Sprints submenu --- */}
            <div className="relative" onMouseEnter={() => setCtxSub("sprints")} onMouseLeave={() => setCtxSub(null)}>
              <div className="px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center justify-between cursor-default">
                🏃 Sprints <ChevronRight size={12} className="text-white/30" />
              </div>
              {ctxSub === "sprints" && (
                <div className="absolute top-0 bg-[#1a1a2e] border border-white/10 rounded-lg shadow-xl py-1 min-w-48 z-50"
                  style={ctxMenu && ctxMenu.x > window.innerWidth - 520 ? { right: "100%" } : { left: "100%" }}>
                  {taskSprints.filter(ts => ts.task_id === t.id).map(ts => (
                    <button key={`rm-${ts.sprint_id}`} onClick={async () => {
                      await apiCall("DELETE", `/api/sprints/${ts.sprint_id}/tasks/${t.id}`);
                      useStore.getState().loadTasks(); setCtxMenu(null);
                    }} className="w-full text-left px-3 py-1.5 text-red-400/70 hover:bg-white/5 flex items-center gap-2">
                      ✕ Remove from {ts.sprint_name}
                    </button>
                  ))}
                  {ctxSprints.filter(s => !taskSprints.some(ts => ts.task_id === t.id && ts.sprint_id === s.id)).map(s => (
                    <button key={`add-${s.id}`} onClick={async () => {
                      await apiCall("POST", `/api/sprints/${s.id}/tasks`, { task_ids: [t.id] });
                      useStore.getState().loadTasks(); setCtxMenu(null);
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

            {/* --- Assign submenu --- */}
            <div className="relative" onMouseEnter={() => setCtxSub("assign")} onMouseLeave={() => setCtxSub(null)}>
              <div className="px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center justify-between cursor-default">
                👤 Assignees <ChevronRight size={12} className="text-white/30" />
              </div>
              {ctxSub === "assign" && (
                <div className="absolute top-0 bg-[#1a1a2e] border border-white/10 rounded-lg shadow-xl py-1 min-w-48 z-50 max-h-64 overflow-y-auto"
                  style={ctxMenu && ctxMenu.x > window.innerWidth - 520 ? { right: "100%" } : { left: "100%" }}>
                  {assignees.length > 0 && (
                    <>
                      <div className="px-3 py-1 text-white/20 text-[10px] uppercase tracking-wider">Assigned</div>
                      {assignees.map(a => {
                        const hasBurns = ctxBurnUsers.includes(a);
                        return (
                          <button key={`rm-${a}`} disabled={hasBurns} onClick={async () => {
                            await apiCall("DELETE", `/api/tasks/${t.id}/assignees/${a}`);
                            setAssignees(prev => prev.filter(x => x !== a)); setCtxMenu(null);
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
                          setAssignees(prev => [...prev, u]); setCtxMenu(null);
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

            {/* --- Quick actions --- */}
            <button onClick={async () => {
              const allTasks = useStore.getState().tasks;
              const siblings = allTasks.filter((s: Task) => s.parent_id === t.parent_id).sort((a: Task, b: Task) => a.sort_order - b.sort_order);
              const idx = siblings.findIndex((s: Task) => s.id === t.id);
              if (idx > 0) {
                const prev = siblings[idx - 1];
                await updateTask(t.id, { sort_order: prev.sort_order });
                await updateTask(prev.id, { sort_order: t.sort_order });
              }
              setCtxMenu(null);
            }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              ↑ Move up
            </button>
            <button onClick={async () => {
              const allTasks = useStore.getState().tasks;
              const siblings = allTasks.filter((s: Task) => s.parent_id === t.parent_id).sort((a: Task, b: Task) => a.sort_order - b.sort_order);
              const idx = siblings.findIndex((s: Task) => s.id === t.id);
              if (idx < siblings.length - 1) {
                const next = siblings[idx + 1];
                await updateTask(t.id, { sort_order: next.sort_order });
                await updateTask(next.id, { sort_order: t.sort_order });
              }
              setCtxMenu(null);
            }} className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              ↓ Move down
            </button>
            <div className="border-t border-white/5 my-1" />
            <button onClick={() => { start(t.id); setCtxMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2"
              disabled={t.status === "completed" || (config?.leaf_only_mode && node.children.length > 0)}>
              ▶ Start timer
            </button>
            <button onClick={() => { setTimeReporting(true); setCtxMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              🕐 Log time
            </button>
            <button onClick={() => { setCommenting(true); setCtxMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              💬 Comment
            </button>
            <button onClick={() => { setAdding(true); setCtxMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              ＋ Add subtask
            </button>
            <button onClick={() => { onView(t.id); setCtxMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
              👁 View details
            </button>

            {isOwner && (
              <>
                <div className="border-t border-white/5 my-1" />
                <button onClick={() => { setEditingTitle(true); setTitleDraft(t.title); setCtxMenu(null); }}
                  className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
                  ✏️ Rename
                </button>
                <button onClick={() => { setEditingDesc(true); setDescDraft(t.description || ""); setCtxMenu(null); }}
                  className="w-full text-left px-3 py-1.5 text-white/60 hover:bg-white/5 flex items-center gap-2">
                  📝 Edit description
                </button>
                <button onClick={() => { handleDelete(); setCtxMenu(null); }}
                  className="w-full text-left px-3 py-1.5 text-red-400/70 hover:bg-red-500/10 flex items-center gap-2">
                  🗑 Delete
                </button>
              </>
            )}
          </div>
        </>
      )}

      {/* Inline time report */}
      <InlineTimeReport taskId={t.id} depth={depth} show={timeReporting} onClose={() => setTimeReporting(false)}
        onLogged={(h) => { setTotalHours(prev => prev + h); apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).then(setAssignees).catch(() => {}); }} />

      {/* Inline comments */}
      <AnimatePresence>
        {commenting && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden" style={{ marginLeft: depth * 24 + 48 }}>
            <div className="py-2 px-4">
              <CommentSection taskId={t.id} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Children */}
      <AnimatePresence>
        {expanded && node.children.length > 0 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden"
          >
            {node.children.map((child) => (
              <TaskNode key={child.task.id} node={child} depth={depth + 1} onView={onView}
                selectMode={selectMode} onSelect={onSelect} selectedTaskId={selectedTaskId} votedTaskIds={votedTaskIds}
                selectLabel={selectLabel} selectClassName={selectClassName} />
            ))}
          </motion.div>
        )}
      </AnimatePresence>
      {dropZone === "below" && <div className="h-0.5 bg-[var(--color-accent)] rounded mx-2" />}
    </div>
  );
}

export default function TaskList({ selectMode, onSelect, selectedTaskId, votedTaskIds, selectLabel, selectClassName, filterIds, excludeIds, rootOnly, leafOnly }: {
  selectMode?: boolean; onSelect?: (id: number) => void; selectedTaskId?: number | null; votedTaskIds?: Set<number>;
  selectLabel?: string; selectClassName?: string; filterIds?: Set<number>; excludeIds?: Set<number>; rootOnly?: boolean; leafOnly?: boolean;
} = {}) {
  const { tasks, createTask, teamScope } = useStore();
  const [newTitle, setNewTitle] = useState("");
  const [filter, setFilter] = useState<"all" | "active">("all");
  const [viewingTask, setViewingTask] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [bulkSelected, setBulkSelected] = useState<Set<number>>(new Set());

  const tree = useMemo(() => {
    let t = tasks;
    if (teamScope) t = t.filter(task => teamScope.has(task.id));
    if (filterIds) t = t.filter(task => filterIds.has(task.id));
    if (excludeIds) t = t.filter(task => !excludeIds.has(task.id));
    if (rootOnly) t = t.filter(task => task.parent_id === null);
    if (leafOnly) { const parentIds = new Set(tasks.map(tk => tk.parent_id).filter(Boolean)); t = t.filter(task => !parentIds.has(task.id)); }
    if (search.trim()) {
      const q = search.toLowerCase();
      const matchIds = new Set<number>();
      // Find matching tasks and all their ancestors
      for (const task of t) {
        if (matchSearch(task.title, q) || matchSearch(task.project ?? "", q) || matchSearch(task.user, q) || matchSearch(task.tags ?? "", q)) {
          matchIds.add(task.id);
          // Add ancestors
          let pid = task.parent_id;
          while (pid) { matchIds.add(pid); const parent = t.find(x => x.id === pid); pid = parent?.parent_id ?? null; }
        }
      }
      t = t.filter(task => matchIds.has(task.id));
    }
    return buildTree(t);
  }, [tasks, filterIds, excludeIds, search, rootOnly, leafOnly, teamScope]);
  const filtered = filter === "all" ? tree : tree.filter((n) => n.task.status !== "completed");

  const handleAddRoot = () => {
    if (!newTitle.trim()) return;
    createTask(newTitle.trim());
    setNewTitle("");
  };

  // Keyboard shortcuts (#37)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "/" && !e.ctrlKey) { e.preventDefault(); document.getElementById("task-search")?.focus(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  if (viewingTask !== null) {
    return <TaskDetailView taskId={viewingTask} onBack={() => setViewingTask(null)} onNavigate={(id) => setViewingTask(id)} />;
  }

  return (
    <div className={selectMode ? "flex flex-col gap-2 max-h-[50vh] overflow-hidden" : "flex flex-col gap-5 p-8 h-full overflow-hidden"}>
      {/* Add root project/task — only in full mode */}
      {!selectMode && (
      <div className="glass p-4 flex gap-3 items-center">
        <FolderOpen size={18} className="text-white/40 shrink-0" />
        <input
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAddRoot()}
          placeholder="New project or top-level task..."
          className="flex-1 bg-transparent border-none outline-none text-white placeholder-white/30 text-sm py-1"
        />
        <motion.button
          whileHover={{ scale: 1.1 }} whileTap={{ scale: 0.9 }}
          onClick={handleAddRoot}
          className="w-9 h-9 flex items-center justify-center rounded-lg bg-[var(--color-accent)] text-white shrink-0"
        >
          <Plus size={18} />
        </motion.button>
      </div>
      )}

      {/* Search + Filter */}
      <div className="flex gap-2 items-center">
        <div className="relative flex-1">
          <input id="task-search" value={search} onChange={e => setSearch(e.target.value)}
            placeholder={selectMode ? "Search (regex)..." : "Search tasks (regex)... (press /)"}
            className={`w-full bg-white/5 border border-white/10 text-xs text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)] ${selectMode ? "rounded px-2 py-1" : "rounded-full px-4 py-2 pr-16"}`} />
          {search && (
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
              <span className="text-[10px] text-white/30">{filtered.length} results</span>
              <button onClick={() => setSearch("")} className="text-white/30 hover:text-white/60 text-xs" aria-label="Clear search">✕</button>
            </div>
          )}
        </div>
        <button onClick={() => setFilter(filter === "all" ? "active" : "all")}
          className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium transition-all ${filter === "active" ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white/60"}`}>
          {filter === "active" ? "Active" : "All"} ({filtered.length})
        </button>
      </div>

      {/* Bulk actions toolbar */}
      {!selectMode && bulkSelected.size > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[var(--color-accent)]/10 border border-[var(--color-accent)]/20" role="toolbar" aria-live="polite" aria-label="Bulk actions">
          <span className="text-xs text-[var(--color-accent)] font-medium">{bulkSelected.size} selected</span>
          <button onClick={async () => {
            for (const id of bulkSelected) await useStore.getState().updateTask(id, { status: "completed" });
            setBulkSelected(new Set());
          }} className="px-2 py-0.5 rounded text-xs bg-[var(--color-success)]/20 text-[var(--color-success)]">✓ Done</button>
          <button onClick={async () => {
            for (const id of bulkSelected) await useStore.getState().updateTask(id, { status: "active" });
            setBulkSelected(new Set());
          }} className="px-2 py-0.5 rounded text-xs bg-[var(--color-accent)]/20 text-[var(--color-accent)]">▶ Active</button>
          <button onClick={() => {
            useStore.getState().showConfirm(`Delete ${bulkSelected.size} tasks?`, async () => {
              for (const id of bulkSelected) await apiCall("DELETE", `/api/tasks/${id}`);
              useStore.getState().loadTasks();
              setBulkSelected(new Set());
            });
          }} className="px-2 py-0.5 rounded text-xs bg-[var(--color-danger)]/20 text-[var(--color-danger)]">🗑 Delete</button>
          <button onClick={() => setBulkSelected(new Set())} className="ml-auto text-xs text-white/30 hover:text-white/50">Clear</button>
        </div>
      )}

      {/* Tree */}
      <div className="flex-1 overflow-y-auto space-y-2 pr-1"
        onDragOver={e => e.preventDefault()}
        onDrop={async e => {
          const dragId = Number(e.dataTransfer?.getData("text/plain"));
          if (dragId) { await useStore.getState().updateTask(dragId, { parent_id: null, sort_order: Date.now() }); }
        }}>
        <AnimatePresence>
          {filtered.map((node) => (
            <div key={node.task.id} className="flex items-start gap-1">
              {!selectMode && (
                <input type="checkbox" checked={bulkSelected.has(node.task.id)}
                  onChange={e => {
                    const next = new Set(bulkSelected);
                    if (e.target.checked) next.add(node.task.id); else next.delete(node.task.id);
                    setBulkSelected(next);
                  }}
                  className={`mt-3 shrink-0 accent-[var(--color-accent)] cursor-pointer ${bulkSelected.size > 0 ? "opacity-100" : "opacity-0 hover:opacity-100 focus:opacity-100"} peer`}
                  style={bulkSelected.size > 0 ? { opacity: 1 } : {}}
                />
              )}
              <div className="flex-1">
                <TaskNode node={node} depth={0} onView={setViewingTask}
                  selectMode={selectMode} onSelect={onSelect} selectedTaskId={selectedTaskId} votedTaskIds={votedTaskIds}
                  selectLabel={selectLabel} selectClassName={selectClassName} />
              </div>
            </div>
          ))}
        </AnimatePresence>

        {filtered.length === 0 && (
          <div className="text-center text-white/20 text-sm py-16">
            No projects yet. Create one above!
          </div>
        )}
      </div>
    </div>
  );
}

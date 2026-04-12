import { motion, AnimatePresence } from "framer-motion";
import { Plus, Trash2, Play, CheckCircle, Circle, ChevronRight, FolderOpen, Folder, MessageSquare, Eye, FileText, Clock } from "lucide-react";
import { useStore } from "../store/store";
import { useShallow } from "zustand/react/shallow";
import React, { useState, useCallback, useEffect, useContext } from "react";
import { countDescendants } from "../tree";
import type { TreeNode } from "../tree";
import { apiCall } from "../store/api";
import CommentSection from "./CommentSection";
import { InlineTimeReport } from "./TaskInlineEditors";
import TaskContextMenu from "./TaskContextMenu";
import { PRIORITY_COLORS } from "../constants";
import { SearchCtx, highlightMatch, ctxCacheTime, setCtxCacheTime } from "./TaskListContext";

export default function TaskNode({ node, depth, onView, selectMode, onSelect, selectedTaskId, votedTaskIds, selectLabel, selectClassName, bulkSelected, setBulkSelected }: {
  node: TreeNode; depth: number; onView: (id: number) => void;
  selectMode?: boolean; onSelect?: (id: number) => void; selectedTaskId?: number | null; votedTaskIds?: Set<number>;
  selectLabel?: string; selectClassName?: string;
  bulkSelected?: Set<number>; setBulkSelected?: (fn: (prev: Set<number>) => Set<number>) => void;
}) {
  const { engine, createTask, updateTask, deleteTask, start, username: currentUser, role, taskSprints, taskSprintsMap, burnTotals, allAssignees, config } = useStore(
    useShallow(s => ({ engine: s.engine, createTask: s.createTask, updateTask: s.updateTask, deleteTask: s.deleteTask, start: s.start, username: s.username, role: s.role, taskSprints: s.taskSprints, taskSprintsMap: s.taskSprintsMap, burnTotals: s.burnTotals, allAssignees: s.allAssignees, config: s.config }))
  );
  const searchQuery = useContext(SearchCtx);
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
  const [ctxSprints, setCtxSprints] = useState<{ id: number; name: string; status: string }[]>([]);
  const [ctxUsers, setCtxUsers] = useState<string[]>([]);
  const [ctxBurnUsers, setCtxBurnUsers] = useState<string[]>([]);
  const [dropZone, setDropZone] = useState<"above" | "on" | "below" | null>(null);

  const t = node.task;

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
    try { await deleteTask(t.id); } catch (e) { useStore.getState().toast(String(e), "error"); }
  }, [t.id, deleteTask]);

  const handleAdd = useCallback(() => {
    if (!newTitle.trim()) return;
    createTask(newTitle.trim(), t.id);
    setNewTitle(""); setAdding(false); setExpanded(true);
  }, [newTitle, t.id, createTask]);

  const longPressRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

  return (
    <div>
      {dropZone === "above" && <div className="h-0.5 bg-[var(--color-accent)] rounded mx-2" />}
      <motion.div
        layout="position"
        initial={{ opacity: 0, x: -10 }}
        animate={{ opacity: 1, x: 0 }}
        draggable
        onDragStart={((e: React.DragEvent) => {
          e.dataTransfer?.setData("text/plain", String(t.id));
          e.dataTransfer.effectAllowed = "move";
          setTimeout(() => { if (e.target) (e.target as HTMLElement).style.opacity = "0.4"; }, 0);
        }) as any}
        onDragEnd={((e: React.DragEvent) => { if (e.target) (e.target as HTMLElement).style.opacity = "1"; }) as any}
        onDragOver={(e: React.DragEvent) => {
          e.preventDefault();
          const rect = e.currentTarget.getBoundingClientRect();
          const y = e.clientY - rect.top;
          const zone = y < rect.height * 0.25 ? "above" : y > rect.height * 0.75 ? "below" : "on";
          setDropZone(zone);
        }}
        onDragLeave={() => setDropZone(null)}
        onDrop={async (e: React.DragEvent) => {
          e.preventDefault(); e.stopPropagation(); setDropZone(null);
          const dragId = Number(e.dataTransfer?.getData("text/plain"));
          if (!dragId || dragId === t.id) return;
          const isDescendantOf = (nodeId: number, ancestorId: number): boolean => {
            const taskMap = new Map(useStore.getState().tasks.map(tk => [tk.id, tk.parent_id]));
            let pid: number | null = nodeId;
            while (pid) { if (pid === ancestorId) return true; pid = taskMap.get(pid) ?? null; }
            return false;
          };
          if (isDescendantOf(t.id, dragId) || t.id === dragId) return;
          if (dropZone === "on") {
            await updateTask(dragId, { parent_id: t.id, sort_order: 0 });
          } else {
            const newParent = t.parent_id;
            const siblings = useStore.getState().tasks.filter(s => s.parent_id === newParent && s.id !== dragId).sort((a, b) => a.sort_order - b.sort_order);
            const idx = siblings.findIndex(s => s.id === t.id);
            const insertAt = dropZone === "above" ? idx : idx + 1;
            const before = insertAt > 0 ? siblings[insertAt - 1].sort_order : 0;
            const after = insertAt < siblings.length ? siblings[insertAt].sort_order : before + 2;
            const newOrder = Math.floor((before + after) / 2);
            await updateTask(dragId, { parent_id: newParent, sort_order: newOrder === before ? after + 1 : newOrder });
          }
        }}
        onTouchStart={(e) => {
          const touch = e.touches[0];
          longPressRef.current = setTimeout(() => {
            longPressRef.current = null;
            setCtxMenu({ x: touch.clientX, y: touch.clientY });
          }, 500);
        }}
        onTouchEnd={() => { if (longPressRef.current) { clearTimeout(longPressRef.current); longPressRef.current = null; } }}
        onTouchMove={() => { if (longPressRef.current) { clearTimeout(longPressRef.current); longPressRef.current = null; } }}
        onContextMenu={async (e) => {
          e.preventDefault();
          setCtxBurnUsers(allAssignees.get(t.id) || []);
          setCtxMenu({ x: e.clientX, y: e.clientY });
          const now = Date.now();
          if (!ctxSprints.length || ctxCacheTime < now - 5000) {
            const [sprints, planning, users] = await Promise.all([
              apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=active").catch(() => []),
              apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=planning").catch(() => []),
              apiCall<{id: number; username: string}[]>("GET", "/api/users").catch(() => []),
            ]);
            setCtxSprints([...sprints, ...planning]);
            setCtxUsers(users.map(u => u.username));
            setCtxCacheTime(now);
          }
        }}
        className={`flex items-center gap-3 group transition-all rounded-xl ${
          isProject ? "glass p-4" : "px-4 py-3 hover:bg-white/5"
        } ${engine?.current_task_id === t.id ? "ring-1 ring-[var(--color-work)]" : ""} ${dropZone === "on" ? "ring-1 ring-[var(--color-accent)]" : ""}`}
        style={{ marginLeft: depth > 0 ? depth * 24 : 0 }}
        tabIndex={0}
        onKeyDown={e => {
          if (e.key === "Enter") onView(t.id);
          if ((e.key === "F10" && e.shiftKey) || e.key === "ContextMenu") {
            e.preventDefault();
            const rect = e.currentTarget.getBoundingClientRect();
            setCtxBurnUsers(allAssignees.get(t.id) || []);
            setCtxMenu({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
          }
          if (e.key === "s" && !e.ctrlKey && !e.metaKey && !e.altKey && isOwner) {
            e.preventDefault();
            const cycle = ["backlog", "active", "in_progress", "completed"];
            const idx = cycle.indexOf(t.status);
            const next = cycle[(idx + 1) % cycle.length];
            updateTask(t.id, { status: next });
          }
          if (e.altKey) {
            const siblings = useStore.getState().tasks.filter(s => s.parent_id === t.parent_id).sort((a, b) => a.sort_order - b.sort_order);
            const idx = siblings.findIndex(s => s.id === t.id);
            if (e.key === "ArrowUp" && idx > 0) { e.preventDefault(); updateTask(t.id, { sort_order: siblings[idx - 1].sort_order - 1 }); }
            else if (e.key === "ArrowDown" && idx < siblings.length - 1) { e.preventDefault(); updateTask(t.id, { sort_order: siblings[idx + 1].sort_order + 1 }); }
          }
        }}
      >
        {!selectMode && depth > 0 && bulkSelected && setBulkSelected && (
          <input type="checkbox" checked={bulkSelected.has(t.id)}
            onChange={e => {
              setBulkSelected(prev => {
                const next = new Set(prev);
                if (e.target.checked) next.add(t.id); else next.delete(t.id);
                return next;
              });
            }}
            className={`shrink-0 accent-[var(--color-accent)] cursor-pointer ${bulkSelected.size > 0 ? "opacity-100" : "opacity-0 hover:opacity-100 focus:opacity-100"}`}
            style={bulkSelected.size > 0 ? { opacity: 1 } : {}}
          />
        )}
        <button onClick={() => setExpanded(!expanded)}
          className={`w-6 h-6 flex items-center justify-center rounded transition-all shrink-0 ${hasChildren ? "text-white/40 hover:text-white" : "text-transparent"}`}>
          <ChevronRight size={14} className={`transition-transform ${expanded ? "rotate-90" : ""}`} />
        </button>
        <button onClick={() => isOwner && updateTask(t.id, { status: t.status === "completed" ? "backlog" : "completed" })}
          className={`shrink-0 transition-colors ${isOwner ? "text-white/40 hover:text-white" : "text-white/20 cursor-default"}`}>
          {t.status === "completed" ? <CheckCircle size={18} className="text-[var(--color-success)]" /> : <Circle size={18} />}
        </button>
        {isProject && <span className="shrink-0 text-white/40">{expanded ? <FolderOpen size={16} /> : <Folder size={16} />}</span>}
        <div className="w-2 h-2 rounded-full shrink-0" style={{ background: PRIORITY_COLORS[t.priority] ?? "#6C7A89" }} title={`Priority ${t.priority}`} aria-label={`Priority ${t.priority}`} />
        <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${
          t.status === "completed" ? "bg-[var(--color-success)]/20 text-[var(--color-success)]"
          : t.status === "active" ? "bg-[var(--color-work)]/20 text-[var(--color-work)]"
          : "bg-white/5 text-white/25"
        }`}>{t.status === "completed" ? "Done" : t.status === "active" ? "WIP" : "Todo"}</span>

        <div className="flex-1 min-w-0">
          {editingTitle && isOwner ? (
            <input value={titleDraft} onChange={e => setTitleDraft(e.target.value)} autoFocus aria-label="Edit task title"
              onKeyDown={e => { if (e.key === "Enter" && titleDraft.trim()) { updateTask(t.id, { title: titleDraft.trim() }); setEditingTitle(false); } if (e.key === "Escape") setEditingTitle(false); }}
              onBlur={() => { if (titleDraft.trim()) updateTask(t.id, { title: titleDraft.trim() }); setEditingTitle(false); }}
              className={`text-sm w-full bg-transparent border-b border-[var(--color-accent)] outline-none ${isProject ? "font-semibold" : ""} text-white`} />
          ) : (
            <div className={`text-sm truncate ${isProject ? "font-semibold" : ""} ${t.status === "completed" ? "line-through text-white/30" : "text-white/90"} ${isOwner ? "cursor-text" : "cursor-not-allowed"}`}
              onDoubleClick={() => { if (isOwner) { setTitleDraft(t.title); setEditingTitle(true); } }}>
              {searchQuery ? highlightMatch(t.title, searchQuery) : t.title}
            </div>
          )}
          {t.description && !editingDesc && (
            <div className="text-xs text-white/40 mt-0.5 truncate cursor-pointer hover:text-white/60"
              onClick={() => { setEditingDesc(true); setDescDraft(t.description || ""); }}>{t.description}</div>
          )}
          <div className="flex gap-2 text-xs text-white/30 mt-0.5 flex-wrap">
            {t.project && <span className="bg-white/5 px-1.5 py-0.5 rounded">{t.project}</span>}
            {(taskSprintsMap.get(t.id) || []).map(ts => (
              <span key={ts.sprint_id} className={`px-1.5 py-0.5 rounded text-[10px] ${ts.sprint_status === "active" ? "bg-green-500/20 text-green-400" : "bg-green-500/10 text-green-400/40"}`}>🏃 {ts.sprint_name}</span>
            ))}
            <span className="bg-white/5 px-1.5 py-0.5 rounded">👤 {t.user}</span>
            {assignees.filter(a => a !== t.user).map(a => <span key={a} className="bg-white/5 px-1.5 py-0.5 rounded text-white/20">{a}</span>)}
            {descendantCount > 0 && <span>{doneCount}/{node.children.length} done</span>}
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
            {t.attachment_count > 0 && <span className="text-white/30" title={`${t.attachment_count} attachment${t.attachment_count > 1 ? "s" : ""}`}>📎{t.attachment_count}</span>}
            {t.status === "blocked" && <span className="text-red-400/60" title="Blocked by dependencies">🔗</span>}
            {remainingPct !== null && (
              <span className={`${remainingPct > 50 ? "text-[var(--color-success)]" : remainingPct > 20 ? "text-[var(--color-warning)]" : "text-[var(--color-danger)]"}`}>{remainingPct}% left</span>
            )}
          </div>
        </div>

        {selectMode && (
          <div className="flex items-center gap-2 shrink-0">
            {votedTaskIds?.has(t.id) && <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--color-success)]/10 text-[var(--color-success)]">✓ estimated</span>}
            {selectedTaskId === t.id && <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--color-accent)]/20 text-[var(--color-accent)]">voting</span>}
            {onSelect && (
              <button onClick={(e) => { e.stopPropagation(); onSelect(t.id); }}
                className={selectClassName || `px-2 py-1 rounded-lg text-xs font-semibold transition-all ${selectedTaskId === t.id ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"}`}>
                {selectLabel || (selectedTaskId === t.id ? "voting" : votedTaskIds?.has(t.id) ? "re-vote" : "vote")}
              </button>
            )}
          </div>
        )}

        <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
          <button onClick={() => setCommenting(!commenting)} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all" title="Comment"><MessageSquare size={14} /></button>
          <button onClick={() => setTimeReporting(!timeReporting)} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-work)] hover:bg-white/5 transition-all" title="Log time"><Clock size={14} /></button>
          <button onClick={() => { setEditingDesc(!editingDesc); setDescDraft(t.description || ""); }} className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all ${isOwner ? "text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5" : "hidden"}`} title="Edit description"><FileText size={14} /></button>
          <button onClick={() => onView(t.id)} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all" title="View & Export"><Eye size={14} /></button>
          <button onClick={() => setAdding(!adding)} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all" title="Add subtask"><Plus size={14} /></button>
          {t.status !== "completed" && (!config?.leaf_only_mode || node.children.length === 0) && (
            <button onClick={() => start(t.id)} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-work)] hover:bg-white/5 transition-all" title="Focus on this"><Play size={14} /></button>
          )}
          <button onClick={handleDelete} className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all ${isOwner ? "text-white/30 hover:text-[var(--color-danger)] hover:bg-white/5" : "hidden"}`} title={isActive ? "Stop timer first" : "Delete"}><Trash2 size={14} /></button>
        </div>
      </motion.div>

      <AnimatePresence>
        {adding && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden" style={{ marginLeft: (depth + 1) * 24 + 24 }}>
            <div className="flex gap-2 items-center py-2 px-4">
              <input value={newTitle} onChange={(e) => setNewTitle(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") handleAdd(); if (e.key === "Escape") { setAdding(false); setNewTitle(""); } }}
                placeholder={`Add subtask to "${t.title}"...`} aria-label={`Add subtask to ${t.title}`}
                className="flex-1 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-accent)]" autoFocus />
              <button onClick={handleAdd} className="w-8 h-8 flex items-center justify-center rounded-lg bg-[var(--color-accent)] text-white shrink-0"><Plus size={14} /></button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {editingDesc && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden" style={{ marginLeft: depth * 24 + 48 }}>
            <div className="flex gap-2 items-start py-2 px-4">
              <textarea value={descDraft} onChange={(e) => setDescDraft(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); updateTask(t.id, { description: descDraft || null }); setEditingDesc(false); } if (e.key === "Escape") setEditingDesc(false); }}
                placeholder="Add description... (Enter to save, Esc to cancel)" aria-label="Edit description"
                className="flex-1 bg-white/5 border border-white/10 rounded-lg text-xs text-white placeholder-white/30 px-3 py-2 outline-none focus:border-[var(--color-accent)] resize-none min-h-[60px]" autoFocus rows={3} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {ctxMenu && (
        <TaskContextMenu pos={ctxMenu} task={t} node={node} isOwner={isOwner}
          assignees={assignees} ctxSprints={ctxSprints} ctxUsers={ctxUsers} ctxBurnUsers={ctxBurnUsers}
          taskSprints={taskSprints} config={config}
          onClose={() => setCtxMenu(null)} updateTask={updateTask} start={start} setAssignees={setAssignees}
          setEditingTitle={setEditingTitle} setTitleDraft={setTitleDraft}
          setEditingDesc={setEditingDesc} setDescDraft={setDescDraft}
          handleDelete={handleDelete} setTimeReporting={setTimeReporting} setCommenting={setCommenting} setAdding={setAdding} onView={onView} />
      )}

      <InlineTimeReport taskId={t.id} depth={depth} show={timeReporting} onClose={() => setTimeReporting(false)}
        onLogged={(h) => { setTotalHours(prev => prev + h); apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).then(setAssignees).catch(() => {}); }} />

      <AnimatePresence>
        {commenting && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden" style={{ marginLeft: depth * 24 + 48 }}>
            <div className="py-2 px-4"><CommentSection taskId={t.id} /></div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {expanded && node.children.length > 0 && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden">
            {node.children.map((child) => (
              <TaskNode key={child.task.id} node={child} depth={depth + 1} onView={onView}
                selectMode={selectMode} onSelect={onSelect} selectedTaskId={selectedTaskId} votedTaskIds={votedTaskIds}
                selectLabel={selectLabel} selectClassName={selectClassName}
                bulkSelected={bulkSelected} setBulkSelected={setBulkSelected} />
            ))}
          </motion.div>
        )}
      </AnimatePresence>
      {dropZone === "below" && <div className="h-0.5 bg-[var(--color-accent)] rounded mx-2" />}
    </div>
  );
}

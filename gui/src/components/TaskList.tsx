import { motion, AnimatePresence } from "framer-motion";
import { Plus, Trash2, Play, CheckCircle, Circle, ChevronRight, FolderOpen, Folder, MessageSquare, Eye, FileText, Clock } from "lucide-react";
import { useStore } from "../store/store";
import { useState, useMemo, useCallback, useEffect, createContext, useContext } from "react";
import { buildTree, countDescendants } from "../tree";

const SearchCtx = createContext("");

function highlightMatch(text: string, query: string) {
  if (!query) return text;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return text;
  return <>{text.slice(0, idx)}<mark className="bg-[var(--color-accent)]/30 text-inherit rounded px-0.5">{text.slice(idx, idx + query.length)}</mark>{text.slice(idx + query.length)}</>;
}
import { matchSearch } from "../utils";
import type { TreeNode } from "../tree";
import { apiCall } from "../store/api";
import type { Task } from "../store/api";
import TaskDetailView from "./TaskDetailView";
import CommentSection from "./CommentSection";
import { InlineTimeReport, InlineComment } from "./TaskInlineEditors";
import TaskContextMenu from "./TaskContextMenu";

import { PRIORITY_COLORS } from "../constants";

let ctxCacheTime = 0;

function TaskNode({ node, depth, onView, selectMode, onSelect, selectedTaskId, votedTaskIds, selectLabel, selectClassName, bulkSelected, setBulkSelected }: {
  node: TreeNode; depth: number; onView: (id: number) => void;
  selectMode?: boolean; onSelect?: (id: number) => void; selectedTaskId?: number | null; votedTaskIds?: Set<number>;
  selectLabel?: string; selectClassName?: string;
  bulkSelected?: Set<number>; setBulkSelected?: (fn: (prev: Set<number>) => Set<number>) => void;
}) {
  const engine = useStore(s => s.engine);
  const createTask = useStore(s => s.createTask);
  const updateTask = useStore(s => s.updateTask);
  const deleteTask = useStore(s => s.deleteTask);
  const start = useStore(s => s.start);
  const currentUser = useStore(s => s.username);
  const role = useStore(s => s.role);
  const taskSprints = useStore(s => s.taskSprints);
  const taskSprintsMap = useStore(s => s.taskSprintsMap);
  const burnTotals = useStore(s => s.burnTotals);
  const allAssignees = useStore(s => s.allAssignees);
  const config = useStore(s => s.config);
  const tasks = useStore(s => s.tasks);
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
        onDragStart={(e: React.DragEvent) => {
          e.dataTransfer?.setData("text/plain", String(t.id));
          e.dataTransfer.effectAllowed = "move";
          // Reduce opacity of dragged element
          setTimeout(() => { if (e.target) e.target.style.opacity = "0.4"; }, 0);
        }}
        onDragEnd={(e: React.DragEvent) => { if (e.target) e.target.style.opacity = "1"; }}
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
          // Prevent dropping parent onto its own descendant
          const isDescendantOf = (nodeId: number, ancestorId: number): boolean => {
            let pid: number | null = nodeId;
            while (pid) { if (pid === ancestorId) return true; const p = tasks.find(tk => tk.id === pid); pid = p?.parent_id ?? null; }
            return false;
          };
          if (isDescendantOf(t.id, dragId) || t.id === dragId) return;
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
          setCtxBurnUsers(allAssignees.get(t.id) || []);
          setCtxMenu({ x: e.clientX, y: e.clientY });
          // Refresh cached data in background
          const now = Date.now();
          if (!ctxSprints.length || ctxCacheTime < now - 5000) {
            const [sprints, planning, users] = await Promise.all([
              apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=active").catch(() => []),
              apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=planning").catch(() => []),
              apiCall<string[]>("GET", "/api/users").catch(() => []),
            ]);
            setCtxSprints([...sprints, ...planning]);
            setCtxUsers(users);
            ctxCacheTime = now;
          }
        }}
        className={`flex items-center gap-3 group transition-all rounded-xl ${
          isProject ? "glass p-4" : "px-4 py-3 hover:bg-white/5"
        } ${engine?.current_task_id === t.id ? "ring-1 ring-[var(--color-work)]" : ""} ${dropZone === "on" ? "ring-1 ring-[var(--color-accent)]" : ""}`}
        style={{ marginLeft: depth > 0 ? depth * 24 : 0 }}
        tabIndex={0}
        onKeyDown={e => { if (e.key === "Enter") onView(t.id); }}
      >
        {/* Bulk checkbox for subtasks */}
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
          title={`Priority ${t.priority}`}
          aria-label={`Priority ${t.priority}`}
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
              aria-label="Edit task title"
              onKeyDown={e => {
                if (e.key === "Enter" && titleDraft.trim()) { updateTask(t.id, { title: titleDraft.trim() }); setEditingTitle(false); }
                if (e.key === "Escape") setEditingTitle(false);
              }}
              onBlur={() => { if (titleDraft.trim()) { updateTask(t.id, { title: titleDraft.trim() }); } setEditingTitle(false); }}
              className={`text-sm w-full bg-transparent border-b border-[var(--color-accent)] outline-none ${isProject ? "font-semibold" : ""} text-white`} />
          ) : (
            <div className={`text-sm truncate ${isProject ? "font-semibold" : ""} ${t.status === "completed" ? "line-through text-white/30" : "text-white/90"} ${isOwner ? "cursor-text" : "cursor-not-allowed"}`}
              onDoubleClick={() => { if (isOwner) { setTitleDraft(t.title); setEditingTitle(true); } }}>
              {searchQuery ? highlightMatch(t.title, searchQuery) : t.title}
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
            {(taskSprintsMap.get(t.id) || []).map(ts => (
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
            {t.attachment_count > 0 && <span className="text-white/30" title={`${t.attachment_count} attachment${t.attachment_count > 1 ? "s" : ""}`}>📎{t.attachment_count}</span>}
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
                placeholder={`Add subtask to "${t.title}"...`} aria-label={`Add subtask to ${t.title}`}
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
                placeholder="Add description... (Enter to save, Esc to cancel)" aria-label="Edit description"
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
        <TaskContextMenu
          pos={ctxMenu} task={t} node={node} isOwner={isOwner}
          assignees={assignees} ctxSprints={ctxSprints} ctxUsers={ctxUsers} ctxBurnUsers={ctxBurnUsers}
          taskSprints={taskSprints} config={config}
          onClose={() => setCtxMenu(null)}
          updateTask={updateTask} start={start}
          setAssignees={setAssignees}
          setEditingTitle={setEditingTitle} setTitleDraft={setTitleDraft}
          setEditingDesc={setEditingDesc} setDescDraft={setDescDraft}
          handleDelete={handleDelete}
          setTimeReporting={setTimeReporting} setCommenting={setCommenting} setAdding={setAdding}
          onView={onView}
        />
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
  const [treeKey, setTreeKey] = useState(0);
  const [bulkSprints, setBulkSprints] = useState<{ id: number; name: string }[]>([]);

  useEffect(() => {
    if (bulkSelected.size > 0 && bulkSprints.length === 0) {
      apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=active")
        .then(s => setBulkSprints(s || [])).catch(() => {});
    }
  }, [bulkSelected.size]);

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
  const filtered = filter === "all" ? tree.filter(n => n.task.status !== "archived") : tree.filter((n) => n.task.status !== "completed" && n.task.status !== "archived");

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
          aria-label="New project or top-level task"
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
            placeholder={selectMode ? "Search..." : "Search tasks... (press /)"}
            aria-label="Search tasks"
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
        <button onClick={() => setTreeKey(k => k + 1)} title="Expand all"
          className="shrink-0 px-2 py-1 rounded-full text-xs bg-white/5 text-white/40 hover:text-white/60">⊞</button>
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
          <select onChange={async e => {
            const sid = e.target.value; if (!sid) return;
            await apiCall("POST", `/api/sprints/${sid}/tasks`, { task_ids: [...bulkSelected] });
            useStore.getState().loadTasks(); useStore.getState().toast(`Added ${bulkSelected.size} tasks to sprint`);
            setBulkSelected(new Set()); e.target.value = "";
          }} className="px-2 py-0.5 rounded text-xs bg-white/5 text-white/40 outline-none" defaultValue="">
            <option value="" disabled>🏃 Add to sprint...</option>
            {bulkSprints.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
          <button onClick={() => setBulkSelected(new Set())} className="ml-auto text-xs text-white/30 hover:text-white/50">Clear</button>
        </div>
      )}

      {/* Tree */}
      <SearchCtx.Provider value={search}>
      <div key={treeKey} className="flex-1 overflow-y-auto space-y-2 pr-1"
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
                  selectLabel={selectLabel} selectClassName={selectClassName}
                  bulkSelected={bulkSelected} setBulkSelected={setBulkSelected} />
              </div>
            </div>
          ))}
        </AnimatePresence>

        {filtered.length === 0 && (
          <div className="text-center text-white/20 text-sm py-16">
            {searchQuery ? "No matching tasks" : "No projects yet. Create one above!"}
          </div>
        )}
      </div>
      </SearchCtx.Provider>
    </div>
  );
}

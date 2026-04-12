import { motion, AnimatePresence } from "framer-motion";
import { Plus, FolderOpen } from "lucide-react";
import { useStore } from "../store/store";
import React, { useState, useMemo, useCallback, useEffect } from "react";
import { buildTree } from "../tree";
import { matchSearch } from "../utils";
import type { TreeNode } from "../tree";
import { apiCall } from "../store/api";
import type { Task } from "../store/api";
import TaskDetailView from "./TaskDetailView";
import TaskNode from "./TaskNode";
import { SearchCtx } from "./TaskListContext";

export default function TaskList({ selectMode, onSelect, selectedTaskId, votedTaskIds, selectLabel, selectClassName, filterIds, excludeIds, rootOnly, leafOnly }: {
  selectMode?: boolean; onSelect?: (id: number) => void; selectedTaskId?: number | null; votedTaskIds?: Set<number>;
  selectLabel?: string; selectClassName?: string; filterIds?: Set<number>; excludeIds?: Set<number>; rootOnly?: boolean; leafOnly?: boolean;
} = {}) {
  const { tasks, createTask, teamScope, username } = useStore();
  const [newTitle, setNewTitle] = useState("");
  const [filter, setFilter] = useState<"all" | "active" | "mine">("all");
  // F10: Saved filters
  const [savedFilters, setSavedFilters] = useState<{ name: string; search: string; filter: string }[]>(() => {
    try { return JSON.parse(localStorage.getItem("pomo_saved_filters") || "[]"); } catch { return []; }
  });
  const [savingFilter, setSavingFilter] = useState(false);
  const [filterNameDraft, setFilterNameDraft] = useState("");
  const saveCurrentFilter = () => {
    if (!search.trim()) return;
    if (!savingFilter) { setSavingFilter(true); setFilterNameDraft(""); return; }
    if (!filterNameDraft.trim()) return;
    const next = [...savedFilters, { name: filterNameDraft.trim(), search, filter }];
    setSavedFilters(next);
    localStorage.setItem("pomo_saved_filters", JSON.stringify(next));
    setSavingFilter(false);
  };
  const [viewStack, setViewStack] = useState<number[]>([]);
  const [search, setSearch] = useState("");
  const [bulkSelected, setBulkSelected] = useState<Set<number>>(new Set());
  const [viewMode, setViewMode] = useState<"tree" | "table">("tree");
  const [treeKey, setTreeKey] = useState(0);
  const [tableSort, setTableSort] = useState<"title" | "status" | "priority" | "estimated" | "due" | "user">("title");
  const [bulkSprints, setBulkSprints] = useState<{ id: number; name: string }[]>([]);
  // BL12: FTS5 search results with highlights
  const [ftsResults, setFtsResults] = useState<{ id: number; title: string; snippet: string }[] | null>(null);
  useEffect(() => {
    if (search.trim().length < 2) { setFtsResults(null); return; }
    const timer = setTimeout(() => {
      apiCall<typeof ftsResults>("GET", `/api/tasks/search?q=${encodeURIComponent(search)}&limit=20`).then(setFtsResults).catch(() => setFtsResults(null));
    }, 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    if (bulkSelected.size > 0 && bulkSprints.length === 0) {
      apiCall<{ id: number; name: string; status: string }[]>("GET", "/api/sprints?status=active")
        .then(s => setBulkSprints(s || [])).catch(() => {});
    }
  }, [bulkSelected.size]);

  const loading = useStore(s => s.loading.tasks);
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
  const filtered = filter === "mine" ? tree.filter(n => n.task.user === username && n.task.status !== "archived")
    : filter === "all" ? tree.filter(n => n.task.status !== "archived")
    : tree.filter((n) => n.task.status !== "completed" && n.task.status !== "archived");
  const [sortBy, setSortBy] = useState<"order" | "priority" | "due" | "updated">("order");
  const sorted = sortBy === "order" ? filtered : [...filtered].sort((a, b) => {
    if (sortBy === "priority") return a.task.priority - b.task.priority;
    if (sortBy === "due") return (a.task.due_date || "9999") < (b.task.due_date || "9999") ? -1 : 1;
    return b.task.updated_at < a.task.updated_at ? -1 : 1;
  });

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

  if (viewStack.length > 0) {
    const viewingTask = viewStack[viewStack.length - 1];
    return <TaskDetailView taskId={viewingTask} onBack={() => setViewStack(s => s.slice(0, -1))} onNavigate={(id) => setViewStack(s => [...s, id])} />;
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
          onPaste={(e) => {
            const text = e.clipboardData.getData("text");
            const lines = text.split("\n").map(l => l.replace(/^[-*•]\s*/, "").replace(/^- \[[ x]\] /, "").trim()).filter(Boolean);
            if (lines.length > 1) {
              e.preventDefault();
              // UX8: Limit bulk paste to 50 tasks
              const limited = lines.slice(0, 50);
              limited.forEach(l => createTask(l));
              const msg = lines.length > 50 ? `Created 50 tasks (${lines.length - 50} skipped, max 50)` : `Created ${limited.length} tasks from clipboard`;
              useStore.getState().toast(msg);
            }
          }}
          placeholder="New project or top-level task..."
          aria-label="New project or top-level task"
          data-new-task-input
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
              <span className="text-[10px] text-white/30">{sorted.length} results</span>
              <button onClick={() => setSearch("")} className="text-white/30 hover:text-white/60 text-xs" aria-label="Clear search">✕</button>
              <button onClick={saveCurrentFilter} className="text-white/30 hover:text-[var(--color-accent)] text-xs" title="Save filter">💾</button>
              {savingFilter && <input autoFocus value={filterNameDraft} onChange={e => setFilterNameDraft(e.target.value)}
                onKeyDown={e => { if (e.key === "Enter") saveCurrentFilter(); if (e.key === "Escape") setSavingFilter(false); }}
                onBlur={() => setSavingFilter(false)}
                placeholder="Name..." className="bg-white/10 text-xs text-white/70 rounded px-1 py-0.5 w-20 outline-none" />}
            </div>
          )}
        </div>
        <button onClick={() => setFilter(f => f === "all" ? "active" : f === "active" ? "mine" : "all")}
          className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium transition-all ${filter !== "all" ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white/60"}`}>
          {filter === "active" ? "Active" : filter === "mine" ? "Mine" : "All"} ({sorted.length})
        </button>
        <select value={sortBy} onChange={e => setSortBy(e.target.value as any)} aria-label="Sort tasks"
          className="text-[10px] bg-transparent border border-white/10 rounded px-1 py-0.5 text-white/40">
          <option value="order">Manual</option>
          <option value="priority">Priority</option>
          <option value="due">Due date</option>
          <option value="updated">Updated</option>
        </select>
        <button onClick={() => setTreeKey(k => k + 1)} title="Expand all"
          className="shrink-0 px-2 py-1 rounded-full text-xs bg-white/5 text-white/40 hover:text-white/60">⊞</button>
        <button onClick={() => setViewMode(v => v === "tree" ? "table" : "tree")} title={viewMode === "tree" ? "Table view" : "Tree view"}
          className="shrink-0 px-2 py-1 rounded-full text-xs bg-white/5 text-white/40 hover:text-white/60">{viewMode === "tree" ? "☰" : "🌳"}</button>
      </div>
      {savedFilters.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {savedFilters.map((sf, i) => (
            <button key={i} onClick={() => { setSearch(sf.search); setFilter(sf.filter as any); }}
              className="text-[10px] px-2 py-0.5 rounded-full bg-white/5 text-white/40 hover:text-white/60 flex items-center gap-1">
              {sf.name}
              <span onClick={e => { e.stopPropagation(); const next = savedFilters.filter((_, j) => j !== i); setSavedFilters(next); localStorage.setItem("pomo_saved_filters", JSON.stringify(next)); }}
                className="hover:text-red-400">✕</span>
            </button>
          ))}
        </div>
      )}

      {/* BL12: FTS5 search results with highlighted snippets */}
      {ftsResults && ftsResults.length > 0 && (
        <div className="bg-[var(--color-surface)] rounded-lg border border-white/5 p-2 space-y-1">
          <div className="text-[10px] text-white/30 mb-1">Search results ({ftsResults.length})</div>
          {ftsResults.map(r => (
            <div key={r.id} role="button" tabIndex={0} className="text-xs text-white/60 py-0.5 cursor-pointer hover:text-white/80"
              onClick={() => { setSearch(""); setFtsResults(null); setViewStack([r.id]); }}
              onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setSearch(""); setFtsResults(null); setViewStack([r.id]); } }}>
              <span>{r.title.replace(/<[^>]*>/g, "")}</span>
              {r.snippet && <span className="text-white/30 ml-2">{r.snippet.replace(/<[^>]*>/g, "")}</span>}
            </div>
          ))}
        </div>
      )}

      {/* Bulk actions toolbar */}
      {!selectMode && bulkSelected.size > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[var(--color-accent)]/10 border border-[var(--color-accent)]/20" role="toolbar" aria-live="polite" aria-label="Bulk actions">
          <span className="text-xs text-[var(--color-accent)] font-medium">{bulkSelected.size} selected</span>
          <button onClick={() => {
            const visible = tree.flatMap(function flat(n: TreeNode): number[] { return [n.task.id, ...n.children.flatMap(flat)]; });
            setBulkSelected(new Set(visible));
          }} className="px-2 py-0.5 rounded text-xs bg-white/5 text-white/50">Select all</button>
          <button onClick={() => setBulkSelected(new Set())} className="px-2 py-0.5 rounded text-xs bg-white/5 text-white/50">Clear</button>
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

      {/* Tree or Table */}
      {viewMode === "table" ? (
        <div className="flex-1 overflow-y-auto text-xs">
          <table className="w-full">
            <thead className="sticky top-0 bg-[var(--color-bg)]">
              <tr className="text-white/30 text-left">
                <th scope="col" className="py-1 px-1 w-6"></th>
                {([["title","Title"],["status","Status"],["priority","Priority"],["estimated","Est"],["due","Due"],["user","Owner"]] as const).map(([k,label]) => (
                  <th key={k} scope="col" className={`py-1 px-1 cursor-pointer hover:text-white/50 ${tableSort === k ? "text-white/60" : ""}`}
                    aria-sort={tableSort === k ? "descending" : undefined}
                    onClick={() => setTableSort(k as any)}>{label}{tableSort === k ? " ▾" : ""}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {sorted.flatMap(function flat(n: TreeNode): TreeNode[] { return [n, ...n.children.flatMap(flat)]; }).sort((a, b) => {
                const ta = a.task, tb = b.task;
                if (tableSort === "priority") return ta.priority - tb.priority;
                if (tableSort === "status") return ta.status.localeCompare(tb.status);
                if (tableSort === "estimated") return tb.estimated - ta.estimated;
                if (tableSort === "due") return (ta.due_date || "9999").localeCompare(tb.due_date || "9999");
                if (tableSort === "user") return ta.user.localeCompare(tb.user);
                return ta.title.localeCompare(tb.title);
              }).map(n => (
                <tr key={n.task.id} onClick={() => setViewStack([n.task.id])} className="hover:bg-white/5 cursor-pointer border-t border-white/5">
                  <td className="py-1 px-1 w-6">
                    {!selectMode && <input type="checkbox" checked={bulkSelected.has(n.task.id)}
                      onChange={e => { const next = new Set(bulkSelected); if (e.target.checked) next.add(n.task.id); else next.delete(n.task.id); setBulkSelected(next); }}
                      onClick={e => e.stopPropagation()} className="accent-[var(--color-accent)]" />}
                  </td>
                  <td className="py-1 px-2 text-white/80 truncate max-w-[200px]">{n.task.title}</td>
                  <td className="py-1 px-1 text-white/40">{n.task.status}</td>
                  <td className="py-1 px-1 text-white/40">P{n.task.priority}</td>
                  <td className="py-1 px-1 text-white/40">{n.task.estimated}🍅</td>
                  <td className="py-1 px-1 text-white/40">{n.task.due_date?.slice(5) || "—"}</td>
                  <td className="py-1 px-1 text-white/40">{n.task.user}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
      <SearchCtx.Provider value={search}>
      <div key={treeKey} className="flex-1 overflow-y-auto space-y-2 pr-1"
        onDragOver={e => e.preventDefault()}
        onDrop={async e => {
          const dragId = Number(e.dataTransfer?.getData("text/plain"));
          if (dragId) { await useStore.getState().updateTask(dragId, { parent_id: null, sort_order: Date.now() }); }
        }}>
        <AnimatePresence>
          {sorted.map((node) => (
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
                <TaskNode node={node} depth={0} onView={(id) => setViewStack([id])}
                  selectMode={selectMode} onSelect={onSelect} selectedTaskId={selectedTaskId} votedTaskIds={votedTaskIds}
                  selectLabel={selectLabel} selectClassName={selectClassName}
                  bulkSelected={bulkSelected} setBulkSelected={setBulkSelected} />
              </div>
            </div>
          ))}
        </AnimatePresence>

        {sorted.length === 0 && !loading && (
          <div className="text-center text-white/20 text-sm py-16">
            {search ? "No matching tasks" : "No projects yet. Create one above!"}
          </div>
        )}
        {sorted.length === 0 && loading && (
          <div className="space-y-2 py-4">
            {[1,2,3].map(i => <div key={i} className="h-10 rounded-lg bg-white/5 animate-pulse" />)}
          </div>
        )}
      </div>
      </SearchCtx.Provider>
      )}
    </div>
  );
}

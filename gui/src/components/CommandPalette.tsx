import { useState, useEffect, useRef } from "react";
import { Search, FileText, MessageSquare, Zap } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface SearchResult {
  type: string;
  id: number;
  title?: string;
  name?: string;
  snippet?: string;
  task_id?: number;
}

export default function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<{ tasks: SearchResult[]; comments: SearchResult[]; sprints: SearchResult[] }>({ tasks: [], comments: [], sprints: [] });
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Cmd+K / Ctrl+K to open
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") { e.preventDefault(); setOpen(o => !o); }
      if (e.key === "Escape" && open) setOpen(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open]);

  // Focus input when opened
  useEffect(() => { if (open) { setQuery(""); setResults({ tasks: [], comments: [], sprints: [] }); setTimeout(() => inputRef.current?.focus(), 50); } }, [open]);

  // Debounced search
  useEffect(() => {
    if (!query.trim() || !open) { setResults({ tasks: [], comments: [], sprints: [] }); return; }
    const timer = setTimeout(() => {
      apiCall<typeof results>("GET", `/api/search?q=${encodeURIComponent(query)}&limit=8`).then(r => { setResults(r); setSelected(0); }).catch(e => console.error(e));
    }, 200);
    return () => clearTimeout(timer);
  }, [query, open]);

  const allResults = [...results.tasks, ...results.comments, ...results.sprints];

  const navigate = (r: SearchResult) => {
    setOpen(false);
    if (r.type === "task") useStore.getState().setTab("tasks");
    else if (r.type === "sprint") useStore.getState().setTab("sprints");
    else if (r.type === "comment" && r.task_id) useStore.getState().setTab("tasks");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setSelected(s => Math.min(s + 1, allResults.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSelected(s => Math.max(s - 1, 0)); }
    else if (e.key === "Enter" && allResults[selected]) { navigate(allResults[selected]); }
  };

  if (!open) return null;

  const icon = (type: string) => type === "task" ? <FileText size={14} /> : type === "comment" ? <MessageSquare size={14} /> : <Zap size={14} />;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/50" onClick={() => setOpen(false)}>
      <div className="w-full max-w-lg mx-4 glass rounded-xl overflow-hidden shadow-2xl" onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-3 px-4 py-3 border-b border-white/10">
          <Search size={16} className="text-white/30 shrink-0" />
          <input ref={inputRef} value={query} onChange={e => setQuery(e.target.value)} onKeyDown={handleKeyDown}
            placeholder="Search tasks, comments, sprints..." className="flex-1 bg-transparent text-sm text-white placeholder-white/30 outline-none" />
          <kbd className="text-[10px] text-white/20 bg-white/5 px-1.5 py-0.5 rounded">ESC</kbd>
        </div>
        {allResults.length > 0 && (
          <div className="max-h-80 overflow-y-auto py-1">
            {allResults.map((r, i) => (
              <button key={`${r.type}-${r.id}`} onClick={() => navigate(r)}
                className={`w-full text-left px-4 py-2 flex items-center gap-3 text-sm ${i === selected ? "bg-[var(--color-accent)]/20 text-white" : "text-white/60 hover:bg-white/5"}`}>
                <span className="text-white/30">{icon(r.type)}</span>
                <div className="flex-1 min-w-0">
                  <div className="truncate">{r.title || r.name || r.snippet}</div>
                  {r.snippet && r.title && <div className="text-[10px] text-white/30 truncate">{r.snippet}</div>}
                </div>
                <span className="text-[10px] text-white/20 shrink-0">{r.type}</span>
              </button>
            ))}
          </div>
        )}
        {query && allResults.length === 0 && (
          <div className="px-4 py-6 text-center text-sm text-white/30">No results for "{query}"</div>
        )}
        {!query && (
          <div className="px-4 py-4 text-center text-xs text-white/20">Type to search across tasks, comments, and sprints</div>
        )}
      </div>
    </div>
  );
}

import { useState, useEffect } from "react";
import { Edit3, Save, Download } from "lucide-react";
import type { TaskDetail } from "../store/api";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

export function formatDuration(s: number) {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export function EditField({ label, value, type, onSave }: { label: string; value: string | number; type?: string; onSave: (v: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(String(value));

  useEffect(() => { setDraft(String(value)); }, [value]);
  useEffect(() => {
    if (!editing) return;
    const handler = (e: BeforeUnloadEvent) => { e.preventDefault(); };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [editing]);

  if (!editing) {
    return (
      <button className="flex items-center justify-between py-1.5 group cursor-pointer w-full text-left" onClick={() => setEditing(true)}>
        <span className="text-xs text-white/40">{label}</span>
        <span className="text-xs text-white/70 group-hover:text-white flex items-center gap-1">
          {value || <span className="text-white/20 italic">empty</span>}
          <Edit3 size={10} className="opacity-0 group-hover:opacity-100 text-white/30" />
        </span>
      </button>
    );
  }

  return (
    <div className="flex items-center justify-between py-1.5 gap-2">
      <span className="text-xs text-white/40">{label}</span>
      <div className="flex gap-1">
        <input type={type || "text"} value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus
          onKeyDown={(e) => { if (e.key === "Enter") { onSave(draft); setEditing(false); } if (e.key === "Escape") setEditing(false); }}
          onBlur={() => { if (draft !== String(value)) onSave(draft); setEditing(false); }}
          className="w-32 bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white text-right outline-none focus:border-[var(--color-accent)]" />
        <button onClick={() => { onSave(draft); setEditing(false); }} className="text-[var(--color-success)]"><Save size={12} /></button>
      </div>
    </div>
  );
}

export function ProgressBar({ label, pct }: { label: string; pct: number }) {

// F3: Estimate vs actual comparison bars
export function EstimateVsActual({ estimated, actual, unit }: { estimated: number; actual: number; unit: string }) {
  if (estimated === 0 && actual === 0) return null;
  const max = Math.max(estimated, actual, 1);
  return (
    <div className="mb-2 text-[10px]">
      <div className="flex items-center gap-2 mb-1">
        <span className="w-14 text-white/30">Est</span>
        <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
          <div className="h-full bg-blue-500/60 rounded-full" style={{ width: `${(estimated / max) * 100}%` }} />
        </div>
        <span className="text-white/40 w-12 text-right">{estimated}{unit}</span>
      </div>
      <div className="flex items-center gap-2">
        <span className="w-14 text-white/30">Actual</span>
        <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
          <div className={`h-full rounded-full ${actual > estimated ? "bg-red-500/60" : "bg-green-500/60"}`} style={{ width: `${(actual / max) * 100}%` }} />
        </div>
        <span className="text-white/40 w-12 text-right">{actual}{unit}</span>
      </div>
    </div>
  );
}
  return (
    <div className="mb-2">
      <div className="flex justify-between text-[10px] text-white/30 mb-1"><span>{label}</span><span>{pct}% done</span></div>
      <div className="h-1.5 bg-white/5 rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${pct < 30 ? "bg-[var(--color-danger)]" : pct < 70 ? "bg-[var(--color-warning)]" : "bg-[var(--color-success)]"}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

export function ExportButton({ detail }: { detail: TaskDetail }) {
  const t = detail.task;

  const doExport = async (fmt: string) => {
    let content: string, ext: string;
    if (fmt === "json") {
      content = JSON.stringify(detail, null, 2); ext = "json";
    } else {
      const lines = [
        `# ${t.title}`, "",
        `**Status:** ${t.status} | **Priority:** ${t.priority} | **Project:** ${t.project || "—"}`,
        `**Estimated:** ${t.estimated} 🍅 (${t.estimated_hours}h) | **Actual:** ${t.actual} 🍅`,
        `**Remaining Points:** ${t.remaining_points} | **Due:** ${t.due_date || "—"}`,
      ];
      if (t.description) lines.push("", "## Description", t.description);
      if (detail.comments?.length) {
        lines.push("", "## Comments");
        for (const c of detail.comments) lines.push(`- **${c.username}** (${c.created_at.slice(0, 10)}): ${c.content}`);
      }
      if (detail.time_reports?.length) {
        lines.push("", "## Time Reports");
        for (const r of detail.time_reports) lines.push(`- ${r.hours}h by ${r.username} on ${r.created_at.slice(0, 10)}${r.note ? ` — ${r.note}` : ""}`);
      }
      if (detail.children?.length) {
        lines.push("", "## Subtasks");
        for (const c of detail.children) lines.push(`- [${c.task.status === "completed" ? "x" : " "}] ${c.task.title}`);
      }
      content = lines.join("\n"); ext = "md";
    }
    const filename = `${t.title.replace(/[^a-zA-Z0-9]/g, "_")}.${ext}`;
    try {
      const path = await saveDialog({ defaultPath: filename, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
      if (path) await invoke("plugin:fs|write_text_file", { path, contents: content });
    } catch {
      const blob = new Blob([content], { type: ext === "json" ? "application/json" : "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a"); a.href = url; a.download = filename; a.click();
      URL.revokeObjectURL(url);
    }
  };

  return (
    <div className="relative group">
      <button onClick={() => doExport("md")} className="w-9 h-9 flex items-center justify-center rounded-lg glass text-white/60 hover:text-white transition-all" title="Export">
        <Download size={16} />
      </button>
      <div className="absolute right-0 top-full mt-1 hidden group-hover:flex gap-1 glass p-1 z-30 rounded">
        {["md", "json"].map(f => (
          <button key={f} onClick={() => doExport(f)} className="px-2 py-1 text-[10px] text-white/60 hover:text-white hover:bg-white/5 rounded uppercase font-mono">{f}</button>
        ))}
      </div>
    </div>
  );
}

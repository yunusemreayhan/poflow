import { motion, AnimatePresence } from "framer-motion";
import { ArrowLeft, MessageSquare, Download, Clock, Plus, Trash2, Users, Edit3, Save, Paperclip } from "lucide-react";
import { useStore } from "../store/store";
import { useState, useEffect, useCallback } from "react";
import type { TaskDetail, Comment, TimeReport } from "../store/api";
import { TaskLabelPicker } from "./Labels";
import { TaskDependencies } from "./Dependencies";
import { TaskRecurrence } from "./Recurrence";
import Select from "./Select";
import { apiCall } from "../store/api";
import { computeRollup } from "../rollup";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

function formatDuration(s: number) {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

// --- Export ---

function ExportButton({ detail }: { detail: TaskDetail }) {
  const [fmt, setFmt] = useState<string | null>(null);

  const exportTree = useCallback((d: TaskDetail, depth: number): string => {
    const indent = "  ".repeat(depth);
    const totalTime = d.sessions.reduce((a, s) => a + (s.duration_s ?? 0), 0);
    const completed = d.sessions.filter((s) => s.status === "completed").length;
    let out = `${"#".repeat(Math.min(depth + 1, 6))} ${d.task.title}\n\n`;
    out += `${indent}- Owner: ${d.task.user} | Status: ${d.task.status} | Priority: ${d.task.priority}\n`;
    out += `${indent}- Pomodoros: ${d.task.actual}/${d.task.estimated} | Hours: ${d.task.estimated_hours}h est | Points remaining: ${d.task.remaining_points}\n`;
    out += `${indent}- Session time: ${formatDuration(totalTime)}\n`;
    if (d.task.description) out += `${indent}- Description: ${d.task.description}\n`;
    if (d.comments.length > 0) {
      out += `\n${indent}**Comments:**\n`;
      for (const c of d.comments) out += `${indent}- [${c.created_at.slice(0, 16).replace("T", " ")}] @${c.user}: ${c.content}\n`;
    }
    if (d.sessions.length > 0) {
      out += `\n${indent}**Sessions:** ${completed} completed, ${formatDuration(totalTime)} total\n`;
      for (const s of d.sessions) {
        const dur = s.duration_s ? formatDuration(s.duration_s) : "-";
        out += `${indent}- [${s.started_at.slice(0, 16).replace("T", " ")}] @${s.user} ${s.session_type.replace("_", " ")} — ${dur} (${s.status})\n`;
      }
    }
    out += "\n";
    for (const ch of d.children) out += exportTree(ch, depth + 1);
    return out;
  }, []);

  const doExport = useCallback(async (format: string) => {
    let content: string, ext: string;
    if (format === "json") {
      content = JSON.stringify(detail, null, 2); ext = "json";
    } else if (format === "xml") {
      const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
      const toXml = (d: TaskDetail, depth: number): string => {
        const i = "  ".repeat(depth);
        const totalTime = d.sessions.reduce((a, s) => a + (s.duration_s ?? 0), 0);
        let x = `${i}<task id="${d.task.id}" status="${esc(d.task.status)}" owner="${esc(d.task.user)}" priority="${d.task.priority}">\n`;
        x += `${i}  <title>${esc(d.task.title)}</title>\n`;
        if (d.task.description) x += `${i}  <description>${esc(d.task.description)}</description>\n`;
        x += `${i}  <pomodoros actual="${d.task.actual}" estimated="${d.task.estimated}"/>\n`;
        x += `${i}  <hours estimated="${d.task.estimated_hours}"/>\n`;
        x += `${i}  <remaining_points>${d.task.remaining_points}</remaining_points>\n`;
        x += `${i}  <time_spent_s>${totalTime}</time_spent_s>\n`;
        if (d.comments.length) {
          x += `${i}  <comments>\n`;
          for (const c of d.comments) x += `${i}    <comment date="${c.created_at}" user="${esc(c.user)}">${esc(c.content)}</comment>\n`;
          x += `${i}  </comments>\n`;
        }
        if (d.children.length) {
          x += `${i}  <children>\n`;
          for (const ch of d.children) x += toXml(ch, depth + 2);
          x += `${i}  </children>\n`;
        }
        x += `${i}</task>\n`;
        return x;
      };
      content = `<?xml version="1.0" encoding="UTF-8"?>\n${toXml(detail, 0)}`; ext = "xml";
    } else {
      content = exportTree(detail, 0); ext = "md";
    }
    const filePath = await saveDialog({ defaultPath: `${detail.task.title.replace(/[^a-zA-Z0-9]/g, "_")}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (filePath) await invoke("write_file", { path: filePath, content });
    setFmt(null);
  }, [detail, exportTree]);

  return (
    <div className="relative">
      <button onClick={() => setFmt(fmt ? null : "pick")} className="w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5 transition-all" title="Export">
        <Download size={14} />
      </button>
      <AnimatePresence>
        {fmt === "pick" && (
          <motion.div initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 0.9 }} className="absolute right-0 top-full mt-1 glass p-1 z-30 flex gap-1">
            {["md", "json", "xml"].map((f) => (
              <button key={f} onClick={() => doExport(f)} className="px-3 py-1.5 text-xs text-white/70 hover:text-white hover:bg-white/5 rounded-lg transition-all uppercase font-mono">{f}</button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// --- Comments ---

import CommentSection from "./CommentSection";

// --- Editable field ---

function EditField({ label, value, type, onSave }: { label: string; value: string | number; type?: string; onSave: (v: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(String(value));

  useEffect(() => { setDraft(String(value)); }, [value]);

  // Warn on page unload if editing
  useEffect(() => {
    if (!editing) return;
    const handler = (e: BeforeUnloadEvent) => { e.preventDefault(); };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [editing]);

  if (!editing) {
    return (
      <div className="flex items-center justify-between py-1.5 group cursor-pointer" tabIndex={0} role="button"
        onClick={() => setEditing(true)} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setEditing(true); } }}>
        <span className="text-xs text-white/40">{label}</span>
        <span className="text-xs text-white/70 group-hover:text-white flex items-center gap-1">
          {value || <span className="text-white/20 italic">empty</span>}
          <Edit3 size={10} className="opacity-0 group-hover:opacity-100 text-white/30" />
        </span>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between py-1.5 gap-2">
      <span className="text-xs text-white/40">{label}</span>
      <div className="flex gap-1">
        <input type={type || "text"} value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus
          onKeyDown={(e) => { if (e.key === "Enter") { onSave(draft); setEditing(false); } if (e.key === "Escape") setEditing(false); }}
          className="w-32 bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white text-right outline-none focus:border-[var(--color-accent)]" />
        <button onClick={() => { onSave(draft); setEditing(false); }} className="text-[var(--color-success)]"><Save size={12} /></button>
      </div>
    </div>
  );
}

function ProgressBar({ label, pct }: { label: string; pct: number }) {
  return (
    <div className="mb-2">
      <div className="flex justify-between text-[10px] text-white/30 mb-1">
        <span>{label}</span>
        <span>{pct}% done</span>
      </div>
      <div className="h-1.5 bg-white/5 rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${pct < 30 ? "bg-[var(--color-danger)]" : pct < 70 ? "bg-[var(--color-warning)]" : "bg-[var(--color-success)]"}`}
          style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

// --- Detail Node ---

function DetailNode({ detail, depth, onRefresh, hoursMap }: { detail: TaskDetail; depth: number; onRefresh: () => void; hoursMap: Map<number, number> }) {
  const { updateTask, username: currentUser, role, taskSprints } = useStore();
  const [showComments, setShowComments] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [showTimeReports, setShowTimeReports] = useState(false);
  const [showVotes, setShowVotes] = useState(false);
  const [taskVotes, setTaskVotes] = useState<{ username: string; value: number | null; room_id: number }[]>([]);
  const [timeReports, setTimeReports] = useState<TimeReport[]>([]);
  const [assignees, setAssignees] = useState<string[]>([]);
  const [allUsers, setAllUsers] = useState<string[]>([]);
  const [burnUsers, setBurnUsers] = useState<string[]>([]);
  const [reportHours, setReportHours] = useState("");
  const [reportDesc, setReportDesc] = useState("");

  const t = detail.task;
  const completedSessions = detail.sessions.filter((s) => s.status === "completed").length;
  const isOwner = t.user === currentUser || role === "root";
  const hasChildren = detail.children.length > 0;

  const rollup = computeRollup(detail, hoursMap);

  useEffect(() => {
    Promise.all([
      apiCall<TimeReport[]>("GET", `/api/tasks/${t.id}/time`).catch(() => [] as TimeReport[]),
      apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).catch(() => [] as string[]),
      apiCall<string[]>("GET", "/api/users").catch(() => [] as string[]),
      apiCall<string[]>("GET", `/api/tasks/${t.id}/burn-users`).catch(() => [] as string[]),
    ]).then(([tr, a, u, bu]) => { setTimeReports(tr); setAssignees(a); setAllUsers(u); setBurnUsers(bu); });
  }, [t.id]);

  const saveField = (field: string, value: string) => {
    const v = ["priority", "estimated", "sort_order"].includes(field) ? parseInt(value) || 0
      : ["estimated_hours", "remaining_points"].includes(field) ? parseFloat(value) || 0
      : value;
    updateTask(t.id, { [field]: v || null });
    onRefresh();
  };

  const handleTimeReport = async () => {
    const h = parseFloat(reportHours);
    if (!h || h <= 0) return;
    await apiCall("POST", `/api/tasks/${t.id}/time`, { hours: h, description: reportDesc || null });
    setReportHours(""); setReportDesc("");
    apiCall<TimeReport[]>("GET", `/api/tasks/${t.id}/time`).then(setTimeReports);
    apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).then(setAssignees);
  };

  const addAssignee = async (username: string) => {
    if (!username) return;
    await apiCall("POST", `/api/tasks/${t.id}/assignees`, { username });
    apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).then(setAssignees);
  };

  const removeAssignee = async (u: string) => {
    await apiCall("DELETE", `/api/tasks/${t.id}/assignees/${u}`);
    apiCall<string[]>("GET", `/api/tasks/${t.id}/assignees`).then(setAssignees);
  };

  return (
    <div style={{ marginLeft: depth > 0 ? 24 : 0 }}>
      <div className={`${depth === 0 ? "glass p-5" : "px-4 py-3 border-l border-white/10"} mb-3`}>
        {/* Header */}
        <div className="flex items-center gap-3 mb-3">
          <div className={`text-sm flex-1 ${depth === 0 ? "font-semibold" : ""} ${t.status === "completed" ? "line-through text-white/30" : "text-white/90"}`}>
            {t.title}
          </div>
          <span className={`text-xs px-2 py-0.5 rounded ${t.status === "completed" ? "bg-[var(--color-success)]/20 text-[var(--color-success)]" : t.status === "active" ? "bg-[var(--color-work)]/20 text-[var(--color-work)]" : "bg-white/5 text-white/30"}`}>
            {t.status}
          </span>
          <span className="text-xs text-white/30">👤 {t.user}</span>
        </div>

        {/* Description */}
        {t.description && <p className="text-xs text-white/50 mb-3 whitespace-pre-wrap">{t.description}</p>}

        {/* Progress bars */}
        {rollup.progressHours !== null && <ProgressBar label="Hours" pct={rollup.progressHours} />}
        {rollup.progressPoints !== null && <ProgressBar label="Points" pct={rollup.progressPoints} />}

        {/* Estimate vs Actual comparison */}
        {(rollup.totalEstHours > 0 || rollup.totalEstPoints > 0) && (
          <div className="flex gap-3 mb-3 text-xs">
            {rollup.totalEstHours > 0 && (
              <div className="flex-1 bg-white/5 rounded-lg p-2">
                <div className="text-white/30 mb-1">Hours</div>
                <div className="flex items-end gap-1 h-8">
                  <div className="flex-1 bg-blue-500/30 rounded" style={{ height: `${Math.min(100, (rollup.totalEstHours / Math.max(rollup.totalEstHours, rollup.totalSpentHours)) * 100)}%` }} title={`Est: ${rollup.totalEstHours}h`} />
                  <div className={`flex-1 rounded ${rollup.totalSpentHours > rollup.totalEstHours ? "bg-red-500/40" : "bg-green-500/30"}`}
                    style={{ height: `${Math.min(100, (rollup.totalSpentHours / Math.max(rollup.totalEstHours, rollup.totalSpentHours)) * 100)}%` }} title={`Actual: ${rollup.totalSpentHours.toFixed(1)}h`} />
                </div>
                <div className="flex justify-between text-[10px] text-white/20 mt-1">
                  <span>Est {rollup.totalEstHours}h</span>
                  <span>Act {rollup.totalSpentHours.toFixed(1)}h</span>
                </div>
              </div>
            )}
            {rollup.totalEstPoints > 0 && (
              <div className="flex-1 bg-white/5 rounded-lg p-2">
                <div className="text-white/30 mb-1">Points</div>
                <div className="flex items-end gap-1 h-8">
                  <div className="flex-1 bg-purple-500/30 rounded" style={{ height: "100%" }} title={`Est: ${rollup.totalEstPoints}`} />
                  <div className={`flex-1 rounded ${rollup.totalRemPoints === 0 ? "bg-green-500/30" : "bg-yellow-500/30"}`}
                    style={{ height: `${Math.min(100, ((rollup.totalEstPoints - rollup.totalRemPoints) / rollup.totalEstPoints) * 100)}%` }} title={`Done: ${rollup.totalEstPoints - rollup.totalRemPoints}`} />
                </div>
                <div className="flex justify-between text-[10px] text-white/20 mt-1">
                  <span>Est {rollup.totalEstPoints}</span>
                  <span>Rem {rollup.totalRemPoints}</span>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Rollup stats — own vs children vs total */}
        {hasChildren && (
          <div className="border border-white/5 rounded-lg p-3 mb-3 text-xs">
            <div className="grid grid-cols-4 gap-1 text-white/30 mb-1 font-semibold">
              <span></span><span className="text-right">Own</span><span className="text-right">Children</span><span className="text-right">Total</span>
            </div>
            <div className="grid grid-cols-4 gap-1 text-white/40">
              <span>Est. Hours</span>
              <span className="text-right text-white/60">{rollup.ownEstHours}</span>
              <span className="text-right">{rollup.childEstHours.toFixed(1)}</span>
              <span className="text-right text-white/70 font-semibold">{rollup.totalEstHours.toFixed(1)}</span>
            </div>
            <div className="grid grid-cols-4 gap-1 text-white/40">
              <span>Spent Hours</span>
              <span className="text-right text-white/60">{rollup.ownSpentHours.toFixed(1)}</span>
              <span className="text-right">{rollup.childSpentHours.toFixed(1)}</span>
              <span className="text-right text-white/70 font-semibold">{rollup.totalSpentHours.toFixed(1)}</span>
            </div>
            <div className="grid grid-cols-4 gap-1 text-white/40">
              <span>Est. Points</span>
              <span className="text-right text-white/60">{rollup.ownEstPoints}</span>
              <span className="text-right">{rollup.childEstPoints}</span>
              <span className="text-right text-white/70 font-semibold">{rollup.totalEstPoints}</span>
            </div>
            <div className="grid grid-cols-4 gap-1 text-white/40">
              <span>Rem. Points</span>
              <span className="text-right text-white/60">{rollup.ownRemPoints}</span>
              <span className="text-right">{rollup.childRemPoints}</span>
              <span className="text-right text-white/70 font-semibold">{rollup.totalRemPoints}</span>
            </div>
            <div className="grid grid-cols-4 gap-1 text-white/40">
              <span>Session Time</span>
              <span className="text-right text-white/60">{formatDuration(rollup.ownSessionSecs)}</span>
              <span className="text-right">{formatDuration(rollup.childSessionSecs)}</span>
              <span className="text-right text-white/70 font-semibold">{formatDuration(rollup.totalSessionSecs)}</span>
            </div>
          </div>
        )}

        {/* Editable fields — owner/root only */}
        {isOwner && (
          <div className="border-t border-white/5 pt-2 mb-2">
            <EditField label="Title" value={t.title} onSave={(v) => saveField("title", v)} />
            <EditField label="Description" value={t.description || ""} onSave={(v) => saveField("description", v)} />
            <EditField label="Status" value={t.status} onSave={(v) => saveField("status", v)} />
            <EditField label="Priority (1-5)" value={t.priority} type="number" onSave={(v) => saveField("priority", v)} />
            <EditField label="Est. Pomodoros / Points" value={t.estimated} type="number" onSave={(v) => saveField("estimated", v)} />
            <EditField label="Est. Hours" value={t.estimated_hours} type="number" onSave={(v) => saveField("estimated_hours", v)} />
            <EditField label="Remaining Points" value={t.remaining_points} type="number" onSave={(v) => saveField("remaining_points", v)} />
            <EditField label="Project" value={t.project || ""} onSave={(v) => saveField("project", v)} />
            <EditField label="Due Date" value={t.due_date || ""} type="date" onSave={(v) => saveField("due_date", v)} />
          </div>
        )}

        {/* Stats row */}
        <div className="flex flex-wrap gap-3 text-xs text-white/40 mb-3">
          <span>{t.actual}/{t.estimated} 🍅</span>
          {taskSprints.filter(ts => ts.task_id === t.id).map(ts => (
            <span key={ts.sprint_id} className={`px-1.5 py-0.5 rounded ${
              ts.sprint_status === "active" ? "bg-green-500/20 text-green-400" : "bg-green-500/10 text-green-400/40"
            }`}>🏃 {ts.sprint_name}</span>
          ))}
          {rollup.ownSessionSecs > 0 && <span><Clock size={10} className="inline" /> {formatDuration(rollup.ownSessionSecs)}</span>}
          {rollup.ownSpentHours > 0 && <span>📝 {rollup.ownSpentHours.toFixed(1)}h reported{t.estimated_hours > 0 ? ` / ${t.estimated_hours}h est` : ""}</span>}
          {t.remaining_points > 0 && <span>🎯 {t.remaining_points} pts remaining</span>}
          {!hasChildren && rollup.progressHours !== null && <span className={`${rollup.progressHours > 70 ? "text-[var(--color-success)]" : rollup.progressHours > 30 ? "text-[var(--color-warning)]" : "text-[var(--color-danger)]"}`}>{rollup.progressHours}% done</span>}
        </div>

        {/* Labels */}
        <div className="mb-3">
          <TaskLabelPicker taskId={t.id} />
        </div>

        {/* Dependencies */}
        <div className="mb-3">
          <TaskDependencies taskId={t.id} allTasks={tasks} />
        </div>

        {/* Recurrence */}
        <div className="mb-3">
          <TaskRecurrence taskId={t.id} />
        </div>

        {/* Attachments */}
        <TaskAttachments taskId={t.id} />

        {/* Assignees */}
        <div className="flex items-center gap-2 flex-wrap mb-3">
          <Users size={12} className="text-white/30" />
          {assignees.map((a) => {
            const hasBurns = burnUsers.includes(a);
            return (
              <span key={a} className="flex items-center gap-1 text-xs bg-white/5 px-2 py-0.5 rounded group" title={hasBurns ? "Has logged time — cannot remove" : ""}>
                {a}
                {hasBurns
                  ? <Clock size={10} className="text-white/15" />
                  : <button onClick={() => removeAssignee(a)} className="opacity-0 group-hover:opacity-100 text-white/30 hover:text-[var(--color-danger)]"><Trash2 size={10} /></button>
                }
              </span>
            );
          })}
          <div className="flex gap-1">
            {allUsers.filter(u => !assignees.includes(u)).length > 0 && (
              <Select value="" onChange={v => addAssignee(v)} className="w-28 text-xs" placeholder="+ assign"
                options={allUsers.filter(u => !assignees.includes(u)).map(u => ({value:u,label:u}))} />
            )}
          </div>
        </div>

        {/* Time Reports */}
        <button onClick={() => setShowTimeReports(!showTimeReports)}
          className="flex items-center gap-1 text-xs text-white/40 hover:text-white/70 transition-colors mb-2">
          <Clock size={12} /> {showTimeReports ? "Hide" : "Show"} burns ({timeReports.length})
        </button>
        <AnimatePresence>
          {showTimeReports && (
            <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden mb-2">
              <div className="space-y-1 mb-2 max-h-32 overflow-y-auto">
                {timeReports.map((r) => (
                  <div key={r.id} className={`flex items-center gap-2 text-xs text-white/40 ${r.cancelled ? "line-through opacity-30" : ""}`}>
                    {r.hours > 0 && <span className="text-white/60">{r.hours}h</span>}
                    {r.points > 0 && <span className="text-[var(--color-accent)]">{r.points}pt</span>}
                    <span>@{r.username}</span>
                    <span className="text-white/20 text-[10px]">{r.source}</span>
                    {r.note && <span className="text-white/30 truncate flex-1">{r.note}</span>}
                    <span className="text-white/20">{r.created_at.slice(0, 16).replace("T", " ")}</span>
                  </div>
                ))}
              </div>
              <div className="flex gap-2">
                <input type="number" step="0.25" min="0.25" value={reportHours} onChange={(e) => setReportHours(e.target.value)}
                  placeholder="Hours" className="w-16 bg-white/5 border border-white/10 rounded text-xs text-white placeholder-white/20 px-2 py-1 outline-none" />
                <input value={reportDesc} onChange={(e) => setReportDesc(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleTimeReport()}
                  placeholder="What did you do?" className="flex-1 bg-white/5 border border-white/10 rounded text-xs text-white placeholder-white/20 px-2 py-1 outline-none" />
                <button onClick={handleTimeReport} className="px-2 py-1 rounded bg-[var(--color-work)] text-white text-xs">Log</button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Sessions */}
        {detail.sessions.length > 0 && (
          <>
            <button onClick={() => setShowSessions(!showSessions)}
              className="flex items-center gap-1 text-xs text-white/40 hover:text-white/70 transition-colors mb-2">
              🍅 {showSessions ? "Hide" : "Show"} sessions ({completedSessions} completed, {formatDuration(rollup.ownSessionSecs)})
            </button>
            <AnimatePresence>
              {showSessions && (
                <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden mb-2">
                  <div className="space-y-1 max-h-32 overflow-y-auto">
                    {detail.sessions.map((s) => (
                      <div key={s.id} className="flex items-center gap-2 text-xs text-white/40">
                        <div className={`w-1.5 h-1.5 rounded-full ${s.status === "completed" ? "bg-[var(--color-success)]" : "bg-[var(--color-warning)]"}`} />
                        <span>@{s.user}</span>
                        <span>{s.session_type.replace("_", " ")}</span>
                        <span>{s.duration_s ? formatDuration(s.duration_s) : "-"}</span>
                        <span className="text-white/20">{s.started_at.slice(0, 16).replace("T", " ")}</span>
                      </div>
                    ))}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </>
        )}

        {/* Estimation Votes */}
        <button onClick={() => {
            setShowVotes(!showVotes);
            if (!showVotes && taskVotes.length === 0) {
              apiCall<{ username: string; value: number | null; room_id: number }[]>("GET", `/api/tasks/${t.id}/votes`).then(setTaskVotes).catch(() => {});
            }
          }}
          className="flex items-center gap-1 text-xs text-white/40 hover:text-white/70 transition-colors mb-2">
          🎯 {showVotes ? "Hide" : "Show"} estimation votes
        </button>
        <AnimatePresence>
          {showVotes && (
            <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden mb-2">
              {taskVotes.length === 0 ? (
                <div className="text-xs text-white/20 px-2 py-1">No votes recorded for this task.</div>
              ) : (
                <div className="flex flex-wrap gap-2">
                  {taskVotes.map((v, i) => (
                    <div key={i} className="flex items-center gap-1 text-xs bg-white/5 px-2 py-1 rounded">
                      <span className="text-white/40">{v.username}:</span>
                      <span className="text-[var(--color-accent)] font-semibold">{v.value ?? "?"}</span>
                    </div>
                  ))}
                  <div className="text-xs text-white/30 px-2 py-1">
                    avg: {(taskVotes.filter(v => v.value != null).reduce((a, v) => a + v.value!, 0) / taskVotes.filter(v => v.value != null).length).toFixed(1)}
                  </div>
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Comments */}
        <button onClick={() => setShowComments(!showComments)}
          className="flex items-center gap-1 text-xs text-white/40 hover:text-white/70 transition-colors mb-2">
          <MessageSquare size={12} /> {showComments ? "Hide" : "Show"} comments ({detail.comments.length})
        </button>
        {showComments && <CommentSection taskId={t.id} />}
      </div>

      {/* Activity feed */}
      {depth === 0 && <TaskActivityFeed taskId={t.id} />}

      {detail.children.map((ch) => (
        <DetailNode key={ch.task.id} detail={ch} depth={depth + 1} onRefresh={onRefresh} hoursMap={hoursMap} />
      ))}
    </div>
  );
}

// --- Main View ---

export default function TaskDetailView({ taskId, onBack, onNavigate }: { taskId: number; onBack: () => void; onNavigate?: (id: number) => void }) {
  const { getTaskDetail, tasks } = useStore();
  const [detail, setDetail] = useState<TaskDetail | null>(null);
  const [hoursMap, setHoursMap] = useState<Map<number, number>>(new Map());

  const loadHoursMap = useCallback(async (_d: TaskDetail) => {
    // Use batch endpoint instead of per-node API calls
    const map = new Map<number, number>();
    try {
      const totals = await apiCall<{ task_id: number; total_hours: number }[]>("GET", "/api/burn-totals");
      for (const bt of totals) map.set(bt.task_id, bt.total_hours);
    } catch {
      // Fallback: no data
    }
    setHoursMap(map);
  }, []);

  const load = useCallback(() => {
    getTaskDetail(taskId).then((d) => { setDetail(d); loadHoursMap(d); });
  }, [taskId, getTaskDetail, loadHoursMap]);

  useEffect(load, [load]);

  if (!detail) return <div className="p-8 text-white/40 text-sm">Loading...</div>;

  // Build breadcrumb chain from task ancestors
  const tasks = useStore.getState().tasks;
  const breadcrumbs: { id: number; title: string }[] = [];
  let current = detail.task;
  while (current.parent_id) {
    const parent = tasks.find(t => t.id === current.parent_id);
    if (!parent) break;
    breadcrumbs.unshift({ id: parent.id, title: parent.title });
    current = parent as typeof current;
  }

  return (
    <div className="flex flex-col gap-4 p-8 h-full overflow-y-auto">
      {/* Breadcrumbs */}
      {breadcrumbs.length > 0 && onNavigate && (
        <nav aria-label="Breadcrumb" className="flex items-center gap-1 text-xs text-white/30 overflow-x-auto">
          {breadcrumbs.map((b, i) => (
            <span key={b.id} className="flex items-center gap-1 shrink-0">
              {i > 0 && <span>›</span>}
              <button onClick={() => onNavigate(b.id)} className="hover:text-white/60 truncate max-w-[150px]">{b.title}</button>
            </span>
          ))}
          <span>›</span>
          <span className="text-white/50 truncate">{detail.task.title}</span>
        </nav>
      )}
      <div className="flex items-center gap-3">
        <button onClick={onBack} className="w-9 h-9 flex items-center justify-center rounded-lg glass text-white/60 hover:text-white transition-all">
          <ArrowLeft size={18} />
        </button>
        {detail.task.parent_id && onNavigate && (
          <button onClick={() => onNavigate(detail.task.parent_id!)}
            className="text-xs text-white/40 hover:text-white/70 transition-colors">
            ↑ Parent
          </button>
        )}
        <h2 className="text-lg font-semibold text-white flex-1">{detail.task.title}</h2>
        <ExportButton detail={detail} />
      </div>
      <DetailNode detail={detail} depth={0} onRefresh={load} hoursMap={hoursMap} />
    </div>
  );
}

interface AuditEntry { id: number; user_id: number; username: string; action: string; entity_type: string; entity_id: number | null; detail: string | null; created_at: string }

function TaskActivityFeed({ taskId }: { taskId: number }) {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (show) apiCall<AuditEntry[]>("GET", `/api/audit?entity_type=task&entity_id=${taskId}&per_page=50`).then(e => setEntries(e || [])).catch(() => {});
  }, [show, taskId]);

  const icon = (a: string) => a === "create" ? "🆕" : a === "update" ? "✏️" : a === "delete" ? "🗑" : "📋";

  return (
    <div className="mt-2">
      <button onClick={() => setShow(!show)} className="text-xs text-white/30 hover:text-white/50 flex items-center gap-1">
        📋 {show ? "Hide" : "Show"} activity
      </button>
      {show && (
        <div className="mt-1 space-y-1 max-h-48 overflow-y-auto">
          {entries.map(e => (
            <div key={e.id} className="flex items-center gap-2 text-[11px] text-white/40 py-0.5">
              <span>{icon(e.action)}</span>
              <span className="text-white/60">{e.username}</span>
              <span>{e.action}</span>
              {e.detail && <span className="truncate text-white/25 max-w-40" title={e.detail}>{e.detail}</span>}
              <span className="ml-auto text-white/20 shrink-0">{e.created_at.slice(5, 16)}</span>
            </div>
          ))}
          {entries.length === 0 && <div className="text-xs text-white/20 py-2">No activity recorded</div>}
        </div>
      )}
    </div>
  );
}

export { ExportButton };

interface Attachment {
  id: number;
  task_id: number;
  filename: string;
  mime_type: string;
  size_bytes: number;
  created_at: string;
}

function TaskAttachments({ taskId }: { taskId: number }) {
  const [atts, setAtts] = useState<Attachment[]>([]);
  const [uploading, setUploading] = useState(false);

  const load = () => apiCall<Attachment[]>("GET", `/api/tasks/${taskId}/attachments`).then(setAtts).catch(() => {});
  useEffect(load, [taskId]);

  const upload = async (file: File) => {
    setUploading(true);
    try {
      const { serverUrl, token } = useStore.getState();
      const buf = await file.arrayBuffer();
      const resp = await fetch(`${serverUrl}/api/tasks/${taskId}/attachments`, {
        method: "POST",
        headers: {
          "content-type": file.type || "application/octet-stream",
          "x-filename": file.name,
          "x-requested-with": "PomodoroGUI",
          "authorization": `Bearer ${token}`,
        },
        body: buf,
      });
      if (resp.ok) load();
    } catch {}
    setUploading(false);
  };

  const del = async (id: number) => {
    await apiCall("DELETE", `/api/attachments/${id}`);
    setAtts(a => a.filter(x => x.id !== id));
  };

  const download = async (id: number, filename: string) => {
    const { serverUrl, token } = useStore.getState();
    const resp = await fetch(`${serverUrl}/api/attachments/${id}/download`, {
      headers: { "authorization": `Bearer ${token}` },
    });
    if (resp.ok) {
      const blob = await resp.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url; a.download = filename; a.click();
      URL.revokeObjectURL(url);
    }
  };

  const fmt = (bytes: number) => bytes < 1024 ? `${bytes}B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(1)}KB` : `${(bytes / 1048576).toFixed(1)}MB`;

  return (
    <div className="mb-3">
      <div className="flex items-center gap-2 mb-1">
        <Paperclip size={12} className="text-white/30" />
        <span className="text-xs text-[var(--color-dim)]">Attachments ({atts.length})</span>
        <label className="text-xs text-[var(--color-accent)] cursor-pointer hover:underline">
          {uploading ? "Uploading..." : "+ Add"}
          <input type="file" className="hidden" onChange={e => { if (e.target.files?.[0]) upload(e.target.files[0]); e.target.value = ""; }} />
        </label>
      </div>
      {atts.map(a => (
        <div key={a.id} className="flex items-center gap-2 text-xs text-white/60 py-0.5 group">
          <span className="truncate flex-1">{a.filename}</span>
          <span className="text-white/20">{fmt(a.size_bytes)}</span>
          <button onClick={() => download(a.id, a.filename)}
            className="text-[var(--color-accent)] hover:underline">↓</button>
          <button onClick={() => del(a.id)} className="text-white/20 hover:text-[var(--color-danger)] opacity-0 group-hover:opacity-100">✕</button>
        </div>
      ))}
    </div>
  );
}

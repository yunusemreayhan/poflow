import { motion, AnimatePresence } from "framer-motion";
import { ArrowLeft, MessageSquare, Clock, Plus, Trash2, Users } from "lucide-react";
import { useStore } from "../store/store";
import { useState, useEffect, useCallback } from "react";
import type { TaskDetail, Comment, TimeReport } from "../store/api";
import { TaskLabelPicker } from "./Labels";
import { TaskDependencies } from "./Dependencies";
import { TaskRecurrence } from "./Recurrence";
import { TaskActivityFeed, TaskAttachments, TaskTimeChart } from "./TaskDetailParts";
import Select from "./Select";
import { apiCall } from "../store/api";
import { computeRollup } from "../rollup";
import CommentSection from "./CommentSection";
import { formatDuration, EditField, ProgressBar, ExportButton, EstimateVsActual } from "./TaskDetailHelpers";

// F27: Render description with interactive checklists (- [ ] / - [x])
function DescriptionWithChecklists({ taskId, description }: { taskId: number; description: string }) {
  const { updateTask } = useStore();
  const hasChecklist = /^- \[[ x]\]/m.test(description);
  if (!hasChecklist) return <p className="text-xs text-white/50 mb-3 whitespace-pre-wrap">{description}</p>;

  const lines = description.split("\n");
  const total = lines.filter(l => /^- \[[ x]\]/.test(l)).length;
  const checked = lines.filter(l => /^- \[x\]/i.test(l)).length;

  const toggle = (idx: number) => {
    const updated = lines.map((l, i) => {
      if (i !== idx) return l;
      if (/^- \[ \]/.test(l)) return l.replace("- [ ]", "- [x]");
      if (/^- \[x\]/i.test(l)) return l.replace(/^- \[x\]/i, "- [ ]");
      return l;
    }).join("\n");
    updateTask(taskId, { description: updated });
  };

  return (
    <div className="text-xs text-white/50 mb-3 space-y-0.5">
      {total > 0 && (
        <div className="flex items-center gap-2 mb-1">
          <div className="flex-1 h-1 bg-white/5 rounded-full overflow-hidden">
            <div className="h-full bg-green-500 rounded-full transition-all" style={{ width: `${total > 0 ? (checked / total) * 100 : 0}%` }} />
          </div>
          <span className="text-[10px] text-white/20">{checked}/{total}</span>
        </div>
      )}
      {lines.map((line, i) => {
        const checkMatch = /^- \[([ x])\]\s*(.*)/.exec(line);
        if (checkMatch) {
          const done = checkMatch[1] === "x";
          return (
            <label key={i} className="flex items-center gap-2 cursor-pointer hover:text-white/70">
              <input type="checkbox" checked={done} onChange={() => toggle(i)} className="accent-[var(--color-accent)]" />
              <span className={done ? "line-through text-white/30" : ""}>{checkMatch[2]}</span>
            </label>
          );
        }
        return <div key={i} className="whitespace-pre-wrap">{line}</div>;
      })}
    </div>
  );
}

function DetailNode({ detail, depth, onRefresh, hoursMap }: { detail: TaskDetail; depth: number; onRefresh: () => void; hoursMap: Map<number, number> }) {
  const { updateTask, username: currentUser, role, taskSprints } = useStore();
  const [showComments, setShowComments] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [showTimeReports, setShowTimeReports] = useState(false);
  const [showVotes, setShowVotes] = useState(false);
  const [deps, setDeps] = useState<number[]>([]);
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
      allUsers.length ? Promise.resolve(allUsers) : apiCall<string[]>("GET", "/api/users").catch(() => [] as string[]),
      apiCall<string[]>("GET", `/api/tasks/${t.id}/burn-users`).catch(() => [] as string[]),
    ]).then(([tr, a, u, bu]) => { setTimeReports(tr); setAssignees(a); setAllUsers(u); setBurnUsers(bu); });
    apiCall<number[]>("GET", `/api/tasks/${t.id}/dependencies`).then(d => d && setDeps(d)).catch(() => {});
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

        {/* Description with F27 checklist support */}
        {t.description && <DescriptionWithChecklists taskId={t.id} description={t.description} />}

        {/* Progress bars */}
        {rollup.progressHours !== null && <ProgressBar label="Hours" pct={rollup.progressHours} />}
        {rollup.progressPoints !== null && <ProgressBar label="Points" pct={rollup.progressPoints} />}
        <EstimateVsActual estimated={t.estimated} actual={t.actual} unit="🍅" />
        <EstimateVsActual estimated={t.estimated_hours} actual={rollup.totalHours} unit="h" />

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
            <div className="flex items-center justify-between py-1.5">
              <span className="text-xs text-white/40">Status</span>
              <select value={t.status} onChange={e => saveField("status", e.target.value)}
                className="bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white outline-none">
                {["backlog","active","in_progress","completed","done","estimated","archived"].map(s => (
                  <option key={s} value={s}>{s}</option>
                ))}
              </select>
            </div>
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

        {/* BL6: Sprint history */}
        {(() => {
          const sprintInfos = useStore.getState().taskSprintsMap.get(t.id);
          return sprintInfos && sprintInfos.length > 0 ? (
            <div className="mb-3">
              <span className="text-xs text-[var(--color-dim)]">Sprints</span>
              <div className="flex flex-wrap gap-1 mt-0.5">
                {sprintInfos.map(si => (
                  <span key={si.sprint_id} className="text-[10px] px-1.5 py-0.5 rounded bg-white/5 text-white/50">
                    {si.sprint_name || `#${si.sprint_id}`}
                  </span>
                ))}
              </div>
            </div>
          ) : null;
        })()}

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

        {/* U7: Dependencies */}
        {deps.length > 0 && (
          <div className="flex items-center gap-2 flex-wrap mb-3">
            <span className="text-[10px] text-white/30">Blocked by:</span>
            {deps.map(depId => {
              const depTask = useStore.getState().tasks.find(t2 => t2.id === depId);
              return <span key={depId} className="text-xs bg-white/5 px-2 py-0.5 rounded text-white/60">{depTask ? depTask.title : `#${depId}`}</span>;
            })}
          </div>
        )}

        {/* Time Reports */}
        <button onClick={() => setShowTimeReports(!showTimeReports)} aria-expanded={showTimeReports}
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
            <button onClick={() => setShowSessions(!showSessions)} aria-expanded={showSessions}
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
        <button onClick={() => setShowComments(!showComments)} aria-expanded={showComments}
          className="flex items-center gap-1 text-xs text-white/40 hover:text-white/70 transition-colors mb-2">
          <MessageSquare size={12} /> {showComments ? "Hide" : "Show"} comments ({detail.comments.length})
        </button>
        {showComments && <CommentSection taskId={t.id} />}
      </div>

      {/* Activity feed */}
      {depth === 0 && <TaskTimeChart taskId={t.id} />}
      {depth === 0 && <TaskActivityFeed taskId={t.id} />}

      {detail.children.map((ch) => (
        <DetailNode key={ch.task.id} detail={ch} depth={depth + 1} onRefresh={onRefresh} hoursMap={hoursMap} />
      ))}
    </div>
  );
}

// --- Main View ---

export default function TaskDetailView({ taskId, onBack, onNavigate }: { taskId: number; onBack: () => void; onNavigate?: (id: number) => void }) {
  const { getTaskDetail } = useStore();
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

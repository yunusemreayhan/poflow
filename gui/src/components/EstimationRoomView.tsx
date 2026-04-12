import { useState, useEffect, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowLeft, Eye, Check, Crown, X, Edit3 } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import { useRoomWebSocket } from "../hooks/useRoomWebSocket";
import type { RoomState } from "../store/api";
import TaskList from "./TaskList";

const POINT_CARDS = [0, 0.5, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
const HOUR_CARDS = [0, 0.5, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40];
const TSHIRT_CARDS = [0, 1, 2, 3, 5, 8]; // XS=1, S=2, M=3, L=5, XL=8
const TSHIRT_LABELS: Record<number, string> = { 0: "?", 1: "XS", 2: "S", 3: "M", 5: "L", 8: "XL" };
const CARD_PRESETS: Record<string, number[]> = { points: POINT_CARDS, hours: HOUR_CARDS, mandays: HOUR_CARDS, tshirt: TSHIRT_CARDS };

export default function EstimationRoomView({ roomId, onBack }: { roomId: number; onBack: () => void }) {
  const { username } = useStore();
  const allTasks = useStore(s => s.tasks);
  const [state, setState] = useState<RoomState | null>(null);
  const [selectedCard, setSelectedCard] = useState<number | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);
  const [tab, setTab] = useState<"board" | "tasks" | "members" | "history">("board");
  const [customAccept, setCustomAccept] = useState("");
  const [editingTask, setEditingTask] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const mountedRef = useRef(true);
  const stateRef = useRef(state);
  stateRef.current = state;
  useEffect(() => () => { mountedRef.current = false; }, []);
  const [editDesc, setEditDesc] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const load = useCallback(() => {
    apiCall<RoomState>("GET", `/api/rooms/${roomId}`).then(setState).catch(() => {});
  }, [roomId]);

  // F5: Use WebSocket for real-time room state (auto-reconnect)
  useEffect(() => { load(); }, [load]);
  useRoomWebSocket(roomId, setState);

  // Reset card selection when task changes
  // F6: Restore selected card from vote state (value visible after reveal, otherwise keep local)
  const myVoteValue = state?.votes?.find(v => v.username === username)?.value;
  const currentTaskId = state?.room.current_task_id;
  useEffect(() => {
    if (myVoteValue != null) setSelectedCard(myVoteValue);
    else setSelectedCard(null);
  }, [currentTaskId, myVoteValue]);

  // F9: Discussion timer — tracks time spent on current task
  // BL17: Configurable discussion time limit (default 2 min)
  const [discussionLimit] = useState(() => Number(localStorage.getItem("discussion_limit_s") || 120));
  const [discussionStart, setDiscussionStart] = useState<number | null>(null);
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (state?.room.status === "voting" && state?.room.current_task_id) {
      setDiscussionStart(prev => prev ?? Date.now());
    } else { setDiscussionStart(null); setElapsed(0); }
  }, [state?.room.status, state?.room.current_task_id]);
  useEffect(() => {
    if (!discussionStart) return;
    const id = setInterval(() => setElapsed(Math.floor((Date.now() - discussionStart) / 1000)), 1000);
    return () => clearInterval(id);
  }, [discussionStart]);

  if (!state) return <div className="p-8 text-white/40">Loading...</div>;

  const { room, members, current_task, votes, vote_history } = state;
  const isAdmin = members.find(m => m.username === username)?.role === "admin";
  const cards = CARD_PRESETS[room.estimation_unit] || POINT_CARDS;
  const myVote = votes.find(v => v.username === username);
  const allVoted = votes.length > 0 && votes.every(v => v.voted);
  const notVoted = votes.filter(v => !v.voted).map(v => v.username);

  const startVoting = async (taskId: number) => {
    await apiCall("POST", `/api/rooms/${roomId}/start-voting`, { task_id: taskId });
    load();
  };

  const vote = async (value: number) => {
    setSelectedCard(value);
    setSubmitting(true);
    try { await apiCall("POST", `/api/rooms/${roomId}/vote`, { value }); load(); }
    finally { setSubmitting(false); }
  };

  const cancelRevealRef = useRef(false);

  const doReveal = async () => {
    cancelRevealRef.current = false;
    setCountdown(3);
    for (let i = 3; i >= 1; i--) {
      await new Promise(r => setTimeout(r, 1000));
      if (!mountedRef.current || cancelRevealRef.current) { setCountdown(null); return; }
      setCountdown(i - 1);
    }
    setCountdown(null);
    if (stateRef.current?.room.status === "voting") {
      await apiCall("POST", `/api/rooms/${roomId}/reveal`);
    }
    if (mountedRef.current) load();
  };

  const cancelReveal = () => { cancelRevealRef.current = true; setCountdown(null); };

  const acceptEstimate = async (value: number) => {
    await apiCall("POST", `/api/rooms/${roomId}/accept`, { value });
    load();
  };

  const closeRoom = async () => {
    await apiCall("POST", `/api/rooms/${roomId}/close`);
    load();
  };

  const setRole = async (user: string, role: string) => {
    await apiCall("PUT", `/api/rooms/${roomId}/role`, { username: user, role });
    load();
  };

  // Compute average for revealed votes
  const revealedValues = votes.filter(v => v.value !== null).map(v => v.value!);
  const avg = revealedValues.length > 0 ? revealedValues.reduce((a, b) => a + b, 0) / revealedValues.length : 0;
  const consensus = revealedValues.length > 0 && revealedValues.every(v => v === revealedValues[0]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 p-4 border-b border-white/5">
        <button onClick={onBack} className="w-9 h-9 flex items-center justify-center rounded-lg glass text-white/60 hover:text-white">
          <ArrowLeft size={18} />
        </button>
        <div className="flex-1">
          <h2 className="text-sm font-semibold text-white">{room.name}</h2>
          <div className="text-xs text-white/30 flex gap-2">
            <span>{room.estimation_unit}</span>
            <span className={`${room.status === "voting" ? "text-[var(--color-warning)]" : room.status === "revealed" ? "text-[var(--color-success)]" : ""}`}>
              {room.status}
            </span>
            <span>{members.length} members</span>
            {/* BL18: Share room ID */}
            <button onClick={() => { navigator.clipboard.writeText(`Room #${state.room.id}: ${room.name}`); useStore.getState().toast("Room ID copied"); }}
              className="text-white/30 hover:text-white/50">📋 Share</button>
          </div>
        </div>
        {/* Tabs */}
        <div className="flex gap-1">
          {(["board", "tasks", "members", "history"] as const).map(t => (
            <button key={t} onClick={() => setTab(t)}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-all ${tab === t ? "bg-[var(--color-accent)]/20 text-[var(--color-accent)]" : "text-white/30 hover:text-white/60"}`}>
              {t === "board" ? "🃏" : t === "tasks" ? "📋" : t === "members" ? "👥" : "📊"} {t}
            </button>
          ))}
        </div>
        {isAdmin && room.status !== "closed" && (
          <button onClick={closeRoom} className="px-3 py-1.5 rounded-lg text-xs text-white/30 hover:text-[var(--color-danger)] bg-white/5" title="Close room">
            <X size={14} />
          </button>
        )}
        {!isAdmin && (
          <button onClick={() => {
              useStore.getState().showConfirm("Leave this room?", async () => { await apiCall("POST", `/api/rooms/${roomId}/leave`); onBack(); });
            }}
            className="px-3 py-1.5 rounded-lg text-xs text-white/30 hover:text-white/60 bg-white/5" title="Leave room">
            Leave
          </button>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {/* Board tab */}
        {tab === "board" && (
          <div className="space-y-6">
            {/* Current task */}
            {current_task ? (
              <div className="glass p-5">
                <div className="text-xs text-white/30 mb-1">Currently voting on:</div>
                {isAdmin && editingTask ? (
                  <div className="space-y-2">
                    <input value={editTitle} onChange={e => setEditTitle(e.target.value)}
                      className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-lg font-semibold text-white outline-none focus:border-[var(--color-accent)]" />
                    <textarea value={editDesc} onChange={e => setEditDesc(e.target.value)} rows={3}
                      className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white/70 outline-none focus:border-[var(--color-accent)] resize-none" placeholder="Description..." />
                    <div className="flex gap-2">
                      <button onClick={async () => {
                          await apiCall("PUT", `/api/tasks/${current_task.id}`, { title: editTitle, description: editDesc || null });
                          setEditingTask(false); load();
                        }} className="px-3 py-1.5 rounded-lg bg-[var(--color-success)] text-white text-xs font-semibold">Save</button>
                      <button onClick={() => setEditingTask(false)} className="px-3 py-1.5 rounded-lg bg-white/5 text-white/40 text-xs">Cancel</button>
                    </div>
                  </div>
                ) : (
                  <>
                    <div className="text-lg font-semibold text-white mb-2 flex items-center gap-2">
                      {current_task.title}
                      {elapsed > 0 && <span className={`text-xs font-mono ${elapsed > discussionLimit ? "text-red-400" : "text-white/30"}`}>{Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}{elapsed > discussionLimit ? " ⏰" : ""}</span>}
                      {isAdmin && <button onClick={() => { setEditTitle(current_task.title); setEditDesc(current_task.description || ""); setEditingTask(true); }}
                        className="text-white/30 hover:text-white/50"><Edit3 size={14} /></button>}
                    </div>
                    {current_task.description && <p className="text-sm text-white/50 mb-3 whitespace-pre-wrap">{current_task.description}</p>}
                  </>
                )}
                <div className="text-xs text-white/20 flex gap-3">
                  {current_task.project && <span>📁 {current_task.project}</span>}
                  <span>by {current_task.user}</span>
                </div>
                {/* BL17: Show previous estimates for similar tasks */}
                {vote_history.length > 0 && current_task.project && (() => {
                  const similar = vote_history.filter(vh => {
                    const t = allTasks.find(t => t.id === vh.task_id);
                    return t && t.project === current_task.project && vh.task_id !== current_task.id;
                  }).slice(0, 3);
                  return similar.length > 0 ? (
                    <div className="text-[10px] text-white/20 mt-2">
                      Similar ({current_task.project}): {similar.map(vh => `${vh.task_title} → ${vh.average.toFixed(1)}`).join(", ")}
                    </div>
                  ) : null;
                })()}
              </div>
            ) : (
              <div className="glass p-8 text-center text-white/30 text-sm">
                {isAdmin ? "Select a task from the Tasks tab to start voting" : "Waiting for admin to select a task..."}
              </div>
            )}

            {/* Countdown overlay */}
            <AnimatePresence>
              {countdown !== null && countdown > 0 && (
                <motion.div initial={{ opacity: 0, scale: 0.5 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 0.5 }}
                  className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-black/60" aria-live="off" aria-label={`Revealing in ${countdown}`}>
                  <motion.div key={countdown} initial={{ scale: 2, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} exit={{ scale: 0.5, opacity: 0 }}
                    className="text-8xl font-bold text-[var(--color-accent)]">
                    {countdown}
                  </motion.div>
                  <button onClick={cancelReveal} className="mt-6 text-xs text-white/40 hover:text-white/70">Cancel</button>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Voting cards */}
            {room.status === "voting" && (
              <div>
                <div className="text-xs text-white/30 mb-3">Pick your estimate ({room.estimation_unit}):</div>
                <div className="flex flex-wrap gap-2" role="radiogroup" aria-label="Estimation cards">
                  {cards.map(c => (
                    <motion.button key={c} whileHover={{ scale: 1.1, y: -8 }} whileTap={{ scale: 0.95 }}
                      animate={selectedCard === c ? { scale: [1, 1.15, 1.05], y: -4 } : { scale: 1, y: 0 }}
                      transition={{ duration: 0.2 }}
                      onClick={() => vote(c)}
                      disabled={submitting}
                      role="radio" aria-checked={selectedCard === c}
                      aria-label={`Vote ${c} ${room.estimation_unit}`}
                      className={`w-14 h-20 rounded-xl flex items-center justify-center text-lg font-bold transition-all border-2 ${
                        selectedCard === c ? "border-[var(--color-accent)] bg-[var(--color-accent)]/20 text-[var(--color-accent)]"
                        : myVote?.voted ? "border-white/10 bg-white/5 text-white/30"
                        : "border-white/10 bg-white/5 text-white/70 hover:border-white/30"
                      }`}>
                      {room.estimation_unit === "tshirt" ? (TSHIRT_LABELS[c] || c) : c}
                    </motion.button>
                  ))}
                </div>
              </div>
            )}

            {/* Vote status */}
            {room.status === "voting" && votes.length > 0 && (
              <div className="glass p-4">
                <div className="text-xs text-white/30 mb-2">Votes:</div>
                <div className="flex flex-wrap gap-2">
                  {votes.map(v => (
                    <div key={v.username} className={`flex items-center gap-2 px-3 py-2 rounded-lg ${v.voted ? "bg-[var(--color-success)]/10" : "bg-white/5"}`}>
                      <div className={`w-2 h-2 rounded-full ${v.voted ? "bg-[var(--color-success)]" : "bg-white/20"}`} />
                      <span className="text-xs text-white/60">{v.username}</span>
                      {v.voted && <span className="text-xs text-[var(--color-success)]">✓</span>}
                    </div>
                  ))}
                </div>
                {/* Not voted warning */}
                {notVoted.length > 0 && (
                  <div className="mt-2 text-xs text-[var(--color-warning)]">
                    ⚠️ Waiting: {notVoted.join(", ")}
                  </div>
                )}
                {/* Reveal button */}
                {isAdmin && (
                  <motion.button whileTap={{ scale: 0.95 }} onClick={doReveal}
                    className={`mt-3 flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold ${
                      allVoted ? "bg-[var(--color-success)] text-white" : "bg-[var(--color-warning)] text-white"
                    }`}>
                    <Eye size={16} /> {allVoted ? "Reveal Cards" : `Reveal (${notVoted.length} missing)`}
                  </motion.button>
                )}
              </div>
            )}

            {/* Revealed results */}
            {room.status === "revealed" && (
              <div className="glass p-5 space-y-4">
                <div className="text-xs text-white/30 mb-2">Results:</div>
                <div className="flex flex-wrap gap-3">
                  {votes.map(v => (
                    <motion.div key={v.username} initial={{ rotateY: 180 }} animate={{ rotateY: 0 }} transition={{ duration: 0.5 }}
                      className="w-16 text-center">
                      <div className={`w-14 h-20 mx-auto rounded-xl flex items-center justify-center text-lg font-bold border-2 ${
                        v.value !== null ? "border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-accent)]" : "border-white/10 bg-white/5 text-white/20"
                      }`}>
                        {v.value ?? "?"}
                      </div>
                      <div className="text-[10px] text-white/40 mt-1 truncate">{v.username}</div>
                    </motion.div>
                  ))}
                </div>

                {/* Stats */}
                <div className="flex gap-4 text-sm">
                  <div className="text-white/40">Average: <span className="text-white font-semibold">{avg.toFixed(1)}</span></div>
                  {consensus && <div className="text-[var(--color-success)] font-semibold">✓ Consensus!</div>}
                  {!consensus && <div className="text-[var(--color-warning)]">No consensus — discuss and re-vote?</div>}
                </div>

                {/* Admin: accept or re-vote */}
                {isAdmin && (
                  <div className="flex gap-2 flex-wrap items-center">
                    {[...new Set(revealedValues)].sort((a, b) => a - b).map(v => (
                      <motion.button key={v} whileTap={{ scale: 0.95 }} onClick={() => acceptEstimate(v)}
                        className="px-3 py-2 rounded-lg bg-[var(--color-success)]/20 text-[var(--color-success)] text-sm font-semibold hover:bg-[var(--color-success)]/30">
                        <Check size={12} className="inline mr-1" /> Accept {v}
                      </motion.button>
                    ))}
                    <motion.button whileTap={{ scale: 0.95 }} onClick={() => acceptEstimate(Math.round(avg * 2) / 2)}
                      className="px-3 py-2 rounded-lg bg-[var(--color-accent)]/20 text-[var(--color-accent)] text-sm font-semibold">
                      Accept avg ({(Math.round(avg * 2) / 2).toFixed(1)})
                    </motion.button>
                    <div className="flex items-center gap-1">
                      <input type="number" step="0.5" min="0" value={customAccept} onChange={e => setCustomAccept(e.target.value)}
                        placeholder="custom" onKeyDown={e => { if (e.key === "Enter" && customAccept && !isNaN(parseFloat(customAccept))) acceptEstimate(parseFloat(customAccept)); }}
                        className="w-20 bg-white/5 border border-white/10 rounded-lg px-2 py-1.5 text-xs text-white placeholder-white/20 outline-none focus:border-[var(--color-accent)] text-center" />
                      {customAccept && (
                        <motion.button whileTap={{ scale: 0.95 }} onClick={() => { acceptEstimate(parseFloat(customAccept)); setCustomAccept(""); }}
                          className="px-2 py-1.5 rounded-lg bg-[var(--color-success)]/20 text-[var(--color-success)] text-xs font-semibold">
                          <Check size={12} />
                        </motion.button>
                      )}
                    </div>
                    <motion.button whileTap={{ scale: 0.95 }} onClick={() => startVoting(current_task!.id)}
                      className="px-3 py-2 rounded-lg bg-[var(--color-warning)]/20 text-[var(--color-warning)] text-sm font-semibold">
                      🔄 Re-vote
                    </motion.button>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* Tasks tab — reuse TaskList in select mode */}
        {tab === "tasks" && (
          <TaskList selectMode onSelect={isAdmin && room.status !== "closed" ? startVoting : undefined}
            selectedTaskId={room.current_task_id}
            votedTaskIds={new Set(vote_history.map(v => v.task_id))} leafOnly={useStore.getState().config?.leaf_only_mode ?? false} />
        )}

        {/* Members tab */}
        {tab === "members" && (
          <div className="space-y-2">
            {members.map(m => (
              <div key={m.username} className="flex items-center gap-3 px-4 py-3 rounded-lg glass">
                <div className="flex-1 flex items-center gap-2">
                  <span className="text-sm text-white/80">{m.username}</span>
                  {m.role === "admin" && <Crown size={14} className="text-[var(--color-warning)]" />}
                </div>
                <span className={`text-xs px-2 py-0.5 rounded ${m.role === "admin" ? "bg-[var(--color-warning)]/10 text-[var(--color-warning)]" : "bg-white/5 text-white/30"}`}>
                  {m.role}
                </span>
                {isAdmin && m.username !== username && (
                  <>
                    <button onClick={() => setRole(m.username, m.role === "admin" ? "voter" : "admin")}
                      className="text-xs text-white/30 hover:text-white/60 px-2 py-1 rounded bg-white/5">
                      {m.role === "admin" ? "demote" : "promote"}
                    </button>
                    <button onClick={async () => { await apiCall("DELETE", `/api/rooms/${roomId}/members/${m.username}`); load(); }}
                      className="text-xs text-white/30 hover:text-[var(--color-danger)] px-2 py-1 rounded bg-white/5">
                      kick
                    </button>
                  </>
                )}
              </div>
            ))}
          </div>
        )}

        {/* History tab */}
        {tab === "history" && (
          <div className="space-y-2">
            {vote_history.length === 0 && <p className="text-sm text-white/30">No completed votes yet.</p>}
            {vote_history.map(vh => (
              <div key={vh.task_id} className="glass p-4">
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-sm text-white/80 font-semibold">{vh.task_title}</span>
                  <span className="text-xs px-2 py-0.5 rounded bg-[var(--color-accent)]/10 text-[var(--color-accent)]">
                    avg: {vh.average.toFixed(1)}
                  </span>
                  {vh.consensus && <span className="text-xs text-[var(--color-success)]">✓ consensus</span>}
                  {/* BL20: Highlight high variance */}
                  {!vh.consensus && vh.votes.length >= 2 && (() => {
                    const vals = vh.votes.map(v => v.value).filter((v): v is number => v != null);
                    if (vals.length < 2) return null;
                    const spread = Math.max(...vals) - Math.min(...vals);
                    return spread > vh.average * 0.5 ? <span className="text-xs text-amber-400">⚠ high variance (spread: {spread})</span> : null;
                  })()}
                </div>
                <div className="flex flex-wrap gap-2">
                  {vh.votes.map(v => (
                    <span key={v.username} className="text-xs text-white/40 bg-white/5 px-2 py-1 rounded">
                      {v.username}: <span className="text-white/60 font-semibold">{v.value ?? "?"}</span>
                    </span>
                  ))}
                </div>
                {/* BL16: Estimation accuracy — compare estimate vs actual */}
                {(() => {
                  const task = allTasks.find(t => t.id === vh.task_id);
                  if (!task || task.actual === 0) return null;
                  const ratio = task.actual / vh.average;
                  const color = ratio <= 1.2 ? "text-green-400" : ratio <= 1.5 ? "text-yellow-400" : "text-red-400";
                  return (
                    <div className="text-[10px] text-white/30 mt-2">
                      Actual: {task.actual} pomodoros vs estimated avg {vh.average.toFixed(1)} →{" "}
                      <span className={color}>{ratio <= 1 ? "on target" : `+${Math.round((ratio - 1) * 100)}% over`}</span>
                    </div>
                  );
                })()}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

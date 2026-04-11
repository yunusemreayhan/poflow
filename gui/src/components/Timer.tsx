import { motion, AnimatePresence } from "framer-motion";
import { Play, Pause, Square, SkipForward, Coffee, MessageSquare } from "lucide-react";
import { useStore } from "../store/store";
import { useEffect, useRef, useState } from "react";
import { CommentSection } from "./TaskDetailView";

const PHASE_COLORS: Record<string, string> = {
  Work: "#FF6B6B",
  ShortBreak: "#4ECDC4",
  LongBreak: "#45B7D1",
  Idle: "#6C7A89",
};

const PHASE_LABELS: Record<string, string> = {
  Work: "FOCUS",
  ShortBreak: "SHORT BREAK",
  LongBreak: "LONG BREAK",
  Idle: "READY",
};

function formatTime(s: number) {
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

const SIZE = 280;
const STROKE = 14;
const R = (SIZE - STROKE) / 2;
const CIRC = 2 * Math.PI * R;

export default function Timer() {
  const { engine, tasks, start, pause, resume, stop, skip, startBreak, config } = useStore();
  const prevPhase = useRef<string | null>(null);
  const [showComment, setShowComment] = useState(false);

  const phase = engine?.phase ?? "Idle";
  const status = engine?.status ?? "Idle";
  const elapsed = engine?.elapsed_s ?? 0;
  const duration = engine?.duration_s ?? 1500;
  const remaining = Math.max(0, duration - elapsed);
  const progress = duration > 0 ? elapsed / duration : 0;
  const color = PHASE_COLORS[phase] ?? "#6C7A89";
  const dashOffset = CIRC * (1 - progress);

  const isRunning = status === "Running";
  const isPaused = status === "Paused";
  const isIdle = status === "Idle";
  const isActive = isRunning || isPaused;

  const currentTask = engine?.current_task_id
    ? tasks.find((t) => t.id === engine.current_task_id)
    : null;

  useEffect(() => { prevPhase.current = phase; }, [phase]);

  return (
    <div className="flex flex-col items-center justify-center gap-6 h-full px-8">
      {/* Phase label */}
      <AnimatePresence mode="wait">
        <motion.div
          key={phase}
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 10 }}
          className="text-sm font-bold tracking-[0.3em] uppercase"
          style={{ color }}
        >
          {PHASE_LABELS[phase] ?? phase}
        </motion.div>
      </AnimatePresence>

      {/* Ring timer */}
      <div className="relative" style={{ width: SIZE, height: SIZE }}>
        {/* Background glow — only when running */}
        {isActive && (
          <motion.div
            className="absolute inset-0 rounded-full"
            animate={{ opacity: isRunning ? [0.15, 0.35, 0.15] : 0.15 }}
            transition={{ duration: 2, repeat: Infinity, ease: "easeInOut" }}
            style={{
              background: `radial-gradient(circle, ${color}40 0%, transparent 70%)`,
            }}
          />
        )}

        <svg width={SIZE} height={SIZE} style={{ transform: "rotate(-90deg)" }}
          role="progressbar" aria-valuenow={elapsed} aria-valuemin={0} aria-valuemax={duration}
          aria-label={`${PHASE_LABELS[phase] ?? phase}: ${formatTime(remaining)} remaining`}>
          <circle
            cx={SIZE / 2} cy={SIZE / 2} r={R}
            fill="none"
            stroke="rgba(255,255,255,0.06)"
            strokeWidth={STROKE}
          />
          <motion.circle
            cx={SIZE / 2} cy={SIZE / 2} r={R}
            fill="none"
            stroke={color}
            strokeWidth={STROKE}
            strokeLinecap="round"
            strokeDasharray={CIRC}
            animate={{ strokeDashoffset: dashOffset }}
            transition={{ duration: 0.5, ease: "easeOut" }}
            style={{ filter: isActive ? `drop-shadow(0 0 8px ${color})` : "none" }}
          />
          {Array.from({ length: 60 }).map((_, i) => {
            const angle = (i / 60) * 2 * Math.PI;
            const isMajor = i % 5 === 0;
            const inner = R - (isMajor ? 12 : 6);
            const outer = R - 2;
            const x1 = SIZE / 2 + inner * Math.cos(angle);
            const y1 = SIZE / 2 + inner * Math.sin(angle);
            const x2 = SIZE / 2 + outer * Math.cos(angle);
            const y2 = SIZE / 2 + outer * Math.sin(angle);
            return (
              <line key={i} x1={x1} y1={y1} x2={x2} y2={y2}
                stroke="rgba(255,255,255,0.12)"
                strokeWidth={isMajor ? 2 : 1}
              />
            );
          })}
        </svg>

        {/* Center content */}
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-1">
          <div
            className="text-6xl font-mono font-bold tabular-nums"
            style={{ color, textShadow: isActive ? `0 0 30px ${color}60` : "none" }}
          >
            {formatTime(remaining)}
          </div>
          <div className="text-xs text-white/30 font-mono">
            {engine?.session_count ?? 0} sessions today
          </div>
          <div className="flex gap-1.5 mt-1">
            {Array.from({ length: engine?.daily_goal ?? 8 }).map((_, i) => (
              <div
                key={i}
                className="w-2 h-2 rounded-full transition-all duration-500"
                style={{
                  background: i < (engine?.daily_completed ?? 0) ? color : "rgba(255,255,255,0.1)",
                  boxShadow: i < (engine?.daily_completed ?? 0) ? `0 0 6px ${color}` : "none",
                }}
              />
            ))}
          </div>
        </div>
      </div>

      {/* Current task */}
      <AnimatePresence>
        {currentTask && (
          <motion.div
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -5 }}
            className="glass px-5 py-3 text-sm font-semibold text-white max-w-sm text-center truncate"
          >
            🎯 {currentTask.title}
            {currentTask.project && <span className="text-xs text-white/40 ml-2">({currentTask.project})</span>}
          </motion.div>
        )}
      </AnimatePresence>

      {/* Timer owner indicator */}
      {isActive && engine?.current_user_id !== 0 && (
        <div className="text-xs text-white/30">
          ⏱️ Timer owned by {currentTask?.user ?? "user #" + engine?.current_user_id}
        </div>
      )}

      {/* Controls */}
      <div className="flex items-center gap-5">
        {isIdle && (
          <motion.button
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
            onClick={() => start()}
            className="flex items-center gap-3 px-10 py-4 rounded-full font-semibold text-white text-base transition-all"
            style={{ background: `linear-gradient(135deg, ${color}, ${color}99)`, boxShadow: `0 4px 20px ${color}40` }}
          >
            <Play size={20} fill="white" />
            Start Focus
          </motion.button>
        )}

        {isRunning && (
          <>
            <motion.button
              whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }}
              onClick={pause} aria-label="Pause"
              className="glass glass-hover w-12 h-12 flex items-center justify-center rounded-full text-white/70 hover:text-white"
            >
              <Pause size={22} />
            </motion.button>
            <motion.button
              whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }}
              onClick={stop} aria-label="Stop"
              className="glass glass-hover w-12 h-12 flex items-center justify-center rounded-full text-white/70 hover:text-white"
            >
              <Square size={22} />
            </motion.button>
            <motion.button
              whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }}
              onClick={skip} aria-label="Skip"
              className="glass glass-hover w-12 h-12 flex items-center justify-center rounded-full text-white/70 hover:text-white"
            >
              <SkipForward size={22} />
            </motion.button>
          </>
        )}

        {isPaused && (
          <>
            <motion.button
              whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }}
              onClick={resume}
              className="flex items-center gap-3 px-8 py-4 rounded-full font-semibold text-white text-base"
              style={{ background: `linear-gradient(135deg, ${color}, ${color}99)` }}
            >
              <Play size={20} fill="white" />
              Resume
            </motion.button>
            <motion.button
              whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }}
              onClick={stop}
              className="glass glass-hover w-12 h-12 flex items-center justify-center rounded-full text-white/70 hover:text-white"
            >
              <Square size={22} />
            </motion.button>
          </>
        )}
      </div>

      {/* Break shortcuts */}
      {isIdle && (
        <div className="flex gap-4">
          <motion.button
            whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.97 }}
            onClick={() => startBreak("short_break")}
            className="glass glass-hover flex items-center gap-2 px-5 py-2.5 rounded-full text-sm text-white/60 hover:text-white"
          >
            <Coffee size={15} />
            Short Break ({config?.short_break_min || 5}m)
          </motion.button>
          <motion.button
            whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.97 }}
            onClick={() => startBreak("long_break")}
            className="glass glass-hover flex items-center gap-2 px-5 py-2.5 rounded-full text-sm text-white/60 hover:text-white"
          >
            <Coffee size={15} />
            Long Break ({config?.long_break_min || 15}m)
          </motion.button>
        </div>
      )}

      {/* Session comment */}
      {isActive && engine?.current_task_id && (
        <div className="w-full max-w-md">
          <button onClick={() => setShowComment(!showComment)}
            className="flex items-center gap-2 text-xs text-white/40 hover:text-white/70 transition-colors mb-2 mx-auto">
            <MessageSquare size={14} />
            {showComment ? "Hide" : "Add"} session note
          </button>
          <AnimatePresence>
            {showComment && (
              <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
                className="glass p-4 overflow-hidden">
                <CommentSection taskId={engine.current_task_id} sessionId={engine.current_session_id ?? undefined} />
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}
    </div>
  );
}

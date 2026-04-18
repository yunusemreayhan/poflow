import type { StateCreator } from "zustand";
import { apiCall } from "./api";
import type { EngineState } from "./api";

// Dedup guard
const _inflight = new Set<string>();
function dedup(key: string, fn: () => Promise<void>): Promise<void> {
  if (_inflight.has(key)) return Promise.resolve();
  _inflight.add(key);
  return fn().finally(() => _inflight.delete(key));
}

export interface TimerSlice {
  engine: EngineState | null;
  connected: boolean;
  timerTaskId: number | undefined;
  poll: () => Promise<void>;
  start: (taskId?: number) => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  skip: () => Promise<void>;
  startBreak: (type: "short_break" | "long_break") => Promise<void>;
}

export const createTimerSlice: StateCreator<TimerSlice & { token: string | null; error: string | null }, [], [], TimerSlice> = (set, get) => ({
  engine: null,
  connected: false,
  timerTaskId: undefined,

  poll: async () => {
    if (!get().token) return;
    try {
      const engine = await apiCall<EngineState>("GET", "/api/timer");
      set({ engine, connected: true, error: null });
    } catch (e) {
      set({ connected: false, error: String(e) });
    }
  },

  start: async (taskId) => dedup("timer:start", async () => {
    const body: Record<string, unknown> = {};
    if (taskId) body.task_id = taskId;
    const engine = await apiCall<EngineState>("POST", "/api/timer/start", body);
    set({ engine });
  }),

  pause: async () => { set({ engine: await apiCall<EngineState>("POST", "/api/timer/pause") }); },
  resume: async () => { set({ engine: await apiCall<EngineState>("POST", "/api/timer/resume") }); },
  stop: async () => { set({ engine: await apiCall<EngineState>("POST", "/api/timer/stop") }); },
  skip: async () => { set({ engine: await apiCall<EngineState>("POST", "/api/timer/skip") }); },
  startBreak: async (type) => { set({ engine: await apiCall<EngineState>("POST", "/api/timer/start", { phase: type }) }); },
});

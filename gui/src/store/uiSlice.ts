import type { StateCreator } from "zustand";
import { apiCall } from "./api";
import type { DayStat, Session, Config, Sprint } from "./api";
import { processSyncQueue } from "../offlineStore";

let toastCounter = 0;

export interface UiSlice {
  loading: { tasks: boolean; history: boolean; stats: boolean; config: boolean };
  mutating: boolean;
  activeTab: string;
  activeTeamId: number | null;
  teamScope: Set<number> | null;
  error: string | null;
  stats: DayStat[];
  history: Session[];
  sprints: Sprint[];
  config: Config | null;
  toasts: { id: number; msg: string; type: "success" | "error" | "info"; onUndo?: () => void }[];
  toast: (msg: string, type?: "success" | "error" | "info", onUndo?: () => void) => void;
  dismissToast: (id: number) => void;
  confirmDialog: { msg: string; onConfirm: () => void; confirmLabel?: string } | null;
  showConfirm: (msg: string, onConfirm: () => void, confirmLabel?: string) => void;
  dismissConfirm: () => void;
  focusMode: boolean;
  toggleFocusMode: () => void;
  setTab: (tab: string) => void;
  syncOfflineQueue: () => Promise<void>;
  loadStats: () => Promise<void>;
  loadHistory: () => Promise<void>;
  loadSprints: () => Promise<void>;
  loadConfig: () => Promise<void>;
  updateConfig: (cfg: Config) => Promise<void>;
}

export const createUiSlice: StateCreator<UiSlice & { token: string | null; loadTasks: () => Promise<void> }, [], [], UiSlice> = (set, get) => ({
  loading: { tasks: false, history: false, stats: false, config: false },
  mutating: false,
  activeTab: "timer",
  activeTeamId: (() => { try { return JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("activeTeamId")) || "null"); } catch { return null; } })(),
  teamScope: null,
  error: null,
  stats: [],
  history: [],
  sprints: [],
  config: null,
  toasts: [],
  toast: (msg, type = "success", onUndo) => {
    const id = ++toastCounter;
    set(s => ({ toasts: [...s.toasts.slice(-2), { id, msg, type, onUndo }] }));
    setTimeout(() => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })), onUndo ? 8000 : type === "error" ? 6000 : 3000);
  },
  dismissToast: (id) => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })),
  confirmDialog: null,
  showConfirm: (msg, onConfirm, confirmLabel) => set({ confirmDialog: { msg, onConfirm, confirmLabel } }),
  dismissConfirm: () => set({ confirmDialog: null }),
  focusMode: false,
  toggleFocusMode: () => set(s => ({ focusMode: !s.focusMode })),
  setTab: (tab) => set({ activeTab: tab }),

  syncOfflineQueue: async () => {
    const token = get().token;
    if (!token || !navigator.onLine) return;
    const { synced, failed } = await processSyncQueue(token);
    if (synced > 0) { get().toast(`Synced ${synced} offline action${synced > 1 ? "s" : ""}`, "success"); get().loadTasks(); }
    if (failed > 0) get().toast(`${failed} offline action${failed > 1 ? "s" : ""} failed to sync`, "error");
  },

  loadStats: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, stats: true } }));
    try {
      const stats = await apiCall<DayStat[]>("GET", "/api/stats?days=365");
      set(s => ({ stats, loading: { ...s.loading, stats: false } }));
    } catch { set(s => ({ loading: { ...s.loading, stats: false } })); }
  },

  loadHistory: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, history: true } }));
    try {
      const history = await apiCall<Session[]>("GET", "/api/history");
      set(s => ({ history, loading: { ...s.loading, history: false } }));
    } catch { set(s => ({ loading: { ...s.loading, history: false } })); }
  },

  loadSprints: async () => {
    if (!get().token) return;
    const sprints = await apiCall<Sprint[]>("GET", "/api/sprints").catch(() => [] as Sprint[]);
    set({ sprints });
  },

  loadConfig: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, config: true } }));
    const config = await apiCall<Config>("GET", "/api/config");
    set(s => ({ config, loading: { ...s.loading, config: false } }));
  },

  updateConfig: async (cfg) => {
    await apiCall("PUT", "/api/config", cfg);
    set({ config: cfg });
  },
});

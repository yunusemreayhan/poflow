import { create } from "zustand";
import { apiCall, setToken } from "./api";
import { invoke } from "@tauri-apps/api/core";
import type { EngineState, Task, DayStat, Session, Config, Comment, TaskDetail, AuthResponse, TaskSprintInfo, BurnTotalEntry, TaskAssignee } from "./api";

export interface SavedServer {
  url: string;
  username: string;
  token: string;
  refresh_token: string;
  role: string;
}

function loadServers(): SavedServer[] {
  try { return JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("servers")) || "[]"); } catch { return []; }
}
function saveServers(servers: SavedServer[]) {
  localStorage.setItem("servers", JSON.stringify(servers));
}

interface Store {
  // State
  engine: EngineState | null;
  tasks: Task[];
  taskSprints: TaskSprintInfo[];
  taskSprintsMap: Map<number, TaskSprintInfo[]>;
  burnTotals: Map<number, BurnTotalEntry>;
  allAssignees: Map<number, string[]>;
  stats: DayStat[];
  history: Session[];
  config: Config | null;
  connected: boolean;
  loading: { tasks: boolean; history: boolean; stats: boolean; config: boolean };
  activeTab: string;
  activeTeamId: number | null;
  teamScope: Set<number> | null; // descendant IDs for active team
  error: string | null;
  // Auth
  token: string | null;
  username: string | null;
  role: string | null;
  serverUrl: string;
  savedServers: SavedServer[];

  // Actions
  setTab: (tab: string) => void;
  poll: () => Promise<void>;
  start: (taskId?: number) => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  skip: () => Promise<void>;
  startBreak: (type: "short_break" | "long_break") => Promise<void>;
  loadTasks: () => Promise<void>;
  createTask: (title: string, parentId?: number, project?: string, priority?: number, estimated?: number) => Promise<void>;
  updateTask: (id: number, fields: Record<string, unknown>) => Promise<void>;
  deleteTask: (id: number) => Promise<void>;
  setActiveTeam: (teamId: number | null) => void;
  loadStats: () => Promise<void>;
  loadHistory: () => Promise<void>;
  loadConfig: () => Promise<void>;
  updateConfig: (cfg: Config) => Promise<void>;
  addComment: (taskId: number, content: string, sessionId?: number) => Promise<Comment>;
  getTaskDetail: (id: number) => Promise<TaskDetail>;
  // Auth
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<void>;
  logout: () => void;
  restoreAuth: () => Promise<void> | void;
  setServerUrl: (url: string) => Promise<void>;
  switchToServer: (server: SavedServer) => Promise<void>;
  removeServer: (url: string, username: string) => void;
  // Toast
  toasts: { id: number; msg: string; type: "success" | "error"; onUndo?: () => void }[];
  toast: (msg: string, type?: "success" | "error", onUndo?: () => void) => void;
  dismissToast: (id: number) => void;
  // Confirm dialog
  confirmDialog: { msg: string; onConfirm: () => void } | null;
  showConfirm: (msg: string, onConfirm: () => void) => void;
  dismissConfirm: () => void;
}

export const useStore = create<Store>((set, get) => ({
  engine: null,
  tasks: [],
  taskSprints: [],
  taskSprintsMap: new Map(),
  burnTotals: new Map(),
  allAssignees: new Map(),
  stats: [],
  history: [],
  config: null,
  connected: false,
  loading: { tasks: false, history: false, stats: false, config: false },
  activeTab: "timer",
  activeTeamId: JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("activeTeamId")) || "null"),
  teamScope: null,
  error: null,
  token: null,
  username: null,
  role: null,
  serverUrl: (typeof localStorage !== "undefined" && localStorage.getItem("serverUrl")) || "http://127.0.0.1:9090",
  savedServers: loadServers(),
  toasts: [],
  toast: (msg, type = "success", onUndo) => {
    const id = Date.now() * 1000 + Math.floor(Math.random() * 1000);
    set(s => ({ toasts: [...s.toasts, { id, msg, type, onUndo }] }));
    setTimeout(() => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })), onUndo ? 8000 : type === "error" ? 6000 : 3000);
  },
  dismissToast: (id) => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })),
  confirmDialog: null,
  showConfirm: (msg, onConfirm) => set({ confirmDialog: { msg, onConfirm } }),
  dismissConfirm: () => set({ confirmDialog: null }),

  setTab: (tab) => set({ activeTab: tab }),

  login: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/login", { username, password });
    await setToken(resp.token);
    invoke("save_auth", { data: JSON.stringify(resp) }).catch(() => {
      localStorage.setItem("auth", JSON.stringify(resp)); // fallback
    });
    set({ token: resp.token, username: resp.username, role: resp.role });
    // Save to server list
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, refresh_token: resp.refresh_token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  },

  register: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/register", { username, password });
    await setToken(resp.token);
    invoke("save_auth", { data: JSON.stringify(resp) }).catch(() => {
      localStorage.setItem("auth", JSON.stringify(resp));
    });
    set({ token: resp.token, username: resp.username, role: resp.role });
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, refresh_token: resp.refresh_token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  },

  logout: () => {
    apiCall("POST", "/api/auth/logout").catch(() => {});
    invoke("clear_auth").catch(() => {});
    localStorage.removeItem("auth");
    set({ token: null, username: null, role: null });
    invoke("set_token", { token: "" }).catch(() => {});
  },

  restoreAuth: async () => {
    const url = localStorage.getItem("serverUrl");
    if (url) {
      set({ serverUrl: url });
      invoke("set_connection", { baseUrl: url });
    }
    // Try Tauri secure store first, fall back to localStorage
    let saved: string | null = null;
    try { saved = await invoke<string>("load_auth"); } catch {}
    if (!saved) saved = localStorage.getItem("auth");
    if (saved) {
      try {
        const auth = JSON.parse(saved) as AuthResponse;
        set({ token: auth.token, username: auth.username, role: auth.role });
        setToken(auth.token);
      } catch { /* ignore */ }
    }
  },

  setServerUrl: async (url) => {
    const clean = url.replace(/\/+$/, "");
    localStorage.setItem("serverUrl", clean);
    await invoke("set_connection", { baseUrl: clean });
    set({ serverUrl: clean, token: null, username: null, role: null });
    localStorage.removeItem("auth");
  },

  switchToServer: async (server) => {
    localStorage.setItem("serverUrl", server.url);
    await invoke("set_connection", { baseUrl: server.url });
    await setToken(server.token);
    invoke("save_auth", { data: JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role }) }).catch(() => {
      localStorage.setItem("auth", JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role }));
    });
    set({ serverUrl: server.url, token: server.token, username: server.username, role: server.role });
  },

  removeServer: (url, username) => {
    const servers = loadServers().filter(s => !(s.url === url && s.username === username));
    saveServers(servers);
    set({ savedServers: servers });
  },

  poll: async () => {
    if (!get().token) return;
    try {
      const engine = await apiCall<EngineState>("GET", "/api/timer");
      set({ engine, connected: true, error: null });
    } catch (e) {
      set({ connected: false, error: String(e) });
    }
  },

  start: async (taskId) => {
    const body: Record<string, unknown> = {};
    if (taskId) body.task_id = taskId;
    const engine = await apiCall<EngineState>("POST", "/api/timer/start", body);
    set({ engine });
  },

  pause: async () => {
    const engine = await apiCall<EngineState>("POST", "/api/timer/pause");
    set({ engine });
  },

  resume: async () => {
    const engine = await apiCall<EngineState>("POST", "/api/timer/resume");
    set({ engine });
  },

  stop: async () => {
    const engine = await apiCall<EngineState>("POST", "/api/timer/stop");
    set({ engine });
  },

  skip: async () => {
    const engine = await apiCall<EngineState>("POST", "/api/timer/skip");
    set({ engine });
  },

  startBreak: async (type) => {
    const engine = await apiCall<EngineState>("POST", "/api/timer/start", { phase: type });
    set({ engine });
  },

  loadTasks: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, tasks: true } }));
    try {
      const resp = await apiCall<{ tasks: Task[]; task_sprints: TaskSprintInfo[]; burn_totals: BurnTotalEntry[]; assignees: TaskAssignee[] }>("GET", "/api/tasks/full");
      const burnTotals = new Map<number, BurnTotalEntry>();
      for (const bt of resp.burn_totals) burnTotals.set(bt.task_id, bt);
      const allAssignees = new Map<number, string[]>();
      for (const a of resp.assignees) {
        const list = allAssignees.get(a.task_id) || [];
        list.push(a.username);
        allAssignees.set(a.task_id, list);
      }
      const ts = resp.task_sprints || [];
      const taskSprintsMap = new Map<number, TaskSprintInfo[]>();
      for (const s of ts) {
        const list = taskSprintsMap.get(s.task_id) || [];
        list.push(s);
        taskSprintsMap.set(s.task_id, list);
      }
      // Only update tasks if data actually changed (avoid unnecessary tree rebuilds)
      const prev = get().tasks;
      const tasksChanged = prev.length !== resp.tasks.length || resp.tasks.some((t, i) => t.id !== prev[i]?.id || t.updated_at !== prev[i]?.updated_at);
      set({ tasks: tasksChanged ? resp.tasks : prev, taskSprints: ts, taskSprintsMap, burnTotals, allAssignees });
      (window as unknown as Record<string, number>).__tasksLoadedAt = Date.now();
    } catch { /* ignore */ }
    set(s => ({ loading: { ...s.loading, tasks: false } }));
  },

  createTask: async (title, parentId, project, priority = 3, estimated = 1) => {
    const task = await apiCall<Task>("POST", "/api/tasks", { title, parent_id: parentId, project, priority, estimated });
    if (task) set(s => ({ tasks: [...s.tasks, task] }));
    get().toast("Task created");
  },

  updateTask: async (id, fields) => {
    try {
      const updated = await apiCall<Task>("PUT", `/api/tasks/${id}`, fields);
      if (updated) set(s => ({ tasks: s.tasks.map(t => t.id === id ? updated : t) }));
    } catch (e) {
      const msg = String(e);
      if (msg.includes("modified by another")) {
        get().toast("Conflict: task was modified by someone else. Refreshing...", "error");
        get().loadTasks();
        return;
      }
      throw e;
    }
  },

  deleteTask: async (id) => {
    const task = get().tasks.find(t => t.id === id);
    get().showConfirm("Delete this task and all subtasks?", async () => {
      await apiCall("DELETE", `/api/tasks/${id}`);
      // Remove task and all descendants from local state
      const descendants = new Set<number>();
      const collect = (pid: number) => {
        descendants.add(pid);
        get().tasks.filter(t => t.parent_id === pid).forEach(t => collect(t.id));
      };
      collect(id);
      set(s => ({ tasks: s.tasks.filter(t => !descendants.has(t.id)) }));
      get().toast(`Deleted "${task?.title || "task"}"`, "success");
    });
  },

  setActiveTeam: (teamId) => {
    localStorage.setItem("activeTeamId", JSON.stringify(teamId));
    if (teamId) {
      apiCall<number[]>("GET", `/api/teams/${teamId}/scope`).then(ids => {
        set({ activeTeamId: teamId, teamScope: ids && ids.length > 0 ? new Set(ids) : new Set() });
      });
    } else {
      set({ activeTeamId: null, teamScope: null });
    }
  },

  loadStats: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, stats: true } }));
    const stats = await apiCall<DayStat[]>("GET", "/api/stats?days=365");
    set(s => ({ stats, loading: { ...s.loading, stats: false } }));
  },

  loadHistory: async () => {
    if (!get().token) return;
    set(s => ({ loading: { ...s.loading, history: true } }));
    const history = await apiCall<Session[]>("GET", "/api/history");
    set(s => ({ history, loading: { ...s.loading, history: false } }));
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

  addComment: async (taskId, content, sessionId) => {
    const body: Record<string, unknown> = { content };
    if (sessionId) body.session_id = sessionId;
    return apiCall<Comment>("POST", `/api/tasks/${taskId}/comments`, body);
  },

  getTaskDetail: async (id) => {
    return apiCall<TaskDetail>("GET", `/api/tasks/${id}`);
  },
}));

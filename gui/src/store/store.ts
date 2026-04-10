import { create } from "zustand";
import { apiCall, setToken } from "./api";
import { invoke } from "@tauri-apps/api/core";
import type { EngineState, Task, DayStat, Session, Config, Comment, TaskDetail, AuthResponse, TaskSprintInfo, BurnTotalEntry, TaskAssignee } from "./api";

export interface SavedServer {
  url: string;
  username: string;
  token: string;
  role: string;
}

function loadServers(): SavedServer[] {
  try { return JSON.parse(localStorage.getItem("servers") || "[]"); } catch { return []; }
}
function saveServers(servers: SavedServer[]) {
  localStorage.setItem("servers", JSON.stringify(servers));
}

interface Store {
  // State
  engine: EngineState | null;
  tasks: Task[];
  taskSprints: TaskSprintInfo[];
  burnTotals: Map<number, BurnTotalEntry>;
  allAssignees: Map<number, string[]>;
  stats: DayStat[];
  history: Session[];
  config: Config | null;
  connected: boolean;
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
  restoreAuth: () => void;
  setServerUrl: (url: string) => Promise<void>;
  switchToServer: (server: SavedServer) => Promise<void>;
  removeServer: (url: string, username: string) => void;
  // Toast
  toasts: { id: number; msg: string; type: "success" | "error" }[];
  toast: (msg: string, type?: "success" | "error") => void;
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
  burnTotals: new Map(),
  allAssignees: new Map(),
  stats: [],
  history: [],
  config: null,
  connected: false,
  activeTab: "timer",
  activeTeamId: JSON.parse(localStorage.getItem("activeTeamId") || "null"),
  teamScope: null,
  error: null,
  token: null,
  username: null,
  role: null,
  serverUrl: localStorage.getItem("serverUrl") || "http://127.0.0.1:9090",
  savedServers: loadServers(),
  toasts: [],
  toast: (msg, type = "success") => {
    const id = Date.now();
    set(s => ({ toasts: [...s.toasts, { id, msg, type }] }));
    setTimeout(() => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })), 3000);
  },
  dismissToast: (id) => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })),
  confirmDialog: null,
  showConfirm: (msg, onConfirm) => set({ confirmDialog: { msg, onConfirm } }),
  dismissConfirm: () => set({ confirmDialog: null }),

  setTab: (tab) => set({ activeTab: tab }),

  login: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/login", { username, password });
    await setToken(resp.token);
    localStorage.setItem("auth", JSON.stringify(resp));
    set({ token: resp.token, username: resp.username, role: resp.role });
    // Save to server list
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  },

  register: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/register", { username, password });
    await setToken(resp.token);
    localStorage.setItem("auth", JSON.stringify(resp));
    set({ token: resp.token, username: resp.username, role: resp.role });
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  },

  logout: () => {
    localStorage.removeItem("auth");
    set({ token: null, username: null, role: null });
    setToken("");
  },

  restoreAuth: () => {
    const url = localStorage.getItem("serverUrl");
    if (url) {
      set({ serverUrl: url });
      invoke("set_connection", { baseUrl: url });
    }
    const saved = localStorage.getItem("auth");
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
    localStorage.setItem("auth", JSON.stringify({ token: server.token, username: server.username, role: server.role }));
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
      set({ tasks: resp.tasks, taskSprints: resp.task_sprints || [], burnTotals, allAssignees });
    } catch { /* ignore */ }
  },

  createTask: async (title, parentId, project, priority = 3, estimated = 1) => {
    await apiCall("POST", "/api/tasks", { title, parent_id: parentId, project, priority, estimated });
    get().toast("Task created");
    get().loadTasks();
  },

  updateTask: async (id, fields) => {
    try {
      await apiCall("PUT", `/api/tasks/${id}`, fields);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("modified by another")) {
        get().toast("Conflict: task was modified by someone else. Refreshing...", "error");
        get().loadTasks();
        return;
      }
      throw e;
    }
    get().loadTasks();
  },

  deleteTask: async (id) => {
    get().showConfirm("Delete this task and all subtasks?", async () => {
      await apiCall("DELETE", `/api/tasks/${id}`);
      get().toast("Task deleted");
      await get().loadTasks();
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
    const stats = await apiCall<DayStat[]>("GET", "/api/stats?days=365");
    set({ stats });
  },

  loadHistory: async () => {
    if (!get().token) return;
    const history = await apiCall<Session[]>("GET", "/api/history");
    set({ history });
  },

  loadConfig: async () => {
    if (!get().token) return;
    const config = await apiCall<Config>("GET", "/api/config");
    set({ config });
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

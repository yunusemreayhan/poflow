import { create } from "zustand";
import { apiCall, setToken } from "./api";
import { platformSaveAuth, platformClearAuth, platformSetConnection, platformSetToken, platformApiCall } from "../platform";
import type { EngineState, Task, DayStat, Session, Config, Comment, TaskDetail, AuthResponse, TaskSprintInfo, BurnTotalEntry, TaskAssignee, Sprint } from "./api";
import { cacheTasksOffline, getOfflineTasks, enqueueOfflineAction, processSyncQueue } from "../offlineStore";

// Task load timestamp tracked in store

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
  // --- Timer & Engine State ---
  engine: EngineState | null;

  // --- Task Data ---
  tasks: Task[];
  taskSprints: TaskSprintInfo[];
  taskSprintsMap: Map<number, TaskSprintInfo[]>;
  burnTotals: Map<number, BurnTotalEntry>;
  allAssignees: Map<number, string[]>;
  taskLabelsMap: Map<number, { name: string; color: string }[]>;

  // --- History & Stats ---
  stats: DayStat[];
  history: Session[];
  sprints: Sprint[];

  // --- Config & UI State ---
  config: Config | null;
  connected: boolean;
  loading: { tasks: boolean; history: boolean; stats: boolean; config: boolean };
  mutating: boolean;
  tasksLoadedAt: number;
  activeTab: string;
  timerTaskId: number | undefined;
  activeTeamId: number | null;
  teamScope: Set<number> | null;
  error: string | null;

  // --- Auth ---
  token: string | null;
  username: string | null;
  role: string | null;
  serverUrl: string;
  savedServers: SavedServer[];

  // --- Timer Actions ---
  setTab: (tab: string) => void;
  poll: () => Promise<void>;
  start: (taskId?: number) => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  skip: () => Promise<void>;
  startBreak: (type: "short_break" | "long_break") => Promise<void>;

  // --- Task Actions ---
  loadTasks: () => Promise<void>;
  createTask: (title: string, parentId?: number, project?: string, priority?: number, estimated?: number) => Promise<void>;
  updateTask: (id: number, fields: Record<string, unknown>) => Promise<void>;
  deleteTask: (id: number) => void;
  setActiveTeam: (teamId: number | null) => void;
  addComment: (taskId: number, content: string, sessionId?: number) => Promise<Comment>;
  getTaskDetail: (id: number) => Promise<TaskDetail>;

  // --- Data Loading ---
  loadStats: () => Promise<void>;
  loadHistory: () => Promise<void>;
  loadSprints: () => Promise<void>;
  loadConfig: () => Promise<void>;
  updateConfig: (cfg: Config) => Promise<void>;

  // --- Auth Actions ---
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<void>;
  logout: () => void;
  restoreAuth: () => Promise<void> | void;
  setServerUrl: (url: string) => Promise<void>;
  switchToServer: (server: SavedServer) => Promise<void>;
  removeServer: (url: string, username: string) => void;
  // --- Toast & Dialog ---
  toasts: { id: number; msg: string; type: "success" | "error" | "info"; onUndo?: () => void }[];
  toast: (msg: string, type?: "success" | "error" | "info", onUndo?: () => void) => void;
  syncOfflineQueue: () => Promise<void>;
  dismissToast: (id: number) => void;
  // Confirm dialog
  confirmDialog: { msg: string; onConfirm: () => void; confirmLabel?: string } | null;
  showConfirm: (msg: string, onConfirm: () => void, confirmLabel?: string) => void;
  dismissConfirm: () => void;
  // Focus mode
  focusMode: boolean;
  toggleFocusMode: () => void;
}

// V32-6: Monotonic toast ID counter
let toastCounter = 0;

export const useStore = create<Store>((set, get) => ({
  engine: null,
  tasks: [],
  taskSprints: [],
  taskSprintsMap: new Map(),
  burnTotals: new Map(),
  allAssignees: new Map(),
  taskLabelsMap: new Map(),
  stats: [],
  history: [],
  sprints: [],
  config: null,
  connected: false,
  loading: { tasks: false, history: false, stats: false, config: false },
  mutating: false,
  tasksLoadedAt: 0,
  activeTab: "timer",
  timerTaskId: undefined,
  activeTeamId: (() => { try { return JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("activeTeamId")) || "null"); } catch { return null; } })(),
  teamScope: null,
  error: null,
  token: null,
  username: null,
  role: null,
  serverUrl: (typeof localStorage !== "undefined" && localStorage.getItem("serverUrl")) || "http://127.0.0.1:9090",
  savedServers: loadServers(),
  toasts: [],
  toast: (msg, type = "success", onUndo) => {
    // V32-6: Monotonic counter to prevent ID collisions
    const id = ++toastCounter;
    // U5: Cap visible toasts at 3
    set(s => ({ toasts: [...s.toasts.slice(-2), { id, msg, type, onUndo }] }));
    setTimeout(() => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })), onUndo ? 8000 : type === "error" ? 6000 : 3000);
  },
  // F16: Sync offline queue when back online
  syncOfflineQueue: async () => {
    const token = get().token;
    if (!token || !navigator.onLine) return;
    const { synced, failed } = await processSyncQueue(token);
    if (synced > 0) { get().toast(`Synced ${synced} offline action${synced > 1 ? "s" : ""}`, "success"); get().loadTasks(); }
    if (failed > 0) get().toast(`${failed} offline action${failed > 1 ? "s" : ""} failed to sync`, "error");
  },
  dismissToast: (id) => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })),
  confirmDialog: null,
  showConfirm: (msg, onConfirm, confirmLabel) => set({ confirmDialog: { msg, onConfirm, confirmLabel } }),
  dismissConfirm: () => set({ confirmDialog: null }),
  focusMode: false,
  toggleFocusMode: () => set(s => ({ focusMode: !s.focusMode })),

  setTab: (tab) => set({ activeTab: tab }),

  login: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/login", { username, password });
    await setToken(resp.token);
    platformSaveAuth(JSON.stringify(resp)).catch(() => {
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
    platformSaveAuth(JSON.stringify(resp)).catch(() => {
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
    platformClearAuth().catch(() => {});
    localStorage.removeItem("auth");
    // S2: Remove tokens for this server from savedServers
    const url = get().serverUrl;
    const username = get().username;
    if (url && username) {
      const servers = loadServers().filter(s => !(s.url === url && s.username === username));
      saveServers(servers);
      set({ savedServers: servers });
    }
    set({ token: null, username: null, role: null });
    platformSetToken("").catch(() => {});
    // PF8: Clear SW API cache on logout to prevent stale auth data leaking
    if (typeof caches !== 'undefined') caches.delete('pomo-v1').catch(() => {});
  },

  restoreAuth: async () => {
    // Restore server URL only — user must explicitly login.
    // Saved servers are shown as quick-switch buttons on the login screen.
    const url = localStorage.getItem("serverUrl");
    if (url) {
      set({ serverUrl: url });
      platformSetConnection(url);
    }
  },

  setServerUrl: async (url) => {
    const clean = url.replace(/\/+$/, "");
    localStorage.setItem("serverUrl", clean);
    await platformSetConnection(clean);
    set({ serverUrl: clean, token: null, username: null, role: null });
    localStorage.removeItem("auth");
  },

  switchToServer: async (server) => {
    set({ mutating: true }); // V32-20: Show loading during server switch
    localStorage.setItem("serverUrl", server.url);
    await platformSetConnection(server.url);
    await setToken(server.token);
    platformSaveAuth(JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role })).catch(() => {
      localStorage.setItem("auth", JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role }));
    });
    set({ serverUrl: server.url, token: server.token, username: server.username, role: server.role });
    // V30-19: Validate token — if expired, try refresh or force re-login
    try {
      await platformApiCall("GET", "/api/timer");
    } catch {
      set({ token: null, username: null, role: null });
      get().toast("Session expired — please log in again", "error");
    }
    set({ mutating: false }); // V32-20
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
      const resp = await apiCall<{ tasks: Task[]; task_sprints: TaskSprintInfo[]; burn_totals: BurnTotalEntry[]; assignees: TaskAssignee[]; labels?: { task_id: number; name: string; color: string }[] }>("GET", "/api/tasks/full");
      const burnTotals = new Map<number, BurnTotalEntry>();
      for (const bt of resp.burn_totals) burnTotals.set(bt.task_id, bt);
      const allAssignees = new Map<number, string[]>();
      for (const a of resp.assignees) {
        const list = allAssignees.get(a.task_id) || [];
        list.push(a.username);
        allAssignees.set(a.task_id, list);
      }
      // B7: Parse labels from /api/tasks/full to avoid N+1 calls
      const taskLabelsMap = new Map<number, { name: string; color: string }[]>();
      for (const l of resp.labels || []) {
        const list = taskLabelsMap.get(l.task_id) || [];
        list.push({ name: l.name, color: l.color });
        taskLabelsMap.set(l.task_id, list);
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
      const tasksChanged = prev.length !== resp.tasks.length || resp.tasks.some((t) => { const p = prev.find(p => p.id === t.id); return !p || p.updated_at !== t.updated_at; });
      // F10: Detect status changes on tasks assigned to current user
      if (tasksChanged && prev.length > 0) {
        const me = get().username;
        const prevMap = new Map(prev.map(t => [t.id, t.status]));
        for (const t of resp.tasks) {
          const oldStatus = prevMap.get(t.id);
          if (oldStatus && oldStatus !== t.status && t.user !== me) {
            const assigned = allAssignees.get(t.id);
            if (assigned?.some(a => a === me)) {
              get().toast(`"${t.title}" → ${t.status} (by ${t.user})`, "success");
            }
          }
        }
      }
      set({ tasks: tasksChanged ? resp.tasks : prev, taskSprints: ts, taskSprintsMap, burnTotals, allAssignees, taskLabelsMap, tasksLoadedAt: Date.now() });
      // F16: Cache tasks offline
      cacheTasksOffline(resp.tasks).catch(() => {});
    } catch {
      // F16: Fallback to offline cached tasks
      if (!navigator.onLine) {
        try {
          const offline = await getOfflineTasks() as Task[];
          if (offline.length > 0) {
            set({ tasks: offline, tasksLoadedAt: Date.now() });
            get().toast("Offline mode — showing cached tasks", "info");
          }
        } catch { /* ignore */ }
      }
    }
    set(s => ({ loading: { ...s.loading, tasks: false } }));
  },

  createTask: async (title, parentId, project, priority = 3, estimated = 1) => {
    set({ mutating: true });
    try {
      const task = await apiCall<Task>("POST", "/api/tasks", { title, parent_id: parentId, project, priority, estimated });
      if (task) set(s => ({ tasks: [...s.tasks, task] }));
      get().toast("Task created");
    } catch {
      // F16: Queue offline
      if (!navigator.onLine) {
        const base = get().serverUrl || "";
        await enqueueOfflineAction("POST", `${base}/api/tasks`, { title, parent_id: parentId, project, priority, estimated });
        get().toast("Offline — task queued for sync", "info");
      }
    } finally { set({ mutating: false }); }
  },

  updateTask: async (id, fields) => {
    set({ mutating: true });
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
      // F16: Queue offline
      if (!navigator.onLine) {
        const base = get().serverUrl || "";
        await enqueueOfflineAction("PUT", `${base}/api/tasks/${id}`, fields);
        set(s => ({ tasks: s.tasks.map(t => t.id === id ? { ...t, ...fields as Partial<Task> } : t) }));
        get().toast("Offline — update queued for sync", "info");
        return;
      }
      throw e;
    } finally { set({ mutating: false }); }
  },

  // Note: deleteTask shows confirmation dialog before deleting (UI concern in store for convenience)
  deleteTask: (id) => {
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
      }).catch(() => {});
    } else {
      set({ activeTeamId: null, teamScope: null });
    }
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

  // B7: Load sprints into store for Dashboard widgets
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

  addComment: async (taskId, content, sessionId) => {
    const body: Record<string, unknown> = { content };
    if (sessionId) body.session_id = sessionId;
    return apiCall<Comment>("POST", `/api/tasks/${taskId}/comments`, body);
  },

  getTaskDetail: async (id) => {
    return apiCall<TaskDetail>("GET", `/api/tasks/${id}`);
  },
}));

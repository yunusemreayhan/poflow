import type { StateCreator } from "zustand";
import { apiCall } from "./api";
import type { Task, Comment, TaskDetail, TaskSprintInfo, BurnTotalEntry, TaskAssignee } from "./api";
import { cacheTasksOffline, getOfflineTasks, enqueueOfflineAction } from "../offlineStore";

// Dedup guard
const _inflight = new Set<string>();
function dedup(key: string, fn: () => Promise<void>): Promise<void> {
  if (_inflight.has(key)) return Promise.resolve();
  _inflight.add(key);
  return fn().finally(() => _inflight.delete(key));
}

export interface TasksSlice {
  tasks: Task[];
  taskSprints: TaskSprintInfo[];
  taskSprintsMap: Map<number, TaskSprintInfo[]>;
  burnTotals: Map<number, BurnTotalEntry>;
  allAssignees: Map<number, string[]>;
  taskLabelsMap: Map<number, { name: string; color: string }[]>;
  tasksLoadedAt: number;
  loadTasks: () => Promise<void>;
  createTask: (title: string, parentId?: number, project?: string, priority?: number, estimated?: number) => Promise<void>;
  updateTask: (id: number, fields: Record<string, unknown>) => Promise<void>;
  deleteTask: (id: number) => void;
  setActiveTeam: (teamId: number | null) => void;
  addComment: (taskId: number, content: string, sessionId?: number) => Promise<Comment>;
  getTaskDetail: (id: number) => Promise<TaskDetail>;
}

export const createTasksSlice: StateCreator<
  TasksSlice & { token: string | null; username: string | null; mutating: boolean; loading: Record<string, boolean>; activeTeamId: number | null; teamScope: Set<number> | null; toast: (msg: string, type?: "success" | "error" | "info") => void; showConfirm: (msg: string, onConfirm: () => void) => void },
  [], [], TasksSlice
> = (set, get) => ({
  tasks: [],
  taskSprints: [],
  taskSprintsMap: new Map(),
  burnTotals: new Map(),
  allAssignees: new Map(),
  taskLabelsMap: new Map(),
  tasksLoadedAt: 0,

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
      const prev = get().tasks;
      const tasksChanged = prev.length !== resp.tasks.length || resp.tasks.some((t) => { const p = prev.find(p => p.id === t.id); return !p || p.updated_at !== t.updated_at; });
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
      cacheTasksOffline(resp.tasks).catch(e => console.error("Cache tasks offline:", e));
    } catch {
      if (!navigator.onLine) {
        try {
          const offline = await getOfflineTasks();
          if (offline.length > 0) {
            set({ tasks: offline, tasksLoadedAt: Date.now() });
            get().toast("Offline mode — showing cached tasks", "info");
          }
        } catch { /* ignore */ }
      }
    }
    set(s => ({ loading: { ...s.loading, tasks: false } }));
  },

  createTask: async (title, parentId, project, priority = 3, estimated = 1) => dedup("task:create", async () => {
    set({ mutating: true } as any);
    try {
      const task = await apiCall<Task>("POST", "/api/tasks", { title, parent_id: parentId, project, priority, estimated });
      if (task) set(s => ({ tasks: [...s.tasks, task] }));
      get().toast("Task created");
    } catch {
      if (!navigator.onLine) {
        await enqueueOfflineAction("POST", "/api/tasks", { title, parent_id: parentId, project, priority, estimated });
        get().toast("Offline — task queued for sync", "info");
      }
    } finally { set({ mutating: false } as any); }
  }),

  updateTask: async (id, fields) => {
    set({ mutating: true } as any);
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
      if (!navigator.onLine) {
        await enqueueOfflineAction("PUT", `/api/tasks/${id}`, fields);
        set(s => ({ tasks: s.tasks.map(t => t.id === id ? { ...t, ...fields as Partial<Task> } : t) }));
        get().toast("Offline — update queued for sync", "info");
        return;
      }
      throw e;
    } finally { set({ mutating: false } as any); }
  },

  deleteTask: (id) => {
    const task = get().tasks.find(t => t.id === id);
    get().showConfirm("Delete this task and all subtasks?", async () => {
      await apiCall("DELETE", `/api/tasks/${id}`);
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
        set({ activeTeamId: teamId, teamScope: ids && ids.length > 0 ? new Set(ids) : new Set() } as any);
      }).catch(e => console.error("Load team scope:", e));
    } else {
      set({ activeTeamId: null, teamScope: null } as any);
    }
  },

  addComment: async (taskId, content, sessionId) => {
    const body: Record<string, unknown> = { content };
    if (sessionId) body.session_id = sessionId;
    return apiCall<Comment>("POST", `/api/tasks/${taskId}/comments`, body);
  },

  getTaskDetail: async (id) => apiCall<TaskDetail>("GET", `/api/tasks/${id}`),
});

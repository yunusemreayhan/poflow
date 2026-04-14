import { describe, it, expect, beforeEach, vi } from "vitest";

// Mock localStorage globally before any imports
const storage: Record<string, string> = {};
vi.stubGlobal("localStorage", {
  getItem: (k: string) => storage[k] ?? null,
  setItem: (k: string, v: string) => { storage[k] = v; },
  removeItem: (k: string) => { delete storage[k]; },
});

// Mock Tauri invoke — must be before store import
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => null),
}));

const mockApiCall = vi.fn(async (method: string, path: string, body?: unknown) => {
  if (method === "POST" && path === "/api/auth/login") {
    return { token: "test-token", refresh_token: "test-refresh", user_id: 1, username: "testuser", role: "user" };
  }
  if (method === "GET" && path === "/api/timer") {
    return { phase: "Idle", status: "Idle", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: null, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
  }
  if (method === "POST" && path === "/api/tasks") {
    return { id: 99, parent_id: null, user_id: 1, user: "testuser", title: (body as any)?.title || "T", description: null, project: null, tags: null, priority: 3, estimated: 1, actual: 0, estimated_hours: 0, remaining_points: 0, due_date: null, status: "backlog", sort_order: 0, created_at: "", updated_at: "" };
  }
  if (method === "GET" && path.startsWith("/api/tasks/full")) {
    return { tasks: [], task_sprints: [], burn_totals: [], assignees: [] };
  }
  return null;
});

vi.mock("../platform", () => ({
  isTauri: false,
  platformApiCall: (m: string, p: string, b?: unknown) => mockApiCall(m, p, b),
  platformSetToken: vi.fn(async () => {}),
  platformSaveAuth: vi.fn(async () => {}),
  platformClearAuth: vi.fn(async () => {}),
  platformSetConnection: vi.fn(async () => {}),
}));

import { useStore } from "../store/store";

describe("store actions", () => {
  beforeEach(() => {
    useStore.setState({
      token: null, username: null, role: null, tasks: [], engine: null,
      toasts: [], confirmDialog: null, activeTab: "timer",
    });
  });

  it("login sets token and username", async () => {
    await useStore.getState().login("testuser", "Password1");
    const { token, username, role } = useStore.getState();
    expect(token).toBe("test-token");
    expect(username).toBe("testuser");
    expect(role).toBe("user");
  });

  it("logout clears auth state", async () => {
    useStore.setState({ token: "old", username: "old", role: "user" });
    useStore.getState().logout();
    const { token, username, role } = useStore.getState();
    expect(token).toBeNull();
    expect(username).toBeNull();
    expect(role).toBeNull();
  });

  it("setTab changes active tab", () => {
    useStore.getState().setTab("tasks");
    expect(useStore.getState().activeTab).toBe("tasks");
  });

  it("toast adds and auto-removes", async () => {
    useStore.getState().toast("hello", "success");
    expect(useStore.getState().toasts.length).toBe(1);
    expect(useStore.getState().toasts[0].msg).toBe("hello");
  });

  it("dismissToast removes specific toast", () => {
    useStore.getState().toast("a");
    useStore.getState().toast("b");
    const id = useStore.getState().toasts[0].id;
    useStore.getState().dismissToast(id);
    expect(useStore.getState().toasts.every(t => t.id !== id)).toBe(true);
  });

  it("showConfirm and dismissConfirm", () => {
    const fn = vi.fn();
    useStore.getState().showConfirm("Delete?", fn);
    expect(useStore.getState().confirmDialog?.msg).toBe("Delete?");
    useStore.getState().dismissConfirm();
    expect(useStore.getState().confirmDialog).toBeNull();
  });

  it("createTask adds task to local state", async () => {
    useStore.setState({ token: "t", tasks: [] });
    await useStore.getState().createTask("New Task");
    expect(useStore.getState().tasks.length).toBe(1);
    expect(useStore.getState().tasks[0].id).toBe(99);
  });

  it("poll updates engine state", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().poll();
    expect(useStore.getState().engine?.phase).toBe("Idle");
    expect(useStore.getState().connected).toBe(true);
  });
});

describe("store - taskSprintsMap", () => {
  it("builds taskSprintsMap from task_sprints", () => {
    useStore.setState({
      taskSprints: [
        { task_id: 1, sprint_id: 10, sprint_name: "S1", sprint_status: "active" },
        { task_id: 1, sprint_id: 20, sprint_name: "S2", sprint_status: "active" },
        { task_id: 2, sprint_id: 10, sprint_name: "S1", sprint_status: "active" },
      ],
      taskSprintsMap: new Map([
        [1, [{ task_id: 1, sprint_id: 10, sprint_name: "S1", sprint_status: "active" }, { task_id: 1, sprint_id: 20, sprint_name: "S2", sprint_status: "active" }]],
        [2, [{ task_id: 2, sprint_id: 10, sprint_name: "S1", sprint_status: "active" }]],
      ]),
    });
    const map = useStore.getState().taskSprintsMap;
    expect(map.get(1)?.length).toBe(2);
    expect(map.get(2)?.length).toBe(1);
    expect(map.get(999)).toBeUndefined();
  });
});

import { describe, it, expect, beforeEach, vi } from "vitest";

// Mock localStorage
const storage: Record<string, string> = {};
vi.stubGlobal("localStorage", {
  getItem: (k: string) => storage[k] ?? null,
  setItem: (k: string, v: string) => { storage[k] = v; },
  removeItem: (k: string) => { delete storage[k]; },
});

// Mock Tauri invoke
const mockApiCall = vi.fn(async (method: string, path: string, body?: unknown) => {
    if (method === "POST" && path === "/api/auth/login") {
      return { token: "test-token", refresh_token: "test-refresh", user_id: 1, username: "testuser", role: "user" };
    }
    if (method === "POST" && path === "/api/auth/register") {
      return { token: "reg-token", refresh_token: "reg-refresh", user_id: 2, username: (body as any)?.username || "newuser", role: "user" };
    }
    if (method === "GET" && path === "/api/timer") {
      return { phase: "Idle", status: "Idle", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: null, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
    }
    if (method === "POST" && path === "/api/timer/start") {
      return { phase: "Work", status: "Running", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
    }
    if (method === "POST" && path === "/api/timer/pause") {
      return { phase: "Work", status: "Paused", elapsed_s: 100, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
    }
    if (method === "POST" && path === "/api/timer/resume") {
      return { phase: "Work", status: "Running", elapsed_s: 100, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
    }
    if (method === "POST" && path === "/api/timer/stop") {
      return { phase: "Idle", status: "Idle", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: null, current_user_id: 1, daily_completed: 0, daily_goal: 8 };
    }
    if (method === "POST" && path === "/api/timer/skip") {
      return { phase: "ShortBreak", status: "Idle", elapsed_s: 0, duration_s: 300, session_count: 1, current_task_id: null, current_session_id: null, current_user_id: 1, daily_completed: 1, daily_goal: 8 };
    }
    if (method === "GET" && path.startsWith("/api/tasks/full")) {
      return { tasks: [], task_sprints: [], burn_totals: [], assignees: [] };
    }
    if (method === "GET" && path === "/api/config") {
      return { work_duration_min: 25, short_break_min: 5, long_break_min: 15, long_break_interval: 4, auto_start_breaks: false, auto_start_work: false, sound_enabled: true, notification_enabled: true, daily_goal: 8, estimation_mode: "hours", leaf_only_mode: false, theme: "dark" };
    }
    if (method === "POST" && path === "/api/tasks") {
      return { id: 99, parent_id: null, user_id: 1, user: "testuser", title: (body as any)?.title || "T", description: null, project: null, tags: null, priority: 3, estimated: 1, actual: 0, estimated_hours: 0, remaining_points: 0, due_date: null, status: "backlog", sort_order: 0, created_at: "", updated_at: "", attachment_count: 0 };
    }
    return null;
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

vi.mock("../platform", () => ({
  isTauri: false,
  platformApiCall: (...args: any[]) => mockApiCall(...args),
  platformSetToken: vi.fn(async () => {}),
  platformSaveAuth: vi.fn(async () => {}),
  platformClearAuth: vi.fn(async () => {}),
  platformSetConnection: vi.fn(async () => {}),
}));

const { useStore } = await import("../store/store");

beforeEach(() => {
  useStore.setState({
    token: null, username: null, role: null, engine: null,
    tasks: [], connected: false, error: null,
    toasts: [], confirmDialog: null,
  });
  mockApiCall.mockClear();
});

describe("store - auth", () => {
  it("login sets token and username", async () => {
    await useStore.getState().login("testuser", "Pass1234");
    const s = useStore.getState();
    expect(s.token).toBe("test-token");
    expect(s.username).toBe("testuser");
    expect(s.role).toBe("user");
  });

  it("register sets token and username", async () => {
    await useStore.getState().register("newuser", "Pass1234");
    const s = useStore.getState();
    expect(s.token).toBe("reg-token");
    expect(s.username).toBe("newuser");
  });

  it("logout clears auth state", async () => {
    await useStore.getState().login("testuser", "Pass1234");
    useStore.getState().logout();
    const s = useStore.getState();
    expect(s.token).toBeNull();
    expect(s.username).toBeNull();
  });
});

describe("store - timer actions", () => {
  it("start sets engine to Running", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().start();
    expect(useStore.getState().engine?.status).toBe("Running");
    expect(useStore.getState().engine?.phase).toBe("Work");
  });

  it("pause sets engine to Paused", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().pause();
    expect(useStore.getState().engine?.status).toBe("Paused");
  });

  it("resume sets engine to Running", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().resume();
    expect(useStore.getState().engine?.status).toBe("Running");
  });

  it("stop sets engine to Idle", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().stop();
    expect(useStore.getState().engine?.status).toBe("Idle");
  });

  it("skip advances phase", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().skip();
    expect(useStore.getState().engine?.phase).toBe("ShortBreak");
  });
});

describe("store - tasks", () => {
  it("createTask adds to tasks array", async () => {
    useStore.setState({ token: "t", tasks: [] });
    await useStore.getState().createTask("New Task");
    expect(useStore.getState().tasks.length).toBe(1);
    expect(useStore.getState().tasks[0].title).toBe("New Task");
  });

  it("poll updates engine and connected state", async () => {
    useStore.setState({ token: "t" });
    await useStore.getState().poll();
    expect(useStore.getState().engine?.phase).toBe("Idle");
    expect(useStore.getState().connected).toBe(true);
  });
});

describe("store - toast", () => {
  it("toast adds notification", () => {
    useStore.getState().toast("Hello", "success");
    expect(useStore.getState().toasts.length).toBe(1);
    expect(useStore.getState().toasts[0].msg).toBe("Hello");
    expect(useStore.getState().toasts[0].type).toBe("success");
  });

  it("dismissToast removes notification", () => {
    useStore.getState().toast("A");
    const id = useStore.getState().toasts[0].id;
    useStore.getState().dismissToast(id);
    expect(useStore.getState().toasts.length).toBe(0);
  });

  it("error toast", () => {
    useStore.getState().toast("Error!", "error");
    expect(useStore.getState().toasts[0].type).toBe("error");
  });
});

describe("store - confirm dialog", () => {
  it("showConfirm sets dialog", () => {
    const fn = vi.fn();
    useStore.getState().showConfirm("Are you sure?", fn);
    expect(useStore.getState().confirmDialog).not.toBeNull();
    expect(useStore.getState().confirmDialog?.msg).toBe("Are you sure?");
  });

  it("dismissConfirm clears dialog", () => {
    useStore.getState().showConfirm("Test", () => {});
    useStore.getState().dismissConfirm();
    expect(useStore.getState().confirmDialog).toBeNull();
  });
});

describe("store - tab navigation", () => {
  it("setTab changes active tab", () => {
    useStore.getState().setTab("tasks");
    expect(useStore.getState().activeTab).toBe("tasks");
    useStore.getState().setTab("timer");
    expect(useStore.getState().activeTab).toBe("timer");
  });
});

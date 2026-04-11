import { describe, it, expect } from "vitest";
import { buildTree, countDescendants } from "../tree";
import { computeRollup } from "../rollup";
import type { Task, TaskDetail } from "../store/api";

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: 1, parent_id: null, user_id: 1, user: "root", title: "T", description: null,
    project: null, tags: null, priority: 3, estimated: 1, actual: 0,
    estimated_hours: 0, remaining_points: 0, due_date: null, status: "backlog",
    sort_order: 1, created_at: "", updated_at: "", ...overrides,
  };
}

// --- buildTree ---

describe("buildTree", () => {
  it("returns empty for empty input", () => {
    expect(buildTree([])).toEqual([]);
  });

  it("builds flat list as roots", () => {
    const tasks = [makeTask({ id: 1 }), makeTask({ id: 2 })];
    const tree = buildTree(tasks);
    expect(tree.length).toBe(2);
    expect(tree[0].children.length).toBe(0);
  });

  it("nests children under parent", () => {
    const tasks = [
      makeTask({ id: 1 }),
      makeTask({ id: 2, parent_id: 1 }),
      makeTask({ id: 3, parent_id: 1 }),
    ];
    const tree = buildTree(tasks);
    expect(tree.length).toBe(1);
    expect(tree[0].task.id).toBe(1);
    expect(tree[0].children.length).toBe(2);
  });

  it("handles deep nesting", () => {
    const tasks = [
      makeTask({ id: 1 }),
      makeTask({ id: 2, parent_id: 1 }),
      makeTask({ id: 3, parent_id: 2 }),
    ];
    const tree = buildTree(tasks);
    expect(tree.length).toBe(1);
    expect(tree[0].children[0].children[0].task.id).toBe(3);
  });

  it("orphans become roots", () => {
    const tasks = [makeTask({ id: 2, parent_id: 999 })];
    const tree = buildTree(tasks);
    expect(tree.length).toBe(1);
  });
});

// --- countDescendants ---

describe("countDescendants", () => {
  it("returns 0 for leaf", () => {
    expect(countDescendants({ task: makeTask(), children: [] })).toBe(0);
  });

  it("counts all levels", () => {
    const node = {
      task: makeTask({ id: 1 }),
      children: [
        { task: makeTask({ id: 2 }), children: [
          { task: makeTask({ id: 3 }), children: [] },
        ]},
        { task: makeTask({ id: 4 }), children: [] },
      ],
    };
    expect(countDescendants(node)).toBe(3);
  });
});

// --- computeRollup ---

describe("computeRollup", () => {
  function makeDetail(overrides: Partial<TaskDetail> = {}): TaskDetail {
    return {
      task: makeTask(),
      comments: [],
      sessions: [],
      children: [],
      ...overrides,
    };
  }

  it("returns zeros for empty task", () => {
    const r = computeRollup(makeDetail(), new Map());
    expect(r.ownEstHours).toBe(0);
    expect(r.ownSpentHours).toBe(0);
    expect(r.totalSessionSecs).toBe(0);
    expect(r.progressHours).toBeNull();
  });

  it("computes own hours from hoursMap", () => {
    const d = makeDetail({ task: makeTask({ id: 5, estimated_hours: 10 }) });
    const map = new Map([[5, 4]]);
    const r = computeRollup(d, map);
    expect(r.ownSpentHours).toBe(4);
    expect(r.progressHours).toBe(40);
  });

  it("computes session seconds", () => {
    const d = makeDetail({
      sessions: [
        { id: 1, task_id: 1, user_id: 1, user: "root", session_type: "work", status: "completed", started_at: "", ended_at: null, duration_s: 1500, notes: null },
        { id: 2, task_id: 1, user_id: 1, user: "root", session_type: "work", status: "completed", started_at: "", ended_at: null, duration_s: 1500, notes: null },
      ],
    });
    const r = computeRollup(d, new Map());
    expect(r.ownSessionSecs).toBe(3000);
  });

  it("rolls up children", () => {
    const child = makeDetail({
      task: makeTask({ id: 2, estimated_hours: 5, estimated: 3, remaining_points: 1 }),
      sessions: [{ id: 1, task_id: 2, user_id: 1, user: "root", session_type: "work", status: "completed", started_at: "", ended_at: null, duration_s: 600, notes: null }],
    });
    const parent = makeDetail({
      task: makeTask({ id: 1, estimated_hours: 10, estimated: 5, remaining_points: 2 }),
      children: [child],
    });
    const map = new Map([[1, 3], [2, 2]]);
    const r = computeRollup(parent, map);
    expect(r.totalEstHours).toBe(15);
    expect(r.totalSpentHours).toBe(5);
    expect(r.childEstHours).toBe(5);
    expect(r.childSpentHours).toBe(2);
    expect(r.totalSessionSecs).toBe(600);
    expect(r.totalEstPoints).toBe(8);
    expect(r.totalRemPoints).toBe(3);
  });

  it("computes points progress", () => {
    const d = makeDetail({ task: makeTask({ estimated: 10, remaining_points: 3 }) });
    const r = computeRollup(d, new Map());
    expect(r.progressPoints).toBe(70);
  });

  it("caps progress at 100", () => {
    const d = makeDetail({ task: makeTask({ estimated_hours: 5 }) });
    const map = new Map([[1, 10]]);
    const r = computeRollup(d, map);
    expect(r.progressHours).toBe(100);
  });
});

// --- Heatmap filter test ---

describe("heatmap user filter", () => {
  it("filteredStats should only include selected user sessions", () => {
    // Simulate the History component's filteredStats logic
    const history = [
      { user: "alice", session_type: "work", status: "completed", started_at: "2026-04-10T10:00:00", duration_s: 1500 },
      { user: "bob", session_type: "work", status: "completed", started_at: "2026-04-10T11:00:00", duration_s: 1500 },
      { user: "alice", session_type: "work", status: "completed", started_at: "2026-04-10T12:00:00", duration_s: 1500 },
    ];
    const userFilter = "alice";
    const filtered = history.filter(s => s.user === userFilter);
    const map = new Map<string, { date: string; completed: number }>();
    for (const s of filtered) {
      if (s.session_type !== "work") continue;
      const date = s.started_at.slice(0, 10);
      const entry = map.get(date) ?? { date, completed: 0 };
      if (s.status === "completed") entry.completed += 1;
      map.set(date, entry);
    }
    const stats = Array.from(map.values());
    expect(stats.length).toBe(1);
    expect(stats[0].completed).toBe(2); // only alice's 2 sessions
  });
});

// --- XML escape test ---

describe("XML export sanitization", () => {
  it("escapes special characters in XML content", () => {
    const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
    expect(esc('Hello <script>alert("xss")</script>')).toBe('Hello &lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;');
    expect(esc("A & B")).toBe("A &amp; B");
    expect(esc("normal text")).toBe("normal text");
  });
});

// --- matchSearch ---

import { matchSearch } from "../utils";

describe("matchSearch", () => {
  it("matches text case-insensitively", () => {
    expect(matchSearch("Fix Login Bug", "login")).toBe(true);
    expect(matchSearch("Fix Login Bug", "LOGIN")).toBe(true);
    expect(matchSearch("Fix Login Bug", "fix")).toBe(true);
  });

  it("matches with regex", () => {
    expect(matchSearch("backend-api", "back.*api")).toBe(true);
  });

  it("returns false for no match", () => {
    expect(matchSearch("Hello World", "xyz")).toBe(false);
  });

  it("returns true for empty query", () => {
    expect(matchSearch("anything", "")).toBe(true);
  });

  it("handles invalid regex gracefully", () => {
    expect(matchSearch("test [bracket", "[bracket")).toBe(true);
  });
});

// --- countDescendants ---

describe("countDescendants", () => {
  it("returns 0 for leaf node", () => {
    const tree = buildTree([makeTask({ id: 1 })]);
    expect(countDescendants(tree[0])).toBe(0);
  });

  it("counts nested descendants", () => {
    const tasks = [
      makeTask({ id: 1 }),
      makeTask({ id: 2, parent_id: 1 }),
      makeTask({ id: 3, parent_id: 1 }),
      makeTask({ id: 4, parent_id: 2 }),
    ];
    const tree = buildTree(tasks);
    expect(countDescendants(tree[0])).toBe(3);
  });
});

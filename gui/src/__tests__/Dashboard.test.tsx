import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";

const storage: Record<string, string> = {};
vi.stubGlobal("localStorage", {
  getItem: (k: string) => storage[k] ?? null,
  setItem: (k: string, v: string) => { storage[k] = v; },
  removeItem: (k: string) => { delete storage[k]; },
});
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("../platform", () => ({
  isTauri: false,
  platformApiCall: vi.fn(async () => null),
  platformSetToken: vi.fn(async () => {}),
  platformSaveAuth: vi.fn(async () => {}),
  platformClearAuth: vi.fn(async () => {}),
  platformSetConnection: vi.fn(async () => {}),
}));

import { useStore } from "../store/store";
import Dashboard from "../components/Dashboard";
import type { Task, DayStat } from "../store/types";

function makeTask(overrides: Partial<Task> & { id: number; title: string }): Task {
  return {
    parent_id: null, user_id: 1, user: "alice", description: null, project: null, tags: null,
    priority: 3, estimated: 1, actual: 0, estimated_hours: 0, remaining_points: 0, due_date: null,
    status: "active", sort_order: 0, created_at: "2026-01-01T00:00:00", updated_at: "2026-01-01T00:00:00",
    attachment_count: 0, deleted_at: null, work_duration_minutes: null, estimate_optimistic: null, estimate_pessimistic: null,
    ...overrides,
  };
}

describe("Dashboard component", () => {
  beforeEach(() => {
    useStore.setState({
      tasks: [], stats: [], sprints: [],
      token: "t", username: "alice", role: "user",
      taskSprintsMap: new Map(),
      loading: { tasks: false },
      loadSprints: vi.fn(),
    });
  });

  it("renders stats cards", () => {
    render(<Dashboard />);
    expect(screen.getByText("Focus today")).toBeInTheDocument();
    expect(screen.getByText("Sessions")).toBeInTheDocument();
    expect(screen.getByText("Active tasks")).toBeInTheDocument();
    expect(screen.getByText("Completed today")).toBeInTheDocument();
  });

  it("shows 0m focus when no stats", () => {
    render(<Dashboard />);
    expect(screen.getByText("0m")).toBeInTheDocument();
  });

  it("shows active task count", () => {
    useStore.setState({
      tasks: [
        makeTask({ id: 1, title: "A", status: "active" }),
        makeTask({ id: 2, title: "B", status: "active" }),
        makeTask({ id: 3, title: "C", status: "completed" }),
      ],
    });
    render(<Dashboard />);
    expect(screen.getByText("2")).toBeInTheDocument(); // 2 active tasks
  });

  it("shows recently updated tasks", () => {
    useStore.setState({
      tasks: [makeTask({ id: 1, title: "Recent Task", updated_at: "2026-04-19T01:00:00" })],
    });
    render(<Dashboard />);
    expect(screen.getByText(/Recently Updated/)).toBeInTheDocument();
    expect(screen.getAllByText(/Recent Task/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows overdue warning when tasks are overdue", () => {
    const yesterday = new Date(Date.now() - 86400000).toISOString().slice(0, 10);
    useStore.setState({
      tasks: [makeTask({ id: 1, title: "Overdue Task", due_date: yesterday, status: "active" })],
    });
    render(<Dashboard />);
    expect(screen.getAllByText(/Overdue/).length).toBeGreaterThanOrEqual(1);
  });

  it("hides overdue section when no overdue tasks", () => {
    useStore.setState({ tasks: [makeTask({ id: 1, title: "On Time" })] });
    render(<Dashboard />);
    expect(screen.queryByText(/Overdue/)).not.toBeInTheDocument();
  });

  it("shows weekly sparkline when stats exist", () => {
    const stats: DayStat[] = [
      { date: "2026-04-18", completed: 3, interrupted: 0, total_focus_s: 5400 },
      { date: "2026-04-19", completed: 2, interrupted: 1, total_focus_s: 3600 },
    ];
    useStore.setState({ stats });
    render(<Dashboard />);
    expect(screen.getByText(/Last 2 days/)).toBeInTheDocument();
  });

  it("has copy as markdown button", () => {
    render(<Dashboard />);
    expect(screen.getByTitle("Copy as Markdown")).toBeInTheDocument();
  });
});

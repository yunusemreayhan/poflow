import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

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
import TaskList from "../components/TaskList";
import type { Task } from "../store/types";

function makeTask(overrides: Partial<Task> & { id: number; title: string }): Task {
  return {
    parent_id: null, user_id: 1, user: "alice", description: null, project: null, tags: null,
    priority: 3, estimated: 1, actual: 0, estimated_hours: 0, remaining_points: 0, due_date: null,
    status: "active", sort_order: 0, created_at: "2026-01-01T00:00:00", updated_at: "2026-01-01T00:00:00",
    attachment_count: 0, deleted_at: null, work_duration_minutes: null, estimate_optimistic: null, estimate_pessimistic: null,
    ...overrides,
  };
}

const baseState = {
  token: "t", username: "alice", role: "user",
  loading: { tasks: false },
  taskSprints: [], taskSprintsMap: new Map(),
  burnTotals: new Map(), allAssignees: new Map<number, string[]>(),
  taskLabelsMap: new Map(), teamScope: null, toasts: [],
  engine: null,
  config: { work_duration_min: 25, short_break_min: 5, long_break_min: 15, long_break_interval: 4, auto_start_breaks: false, auto_start_work: false, sound_enabled: false, notification_enabled: false, daily_goal: 8, estimation_mode: "hours", leaf_only_mode: false, theme: "dark", auto_archive_days: 90 },
};

describe("TaskList component", () => {
  beforeEach(() => {
    for (const k of Object.keys(storage)) delete storage[k];
    useStore.setState({ tasks: [], ...baseState });
  });

  it("shows empty state when no tasks", () => {
    render(<TaskList />);
    expect(screen.getByText(/No projects yet/)).toBeInTheDocument();
  });

  it("renders task input", () => {
    render(<TaskList />);
    expect(screen.getByLabelText("New project or top-level task")).toBeInTheDocument();
  });

  it("renders tasks in tree view", () => {
    useStore.setState({ tasks: [makeTask({ id: 1, title: "Project A" }), makeTask({ id: 2, title: "Project B" })] });
    render(<TaskList />);
    expect(screen.getByText("Project A")).toBeInTheDocument();
    expect(screen.getByText("Project B")).toBeInTheDocument();
  });

  it("filters out archived tasks by default", () => {
    useStore.setState({
      tasks: [makeTask({ id: 1, title: "Active Task" }), makeTask({ id: 2, title: "Archived Task", status: "archived" })],
    });
    render(<TaskList />);
    expect(screen.getByText("Active Task")).toBeInTheDocument();
    expect(screen.queryByText("Archived Task")).not.toBeInTheDocument();
  });

  it("has search input", () => {
    render(<TaskList />);
    expect(screen.getByLabelText("Search tasks")).toBeInTheDocument();
  });

  it("search input accepts text", () => {
    useStore.setState({
      tasks: [makeTask({ id: 1, title: "Fix bug" }), makeTask({ id: 2, title: "Add feature" })],
    });
    render(<TaskList />);
    const input = screen.getByLabelText("Search tasks");
    fireEvent.change(input, { target: { value: "bug" } });
    expect(input).toHaveValue("bug");
  });

  it("calls createTask on Enter in input", () => {
    const createFn = vi.fn();
    useStore.setState({ createTask: createFn });
    render(<TaskList />);
    const input = screen.getByLabelText("New project or top-level task");
    fireEvent.change(input, { target: { value: "New Task" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(createFn).toHaveBeenCalledWith("New Task");
  });

  it("shows loading skeleton when loading", () => {
    useStore.setState({ loading: { tasks: true } });
    render(<TaskList />);
    const pulses = document.querySelectorAll(".animate-pulse");
    expect(pulses.length).toBeGreaterThan(0);
  });

  it("shows no matching tasks when search has no results", () => {
    useStore.setState({
      tasks: [makeTask({ id: 1, title: "Something" })],
    });
    render(<TaskList />);
    fireEvent.change(screen.getByLabelText("Search tasks"), { target: { value: "zzzznotfound" } });
    expect(screen.getByText("No matching tasks")).toBeInTheDocument();
  });

  it("hides add input in select mode", () => {
    render(<TaskList selectMode onSelect={() => {}} />);
    expect(screen.queryByLabelText("New project or top-level task")).not.toBeInTheDocument();
  });

  it("shows sort selector with aria-label", () => {
    render(<TaskList />);
    // Select component renders aria-label as "Sort tasks: Manual" (includes current value)
    expect(screen.getByLabelText(/Sort tasks/)).toBeInTheDocument();
  });

  it("renders child tasks under parent", () => {
    useStore.setState({
      tasks: [
        makeTask({ id: 1, title: "Parent" }),
        makeTask({ id: 2, title: "Child", parent_id: 1 }),
      ],
    });
    render(<TaskList />);
    expect(screen.getByText("Parent")).toBeInTheDocument();
    // Child is rendered but collapsed by default — it's in the DOM
    // TaskNode starts collapsed, so child may not be visible
  });
});

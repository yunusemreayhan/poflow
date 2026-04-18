import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock localStorage
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

// Stub Audio and Notification
vi.stubGlobal("Audio", class { play() { return Promise.resolve(); } });
vi.stubGlobal("Notification", { permission: "denied", requestPermission: vi.fn() });

import { useStore } from "../store/store";
import Timer from "../components/Timer";

describe("Timer component", () => {
  beforeEach(() => {
    useStore.setState({
      engine: null,
      tasks: [],
      config: { work_duration_min: 25, short_break_min: 5, long_break_min: 15, long_break_interval: 4, auto_start_breaks: false, auto_start_work: false, sound_enabled: false, notification_enabled: false, daily_goal: 8, estimation_mode: "hours", leaf_only_mode: false, theme: "dark", auto_archive_days: 90 },
      timerTaskId: undefined,
      token: "t",
      username: "user",
      toasts: [],
    });
  });

  it("renders idle state with start button", () => {
    render(<Timer />);
    expect(screen.getByText(/start/i)).toBeInTheDocument();
    expect(screen.getByText("25:00")).toBeInTheDocument();
  });

  it("shows phase label for idle", () => {
    render(<Timer />);
    expect(screen.getByText("IDLE")).toBeInTheDocument();
  });

  it("shows work phase when running", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 60, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
    });
    render(<Timer />);
    expect(screen.getByText("WORK")).toBeInTheDocument();
    expect(screen.getByText("24:00")).toBeInTheDocument();
  });

  it("shows pause/stop/skip buttons when running", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
    });
    render(<Timer />);
    expect(screen.getByLabelText("Pause")).toBeInTheDocument();
    expect(screen.getByLabelText("Stop")).toBeInTheDocument();
    expect(screen.getByLabelText("Skip")).toBeInTheDocument();
  });

  it("shows resume/stop buttons when paused", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Paused", elapsed_s: 300, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
    });
    render(<Timer />);
    expect(screen.getByText(/resume/i)).toBeInTheDocument();
  });

  it("shows break shortcuts when idle", () => {
    render(<Timer />);
    expect(screen.getByText(/Short Break/)).toBeInTheDocument();
    expect(screen.getByText(/Long Break/)).toBeInTheDocument();
  });

  it("hides break shortcuts when running", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
    });
    render(<Timer />);
    expect(screen.queryByText(/Short Break/)).not.toBeInTheDocument();
  });

  it("shows daily goal dots", () => {
    useStore.setState({
      engine: { phase: "Idle", status: "Idle", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: null, current_user_id: 1, daily_completed: 3, daily_goal: 8 },
    });
    render(<Timer />);
    expect(screen.getByLabelText("3 of 8 daily goal completed")).toBeInTheDocument();
  });

  it("shows current task when active", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: 1, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
      tasks: [{ id: 1, parent_id: null, user_id: 1, user: "u", title: "My Task", description: null, project: "Proj", tags: null, priority: 3, estimated: 2, actual: 1, estimated_hours: 0, remaining_points: 0, due_date: null, status: "active", sort_order: 0, created_at: "", updated_at: "", attachment_count: 0, deleted_at: null, work_duration_minutes: null, estimate_optimistic: null, estimate_pessimistic: null }],
    });
    render(<Timer />);
    expect(screen.getAllByText(/My Task/).length).toBeGreaterThanOrEqual(1);
  });

  it("has accessible progressbar", () => {
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 300, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
    });
    render(<Timer />);
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "300");
    expect(bar).toHaveAttribute("aria-valuemax", "1500");
  });

  it("calls start when start button clicked", () => {
    const startFn = vi.fn();
    useStore.setState({ start: startFn });
    render(<Timer />);
    fireEvent.click(screen.getByText(/start/i));
    expect(startFn).toHaveBeenCalled();
  });

  it("calls pause when pause button clicked", () => {
    const pauseFn = vi.fn();
    useStore.setState({
      engine: { phase: "Work", status: "Running", elapsed_s: 0, duration_s: 1500, session_count: 0, current_task_id: null, current_session_id: 1, current_user_id: 1, daily_completed: 0, daily_goal: 8 },
      pause: pauseFn,
    });
    render(<Timer />);
    fireEvent.click(screen.getByLabelText("Pause"));
    expect(pauseFn).toHaveBeenCalled();
  });
});

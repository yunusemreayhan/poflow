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
  platformIndicatorStatus: vi.fn(async () => false),
  platformIndicatorToggle: vi.fn(async () => false),
}));

import { useStore } from "../store/store";
import Settings from "../components/Settings";

const defaultConfig = {
  work_duration_min: 25, short_break_min: 5, long_break_min: 15, long_break_interval: 4,
  auto_start_breaks: false, auto_start_work: false, sound_enabled: true, notification_enabled: true,
  daily_goal: 8, estimation_mode: "hours", leaf_only_mode: false, theme: "dark", auto_archive_days: 90,
};

describe("Settings component", () => {
  beforeEach(() => {
    useStore.setState({
      config: defaultConfig,
      token: "t", username: "alice", role: "user",
      serverUrl: "http://localhost:9090",
      loadConfig: vi.fn(),
      updateConfig: vi.fn(),
      setServerUrl: vi.fn(),
      toasts: [],
    });
  });

  it("renders timer duration fields", () => {
    render(<Settings />);
    // NumInput uses aria-label prop
    expect(screen.getByLabelText("Work duration (minutes)")).toBeInTheDocument();
    expect(screen.getByLabelText("Short break (minutes)")).toBeInTheDocument();
    expect(screen.getByLabelText("Long break (minutes)")).toBeInTheDocument();
  });

  it("renders toggle switches", () => {
    render(<Settings />);
    expect(screen.getByLabelText("Auto-start Breaks")).toBeInTheDocument();
    expect(screen.getByLabelText("Auto-start Work")).toBeInTheDocument();
  });

  it("renders daily goal input", () => {
    render(<Settings />);
    expect(screen.getByLabelText("Daily goal")).toBeInTheDocument();
  });

  it("renders account section", () => {
    render(<Settings />);
    expect(screen.getByText(/Account/i)).toBeInTheDocument();
  });

  it("renders save button", () => {
    render(<Settings />);
    expect(screen.getByText(/Save/i)).toBeInTheDocument();
  });

  it("shows work duration with default value", () => {
    render(<Settings />);
    const input = screen.getByLabelText("Work duration (minutes)");
    expect(input).toHaveValue(25);
  });

  it("updates work duration on change", () => {
    render(<Settings />);
    const input = screen.getByLabelText("Work duration (minutes)");
    fireEvent.change(input, { target: { value: "30" } });
    expect(input).toHaveValue(30);
  });

  it("hides admin panel for non-root users", () => {
    render(<Settings />);
    expect(screen.queryByText("User Management")).not.toBeInTheDocument();
  });

  it("shows server URL", () => {
    render(<Settings />);
    expect(screen.getByDisplayValue("http://localhost:9090")).toBeInTheDocument();
  });
});

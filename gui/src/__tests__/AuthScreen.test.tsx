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
import AuthScreen from "../components/AuthScreen";

describe("AuthScreen component", () => {
  const loginFn = vi.fn();
  const registerFn = vi.fn();

  beforeEach(() => {
    loginFn.mockReset();
    registerFn.mockReset();
    useStore.setState({
      token: null, username: null, role: null,
      serverUrl: "http://localhost:9090",
      savedServers: [],
      login: loginFn,
      register: registerFn,
    });
  });

  it("renders login form by default", () => {
    render(<AuthScreen />);
    expect(screen.getByLabelText("Username")).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /sign in/i })).toBeInTheDocument();
  });

  it("toggles to register mode", () => {
    render(<AuthScreen />);
    fireEvent.click(screen.getByText(/need an account/i));
    expect(screen.getByRole("button", { name: /create account/i })).toBeInTheDocument();
  });

  it("shows password strength meter in register mode", () => {
    render(<AuthScreen />);
    fireEvent.click(screen.getByText(/need an account/i));
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "Abc12345" } });
    expect(screen.getByText(/8\+ chars/)).toBeInTheDocument();
    expect(screen.getByText(/Uppercase/)).toBeInTheDocument();
    expect(screen.getByText(/Digit/)).toBeInTheDocument();
  });

  it("hides password strength in login mode", () => {
    render(<AuthScreen />);
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "Abc12345" } });
    expect(screen.queryByText("8+ chars")).not.toBeInTheDocument();
  });

  it("toggles password visibility", () => {
    render(<AuthScreen />);
    const pwInput = screen.getByLabelText("Password");
    expect(pwInput).toHaveAttribute("type", "password");
    fireEvent.click(screen.getByLabelText("Show password"));
    expect(pwInput).toHaveAttribute("type", "text");
    fireEvent.click(screen.getByLabelText("Hide password"));
    expect(pwInput).toHaveAttribute("type", "password");
  });

  it("calls login on form submit", async () => {
    loginFn.mockResolvedValue(undefined);
    render(<AuthScreen />);
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "alice" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "pass1234" } });
    fireEvent.submit(screen.getByRole("button", { name: /sign in/i }).closest("form")!);
    expect(loginFn).toHaveBeenCalledWith("alice", "pass1234");
  });

  it("shows error on login failure", async () => {
    loginFn.mockRejectedValue("Invalid credentials");
    render(<AuthScreen />);
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "alice" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "pass1234" } });
    fireEvent.submit(screen.getByRole("button", { name: /sign in/i }).closest("form")!);
    expect(await screen.findByRole("alert")).toHaveTextContent("Invalid credentials");
  });

  it("validates password on register", () => {
    render(<AuthScreen />);
    fireEvent.click(screen.getByText(/need an account/i));
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "bob" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "weak" } });
    fireEvent.submit(screen.getByRole("button", { name: /create account/i }).closest("form")!);
    expect(registerFn).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(/8 characters/);
  });

  it("shows server URL and edit button", () => {
    render(<AuthScreen />);
    expect(screen.getByLabelText("Edit server URL")).toBeInTheDocument();
    expect(screen.getByText(/localhost:9090/)).toBeInTheDocument();
  });

  it("shows saved servers for quick switch", () => {
    useStore.setState({
      savedServers: [{ url: "http://other:9090", username: "bob", token: "t", refreshToken: "r", role: "user" }],
    });
    render(<AuthScreen />);
    expect(screen.getByText("Quick switch")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();
  });

  it("does not submit with empty fields", () => {
    render(<AuthScreen />);
    fireEvent.submit(screen.getByRole("button", { name: /sign in/i }).closest("form")!);
    expect(loginFn).not.toHaveBeenCalled();
  });
});

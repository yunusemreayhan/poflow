import type { StateCreator } from "zustand";
import { apiCall, setToken } from "./api";
import { platformSaveAuth, platformClearAuth, platformSetConnection, platformSetToken, platformApiCall } from "../platform";
import type { AuthResponse } from "./api";

// Dedup guard
const _inflight = new Set<string>();
function dedup(key: string, fn: () => Promise<void>): Promise<void> {
  if (_inflight.has(key)) return Promise.resolve();
  _inflight.add(key);
  return fn().finally(() => _inflight.delete(key));
}

export interface SavedServer {
  url: string;
  username: string;
  token: string;
  refresh_token: string;
  role: string;
}

export function loadServers(): SavedServer[] {
  try { return JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("servers")) || "[]"); } catch { return []; }
}
function saveServers(servers: SavedServer[]) {
  localStorage.setItem("servers", JSON.stringify(servers));
}

export interface AuthSlice {
  token: string | null;
  username: string | null;
  role: string | null;
  serverUrl: string;
  savedServers: SavedServer[];
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<void>;
  logout: () => void;
  restoreAuth: () => Promise<void> | void;
  setServerUrl: (url: string) => Promise<void>;
  switchToServer: (server: SavedServer) => Promise<void>;
  removeServer: (url: string, username: string) => void;
}

export const createAuthSlice: StateCreator<AuthSlice & { mutating: boolean; toast: (msg: string, type?: "success" | "error" | "info") => void }, [], [], AuthSlice> = (set, get) => ({
  token: null,
  username: null,
  role: null,
  serverUrl: (typeof localStorage !== "undefined" && localStorage.getItem("serverUrl")) || (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__ ? window.location.origin : "http://127.0.0.1:9090"),
  savedServers: loadServers(),

  login: async (username, password) => dedup("auth:login", async () => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/login", { username, password });
    await setToken(resp.token);
    platformSaveAuth(JSON.stringify(resp)).catch(() => { localStorage.setItem("auth", JSON.stringify(resp)); });
    set({ token: resp.token, username: resp.username, role: resp.role });
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, refresh_token: resp.refresh_token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  }),

  register: async (username, password) => {
    const resp = await apiCall<AuthResponse>("POST", "/api/auth/register", { username, password });
    await setToken(resp.token);
    platformSaveAuth(JSON.stringify(resp)).catch(() => { localStorage.setItem("auth", JSON.stringify(resp)); });
    set({ token: resp.token, username: resp.username, role: resp.role });
    const url = get().serverUrl;
    const servers = loadServers().filter(s => !(s.url === url && s.username === resp.username));
    servers.unshift({ url, username: resp.username, token: resp.token, refresh_token: resp.refresh_token, role: resp.role });
    saveServers(servers);
    set({ savedServers: servers });
  },

  logout: () => {
    apiCall("POST", "/api/auth/logout").catch(e => console.debug("Logout API:", e));
    platformClearAuth().catch(e => console.debug("Clear auth:", e));
    localStorage.removeItem("auth");
    const url = get().serverUrl;
    const username = get().username;
    if (url && username) {
      const servers = loadServers().filter(s => !(s.url === url && s.username === username));
      saveServers(servers);
      set({ savedServers: servers });
    }
    set({ token: null, username: null, role: null });
    platformSetToken("").catch(e => console.debug("Clear token:", e));
    if (typeof caches !== 'undefined') caches.delete('pomo-v1').catch(e => console.debug("Clear cache:", e));
  },

  restoreAuth: async () => {
    const url = localStorage.getItem("serverUrl");
    if (url) { set({ serverUrl: url }); platformSetConnection(url); }
  },

  setServerUrl: async (url) => {
    const clean = url.replace(/\/+$/, "");
    localStorage.setItem("serverUrl", clean);
    await platformSetConnection(clean);
    set({ serverUrl: clean, token: null, username: null, role: null });
    localStorage.removeItem("auth");
  },

  switchToServer: async (server) => {
    set({ mutating: true } as any);
    localStorage.setItem("serverUrl", server.url);
    await platformSetConnection(server.url);
    await setToken(server.token);
    platformSaveAuth(JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role })).catch(() => {
      localStorage.setItem("auth", JSON.stringify({ token: server.token, refresh_token: server.refresh_token, username: server.username, role: server.role }));
    });
    set({ serverUrl: server.url, token: server.token, username: server.username, role: server.role });
    try { await platformApiCall("GET", "/api/timer"); } catch {
      set({ token: null, username: null, role: null });
      get().toast("Session expired — please log in again", "error");
    }
    set({ mutating: false } as any);
  },

  removeServer: (url, username) => {
    const servers = loadServers().filter(s => !(s.url === url && s.username === username));
    saveServers(servers);
    set({ savedServers: servers });
  },
});

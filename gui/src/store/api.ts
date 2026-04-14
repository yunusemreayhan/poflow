import { platformApiCall, platformSetToken } from "../platform";

// --- HTTP API helper ---

export async function apiCall<T = unknown>(method: string, path: string, body?: unknown): Promise<T> {
  try {
    return await platformApiCall<T>(method, path, body ?? null);
  } catch (e) {
    const msg = typeof e === "string" ? e : (e as Error)?.message || "";
    // Auto-refresh on 401 (expired token)
    if (msg.includes("401") || msg.includes("expired") || msg.includes("Unauthorized")) {
      const refreshed = await tryRefreshToken();
      if (refreshed) {
        try { return await platformApiCall<T>(method, path, body ?? null); } catch { /* retry failed — fall through to original error */ }
      } else {
        // Refresh failed — force logout so user gets a clean login screen
        // instead of staying in a broken "logged in but disconnected" state
        import("./store").then(({ useStore }) => {
          if (useStore.getState().token) useStore.getState().logout();
        }).catch(() => {});
      }
    }
    if (method !== "GET") {
      // V31-12: Parse error JSON cleanly, show user-friendly message
      try { const parsed = JSON.parse(msg); if (parsed.error) { showErrorToast(parsed.error); throw e; } } catch (parseErr) { if (parseErr === e) throw e; }
      showErrorToast(msg.length > 200 ? "An error occurred" : msg);
    }
    throw e;
  }
}

let refreshing: Promise<boolean> | null = null;
async function tryRefreshToken(): Promise<boolean> {
  if (refreshing) return refreshing;
  refreshing = (async () => {
    try {
      const { useStore } = await import("./store");
      const state = useStore.getState();
      const server = state.savedServers?.find(s => s.url === state.serverUrl);
      const serverIdx = state.savedServers?.findIndex(s => s.url === state.serverUrl) ?? -1;
      if (!server?.refresh_token || serverIdx < 0) return false;
      const resp = await platformApiCall<{ token: string; refresh_token: string }>("POST", "/api/auth/refresh", { refresh_token: server.refresh_token });
      if (resp?.token) {
        await platformSetToken(resp.token);
        const servers = [...state.savedServers];
        servers[serverIdx] = { ...servers[serverIdx], token: resp.token, refresh_token: resp.refresh_token };
        localStorage.setItem("servers", JSON.stringify(servers));
        useStore.setState({ savedServers: servers, token: resp.token });
        return true;
      }
      return false;
    } catch { return false; }
    finally { refreshing = null; }
  })();
  return refreshing;
}

function showErrorToast(msg: string) {
  import("./store").then(({ useStore }) => {
    useStore.getState().toast(msg, "error");
  }).catch(() => {});
}

export async function setToken(token: string) {
  return platformSetToken(token);
}

// S1: Get a fresh token for direct fetch() calls (binary uploads/downloads).
// Attempts refresh if the current token might be stale.
export async function getFreshToken(): Promise<string> {
  const { useStore } = await import("./store");
  const token = useStore.getState().token;
  if (!token) throw new Error("Not authenticated");
  // S1-v23: Use authenticated endpoint to verify token validity
  try {
    await platformApiCall("GET", "/api/timer");
  } catch {
    const refreshed = await tryRefreshToken();
    if (!refreshed) throw new Error("Token expired");
  }
  return useStore.getState().token || token;
}

// --- Types (re-exported from types.ts) ---
export type {
  Task, Session, EngineState, Comment, TaskDetail, DayStat, Config,
  EpicGroup, EpicSnapshot, EpicGroupDetail, Team, TeamMember, TeamDetail,
  TimeReport, AuthResponse, User,
  Room, RoomMember, RoomVoteView, RoomVote, VoteResult, RoomState,
  Sprint, SprintTask, SprintDailyStat, SprintDetail, SprintBoard, TaskSprintInfo,
  BurnEntry, BurnSummaryEntry, BurnTotalEntry, TaskAssignee,
} from "./types";

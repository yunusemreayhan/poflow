import { invoke } from "@tauri-apps/api/core";

// --- HTTP API helper ---

export async function apiCall<T = unknown>(method: string, path: string, body?: unknown): Promise<T> {
  try {
    return await invoke<T>("api_call", { method, path, body: body ?? null });
  } catch (e) {
    const msg = typeof e === "string" ? e : (e as Error)?.message || "";
    // Auto-refresh on 401 (expired token)
    if (msg.includes("401") || msg.includes("expired") || msg.includes("Unauthorized")) {
      const refreshed = await tryRefreshToken();
      if (refreshed) {
        try { return await invoke<T>("api_call", { method, path, body: body ?? null }); } catch {}
      }
    }
    if (method !== "GET") {
      try { const parsed = JSON.parse(msg); if (parsed.error) { showErrorToast(parsed.error); throw e; } } catch {}
      showErrorToast(msg);
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
      const server = state.savedServers?.[0];
      if (!server?.refresh_token) return false;
      const resp = await invoke<{ token: string; refresh_token: string }>("api_call", {
        method: "POST", path: "/api/auth/refresh", body: { refresh_token: server.refresh_token }
      });
      if (resp?.token) {
        await setToken(resp.token);
        const servers = [...state.savedServers];
        servers[0] = { ...servers[0], token: resp.token, refresh_token: resp.refresh_token };
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
  return invoke("set_token", { token });
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

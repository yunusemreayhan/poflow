/**
 * Platform abstraction: Tauri desktop vs Web browser.
 *
 * In Tauri mode, API calls go through invoke("api_call", ...).
 * In Web mode, API calls go through fetch() directly to the server.
 */

export const isTauri = typeof window !== "undefined" && typeof (window as any).__TAURI_INTERNALS__ !== "undefined";

// Current auth token (web mode keeps it in memory; Tauri mode uses Rust-side state)
let _webToken = "";
let _webBaseUrl = "";

function webBaseUrl(): string {
  if (_webBaseUrl) return _webBaseUrl;
  // Default: same origin (daemon serves the GUI)
  return window.location.origin;
}

// ── Core API call ──────────────────────────────────────────────

export async function platformApiCall<T = unknown>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>("api_call", { method, path, body: body ?? null });
  }
  const url = `${webBaseUrl()}${path}`;
  const headers: Record<string, string> = { "x-requested-with": "web-gui" };
  if (_webToken) headers["authorization"] = `Bearer ${_webToken}`;
  if (body !== undefined && body !== null) headers["content-type"] = "application/json";
  const resp = await fetch(url, {
    method,
    headers,
    body: body != null ? JSON.stringify(body) : undefined,
    signal,
  });
  if (!resp.ok) {
    const text = await resp.text().catch(() => "");
    throw new Error(text || `${resp.status} ${resp.statusText}`);
  }
  const text = await resp.text();
  return text ? JSON.parse(text) as T : undefined as unknown as T;
}

// ── Auth helpers ───────────────────────────────────────────────

export async function platformSetToken(token: string): Promise<void> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke("set_token", { token });
  }
  _webToken = token;
}

export async function platformSaveAuth(data: string): Promise<void> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("save_auth", { data }).catch(() => {
      localStorage.setItem("auth", data);
    });
    return;
  }
  localStorage.setItem("auth", data);
}

export async function platformClearAuth(): Promise<void> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("clear_auth").catch(e => console.debug("Tauri clear_auth:", e));
    return;
  }
  localStorage.removeItem("auth");
  _webToken = "";
}

export async function platformSetConnection(baseUrl: string): Promise<void> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke("set_connection", { baseUrl });
  }
  _webBaseUrl = baseUrl;
}

// ── Desktop-only features ──────────────────────────────────────

export async function platformIndicatorStatus(): Promise<boolean> {
  if (!isTauri) return false;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<boolean>("indicator_status");
}

export async function platformIndicatorToggle(enable: boolean): Promise<boolean> {
  if (!isTauri) return false;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<boolean>("indicator_toggle", { enable });
}

export async function platformSaveFile(filename: string, content: string, ext: string): Promise<boolean> {
  if (!isTauri) {
    // Web fallback: trigger browser download
    const blob = new Blob([content], { type: "text/plain" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = filename;
    a.click();
    URL.revokeObjectURL(a.href);
    return true;
  }
  const dialog = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await dialog.save({ defaultPath: filename, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
  if (path) { await invoke("plugin:fs|write_text_file", { path, contents: content }); return true; }
  return false;
}

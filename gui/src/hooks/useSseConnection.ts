import { useEffect } from "react";
import { useStore } from "../store/store";
import { apiCall } from "../store/api";
import type { EngineState } from "../store/api";

export function useSseConnection(token: string | null) {
  const poll = useStore(s => s.poll);
  const loadTasks = useStore(s => s.loadTasks);

  useEffect(() => {
    if (!token) return;

    const url = useStore.getState().serverUrl;
    let sseInstance: EventSource | null = null;
    let unmounted = false;
    let reconnectAttempts = 0;

    let reconnectId: ReturnType<typeof setTimeout> | null = null;
    let debounceTimer: ReturnType<typeof setTimeout> | null = null;

    const connectSse = async () => {
      try {
        const resp = await apiCall<{ ticket: string }>("POST", "/api/timer/ticket");
        if (unmounted) return;
        sseInstance = new EventSource(`${url}/api/timer/sse?ticket=${encodeURIComponent(resp.ticket)}`);
      } catch {
        return;
      }

      sseInstance.addEventListener("timer", (e) => {
        try {
          const engine = JSON.parse(e.data) as EngineState;
          useStore.setState({ engine, connected: true, error: null });
        } catch { /* ignore */ }
      });

      const pending = new Set<string>();
      const flushChanges = () => {
        if (pending.has("Tasks") || pending.has("Sprints")) {
          // V30-12: Throttle task reloads — skip if loaded within last 2s
          const now = Date.now();
          if (now - useStore.getState().tasksLoadedAt > 2000) {
            useStore.getState().loadTasks();
          }
        }
        if (pending.has("Sprints")) {
          window.dispatchEvent(new CustomEvent("sse-sprints"));
        }
        if (pending.has("Rooms")) {
          window.dispatchEvent(new CustomEvent("sse-rooms"));
        }
        pending.clear();
      };

      sseInstance.addEventListener("change", (e) => {
        try {
          const kind = JSON.parse(e.data) as string;
          pending.add(kind);
          if (debounceTimer) clearTimeout(debounceTimer);
          debounceTimer = setTimeout(flushChanges, 300);
        } catch { /* ignore */ }
      });

      sseInstance.onerror = () => {
        useStore.setState({ connected: false });
        sseInstance?.close();
        sseInstance = null;
        const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 30000);
        reconnectAttempts++;
        if (!unmounted) reconnectId = setTimeout(connectSse, delay);
      };
      sseInstance.onopen = () => { useStore.setState({ connected: true }); reconnectAttempts = 0; };
    };
    connectSse();

    const timerFallback = setInterval(() => {
      if (!sseInstance || sseInstance.readyState !== EventSource.OPEN) poll();
    }, 2000);
    const taskSafety = setInterval(() => {
      if (!sseInstance || sseInstance.readyState !== EventSource.OPEN) loadTasks();
    }, 30000);

    return () => {
      unmounted = true;
      sseInstance?.close();
      if (reconnectId) clearTimeout(reconnectId);
      if (debounceTimer) clearTimeout(debounceTimer);
      clearInterval(timerFallback);
      clearInterval(taskSafety);
    };
  }, [token, poll, loadTasks]);
}

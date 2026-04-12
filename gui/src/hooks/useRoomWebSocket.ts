import { useEffect, useRef, useCallback } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import type { RoomState } from "../store/api";

export function useRoomWebSocket(roomId: number, onState: (s: RoomState) => void) {
  const onStateRef = useRef(onState);
  onStateRef.current = onState;
  // B2: Track unmount in a ref so cleanup works even if connect() hasn't resolved
  const unmountedRef = useRef(false);
  const wsRef = useRef<WebSocket | null>(null);

  const connect = useCallback(async () => {
    const { serverUrl } = useStore.getState();
    let attempts = 0;

    const tryConnect = async () => {
      if (unmountedRef.current) return;
      try {
        const resp = await apiCall<{ ticket: string }>("POST", "/api/timer/ticket");
        if (unmountedRef.current) return;
        const wsUrl = serverUrl.replace(/^http/, "ws");
        const ws = new WebSocket(`${wsUrl}/api/rooms/${roomId}/ws?ticket=${encodeURIComponent(resp.ticket)}`);
        wsRef.current = ws;
        ws.onmessage = (e) => {
          try { onStateRef.current(JSON.parse(e.data)); } catch { /* ignore */ }
          lastMsg = Date.now();
        };
        let lastMsg = Date.now();
        // F12: Heartbeat check — force reconnect if no data in 60s
        const hb = setInterval(() => { if (Date.now() - lastMsg > 60000) ws?.close(); }, 15000);
        ws.onopen = () => { attempts = 0; };
        ws.onclose = () => {
          clearInterval(hb);
          if (unmountedRef.current) return;
          const delay = Math.min(1000 * Math.pow(2, attempts), 15000);
          attempts++;
          setTimeout(tryConnect, delay);
        };
        ws.onerror = () => ws?.close();
      } catch {
        if (!unmountedRef.current) {
          const delay = Math.min(1000 * Math.pow(2, attempts), 15000);
          attempts++;
          setTimeout(tryConnect, delay);
        }
      }
    };

    await tryConnect();
  }, [roomId]);

  useEffect(() => {
    unmountedRef.current = false;
    connect();
    return () => { unmountedRef.current = true; wsRef.current?.close(); };
  }, [connect]);
}

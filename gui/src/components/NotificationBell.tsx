import React, { useEffect, useState } from "react";
import { Bell } from "lucide-react";
import { apiCall } from "../store/api";
import { useT } from "../i18n";

// BL21-23: Notification bell with unread count and dropdown
export default function NotificationBell() {
  const t = useT();
  const [count, setCount] = useState(0);
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<{ id: number; kind: string; message: string; read: boolean; created_at: string }[]>([]);
  const ref_ = React.useRef<HTMLDivElement>(null);

  // B5: Close dropdown on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => { if (ref_.current && !ref_.current.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  useEffect(() => {
    const refresh = () => apiCall<{ count: number }>("GET", "/api/notifications/unread").then(d => d && setCount(d.count)).catch(e => console.error("Poll notifications:", e));
    refresh();
    const handler = () => refresh();
    window.addEventListener("sse-notifications", handler);
    window.addEventListener("sse-sprints", handler);
    return () => { window.removeEventListener("sse-notifications", handler); window.removeEventListener("sse-sprints", handler); };
  }, []);

  const loadItems = () => apiCall<typeof items>("GET", "/api/notifications?limit=20").then(d => d && setItems(d)).catch(e => console.error("Load notifications:", e));

  const markRead = async () => {
    await apiCall("POST", "/api/notifications/read", {});
    setCount(0);
    setItems(prev => prev.map(i => ({ ...i, read: true })));
  };

  return (
    <div className="relative" ref={ref_}>
      <button onClick={() => { setOpen(!open); if (!open) loadItems(); }}
        className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all relative" aria-label="Notifications">
        <Bell size={16} />
        {count > 0 && <span className="absolute top-1 right-1 w-4 h-4 bg-red-500 rounded-full text-[9px] text-white flex items-center justify-center">{count > 9 ? "9+" : count}</span>}
      </button>
      {open && (
        <div role="dialog" aria-label="Notifications" tabIndex={-1} ref={el => el?.focus()} onKeyDown={e => {
            if (e.key === "Escape") setOpen(false);
            if (e.key === "Tab") {
              const focusable = e.currentTarget.querySelectorAll<HTMLElement>("button, [tabindex]");
              if (focusable.length === 0) return;
              const first = focusable[0], last = focusable[focusable.length - 1];
              if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
              else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
            }
          }}
          className="absolute left-14 bottom-0 w-72 bg-[var(--color-surface)] border border-white/10 rounded-lg shadow-xl z-50 max-h-80 overflow-y-auto">
          <div className="flex justify-between items-center p-2 border-b border-white/5">
            <span className="text-xs text-white/50 font-medium">{t.notificationsTitle}</span>
            {count > 0 && <button onClick={markRead} className="text-[10px] text-[var(--color-accent)]">{t.markAllRead}</button>}
          </div>
          {items.length === 0 ? (
            <div className="p-4 text-xs text-white/20 text-center">{t.noNotifications}</div>
          ) : items.map(n => (
            <div key={n.id} className={`p-2 border-b border-white/5 text-xs ${n.read ? "text-white/30" : "text-white/60 bg-white/[0.02]"}`}>
              <div className="truncate">{n.message}</div>
              <div className="text-[10px] text-white/20 mt-0.5">{n.created_at.slice(0, 16).replace("T", " ")}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

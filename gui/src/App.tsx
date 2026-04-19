import React, { useEffect, useState } from "react";
import { motion, AnimatePresence, MotionConfig } from "framer-motion";
import { Timer as TimerIcon, ListTodo, BarChart3, Settings as SettingsIcon, Wifi, WifiOff, Code2, LogOut, Users, Zap, Sun, Moon, RefreshCw, LayoutDashboard, Bell, CalendarDays, Columns3, GanttChart as GanttIcon, Map, Activity } from "lucide-react";
import { useStore } from "./store/store";
import { useT } from "./i18n";
import { apiCall } from "./store/api";
import { isTauri } from "./platform";
import { useSseConnection } from "./hooks/useSseConnection";
import Timer from "./components/Timer";
import TaskList from "./components/TaskList";
import History from "./components/History";
import Dashboard from "./components/Dashboard";
import Settings from "./components/Settings";
import ApiReference from "./components/ApiReference";
import AuthScreen from "./components/AuthScreen";
import Rooms from "./components/Rooms";
import Sprints from "./components/Sprints";
import CalendarView from "./components/CalendarView";
import KanbanBoard from "./components/KanbanBoard";
import GanttChart from "./components/GanttChart";
import RoadmapView from "./components/RoadmapView";
import CommandPalette from "./components/CommandPalette";
import WelcomeGuide from "./components/WelcomeGuide";
import ActivityTimeline from "./components/ActivityTimeline";

const TABS = [
  { id: "timer", icon: TimerIcon, labelKey: "timer" },
  { id: "dashboard", icon: LayoutDashboard, labelKey: "dashboard" },
  { id: "tasks", icon: ListTodo, labelKey: "tasks" },
  { id: "kanban", icon: Columns3, labelKey: "kanban" },
  { id: "gantt", icon: GanttIcon, labelKey: "gantt" },
  { id: "roadmap", icon: Map, labelKey: "roadmap" },
  { id: "calendar", icon: CalendarDays, labelKey: "calendar" },
  { id: "sprints", icon: Zap, labelKey: "sprints" },
  { id: "rooms", icon: Users, labelKey: "rooms" },
  { id: "history", icon: BarChart3, labelKey: "history" },
  { id: "activity", icon: Activity, labelKey: "activity" },
  { id: "api", icon: Code2, labelKey: "api" },
  { id: "settings", icon: SettingsIcon, labelKey: "settings" },
] as const;

function Sidebar() {
  const { activeTab, setTab, connected, username, logout, activeTeamId, setActiveTeam } = useStore();
  const t = useT();
  const config = useStore(s => s.config);
  const timerRunning = useStore(s => s.engine?.status === "Running");
  const [theme, setThemeLocal] = useState(() => localStorage.getItem("theme") || "dark");
  const [teams, setTeams] = useState<{ id: number; name: string }[]>([]);

  // Sync theme from server config on load
  useEffect(() => {
    if (config?.theme && config.theme !== theme) {
      setThemeLocal(config.theme);
    }
  }, [config?.theme, theme]);

  const setTheme = (th: string) => {
    setThemeLocal(th);
    const cur = useStore.getState().config;
    // B2: Only sync to server if config is loaded, otherwise just set locally
    if (cur) {
      const updated = { ...cur, theme: th };
      useStore.setState({ config: updated });
      apiCall("PUT", "/api/config", updated).catch(e => console.error("Save config:", e));
    }
  };

  useEffect(() => {
    apiCall<{ id: number; name: string }[]>("GET", "/api/me/teams").then(res => res && setTeams(res)).catch(e => console.error("Load teams:", e));
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  // F16: Sync offline queue when coming back online
  useEffect(() => {
    const handler = () => { useStore.getState().syncOfflineQueue(); };
    window.addEventListener("online", handler);
    return () => window.removeEventListener("online", handler);
  }, []);

  return (
    <div className="w-[72px] h-full flex flex-col items-center py-5 border-r border-white/5 shrink-0">
      {/* Top: Logo + Tabs — scrollable */}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden scrollbar-hide">
        <div className="flex flex-col items-center gap-2 py-1">
      {/* Logo */}
      <div className="mb-2 shrink-0">
        <motion.div
          animate={{ rotate: [0, 360] }}
          transition={{ duration: 20, repeat: Infinity, ease: "linear" }}
          className="w-9 h-9 rounded-full"
          style={{
            background: "conic-gradient(from 0deg, #FF6B6B, #4ECDC4, #45B7D1, #7C3AED, #FF6B6B)",
          }}
        />
      </div>

      {TABS.map((tab) => {
        const Icon = tab.icon;
        const active = activeTab === tab.id;
        const label = t[tab.labelKey] || tab.labelKey;
        return (
          <motion.button
            key={tab.id}
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            onClick={() => setTab(tab.id)}
            className={`relative w-11 h-11 flex items-center justify-center rounded-xl transition-all shrink-0 ${
              active ? "text-white" : "text-white/30 hover:text-white/60"
            }`}
            title={label}
            aria-label={label}
            aria-current={active ? "page" : undefined}
          >
            {active && (
              <motion.div
                layoutId="tab-bg"
                className="absolute inset-0 rounded-xl bg-[var(--color-accent)]/20"
                transition={{ type: "spring", stiffness: 300, damping: 30 }}
              />
            )}
            <Icon size={22} className="relative z-10" />
            {/* U7: Active timer indicator */}
            {tab.id === "timer" && timerRunning && (
              <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-[var(--color-work)] animate-pulse z-20" />
            )}
          </motion.button>
        );
      })}

      </div>
      </div> {/* end scrollable top */}

      {/* Bottom: Teams + User — shrinks if needed, teams scroll */}
      <div className="shrink flex flex-col items-center gap-1 mt-2 min-h-0">
      {/* Team selector — scrollable when many teams */}
      {teams.length > 0 && (
        <div className="flex flex-col items-center gap-0.5 mb-2 overflow-y-auto overflow-x-hidden scrollbar-hide min-h-0 flex-1">
          <button onClick={() => setActiveTeam(null)}
            className={`w-11 h-7 flex items-center justify-center rounded text-[9px] font-medium transition-all shrink-0 ${!activeTeamId ? "bg-[var(--color-accent)] text-white" : "text-white/30 hover:text-white/50"}`}
            title="All teams">All</button>
          {teams.map(t => (
            <button key={t.id} onClick={() => setActiveTeam(t.id)}
              className={`w-11 h-7 flex items-center justify-center rounded text-[9px] font-medium truncate transition-all shrink-0 ${activeTeamId === t.id ? "bg-[var(--color-accent)] text-white" : "text-white/30 hover:text-white/50"}`}
              title={t.name} aria-label={t.name}>{t.name.slice(0, 4)}</button>
          ))}
        </div>
      )}

      {/* User + theme + logout */}
      <div className="shrink-0 flex flex-col items-center gap-1 mb-2">
        <span className="text-[10px] text-white/30 truncate max-w-[60px]">{username}</span>
        <NotificationBell />
        <button onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title="Toggle theme" aria-label="Toggle theme">
          {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
        </button>
        <button onClick={() => { useStore.getState().loadTasks(); useStore.getState().toast("Refreshed"); }}
          className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title="Refresh data" aria-label="Refresh data">
          <RefreshCw size={16} />
        </button>
        <button onClick={logout} className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title={t.logout} aria-label={t.logout}>
          <LogOut size={16} />
        </button>
      </div>

      {/* Connection status */}
      <div
        role="status"
        aria-live="polite"
        aria-label={connected ? "Daemon connected" : "Daemon disconnected"}
        className={`w-11 h-11 flex items-center justify-center rounded-xl mb-1 ${
          connected ? "text-[var(--color-success)]" : "text-[var(--color-danger)]"
        }`}
        title={connected ? "Daemon connected" : "Daemon disconnected"}
      >
        {connected ? <Wifi size={16} /> : <WifiOff size={16} />}
      </div>
      </div> {/* end bottom pinned */}
    </div>
  );
}

export default function App() {
  const { activeTab, poll, loadTasks, connected, token, toasts, dismissToast, confirmDialog, dismissConfirm, loading, focusMode } = useStore();
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [offline, setOffline] = useState(!navigator.onLine);
  const [showWelcome, setShowWelcome] = useState(() => !localStorage.getItem("pomo_welcomed"));
  const t = useT();

  // F16: Track online/offline state
  useEffect(() => {
    const on = () => setOffline(false);
    const off = () => setOffline(true);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => { window.removeEventListener("online", on); window.removeEventListener("offline", off); };
  }, []);

  useEffect(() => {
    useStore.getState().restoreAuth();
  }, []);

  useSseConnection(token);

  // U6: Listen for global shortcut (Tauri) to toggle timer
  useEffect(() => {
    if (!isTauri || !token) return;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("global-timer-toggle", () => {
        const s = useStore.getState();
        if (s.engine?.status === "Running") s.pause();
        else if (s.engine?.status === "Paused") s.resume();
        else s.start(s.timerTaskId);
      }).then(fn => { unlisten = fn; });
    }).catch(e => console.debug("Tauri global-shortcut:", e));
    return () => { unlisten?.(); };
  }, [token]);

  useEffect(() => {
    if (!token) return;
    poll();
    loadTasks();
    useStore.getState().loadConfig();
  }, [token]);

  // Global keyboard shortcuts (#37)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = useStore.getState();
      if (e.key === "Escape" && store.engine?.status === "Running") { store.stop(); }
      // Space handled by Timer.tsx to avoid double-toggle
      // Tab navigation and shortcuts: only when not in an input/select
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag !== "INPUT" && tag !== "TEXTAREA" && tag !== "SELECT" && !(e.target as HTMLElement)?.isContentEditable && !e.ctrlKey && !e.metaKey) {
        if (e.key === "r") { store.loadTasks(); store.toast("Refreshed"); }
        const tabMap: Record<string, string> = { "0": "timer", "1": "dashboard", "2": "tasks", "3": "sprints", "4": "rooms", "5": "history", "6": "api", "7": "settings" };
        if (tabMap[e.key]) { store.setTab(tabMap[e.key]); }
        if (e.key === "n" && ["tasks", "kanban", "calendar"].includes(store.activeTab)) {
          e.preventDefault();
          // Focus the new task input if it exists
          document.querySelector<HTMLInputElement>('[data-new-task-input]')?.focus();
        }
        if (e.key === "/") {
          e.preventDefault();
          document.getElementById("task-search")?.focus();
        }
        if (e.key === "F11") {
          e.preventDefault();
          store.toggleFocusMode();
        }
        if (e.key === "?") { setShowShortcuts(s => !s); }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    // Only reload tasks on tab switch if data is stale (>10s since last load)
    if ((activeTab === "tasks" || activeTab === "sprints") && token) {
      const lastLoad = useStore.getState().tasksLoadedAt;
      if (Date.now() - lastLoad > 10000) loadTasks();
    }
  }, [activeTab, token]);

  if (!token) return <AuthScreen />;

  return (
    <MotionConfig reducedMotion="user">
    <div className="flex h-screen bg-[var(--color-bg)]">
      <CommandPalette />
      {showWelcome && token && <WelcomeGuide onDismiss={() => { setShowWelcome(false); localStorage.setItem("pomo_welcomed", "1"); }} />}
      <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-[var(--color-accent)] focus:text-white focus:rounded-lg focus:text-sm">
        {t.skipToContent}
      </a>
      <nav aria-label="Main navigation" className="hidden md:block" style={{ display: focusMode ? "none" : undefined }}>
        <Sidebar />
      </nav>
      <main id="main-content" className={`flex-1 overflow-hidden relative ${focusMode ? "pb-0" : "pb-14 md:pb-0"}`}>
        {!focusMode && offline && <div className="bg-yellow-600/80 text-white text-xs text-center py-1 px-2" role="alert">⚡ Offline — changes will sync when reconnected</div>}
        {focusMode && (
          <button onClick={() => useStore.getState().toggleFocusMode()}
            className="absolute top-2 right-2 z-50 text-xs text-white/0 hover:text-white/50 px-2 py-1 rounded transition-colors duration-300"
            title="Exit focus mode (F11)" aria-label="Exit focus mode">✕</button>
        )}
        {/* Loading indicator */}
        {(loading.tasks || loading.history || loading.stats || loading.config) && (
          <div className="absolute top-0 left-0 right-0 h-0.5 z-40 bg-[var(--color-accent)]/20 overflow-hidden" role="status" aria-label="Loading">
            <div className="h-full w-1/3 bg-[var(--color-accent)] animate-[slide_1s_ease-in-out_infinite]" />
          </div>
        )}
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.15 }}
            className="h-full overflow-y-auto"
          >
            {(focusMode || activeTab === "timer") && <Timer />}
            {!focusMode && activeTab === "dashboard" && <Dashboard />}
            <div style={{ display: !focusMode && activeTab === "tasks" ? undefined : "none" }}><TaskList /></div>
            {!focusMode && activeTab === "kanban" && <KanbanBoard />}
            {!focusMode && activeTab === "gantt" && <GanttChart />}
            {!focusMode && activeTab === "roadmap" && <RoadmapView />}
            {!focusMode && activeTab === "calendar" && <CalendarView />}
            {!focusMode && activeTab === "sprints" && <Sprints />}
            {!focusMode && activeTab === "rooms" && <Rooms />}
            {!focusMode && activeTab === "history" && <History />}
            {!focusMode && activeTab === "activity" && <ActivityTimeline />}
            {!focusMode && activeTab === "api" && <ApiReference />}
            {!focusMode && activeTab === "settings" && <Settings />}
          </motion.div>
        </AnimatePresence>

        <AnimatePresence>
          {!connected && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute bottom-6 left-6 right-6 glass p-4 flex items-center gap-3 text-sm text-[var(--color-warning)]"
            >
              <WifiOff size={16} />
              Daemon not running. Start with: <code className="bg-white/5 px-2 py-1 rounded text-xs">pomodoro-daemon</code>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Toast notifications */}
        <div className="absolute top-4 right-4 flex flex-col gap-2 z-50 pointer-events-none" role="status" aria-live="polite">
          <AnimatePresence>
            {toasts.map(t => (
              <motion.div key={t.id} initial={{ opacity: 0, x: 50 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: 50 }}
                role={t.type === "error" ? "alert" : undefined}
                className={`pointer-events-auto flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-medium shadow-lg ${
                  t.type === "error" ? "bg-[var(--color-danger)] text-white" : t.type === "info" ? "bg-blue-600/90 text-white" : "bg-[var(--color-success)]/90 text-white"
                }`}>
                <span className="cursor-pointer" onClick={() => dismissToast(t.id)}>{t.msg}</span>
                <button onClick={() => dismissToast(t.id)} className="ml-1 text-white/60 hover:text-white" aria-label="Dismiss">×</button>
                {t.onUndo && (
                  <button onClick={() => { t.onUndo!(); dismissToast(t.id); }}
                    className="ml-2 px-2 py-0.5 rounded bg-white/20 hover:bg-white/30 text-white text-xs font-bold">
                    Undo
                  </button>
                )}
              </motion.div>
            ))}
          </AnimatePresence>
        </div>

        {/* Confirm dialog */}
        <AnimatePresence>
          {confirmDialog && (
            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
              className="absolute inset-0 bg-black/50 flex items-center justify-center z-50"
              onClick={dismissConfirm} role="dialog" aria-modal="true" aria-label="Confirmation dialog"
              onKeyDown={e => {
                if (e.key === "Escape") dismissConfirm();
                if (e.key === "Tab") {
                  const focusable = e.currentTarget.querySelectorAll<HTMLElement>("button, [tabindex]");
                  if (focusable.length === 0) return;
                  const first = focusable[0], last = focusable[focusable.length - 1];
                  if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
                  else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
                }
              }}>
              <motion.div initial={{ scale: 0.9 }} animate={{ scale: 1 }} exit={{ scale: 0.9 }}
                className="glass p-6 max-w-sm w-full mx-4" onClick={e => e.stopPropagation()}>
                <p className="text-sm text-white/80 mb-4">{confirmDialog.msg}</p>
                <div className="flex gap-2 justify-end">
                  <button onClick={dismissConfirm} autoFocus
                    className="px-4 py-2 text-xs text-white/50 hover:text-white rounded-lg bg-white/5 hover:bg-white/10">{t.cancel}</button>
                  <button onClick={() => { confirmDialog.onConfirm(); dismissConfirm(); }}
                    className="px-4 py-2 text-xs text-white rounded-lg bg-[var(--color-danger)]">{confirmDialog.confirmLabel || t.delete}</button>
                </div>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
      {/* Quick-add FAB for mobile */}
      <QuickAddFab />
      {/* F18: Mobile bottom tab bar — visible on small screens */}
      {!focusMode && (
        <nav aria-label="Mobile navigation" className="md:hidden fixed bottom-0 left-0 right-0 z-40 bg-[var(--color-bg)] border-t border-white/5 flex justify-around py-1 safe-bottom">
          {TABS.filter(t => ["timer","tasks","dashboard","sprints","settings"].includes(t.id)).map(tab => {
            const Icon = tab.icon;
            const active = activeTab === tab.id;
            return (
              <button key={tab.id} onClick={() => useStore.getState().setTab(tab.id)}
                className={`flex flex-col items-center gap-0.5 px-2 py-1 ${active ? "text-[var(--color-accent)]" : "text-white/30"}`}
                aria-label={t[tab.labelKey] || tab.labelKey}
                aria-current={active ? "page" : undefined}>
                <Icon size={18} />
                <span className="text-[9px]">{tab.id[0].toUpperCase() + tab.id.slice(1)}</span>
              </button>
            );
          })}
        </nav>
      )}
      {/* Keyboard shortcuts panel */}
      <AnimatePresence>
        {showShortcuts && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={() => setShowShortcuts(false)}
            onKeyDown={e => { if (e.key === "Escape") setShowShortcuts(false); }} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
            <motion.div initial={{ scale: 0.9 }} animate={{ scale: 1 }} exit={{ scale: 0.9 }}
              className="glass p-6 rounded-2xl max-w-sm" onClick={e => e.stopPropagation()}>
              <h2 className="text-sm font-semibold text-white mb-3">{t.keyboardShortcuts}</h2>
              <div className="space-y-1.5 text-xs">
                {[
                  ["0-6", "Switch tabs"],
                  ["r", "Refresh"],
                  ["n", "New task (on tasks tab)"],
                  ["Space", "Pause/Resume timer"],
                  ["Escape", "Stop timer"],
                  ["/", t.focusSearch],
                  ["⌘K", "Command palette (global search)"],
                  ["?", t.toggleShortcuts],
                  ["Double-click", t.renameTask],
                  ["Enter", t.saveEdit],
                  ["Right-click", t.contextMenu],
                ].map(([key, desc]) => (
                  <div key={key} className="flex items-center gap-3">
                    <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-white/70 font-mono text-[10px] min-w-[60px] text-center">{key}</kbd>
                    <span className="text-white/50">{desc}</span>
                  </div>
                ))}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
    </MotionConfig>
  );
}

// BL21-23: Notification bell with unread count and dropdown
function NotificationBell() {
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
    const poll = () => apiCall<{ count: number }>("GET", "/api/notifications/unread").then(d => d && setCount(d.count)).catch(e => console.error("Poll notifications:", e));
    poll();
    const id = setInterval(poll, 30000);
    return () => clearInterval(id);
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
            <span className="text-xs text-white/50 font-medium">Notifications</span>
            {count > 0 && <button onClick={markRead} className="text-[10px] text-[var(--color-accent)]">Mark all read</button>}
          </div>
          {items.length === 0 ? (
            <div className="p-4 text-xs text-white/20 text-center">No notifications</div>
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

function QuickAddFab() {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const token = useStore(s => s.token);
  const createTask = useStore(s => s.createTask);
  if (!token) return null;
  const submit = async () => {
    if (!title.trim()) return;
    await createTask(title.trim());
    setTitle("");
    setOpen(false);
    useStore.getState().toast("Task created");
  };
  return (
    <>
      <button onClick={() => setOpen(true)} className="md:hidden fixed bottom-16 right-4 z-30 w-12 h-12 rounded-full bg-[var(--color-accent)] text-white shadow-lg flex items-center justify-center text-2xl" aria-label="Quick add task">+</button>
      {open && (
        <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/50 md:hidden" onClick={() => setOpen(false)}>
          <div className="w-full bg-[var(--color-bg)] border-t border-white/10 p-4 safe-bottom" onClick={e => e.stopPropagation()}>
            <input autoFocus value={title} onChange={e => setTitle(e.target.value)} onKeyDown={e => e.key === "Enter" && submit()}
              placeholder="New task title..." className="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]" />
            <div className="flex gap-2 mt-2">
              <button onClick={() => setOpen(false)} className="flex-1 py-2 rounded-lg text-xs text-white/40">Cancel</button>
              <button onClick={submit} className="flex-1 py-2 rounded-lg text-xs bg-[var(--color-accent)] text-white font-medium">Create</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

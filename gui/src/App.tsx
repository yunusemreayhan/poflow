import { useEffect, useState } from "react";
import { motion, AnimatePresence, MotionConfig } from "framer-motion";
import { Timer as TimerIcon, ListTodo, BarChart3, Settings as SettingsIcon, Wifi, WifiOff, Code2, LogOut, Users, Zap, Sun, Moon, RefreshCw } from "lucide-react";
import { useStore } from "./store/store";
import { _tasksLoadedAt } from "./store/store";
import { useT } from "./i18n";
import type { EngineState } from "./store/api";
import { apiCall } from "./store/api";
import Timer from "./components/Timer";
import TaskList from "./components/TaskList";
import History from "./components/History";
import Settings from "./components/Settings";
import ApiReference from "./components/ApiReference";
import AuthScreen from "./components/AuthScreen";
import Rooms from "./components/Rooms";
import Sprints from "./components/Sprints";

const TABS = [
  { id: "timer", icon: TimerIcon, labelKey: "timer" },
  { id: "tasks", icon: ListTodo, labelKey: "tasks" },
  { id: "sprints", icon: Zap, labelKey: "sprints" },
  { id: "rooms", icon: Users, labelKey: "rooms" },
  { id: "history", icon: BarChart3, labelKey: "history" },
  { id: "api", icon: Code2, labelKey: "api" },
  { id: "settings", icon: SettingsIcon, labelKey: "settings" },
] as const;

function Sidebar() {
  const { activeTab, setTab, connected, username, logout, activeTeamId, setActiveTeam } = useStore();
  const t = useT();
  const config = useStore(s => s.config);
  const [theme, setThemeLocal] = useState(() => localStorage.getItem("theme") || "dark");
  const [teams, setTeams] = useState<{ id: number; name: string }[]>([]);

  // Sync theme from server config on load
  useEffect(() => {
    if (config?.theme && config.theme !== theme) {
      setThemeLocal(config.theme);
    }
  }, [config?.theme]);

  const setTheme = (t: string) => {
    setThemeLocal(t);
    apiCall("PUT", "/api/config", { theme: t }).catch(() => {});
  };

  useEffect(() => {
    apiCall<{ id: number; name: string }[]>("GET", "/api/me/teams").then(t => t && setTeams(t));
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  return (
    <div className="w-[72px] flex flex-col items-center py-5 gap-2 border-r border-white/5 shrink-0">
      {/* Logo */}
      <div className="mb-4">
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
        const label = (t as Record<string, string>)[tab.labelKey] || tab.labelKey;
        return (
          <motion.button
            key={tab.id}
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            onClick={() => setTab(tab.id)}
            className={`relative w-11 h-11 flex items-center justify-center rounded-xl transition-all ${
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
          </motion.button>
        );
      })}

      <div className="flex-1" />

      {/* Team selector */}
      {teams.length > 0 && (
        <div className="flex flex-col items-center gap-0.5 mb-2">
          <button onClick={() => setActiveTeam(null)}
            className={`w-11 h-7 flex items-center justify-center rounded text-[9px] font-medium transition-all ${!activeTeamId ? "bg-[var(--color-accent)] text-white" : "text-white/30 hover:text-white/50"}`}
            title="All teams">All</button>
          {teams.map(t => (
            <button key={t.id} onClick={() => setActiveTeam(t.id)}
              className={`w-11 h-7 flex items-center justify-center rounded text-[9px] font-medium truncate transition-all ${activeTeamId === t.id ? "bg-[var(--color-accent)] text-white" : "text-white/30 hover:text-white/50"}`}
              title={t.name}>{t.name.slice(0, 4)}</button>
          ))}
        </div>
      )}

      {/* User + theme + logout */}
      <div className="flex flex-col items-center gap-1 mb-2">
        <span className="text-[10px] text-white/30 truncate max-w-[60px]">{username}</span>
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
        className={`w-11 h-11 flex items-center justify-center rounded-xl mb-1 ${
          connected ? "text-[var(--color-success)]" : "text-[var(--color-danger)]"
        }`}
        title={connected ? "Daemon connected" : "Daemon disconnected"}
      >
        {connected ? <Wifi size={16} /> : <WifiOff size={16} />}
      </div>
    </div>
  );
}

export default function App() {
  const { activeTab, poll, loadTasks, connected, token, toasts, dismissToast, confirmDialog, dismissConfirm, loading } = useStore();
  const t = useT();

  useEffect(() => {
    useStore.getState().restoreAuth();
  }, []);

  useEffect(() => {
    if (!token) return;
    poll();
    loadTasks();

    // SSE for real-time timer + data change notifications
    // Use ticket exchange to avoid JWT in URL (logged in server access logs)
    const url = useStore.getState().serverUrl;
    let sseInstance: EventSource | null = null;
    let unmounted = false;

    const connectSse = async () => {
      try {
        const resp = await apiCall<{ ticket: string }>("POST", "/api/timer/ticket");
        if (unmounted) return;
        sseInstance = new EventSource(`${url}/api/timer/sse?ticket=${encodeURIComponent(resp.ticket)}`);
      } catch {
        // Ticket exchange failed — SSE unavailable, rely on polling fallback
        return;
      }

      sseInstance.addEventListener("timer", (e) => {
        try {
          const engine = JSON.parse(e.data) as EngineState;
          useStore.setState({ engine, connected: true, error: null });
        } catch { /* ignore */ }
      });

    // Debounce change events — rapid mutations coalesce into single reload
    const pending = new Set<string>();
    let debounceTimer: ReturnType<typeof setTimeout> | null = null;
    const flushChanges = () => {
      if (pending.has("Tasks")) useStore.getState().loadTasks();
      if (pending.has("Sprints")) {
        useStore.getState().loadTasks();
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

      sseInstance.onerror = () => useStore.setState({ connected: false });
      sseInstance.onopen = () => useStore.setState({ connected: true });
    };
    connectSse();

    // Fallback: poll timer if SSE drops
    const timerFallback = setInterval(() => {
      if (!sseInstance || sseInstance.readyState !== EventSource.OPEN) poll();
    }, 2000);
    const taskSafety = setInterval(loadTasks, 30000);

    return () => {
      unmounted = true;
      sseInstance?.close();
      clearInterval(timerFallback);
      clearInterval(taskSafety);
    };
  }, [token]);

  // Global keyboard shortcuts (#37)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = useStore.getState();
      if (e.key === "Escape" && store.engine?.status === "Running") { store.stop(); }
      if (e.key === " " && store.engine?.status === "Running") { e.preventDefault(); store.pause(); }
      if (e.key === " " && store.engine?.status === "Paused") { e.preventDefault(); store.resume(); }
      // Tab navigation and shortcuts: only when not in an input/select
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag !== "INPUT" && tag !== "TEXTAREA" && tag !== "SELECT" && !(e.target as HTMLElement)?.isContentEditable && !e.ctrlKey && !e.metaKey) {
        if (e.key === "r") { store.loadTasks(); store.toast("Refreshed"); }
        const tabMap: Record<string, string> = { "0": "timer", "1": "tasks", "2": "sprints", "3": "rooms", "4": "history", "5": "settings", "6": "api" };
        if (tabMap[e.key]) { store.setTab(tabMap[e.key]); }
        if (e.key === "n" && store.activeTab === "tasks") {
          e.preventDefault();
          // Focus the new task input if it exists
          document.querySelector<HTMLInputElement>('[data-new-task-input]')?.focus();
        }
        if (e.key === "/") {
          e.preventDefault();
          document.getElementById("task-search")?.focus();
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    // Only reload tasks on tab switch if data is stale (>10s since last load)
    if ((activeTab === "tasks" || activeTab === "sprints") && token) {
      const lastLoad = _tasksLoadedAt;
      if (Date.now() - lastLoad > 10000) loadTasks();
    }
  }, [activeTab, token]);

  const [showShortcuts, setShowShortcuts] = useState(false);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "?") setShowShortcuts(s => !s);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  if (!token) return <AuthScreen />;

  return (
    <MotionConfig reducedMotion="user">
    <div className="flex h-screen bg-[var(--color-bg)]">
      <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-[var(--color-accent)] focus:text-white focus:rounded-lg focus:text-sm">
        {t.skipToContent}
      </a>
      <nav aria-label="Main navigation">
        <Sidebar />
      </nav>
      <main id="main-content" className="flex-1 overflow-hidden relative">
        {/* Loading indicator */}
        {(loading.tasks || loading.history || loading.stats || loading.config) && (
          <div className="absolute top-0 left-0 right-0 h-0.5 z-40 bg-[var(--color-accent)]/20 overflow-hidden">
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
            {activeTab === "timer" && <Timer />}
            {activeTab === "tasks" && <TaskList />}
            {activeTab === "sprints" && <Sprints />}
            {activeTab === "rooms" && <Rooms />}
            {activeTab === "history" && <History />}
            {activeTab === "api" && <ApiReference />}
            {activeTab === "settings" && <Settings />}
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
                  t.type === "error" ? "bg-[var(--color-danger)] text-white" : "bg-[var(--color-success)]/90 text-white"
                }`}>
                <span className="cursor-pointer" onClick={() => dismissToast(t.id)}>{t.msg}</span>
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
                    className="px-4 py-2 text-xs text-white rounded-lg bg-[var(--color-danger)]">{t.delete}</button>
                </div>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
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
                  ["/", t.focusSearch],
                  ["?", t.toggleShortcuts],
                  ["Double-click", t.renameTask],
                  ["Enter", t.saveEdit],
                  ["Escape", t.cancelEdit],
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

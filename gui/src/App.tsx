import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Timer as TimerIcon, ListTodo, BarChart3, Settings as SettingsIcon, Wifi, WifiOff, Code2, LogOut, Users, Zap, Sun, Moon, RefreshCw } from "lucide-react";
import { useStore } from "./store/store";
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
  { id: "timer", icon: TimerIcon, label: "Timer" },
  { id: "tasks", icon: ListTodo, label: "Tasks" },
  { id: "sprints", icon: Zap, label: "Sprints" },
  { id: "rooms", icon: Users, label: "Rooms" },
  { id: "history", icon: BarChart3, label: "History" },
  { id: "api", icon: Code2, label: "API" },
  { id: "settings", icon: SettingsIcon, label: "Settings" },
];

function Sidebar() {
  const { activeTab, setTab, connected, username, logout, activeTeamId, setActiveTeam } = useStore();
  const [theme, setTheme] = useState(() => localStorage.getItem("theme") || "dark");
  const [teams, setTeams] = useState<{ id: number; name: string }[]>([]);

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
        return (
          <motion.button
            key={tab.id}
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            onClick={() => setTab(tab.id)}
            className={`relative w-11 h-11 flex items-center justify-center rounded-xl transition-all ${
              active ? "text-white" : "text-white/30 hover:text-white/60"
            }`}
            title={tab.label}
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
          className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title="Toggle theme">
          {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
        </button>
        <button onClick={() => { useStore.getState().loadTasks(); useStore.getState().toast("Refreshed"); }}
          className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title="Refresh data">
          <RefreshCw size={16} />
        </button>
        <button onClick={logout} className="w-11 h-11 flex items-center justify-center rounded-xl text-white/30 hover:text-white/60 transition-all" title="Logout">
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
  const { activeTab, poll, loadTasks, connected, token, toasts, dismissToast, confirmDialog, dismissConfirm } = useStore();

  useEffect(() => {
    useStore.getState().restoreAuth();
  }, []);

  useEffect(() => {
    if (!token) return;
    poll();
    loadTasks();

    // SSE for real-time timer + data change notifications
    const url = useStore.getState().serverUrl;
    const sse = new EventSource(`${url}/api/timer/sse?token=${encodeURIComponent(token)}`);

    sse.addEventListener("timer", (e) => {
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

    sse.addEventListener("change", (e) => {
      try {
        const kind = JSON.parse(e.data) as string;
        pending.add(kind);
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(flushChanges, 300);
      } catch { /* ignore */ }
    });

    sse.onerror = () => useStore.setState({ connected: false });
    sse.onopen = () => useStore.setState({ connected: true });

    // Fallback: poll timer if SSE drops
    const timerFallback = setInterval(() => {
      if (sse.readyState !== EventSource.OPEN) poll();
    }, 2000);
    const taskSafety = setInterval(loadTasks, 30000);

    return () => {
      sse.close();
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
      if (e.key === "r" && !e.ctrlKey && !e.metaKey) { store.loadTasks(); store.toast("Refreshed"); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    if ((activeTab === "tasks" || activeTab === "sprints") && token) loadTasks();
  }, [activeTab, token]);

  if (!token) return <AuthScreen />;

  return (
    <div className="flex h-screen bg-[var(--color-bg)]">
      <Sidebar />
      <main className="flex-1 overflow-hidden relative">
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
        <div className="absolute top-4 right-4 flex flex-col gap-2 z-50 pointer-events-none">
          <AnimatePresence>
            {toasts.map(t => (
              <motion.div key={t.id} initial={{ opacity: 0, x: 50 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: 50 }}
                onClick={() => dismissToast(t.id)}
                className={`pointer-events-auto cursor-pointer px-4 py-2 rounded-lg text-xs font-medium shadow-lg ${
                  t.type === "error" ? "bg-[var(--color-danger)] text-white" : "bg-[var(--color-success)]/90 text-white"
                }`}>
                {t.msg}
              </motion.div>
            ))}
          </AnimatePresence>
        </div>

        {/* Confirm dialog */}
        <AnimatePresence>
          {confirmDialog && (
            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
              className="absolute inset-0 bg-black/50 flex items-center justify-center z-50"
              onClick={dismissConfirm}>
              <motion.div initial={{ scale: 0.9 }} animate={{ scale: 1 }} exit={{ scale: 0.9 }}
                className="glass p-6 max-w-sm w-full mx-4" onClick={e => e.stopPropagation()}>
                <p className="text-sm text-white/80 mb-4">{confirmDialog.msg}</p>
                <div className="flex gap-2 justify-end">
                  <button onClick={dismissConfirm}
                    className="px-4 py-2 text-xs text-white/50 hover:text-white rounded-lg bg-white/5 hover:bg-white/10">Cancel</button>
                  <button onClick={() => { confirmDialog.onConfirm(); dismissConfirm(); }}
                    className="px-4 py-2 text-xs text-white rounded-lg bg-[var(--color-danger)]">Delete</button>
                </div>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}

import { useEffect, useState } from "react";
import { motion, AnimatePresence, MotionConfig } from "framer-motion";
import { WifiOff } from "lucide-react";
import { useStore } from "./store/store";
import { useT } from "./i18n";
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
import Sidebar, { TABS } from "./components/Sidebar";
import QuickAddFab from "./components/QuickAddFab";


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
    useStore.getState().loadProjects();
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


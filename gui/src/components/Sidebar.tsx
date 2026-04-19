import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Timer as TimerIcon, ListTodo, BarChart3, Settings as SettingsIcon, Wifi, WifiOff, Code2, LogOut, Users, Zap, Sun, Moon, RefreshCw, LayoutDashboard, CalendarDays, Columns3, GanttChart as GanttIcon, Map, Activity } from "lucide-react";
import { useStore } from "../store/store";
import { useT } from "../i18n";
import { apiCall } from "../store/api";
import NotificationBell from "./NotificationBell";

export const TABS = [
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

export default function Sidebar() {
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
            title="All teams">{t.allTeams}</button>
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
        <button onClick={() => { useStore.getState().loadTasks(); useStore.getState().toast(t.refreshed); }}
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

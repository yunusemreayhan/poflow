import { useStore } from "../store/store";
import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Save, Shield, ShieldOff, Check, Trash2 } from "lucide-react";
import type { Config, User, AuthResponse, Team, TeamDetail } from "../store/api";
import { LabelManager } from "./Labels";
import AuditLog from "./AuditLog";
import Select from "./Select";
import { useI18n, useT } from "../i18n";
import { apiCall, setToken } from "../store/api";
import { TemplateManager, WebhookManager, CsvImport, TrashView, NotificationPrefs } from "./SettingsParts";
import TeamManager from "./TeamManager";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between py-3.5 border-b border-white/5 last:border-b-0">
      <span className="text-sm text-white/70">{label}</span>
      {children}
    </div>
  );
}

function NumInput({ value, onChange, min = 1, max = 120, label }: { value: number; onChange: (v: number) => void; min?: number; max?: number; label?: string }) {
  return (
    <input
      type="number"
      value={value}
      min={min}
      max={max}
      onChange={(e) => { const v = Number(e.target.value); onChange(Math.max(min, Math.min(max, v))); }}
      aria-label={label}
      className="w-18 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white text-center outline-none focus:border-[var(--color-accent)]"
    />
  );
}

function Toggle({ value, onChange, label }: { value: boolean; onChange: (v: boolean) => void; label?: string }) {
  return (
    <button
      role="switch"
      aria-checked={value}
      aria-label={label}
      onClick={() => onChange(!value)}
      className={`w-11 h-6 rounded-full transition-all relative ${value ? "bg-[var(--color-accent)]" : "bg-white/10"}`}
    >
      <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all ${value ? "left-5.5" : "left-0.5"}`} />
    </button>
  );
}

function AdminPanel() {
  const [users, setUsers] = useState<User[]>([]);

  const load = () => { apiCall<User[]>("GET", "/api/admin/users").then(u => u && setUsers(u)).catch(() => {}); };
  useEffect(load, []);

  const toggleRole = async (u: User) => {
    const newRole = u.role === "root" ? "user" : "root";
    await apiCall("PUT", `/api/admin/users/${u.id}/role`, { role: newRole });
    load();
  };

  const deleteUser = async (u: User) => {
    useStore.getState().showConfirm(`Delete user "${u.username}"? This cannot be undone.`, async () => {
      await apiCall("DELETE", `/api/admin/users/${u.id}`);
      load();
    });
  };

  return (
    <div className="glass p-5">
      <h3 className="text-sm font-semibold text-white/60 mb-3">User Management</h3>
      <div className="space-y-2">
        {users.map((u) => (
          <div key={u.id} className="flex items-center justify-between py-2 border-b border-white/5 last:border-b-0">
            <div>
              <span className="text-sm text-white/80">{u.username}</span>
              <span className={`ml-2 text-xs px-2 py-0.5 rounded ${u.role === "root" ? "bg-[var(--color-accent)]/20 text-[var(--color-accent)]" : "bg-white/5 text-white/40"}`}>{u.role}</span>
            </div>
            <div className="flex gap-1">
              <button onClick={() => toggleRole(u)}
                className={`w-8 h-8 flex items-center justify-center rounded-lg transition-all ${u.role === "root" ? "text-[var(--color-accent)] hover:text-[var(--color-warning)] hover:bg-white/5" : "text-white/30 hover:text-[var(--color-accent)] hover:bg-white/5"}`}
                title={u.role === "root" ? "Demote to user" : "Promote to root"}>
                {u.role === "root" ? <Shield size={16} /> : <ShieldOff size={16} />}
              </button>
              <button onClick={() => deleteUser(u)}
                className="w-8 h-8 flex items-center justify-center rounded-lg text-white/20 hover:text-[var(--color-danger)] hover:bg-white/5 transition-all"
                title="Delete user">
                <Trash2 size={16} />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function Settings() {
  const { config, loadConfig, updateConfig, username, role, serverUrl, setServerUrl } = useStore();
  const t = useT();
  const { locale, setLocale, availableLocales } = useI18n();
  const [local, setLocal] = useState<Config | null>(null);
  const [saved, setSaved] = useState(false);
  const [newUsername, setNewUsername] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [profileMsg, setProfileMsg] = useState("");
  const [serverDraft, setServerDraft] = useState(serverUrl);

  useEffect(() => { loadConfig(); }, []);
  useEffect(() => { if (config) setLocal({ ...config }); }, [config]);
  useEffect(() => { if (username) setNewUsername(username); }, [username]);

  const saveProfile = async () => {
    try {
      const body: Record<string, string> = {};
      if (newUsername && newUsername !== username) body.username = newUsername;
      if (newPassword) body.password = newPassword;
      if (Object.keys(body).length === 0) return;
      const resp = await apiCall<AuthResponse>("PUT", "/api/profile", body);
      await setToken(resp.token);
      localStorage.setItem("auth", JSON.stringify(resp));
      useStore.setState({ token: resp.token, username: resp.username, role: resp.role });
      setNewPassword("");
      setProfileMsg("Profile updated!");
      setTimeout(() => setProfileMsg(""), 2000);
    } catch (e) {
      setProfileMsg("Error: " + String(e));
    }
  };

  if (!local) return <div className="p-8 text-white/40 text-sm">Loading...</div>;

  const set = (key: keyof Config, val: unknown) => {
    setSaved(false);
    setLocal((prev) => prev ? { ...prev, [key]: val } : prev);
  };

  const isDirty = local && config && JSON.stringify(local) !== JSON.stringify(config);

  const [saving, setSaving] = useState(false);

  const save = async () => {
    if (!local || saving) return;
    setSaving(true);
    try {
      await updateConfig(local);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } finally { setSaving(false); }
  };

  return (
    <div className="flex flex-col gap-5 p-8 h-full overflow-y-auto">
      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">{t.timerDurations}</h3>
        <Field label="Work (minutes)">
          <NumInput value={local.work_duration_min} onChange={(v) => set("work_duration_min", v)} label="Work duration (minutes)" />
        </Field>
        <Field label="Short Break (minutes)">
          <NumInput value={local.short_break_min} onChange={(v) => set("short_break_min", v)} label="Short break (minutes)" />
        </Field>
        <Field label="Long Break (minutes)">
          <NumInput value={local.long_break_min} onChange={(v) => set("long_break_min", v)} label="Long break (minutes)" />
        </Field>
        <Field label="Long Break Interval">
          <NumInput value={local.long_break_interval} onChange={(v) => set("long_break_interval", v)} min={1} max={10} label="Long break interval" />
        </Field>
      </div>

      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">{t.automation}</h3>
        <Field label="Auto-start Breaks">
          <Toggle value={local.auto_start_breaks} onChange={(v) => set("auto_start_breaks", v)} label="Auto-start Breaks" />
        </Field>
        <Field label="Auto-start Work">
          <Toggle value={local.auto_start_work} onChange={(v) => set("auto_start_work", v)} label="Auto-start Work" />
        </Field>
      </div>

      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">{t.notifications}</h3>
        <Field label="Desktop Notifications">
          <Toggle value={local.notification_enabled} onChange={(v) => set("notification_enabled", v)} />
        </Field>
        <Field label="Sound">
          <Toggle value={local.sound_enabled} onChange={(v) => set("sound_enabled", v)} />
        </Field>
        {local.notification_enabled && (
          <div className="ml-4 mt-1 space-y-1 text-xs text-white/40">
            <div>Notifications are sent when:</div>
            <div>• Timer session completes</div>
            <div>• Break ends (if auto-start work is off)</div>
            <div>• Daily goal reached</div>
          </div>
        )}
      </div>

      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">{t.goals}</h3>
        <Field label="Daily Goal (sessions)">
          <NumInput value={local.daily_goal} onChange={(v) => set("daily_goal", v)} min={1} max={20} label="Daily goal" />
        </Field>
        <Field label="Estimation Mode">
          <Select value={local.estimation_mode} onChange={v => set("estimation_mode", v)}
            options={[{value:"hours",label:"Hours"},{value:"points",label:"Story Points"}]} />
        </Field>
        <Field label="Leaf-Only Mode">
          <label className="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" checked={local.leaf_only_mode ?? false} onChange={e => set("leaf_only_mode", e.target.checked)}
              className="accent-[var(--color-accent)]" />
            <span className="text-xs text-white/50">Only leaf tasks (no children) in sprints, voting & timer</span>
          </label>
        </Field>
      </div>

      {/* Account */}
      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">Account</h3>
        <Field label="Username">
          <div className="flex gap-2 items-center">
            <input value={newUsername} onChange={(e) => setNewUsername(e.target.value)}
              className="w-36 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white text-right outline-none focus:border-[var(--color-accent)]" />
          </div>
        </Field>
        <Field label="New Password">
          <input type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} placeholder="unchanged"
            className="w-36 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white text-right outline-none focus:border-[var(--color-accent)] placeholder-white/20" />
        </Field>
        <Field label="Role">
          <span className={`text-sm px-2 py-0.5 rounded ${role === "root" ? "bg-[var(--color-accent)]/20 text-[var(--color-accent)]" : "bg-white/5 text-white/60"}`}>{role}</span>
        </Field>
        {profileMsg && <div className={`text-xs mt-2 ${profileMsg.startsWith("Error") ? "text-[var(--color-danger)]" : "text-[var(--color-success)]"}`}>{profileMsg}</div>}
        <motion.button whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }} onClick={saveProfile}
          className="mt-3 w-full py-2 rounded-lg text-sm font-semibold text-white bg-[var(--color-accent)] flex items-center justify-center gap-2">
          <Check size={14} /> Update Profile
        </motion.button>
      </div>

      {/* Server */}
      <div className="glass p-5">
        <h3 className="text-sm font-semibold text-white/60 mb-3">Server</h3>
        <Field label="Backend URL">
          <div className="flex gap-2 items-center">
            <input value={serverDraft} onChange={(e) => setServerDraft(e.target.value)}
              className="w-52 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white text-right outline-none focus:border-[var(--color-accent)] font-mono" />
            {serverDraft !== serverUrl && (
              <motion.button whileTap={{ scale: 0.95 }}
                onClick={() => setServerUrl(serverDraft)}
                className="px-3 py-1.5 rounded-lg text-xs font-semibold text-white bg-[var(--color-warning)]">
                Switch (logs out)
              </motion.button>
            )}
          </div>
        </Field>
      </div>

      {role === "root" && <AdminPanel />}

      <TeamManager />

      <div className="glass p-6 rounded-2xl">
        <div className="flex items-center gap-3 mb-4">
          <span className="text-sm text-[var(--color-text)]">Language</span>
          <select value={locale} onChange={e => setLocale(e.target.value)} aria-label="Language"
            className="text-xs px-2 py-1 rounded bg-[var(--color-surface)] border border-white/10 text-[var(--color-text)]">
            {availableLocales().map(l => <option key={l} value={l}>{l.toUpperCase()}</option>)}
          </select>
        </div>
        <LabelManager />
        <TemplateManager />
        <NotificationPrefs />
        <WebhookManager />
        <CsvImport />
        <TrashView />
      </div>

      {role === "root" && (
        <div className="glass p-6 rounded-2xl">
          <AuditLog />
        </div>
      )}

      <motion.button
        whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }}
        onClick={save}
        disabled={saving}
        className="sticky bottom-4 flex items-center justify-center gap-2 py-4 rounded-xl font-semibold text-white text-base transition-all z-10"
        style={{ background: saved ? "var(--color-success)" : "var(--color-accent)", opacity: saving ? 0.7 : 1 }}
      >
        <Save size={18} />
        {saving ? "Saving..." : saved ? t.savedChanges : isDirty ? `${t.saveSettings} •` : t.saveSettings}
      </motion.button>
    </div>
  );
}

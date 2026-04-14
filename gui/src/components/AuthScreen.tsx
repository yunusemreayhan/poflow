import { useState } from "react";
import { motion } from "framer-motion";
import { useStore } from "../store/store";
import { useT } from "../i18n";
import { Trash2, Zap } from "lucide-react";

export default function AuthScreen() {
  const { login, register, serverUrl, setServerUrl, savedServers, switchToServer, removeServer } = useStore();
  const t = useT();
  const [isRegister, setIsRegister] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [editingServer, setEditingServer] = useState(false);
  const [showPw, setShowPw] = useState(false);
  const [serverDraft, setServerDraft] = useState(serverUrl);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password.trim()) return;
    // UX7: Client-side password length validation for registration
    if (isRegister && (password.length < 8 || !/[A-Z]/.test(password) || !/\d/.test(password))) { setError("Password must be at least 8 characters with uppercase + digit"); return; }
    setLoading(true);
    setError("");
    try {
      if (isRegister) {
        await register(username.trim(), password);
      } else {
        await login(username.trim(), password);
      }
    } catch (err) {
      setError(String(err));
    }
    setLoading(false);
  };

  return (
    <div className="flex items-center justify-center h-screen bg-[var(--color-bg)]">
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        className="glass p-6 md:p-8 w-full max-w-sm mx-4"
      >
        <div className="text-center mb-6">
          <motion.div
            animate={{ rotate: [0, 360] }}
            transition={{ duration: 20, repeat: Infinity, ease: "linear" }}
            className="w-12 h-12 rounded-full mx-auto mb-4"
            style={{ background: "conic-gradient(from 0deg, #FF6B6B, #4ECDC4, #45B7D1, #7C3AED, #FF6B6B)" }}
          />
          <h1 className="text-xl font-bold text-white">Pomodoro</h1>
          <p className="text-xs text-white/40 mt-1">{isRegister ? t.createAccount : t.signIn}</p>
          <p className="text-xs text-white/30 mt-1">{isRegister ? t.firstUserAdmin : ""}</p>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          {/* Server URL */}
          <div className="flex items-center gap-2 mb-1">
            {editingServer ? (
              <div className="flex-1 flex gap-1">
                <input value={serverDraft} onChange={(e) => setServerDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") { try { new URL(serverDraft); setServerUrl(serverDraft); setEditingServer(false); } catch { setError("Invalid URL format"); } }
                    if (e.key === "Escape") { setServerDraft(serverUrl); setEditingServer(false); }
                  }}
                  className="flex-1 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white outline-none focus:border-[var(--color-accent)] font-mono"
                  autoFocus />
                <button type="button" onClick={() => { try { new URL(serverDraft); setServerUrl(serverDraft); setEditingServer(false); } catch { setError("Invalid URL format"); } }}
                  className="text-xs px-2 py-1 rounded bg-[var(--color-accent)] text-white">✓</button>
                <button type="button" onClick={() => { setServerDraft(serverUrl); setEditingServer(false); }}
                  className="text-xs px-2 py-1 rounded bg-white/10 text-white/60">✕</button>
              </div>
            ) : (
              <button onClick={() => setEditingServer(true)}
                className="flex-1 text-left text-xs text-white/30 hover:text-white/60 font-mono truncate transition-colors flex items-center gap-1"
                title="Click to change server" aria-label="Edit server URL">
                🌐 {serverUrl} <span className="text-[10px] opacity-50">✎</span>
              </button>
            )}
          </div>

          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder={t.username}
            aria-label="Username"
            className="bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]"
            autoFocus
          />
          <div className="relative">
            <input
              type={showPw ? "text" : "password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password (min 8 chars, uppercase + digit)"
              aria-label="Password"
              className="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-3 pr-10 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]"
            />
            <button type="button" onClick={() => setShowPw(!showPw)} className="absolute right-3 top-3 text-white/30 hover:text-white/60 text-xs" aria-label={showPw ? "Hide password" : "Show password"}>
              {showPw ? "🙈" : "👁"}
            </button>
          </div>
          {isRegister && password.length > 0 && (() => {
            const hasLen = password.length >= 8;
            const hasUpper = /[A-Z]/.test(password);
            const hasDigit = /[0-9]/.test(password);
            const hasSpecial = /[^A-Za-z0-9]/.test(password);
            const s = (hasLen ? 1 : 0) + (hasUpper ? 1 : 0) + (hasDigit ? 1 : 0) + (hasSpecial ? 1 : 0);
            const label = ["Weak", "Fair", "Good", "Strong"][Math.min(s, 3)];
            const color = ["#ef4444", "#f59e0b", "#22c55e", "#22c55e"][Math.min(s, 3)];
            const Check = ({ ok, text }: { ok: boolean; text: string }) => <span className={ok ? "text-green-400" : "text-white/25"}>{ok ? "✓" : "○"} {text}</span>;
            return <>
              <div className="flex items-center gap-2 text-xs" role="status" aria-label={`Password strength: ${label}`}><div className="flex-1 h-1 rounded bg-white/10"><div className="h-1 rounded transition-all" style={{ width: `${Math.min(s + 1, 4) * 25}%`, background: color }} /></div><span style={{ color }}>{label}</span></div>
              <div className="flex gap-3 text-[10px]"><Check ok={hasLen} text="8+ chars" /><Check ok={hasUpper} text="Uppercase" /><Check ok={hasDigit} text="Digit" /></div>
            </>;
          })()}

          {error && <div role="alert" className="text-xs text-[var(--color-danger)] bg-[var(--color-danger)]/10 rounded-lg px-3 py-2">{error}</div>}

          <motion.button
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
            type="submit"
            disabled={loading}
            className="py-3 rounded-xl font-semibold text-white text-sm transition-all bg-[var(--color-accent)] disabled:opacity-50"
          >
            {loading ? "..." : isRegister ? t.registerButton : t.loginButton}
          </motion.button>
        </form>

        <button
          onClick={() => { setIsRegister(!isRegister); setError(""); }}
          className="w-full text-center text-xs text-white/40 hover:text-white/70 mt-4 transition-colors"
        >
          {isRegister ? t.switchToLogin : t.switchToRegister}
        </button>

        {savedServers.length > 0 && (
          <div className="mt-4 border-t border-white/5 pt-4">
            <p className="text-xs text-white/30 mb-2">Quick switch</p>
            <div className="space-y-1 max-h-32 overflow-y-auto">
              {savedServers.map((s, i) => (
                <div key={i} className="flex items-center gap-2 group">
                  <button onClick={() => switchToServer(s)}
                    className="flex-1 flex items-center gap-2 text-left px-3 py-2 rounded-lg hover:bg-white/5 transition-all">
                    <Zap size={12} className="text-[var(--color-accent)] shrink-0" />
                    <span className="text-xs text-white/60 truncate">{s.username}</span>
                    <span className="text-[10px] text-white/20 truncate font-mono">{s.url.replace(/^https?:\/\//, "")}</span>
                    <span className={`text-[10px] px-1 rounded ${s.role === "root" ? "text-[var(--color-accent)] bg-[var(--color-accent)]/10" : "text-white/20 bg-white/5"}`}>{s.role}</span>
                  </button>
                  <button onClick={() => removeServer(s.url, s.username)}
                    className="opacity-0 group-hover:opacity-100 w-6 h-6 flex items-center justify-center text-white/30 hover:text-[var(--color-danger)] transition-all shrink-0">
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </motion.div>
    </div>
  );
}

import { useState, useEffect } from "react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";

interface Team { id: number; name: string }
interface TeamDetail { team: Team; members: { user_id: number; username: string; role: string }[]; root_task_ids: number[] }

export default function TeamManager() {
  const [teams, setTeams] = useState<Team[]>([]);
  const [selected, setSelected] = useState<TeamDetail | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [userSearch, setUserSearch] = useState("");
  const [allUsers, setAllUsers] = useState<{ id: number; username: string }[]>([]);
  const [rootSearch, setRootSearch] = useState("");
  const tasks = useStore(s => s.tasks);
  const rootTasks = tasks.filter(t => t.parent_id === null);

  const load = async () => { const t = await apiCall<Team[]>("GET", "/api/me/teams"); if (t) setTeams(t); };
  const loadDetail = async (id: number) => { const d = await apiCall<TeamDetail>("GET", `/api/teams/${id}`); if (d) setSelected(d); };
  useEffect(() => { load(); }, []);
  useEffect(() => { apiCall<{ id: number; username: string }[]>("GET", "/api/users").then(u => u && setAllUsers(u)); }, []);

  const create = async () => { if (!name.trim()) return; await apiCall("POST", "/api/teams", { name: name.trim() }); setName(""); setCreating(false); load(); };
  const addMember = async (userId: number) => { if (!selected) return; await apiCall("POST", `/api/teams/${selected.team.id}/members`, { user_id: userId }); loadDetail(selected.team.id); };
  const removeMember = async (userId: number) => { if (!selected) return; await apiCall("DELETE", `/api/teams/${selected.team.id}/members/${userId}`); loadDetail(selected.team.id); };
  const addRoot = async (taskId: number) => { if (!selected) return; await apiCall("POST", `/api/teams/${selected.team.id}/roots`, { task_ids: [taskId] }); loadDetail(selected.team.id); };
  const removeRoot = async (taskId: number) => { if (!selected) return; await apiCall("DELETE", `/api/teams/${selected.team.id}/roots/${taskId}`); loadDetail(selected.team.id); };
  const del = async (id: number) => { await apiCall("DELETE", `/api/teams/${id}`); if (selected?.team.id === id) setSelected(null); load(); };

  return (
    <div className="glass p-6 rounded-2xl space-y-4">
      <div className="flex items-center gap-2">
        <h3 className="text-sm font-semibold text-white flex-1">Teams</h3>
        <button onClick={() => setCreating(true)} className="text-xs text-[var(--color-accent)]">+ New Team</button>
      </div>
      {creating && (
        <div className="flex gap-2">
          <input placeholder="Team name" value={name} onChange={e => setName(e.target.value)} onKeyDown={e => e.key === "Enter" && create()}
            className="flex-1 bg-transparent border-b border-white/20 text-white text-xs outline-none pb-1" autoFocus />
          <button onClick={create} className="text-xs text-[var(--color-accent)]">Create</button>
          <button onClick={() => setCreating(false)} className="text-xs text-white/30">Cancel</button>
        </div>
      )}
      <div className="flex gap-1 flex-wrap">
        {teams.map(t => (
          <button key={t.id} onClick={() => loadDetail(t.id)}
            className={`px-2 py-1 rounded text-xs transition-colors ${selected?.team.id === t.id ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/50 hover:text-white/70"}`}>{t.name}</button>
        ))}
        {teams.length === 0 && !creating && <span className="text-xs text-white/20">No teams yet</span>}
      </div>
      {selected && (
        <div className="space-y-3 border-t border-white/5 pt-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-white/60">{selected.team.name}</span>
            <button onClick={() => del(selected.team.id)} className="text-xs text-red-400/50 hover:text-red-400">Delete team</button>
          </div>
          <div>
            <div className="text-xs text-white/40 mb-1">Members</div>
            {selected.members.map(m => (
              <div key={m.user_id} className="flex items-center gap-2 text-xs text-white/70 py-0.5">
                <span className="flex-1">{m.username}</span><span className="text-white/30">{m.role}</span>
                <button onClick={() => removeMember(m.user_id)} className="text-red-400/50 hover:text-red-400">✕</button>
              </div>
            ))}
            <input placeholder="Search users to add..." value={userSearch} onChange={e => setUserSearch(e.target.value)}
              className="w-full bg-transparent border-b border-white/10 text-white text-xs outline-none pb-1 mt-1" />
            {userSearch && (
              <div className="max-h-20 overflow-y-auto">
                {allUsers.filter(u => !selected.members.some(m => m.user_id === u.id) && u.username.toLowerCase().includes(userSearch.toLowerCase())).slice(0, 10).map(u => (
                  <button key={u.id} onClick={() => { addMember(u.id); setUserSearch(""); }} className="w-full text-left text-xs text-white/50 hover:text-green-400 py-0.5">+ {u.username}</button>
                ))}
              </div>
            )}
          </div>
          <div>
            <div className="text-xs text-white/40 mb-1">Root Tasks (team scope)</div>
            {selected.root_task_ids.map(rid => {
              const t = tasks.find(tk => tk.id === rid);
              return t ? (
                <div key={rid} className="flex items-center gap-2 text-xs text-white/70 py-0.5">
                  <span className="flex-1 truncate">{t.title}</span>
                  <button onClick={() => removeRoot(rid)} className="text-red-400/50 hover:text-red-400">✕</button>
                </div>
              ) : null;
            })}
            {selected.root_task_ids.length === 0 && <div className="text-xs text-white/20">No root tasks — team sees nothing</div>}
            <input placeholder="Search root tasks..." value={rootSearch} onChange={e => setRootSearch(e.target.value)}
              className="w-full bg-transparent border-b border-white/10 text-white text-xs outline-none pb-1 mt-1" />
            {rootSearch && (
              <div className="max-h-20 overflow-y-auto">
                {rootTasks.filter(t => !selected.root_task_ids.includes(t.id) && t.title.toLowerCase().includes(rootSearch.toLowerCase())).slice(0, 15).map(t => (
                  <button key={t.id} onClick={() => { addRoot(t.id); setRootSearch(""); }} className="w-full text-left text-xs text-white/50 hover:text-green-400 truncate py-0.5">+ {t.title}</button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

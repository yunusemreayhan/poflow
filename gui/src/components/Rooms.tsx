import { useState, useEffect, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Trash2, Plus } from "lucide-react";
import { apiCall } from "../store/api";
import { useStore } from "../store/store";
import type { Room } from "../store/api";
import EstimationRoomView from "./EstimationRoomView";

// --- Room List ---

function RoomList({ onSelect }: { onSelect: (id: number) => void }) {
  const { username } = useStore();
  const [rooms, setRooms] = useState<Room[]>([]);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState("");
  const [unit, setUnit] = useState("points");
  const [project, setProject] = useState("");
  const [loading, setLoading] = useState(true);

  const load = useCallback(() => { apiCall<Room[]>("GET", "/api/rooms").then(r => { setRooms(r); setLoading(false); }).catch(() => setLoading(false)); }, []);
  useEffect(() => {
    load();
    const onSse = () => load();
    window.addEventListener("sse-rooms", onSse);
    return () => window.removeEventListener("sse-rooms", onSse);
  }, [load]);

  const create = async () => {
    if (!name.trim()) return;
    const room = await apiCall<Room>("POST", "/api/rooms", { name: name.trim(), estimation_unit: unit, project: project || null });
    setShowCreate(false); setName(""); setProject("");
    onSelect(room.id);
  };

  const remove = (id: number) => {
    useStore.getState().showConfirm("Delete this room? This cannot be undone.", async () => {
      await apiCall("DELETE", `/api/rooms/${id}`);
      load();
    });
  };

  const [showClosed, setShowClosed] = useState(false);
  const active = rooms.filter(r => showClosed || r.status !== "closed");
  const closed = rooms.filter(r => r.status === "closed");

  return (
    <div className="flex flex-col gap-3 md:gap-5 p-3 md:p-8 h-full overflow-y-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Estimation Rooms</h2>
        <motion.button whileTap={{ scale: 0.95 }} onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[var(--color-accent)] text-white text-sm font-semibold">
          <Plus size={16} /> New Room
        </motion.button>
        <button onClick={() => setShowClosed(!showClosed)} aria-pressed={showClosed}
          className={`text-xs px-2 py-1 rounded ${showClosed ? "bg-white/10 text-white" : "text-white/40"}`}>
          {showClosed ? "Hide closed" : "Show closed"}
        </button>
      </div>

      <AnimatePresence>
        {showCreate && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="glass p-4 space-y-3">
            <input value={name} onChange={e => setName(e.target.value)} placeholder="Room name" autoFocus
              onKeyDown={e => e.key === "Enter" && create()}
              className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]" />
            <div className="flex gap-2">
              {["points", "hours", "mandays"].map(u => (
                <button key={u} onClick={() => setUnit(u)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-all ${unit === u ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white/70"}`}>
                  {u}
                </button>
              ))}
            </div>
            <input value={project} onChange={e => setProject(e.target.value)} placeholder="Project filter (optional)"
              className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]" />
            <div className="flex gap-2">
              <button onClick={create} className="px-4 py-2 rounded-lg bg-[var(--color-accent)] text-white text-sm font-semibold">Create</button>
              <button onClick={() => setShowCreate(false)} className="px-4 py-2 rounded-lg bg-white/5 text-white/40 text-sm">Cancel</button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {loading && rooms.length === 0 && <div className="text-center py-12 text-white/20 text-sm">Loading rooms...</div>}
      {!loading && rooms.length === 0 && <div className="text-center py-12 text-white/20 text-sm">No estimation rooms yet. Create one above to start planning poker!</div>}
      {!loading && active.length === 0 && !showCreate && <div className="text-center py-12"><div className="text-4xl mb-2">🃏</div><p className="text-sm text-white/30">No active rooms</p><p className="text-xs text-white/20 mt-1">Create one to start estimating</p></div>}

      <div className="space-y-2">
        {active.map(r => (
          <motion.div key={r.id} whileHover={{ scale: 1.01 }}
            className="glass p-4 flex items-center gap-3 cursor-pointer group" tabIndex={0} role="button"
            onClick={() => onSelect(r.id)} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onSelect(r.id); } }}>
            <div className="flex-1">
              <div className="text-sm text-white font-semibold">{r.name}</div>
              <div className="text-xs text-white/30 flex gap-3">
                <span>{r.estimation_unit}</span>
                {r.project && <span>📁 {r.project}</span>}
                <span>by {r.creator}</span>
                <span className={`${r.status === "voting" ? "text-[var(--color-warning)]" : r.status === "revealed" ? "text-[var(--color-success)]" : "text-white/30"}`}>{r.status}</span>
              </div>
            </div>
            {(r.creator === username) && (
              <button onClick={e => { e.stopPropagation(); remove(r.id); }}
                className="opacity-0 group-hover:opacity-100 text-white/30 hover:text-[var(--color-danger)] transition-all">
                <Trash2 size={16} />
              </button>
            )}
          </motion.div>
        ))}
      </div>

      {closed.length > 0 && (
        <details className="text-white/30">
          <summary className="text-xs cursor-pointer hover:text-white/50">Closed rooms ({closed.length})</summary>
          <div className="space-y-1 mt-2">
            {closed.map(r => (
              <div key={r.id} className="flex items-center gap-2 text-xs text-white/20 px-2 py-1">
                <span>{r.name}</span>
                <span>{r.estimation_unit}</span>
                <span>{r.created_at.slice(0, 10)}</span>
                <button onClick={() => onSelect(r.id)} className="text-white/30 hover:text-white/50 ml-auto">view</button>
              </div>
            ))}
          </div>
        </details>
      )}
    </div>
  );
}

// --- Main Rooms Component ---

export default function Rooms() {
  const [selectedRoom, setSelectedRoom] = useState<number | null>(null);

  if (selectedRoom !== null) {
    return <EstimationRoomView roomId={selectedRoom} onBack={() => setSelectedRoom(null)} />;
  }

  return <RoomList onSelect={setSelectedRoom} />;
}

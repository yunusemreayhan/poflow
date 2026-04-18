import { useEffect, useState } from "react";
import { apiCall } from "../store/api";

export default function TimerChecklist({ taskId }: { taskId: number }) {
  const [items, setItems] = useState<{ id: number; title: string; checked: boolean }[]>([]);
  useEffect(() => { apiCall<typeof items>("GET", `/api/tasks/${taskId}/checklist`).then(setItems).catch(() => {}); }, [taskId]);
  if (items.length === 0) return null;
  const toggle = async (id: number, checked: boolean) => {
    await apiCall("PUT", `/api/checklist/${id}`, { checked: !checked }).catch(() => {});
    setItems(prev => prev.map(i => i.id === id ? { ...i, checked: !checked } : i));
  };
  const done = items.filter(i => i.checked).length;
  return (
    <div className="w-full max-w-xs">
      <div className="flex items-center gap-2 mb-1 justify-center">
        <span className="text-[10px] text-white/30">{done}/{items.length} checklist</span>
        <div className="w-16 h-1 bg-white/5 rounded-full overflow-hidden"><div className="h-full bg-green-500 rounded-full transition-all" style={{ width: `${(done / items.length) * 100}%` }} /></div>
      </div>
      <div className="space-y-0.5">
        {items.map(item => (
          <button key={item.id} onClick={() => toggle(item.id, item.checked)}
            className={`w-full flex items-center gap-2 text-xs px-2 py-0.5 rounded hover:bg-white/5 text-left ${item.checked ? "text-white/20 line-through" : "text-white/50"}`}>
            <span className={`w-3 h-3 rounded border flex items-center justify-center text-[8px] shrink-0 ${item.checked ? "bg-green-500/20 border-green-500/50 text-green-400" : "border-white/20"}`}>{item.checked ? "✓" : ""}</span>
            <span className="truncate">{item.title}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

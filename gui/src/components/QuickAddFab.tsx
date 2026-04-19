import { useState } from "react";
import { useStore } from "../store/store";
import { useT } from "../i18n";

export default function QuickAddFab() {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const token = useStore(s => s.token);
  const createTask = useStore(s => s.createTask);
  const t = useT();
  if (!token) return null;
  const submit = async () => {
    if (!title.trim()) return;
    await createTask(title.trim());
    setTitle("");
    setOpen(false);
    useStore.getState().toast(t.taskCreated);
  };
  return (
    <>
      <button onClick={() => setOpen(true)} className="md:hidden fixed bottom-16 right-4 z-30 w-12 h-12 rounded-full bg-[var(--color-accent)] text-white shadow-lg flex items-center justify-center text-2xl" aria-label="Quick add task">+</button>
      {open && (
        <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/50 md:hidden" onClick={() => setOpen(false)}>
          <div className="w-full bg-[var(--color-bg)] border-t border-white/10 p-4 safe-bottom" onClick={e => e.stopPropagation()}>
            <input autoFocus value={title} onChange={e => setTitle(e.target.value)} onKeyDown={e => e.key === "Enter" && submit()}
              placeholder={t.newTaskPlaceholder} className="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-3 text-sm text-white placeholder-white/30 outline-none focus:border-[var(--color-accent)]" />
            <div className="flex gap-2 mt-2">
              <button onClick={() => setOpen(false)} className="flex-1 py-2 rounded-lg text-xs text-white/40">{t.cancel}</button>
              <button onClick={submit} className="flex-1 py-2 rounded-lg text-xs bg-[var(--color-accent)] text-white font-medium">{t.createButton}</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

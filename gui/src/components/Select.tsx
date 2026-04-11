import { useState, useRef, useEffect } from "react";
import { ChevronDown } from "lucide-react";

type Option = { value: string; label: string; disabled?: boolean };

export default function Select({ value, options, onChange, className = "", placeholder }: {
  value: string;
  options: Option[];
  onChange: (v: string) => void;
  className?: string;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(-1);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  // Reset focused index when opening
  useEffect(() => {
    if (open) {
      const idx = options.findIndex(o => o.value === value);
      setFocused(idx >= 0 ? idx : 0);
    }
  }, [open, options, value]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!open) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
        e.preventDefault();
        setOpen(true);
      }
      return;
    }
    const enabledOptions = options.map((o, i) => ({ ...o, i })).filter(o => !o.disabled);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      const cur = enabledOptions.findIndex(o => o.i === focused);
      const next = enabledOptions[(cur + 1) % enabledOptions.length];
      if (next) setFocused(next.i);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const cur = enabledOptions.findIndex(o => o.i === focused);
      const prev = enabledOptions[(cur - 1 + enabledOptions.length) % enabledOptions.length];
      if (prev) setFocused(prev.i);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const opt = options[focused];
      if (opt && !opt.disabled) { onChange(opt.value); setOpen(false); }
    } else if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    } else if (e.key === "Home") {
      e.preventDefault();
      const first = enabledOptions[0];
      if (first) setFocused(first.i);
    } else if (e.key === "End") {
      e.preventDefault();
      const last = enabledOptions[enabledOptions.length - 1];
      if (last) setFocused(last.i);
    }
  };

  const selected = options.find(o => o.value === value);

  return (
    <div ref={ref} className={`relative ${className}`} role="combobox" aria-expanded={open} aria-haspopup="listbox" onKeyDown={handleKeyDown}>
      <button type="button" onClick={() => setOpen(!open)}
        aria-label={placeholder || "Select option"}
        className="w-full flex items-center justify-between gap-2 bg-[var(--color-surface)] border border-white/10 rounded-lg px-3 py-1.5 text-sm text-[var(--color-text)] outline-none hover:border-white/20 focus:border-[var(--color-accent)] transition-colors">
        <span className={selected ? "text-[var(--color-text)]" : "text-[var(--color-text)]/40"}>{selected?.label || placeholder || "Select..."}</span>
        <ChevronDown size={14} className="text-[var(--color-text)] opacity-40 transition-transform" style={open ? {transform:"rotate(180deg)"} : {}} />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 w-full max-h-60 overflow-auto rounded-lg border border-white/10 bg-[var(--color-surface)] shadow-xl" role="listbox">
          {options.map((o, i) => (
            <button key={o.value} type="button" disabled={o.disabled}
              role="option" aria-selected={o.value === value}
              onClick={() => { if (!o.disabled) { onChange(o.value); setOpen(false); } }}
              onMouseEnter={() => setFocused(i)}
              className={`w-full text-left px-3 py-1.5 text-sm transition-colors
                ${o.disabled ? "text-[var(--color-text)] opacity-20 cursor-default" : "text-[var(--color-text)] hover:bg-black/10 cursor-pointer"}
                ${o.value === value ? "bg-black/5 text-[var(--color-accent)]" : ""}
                ${i === focused && !o.disabled ? "bg-[var(--color-accent)]/10 outline-none" : ""}`}>
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

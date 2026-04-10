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
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const selected = options.find(o => o.value === value);

  return (
    <div ref={ref} className={`relative ${className}`}>
      <button type="button" onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between gap-2 bg-[var(--color-surface)] border border-white/10 rounded-lg px-3 py-1.5 text-sm text-[var(--color-text)] outline-none hover:border-white/20 transition-colors">
        <span className={selected ? "text-[var(--color-text)]" : "text-[var(--color-text)]/40"}>{selected?.label || placeholder || "Select..."}</span>
        <ChevronDown size={14} className="text-[var(--color-text)] opacity-40 transition-transform" style={open ? {transform:"rotate(180deg)"} : {}} />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 w-full max-h-60 overflow-auto rounded-lg border border-white/10 bg-[var(--color-surface)] shadow-xl">
          {options.map(o => (
            <button key={o.value} type="button" disabled={o.disabled}
              onClick={() => { if (!o.disabled) { onChange(o.value); setOpen(false); } }}
              className={`w-full text-left px-3 py-1.5 text-sm transition-colors
                ${o.disabled ? "text-[var(--color-text)] opacity-20 cursor-default" : "text-[var(--color-text)] hover:bg-black/10 cursor-pointer"}
                ${o.value === value ? "bg-black/5 text-[var(--color-accent)]" : ""}`}>
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

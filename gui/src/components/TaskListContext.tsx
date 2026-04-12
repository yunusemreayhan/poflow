import { createContext } from "react";
export const SearchCtx = createContext("");

export function highlightMatch(text: string, query: string) {
  if (!query) return text;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return text;
  return <>{text.slice(0, idx)}<mark className="bg-[var(--color-accent)]/30 text-inherit rounded px-0.5">{text.slice(idx, idx + query.length)}</mark>{text.slice(idx + query.length)}</>;
}

export let ctxCacheTime = 0;
export function setCtxCacheTime(t: number) { ctxCacheTime = t; }

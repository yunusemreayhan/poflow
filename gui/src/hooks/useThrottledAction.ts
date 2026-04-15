import { useRef, useCallback } from "react";

/**
 * Returns a wrapped version of the action that ignores calls while a previous
 * call is still in-flight. Prevents double-submit from rapid clicks.
 */
export function useThrottledAction<T extends (...args: any[]) => Promise<any>>(action: T): [T, boolean] {
  const busyRef = useRef(false);
  const wrapped = useCallback(async (...args: any[]) => {
    if (busyRef.current) return;
    busyRef.current = true;
    try { return await action(...args); }
    finally { busyRef.current = false; }
  }, [action]) as unknown as T;
  return [wrapped, busyRef.current];
}

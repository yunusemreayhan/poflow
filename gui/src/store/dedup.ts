const _inflight = new Set<string>();

export function dedup(key: string, fn: () => Promise<void>): Promise<void> {
  if (_inflight.has(key)) return Promise.resolve();
  _inflight.add(key);
  return fn().finally(() => _inflight.delete(key));
}

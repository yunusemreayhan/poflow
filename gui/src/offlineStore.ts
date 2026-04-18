// F16: Offline storage with IndexedDB + sync queue

import type { Task } from './store/types';

const DB_NAME = 'pomo-offline';
const DB_VERSION = 1;

// PF9: Singleton DB connection
let dbInstance: IDBDatabase | null = null;

function openDB(): Promise<IDBDatabase> {
  if (dbInstance) return Promise.resolve(dbInstance);
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('tasks')) db.createObjectStore('tasks', { keyPath: 'id' });
      if (!db.objectStoreNames.contains('syncQueue')) db.createObjectStore('syncQueue', { keyPath: 'queueId', autoIncrement: true });
    };
    req.onsuccess = () => { dbInstance = req.result; dbInstance.onclose = () => { dbInstance = null; }; resolve(dbInstance); };
    req.onerror = () => reject(req.error);
  });
}

function tx(db: IDBDatabase, store: string, mode: IDBTransactionMode): IDBObjectStore {
  return db.transaction(store, mode).objectStore(store);
}

function reqToPromise<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => { req.onsuccess = () => resolve(req.result); req.onerror = () => reject(req.error); });
}

// --- Tasks cache ---

export async function cacheTasksOffline(tasks: Task[]): Promise<void> {
  try {
    const db = await openDB();
    const store = tx(db, 'tasks', 'readwrite');
    for (const t of tasks) store.put(t);
  } catch (e) {
    // V29-16: Handle quota exceeded by clearing old data
    if (e instanceof DOMException && e.name === 'QuotaExceededError') {
      try {
        const db = await openDB();
        tx(db, 'tasks', 'readwrite').clear();
        const store = tx(db, 'tasks', 'readwrite');
        for (const t of tasks) store.put(t);
      } catch { /* give up */ }
    }
  }
}

export async function getOfflineTasks(): Promise<Task[]> {
  const db = await openDB();
  const result = await reqToPromise(tx(db, 'tasks', 'readonly').getAll());
  return result;
}

// --- Sync queue ---

interface SyncEntry {
  queueId?: number;
  method: string;
  url: string;
  body?: unknown;
  createdAt: string;
}

export async function enqueueOfflineAction(method: string, url: string, body?: unknown): Promise<void> {
  const db = await openDB();
  const store = tx(db, 'syncQueue', 'readwrite');
  store.add({ method, url, body, createdAt: new Date().toISOString() } as SyncEntry);
}

export async function getSyncQueue(): Promise<SyncEntry[]> {
  const db = await openDB();
  const result = await reqToPromise(tx(db, 'syncQueue', 'readonly').getAll());
  return result;
}

export async function clearSyncEntry(queueId: number): Promise<void> {
  const db = await openDB();
  tx(db, 'syncQueue', 'readwrite').delete(queueId);
}

export async function processSyncQueue(_token: string): Promise<{ synced: number; failed: number }> {
  // V35-14: Use apiCall instead of raw fetch to get token refresh, CSRF header, and error toasts
  const { apiCall: api } = await import('./store/api');
  const queue = await getSyncQueue();
  let synced = 0, failed = 0;
  for (const entry of queue) {
    try {
      // B4: url may be a relative path (new) or full URL (legacy entries). Always resolve to path.
      const path = entry.url.startsWith('/') ? entry.url : (() => { try { return new URL(entry.url).pathname; } catch { return entry.url; } })();
      await api(entry.method, path, entry.body);
      if (entry.queueId) await clearSyncEntry(entry.queueId);
      synced++;
    } catch (e) {
      // V36-19: Treat 409 Conflict as success (skip conflicting entry)
      if (String(e).includes('409') || String(e).includes('Conflict')) {
        if (entry.queueId) await clearSyncEntry(entry.queueId);
        synced++;
      } else { failed++; }
    }
  }
  return { synced, failed };
}

export function isOnline(): boolean {
  return navigator.onLine;
}

// F16: Offline storage with IndexedDB + sync queue

const DB_NAME = 'pomo-offline';
const DB_VERSION = 1;

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('tasks')) db.createObjectStore('tasks', { keyPath: 'id' });
      if (!db.objectStoreNames.contains('syncQueue')) db.createObjectStore('syncQueue', { keyPath: 'queueId', autoIncrement: true });
    };
    req.onsuccess = () => resolve(req.result);
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

export async function cacheTasksOffline(tasks: unknown[]): Promise<void> {
  const db = await openDB();
  const store = tx(db, 'tasks', 'readwrite');
  for (const t of tasks) store.put(t);
  db.close();
}

export async function getOfflineTasks(): Promise<unknown[]> {
  const db = await openDB();
  const result = await reqToPromise(tx(db, 'tasks', 'readonly').getAll());
  db.close();
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
  db.close();
}

export async function getSyncQueue(): Promise<SyncEntry[]> {
  const db = await openDB();
  const result = await reqToPromise(tx(db, 'syncQueue', 'readonly').getAll());
  db.close();
  return result;
}

export async function clearSyncEntry(queueId: number): Promise<void> {
  const db = await openDB();
  tx(db, 'syncQueue', 'readwrite').delete(queueId);
  db.close();
}

export async function processSyncQueue(token: string): Promise<{ synced: number; failed: number }> {
  const queue = await getSyncQueue();
  let synced = 0, failed = 0;
  for (const entry of queue) {
    try {
      const headers: Record<string, string> = { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}`, 'x-requested-with': 'pomo-offline' };
      const opts: RequestInit = { method: entry.method, headers };
      if (entry.body && entry.method !== 'GET') opts.body = JSON.stringify(entry.body);
      const resp = await fetch(entry.url, opts);
      if (resp.ok || resp.status === 409) { // 409 = conflict, skip
        if (entry.queueId) await clearSyncEntry(entry.queueId);
        synced++;
      } else { failed++; }
    } catch { failed++; }
  }
  return { synced, failed };
}

export function isOnline(): boolean {
  return navigator.onLine;
}

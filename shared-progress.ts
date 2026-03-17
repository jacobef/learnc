const PROGRESS_PREFIX = "cboxes-progress-v1:";

type StoredProgress<T> = {
  version: 1;
  savedAt: number;
  state: T;
};

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function currentLevelId(): string {
  const path = String(window.location.pathname || "").trim();
  const leaf = path.split("/").filter(Boolean).pop() || path || "index.html";
  return leaf || "index.html";
}

export function levelProgressKey(levelId: string = currentLevelId()): string {
  return `${PROGRESS_PREFIX}${levelId}`;
}

export function readLevelProgress<T>(
  levelId: string = currentLevelId(),
): T | null {
  const store = storage();
  if (!store) return null;
  const raw = store.getItem(levelProgressKey(levelId));
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as StoredProgress<T> | T | null;
    if (!parsed || typeof parsed !== "object") return null;
    if ("state" in parsed) return (parsed as StoredProgress<T>).state ?? null;
    return parsed as T;
  } catch {
    return null;
  }
}

export function writeLevelProgress<T>(
  state: T,
  levelId: string = currentLevelId(),
): void {
  const store = storage();
  if (!store) return;
  const payload: StoredProgress<T> = {
    version: 1,
    savedAt: Date.now(),
    state,
  };
  try {
    store.setItem(levelProgressKey(levelId), JSON.stringify(payload));
  } catch {
    // Ignore storage quota / privacy-mode failures.
  }
}

export function clearLevelProgress(levelId: string = currentLevelId()): void {
  const store = storage();
  if (!store) return;
  try {
    store.removeItem(levelProgressKey(levelId));
  } catch {
    // Ignore storage failures.
  }
}

export function savedLevelIds(): string[] {
  const store = storage();
  if (!store) return [];
  const ids: string[] = [];
  try {
    for (let i = 0; i < store.length; i++) {
      const key = store.key(i);
      if (!key || !key.startsWith(PROGRESS_PREFIX)) continue;
      ids.push(key.slice(PROGRESS_PREFIX.length));
    }
  } catch {
    return [];
  }
  ids.sort();
  return ids;
}

export function savedLevelCount(): number {
  return savedLevelIds().length;
}

export function clearAllLevelProgress(): void {
  const store = storage();
  if (!store) return;
  const keys = savedLevelIds().map((id) => levelProgressKey(id));
  try {
    keys.forEach((key) => store.removeItem(key));
  } catch {
    // Ignore storage failures.
  }
}

export function maybeRestoreLevelProgress<T>(
  levelId: string = currentLevelId(),
  _label = "this level",
): T | null {
  return readLevelProgress<T>(levelId);
}

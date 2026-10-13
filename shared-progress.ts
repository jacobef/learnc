const PROGRESS_PREFIX = "cboxes-progress-v1:";

type StoredProgress<T> = {
  version: 1;
  state: T;
};

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function readStoredText(key: string): string | null {
  try {
    return storage()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function removeStoredText(key: string): void {
  try {
    storage()?.removeItem(key);
  } catch {
    // Saving progress is optional when browser storage is unavailable.
  }
}

function windowNameState(): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(window.name || "{}");
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // window.name may belong to another page or contain malformed saved data.
  }
  return {};
}

function writeWindowNameProgress(value: string | null): void {
  try {
    const state = windowNameState();
    if (value === null) delete state[sandboxProgressKey()];
    else state[sandboxProgressKey()] = value;
    window.name = Object.keys(state).length ? JSON.stringify(state) : "";
  } catch {
    // This fallback is best effort, just like localStorage.
  }
}

function pathParts(): { folder: string | undefined; file: string } {
  const parts = window.location.pathname.trim().split("/").filter(Boolean);
  const last = parts[parts.length - 1] ?? "";
  return last.includes(".")
    ? { folder: parts[parts.length - 2], file: last }
    : { folder: last, file: "index.html" };
}

// The work-in-progress copy of the site, in a wip folder, keeps its own progress,
// so every saved id there starts with "wip/".
function siteScope(): string {
  return pathParts().folder === "wip" ? "wip/" : "";
}

function sandboxProgressKey(): string {
  return `cboxes:sandbox-state:v1${siteScope() ? ":wip" : ""}`;
}

// A lesson's file name, wherever the site is hosted.
export function currentLevelId(): string {
  return siteScope() + pathParts().file;
}

function levelProgressKey(levelId: string): string {
  return `${PROGRESS_PREFIX}${levelId}`;
}

export function readLevelProgress<T>(
  levelId: string = currentLevelId(),
): T | null {
  const raw = readStoredText(levelProgressKey(levelId));
  if (!raw) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || !("version" in parsed) ||
        parsed.version !== 1 || !("state" in parsed)) return null;
    return parsed.state as T | null;
  } catch {
    return null;
  }
}

export function writeLevelProgress<T>(
  state: T,
  levelId: string = currentLevelId(),
): void {
  const payload: StoredProgress<T> = { version: 1, state };
  try {
    storage()?.setItem(levelProgressKey(levelId), JSON.stringify(payload));
  } catch {
    // Ignore storage quota / privacy-mode failures.
  }
}

export function clearLevelProgress(levelId: string = currentLevelId()): void {
  removeStoredText(levelProgressKey(levelId));
}

// The ids saved by this copy of the site.
function savedLevelIds(): string[] {
  const store = storage();
  if (!store) return [];
  const scope = siteScope();
  try {
    const ids: string[] = [];
    for (let i = 0; i < store.length; i++) {
      const key = store.key(i);
      if (!key?.startsWith(PROGRESS_PREFIX)) continue;
      const id = key.slice(PROGRESS_PREFIX.length);
      if (scope ? id.startsWith(scope) : !id.startsWith("wip/")) ids.push(id);
    }
    return ids;
  } catch {
    return [];
  }
}

export function savedLevelCount(): number {
  return savedLevelIds().length;
}

export function readSandboxProgress(): string | null {
  // A failed localStorage write may leave an older value behind. The fallback
  // is newer until a successful write clears it.
  const fallback = windowNameState()[sandboxProgressKey()];
  return typeof fallback === "string" ? fallback : readStoredText(sandboxProgressKey());
}

export function writeSandboxProgress(value: string): void {
  try {
    const store = storage();
    if (store) {
      store.setItem(sandboxProgressKey(), value);
      writeWindowNameProgress(null);
      return;
    }
  } catch {
    // Preserve the latest edit in window.name when localStorage rejects it.
  }
  writeWindowNameProgress(value);
}

export function hasSandboxProgress(): boolean {
  return Boolean(readSandboxProgress());
}

export function clearAllLevelProgress(): void {
  for (const id of savedLevelIds()) clearLevelProgress(id);
}

export function clearSandboxProgress(): void {
  removeStoredText(sandboxProgressKey());
  writeWindowNameProgress(null);
}

const PROGRESS_PREFIX = "cboxes-progress-v1:";
const SANDBOX_PROGRESS_KEY = "cboxes:sandbox-state:v1";
function storage() {
    try {
        return window.localStorage;
    }
    catch {
        return null;
    }
}
function readStoredText(key) {
    try {
        return storage()?.getItem(key) ?? null;
    }
    catch {
        return null;
    }
}
function removeStoredText(key) {
    try {
        storage()?.removeItem(key);
    }
    catch {
        // Saving progress is optional when browser storage is unavailable.
    }
}
function windowNameState() {
    try {
        const parsed = JSON.parse(window.name || "{}");
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
            return parsed;
        }
    }
    catch {
        // window.name may belong to another page or contain malformed saved data.
    }
    return {};
}
function writeWindowNameProgress(value) {
    try {
        const state = windowNameState();
        if (value === null)
            delete state[SANDBOX_PROGRESS_KEY];
        else
            state[SANDBOX_PROGRESS_KEY] = value;
        window.name = Object.keys(state).length ? JSON.stringify(state) : "";
    }
    catch {
        // This fallback is best effort, just like localStorage.
    }
}
export function currentLevelId() {
    const path = window.location.pathname.trim();
    return path.split("/").filter(Boolean).pop() || "index.html";
}
function levelProgressKey(levelId) {
    return `${PROGRESS_PREFIX}${levelId}`;
}
export function readLevelProgress(levelId = currentLevelId()) {
    const raw = readStoredText(levelProgressKey(levelId));
    if (!raw)
        return null;
    try {
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object" || !("version" in parsed) ||
            parsed.version !== 1 || !("state" in parsed))
            return null;
        return parsed.state;
    }
    catch {
        return null;
    }
}
export function writeLevelProgress(state, levelId = currentLevelId()) {
    const payload = { version: 1, state };
    try {
        storage()?.setItem(levelProgressKey(levelId), JSON.stringify(payload));
    }
    catch {
        // Ignore storage quota / privacy-mode failures.
    }
}
export function clearLevelProgress(levelId = currentLevelId()) {
    removeStoredText(levelProgressKey(levelId));
}
function savedLevelIds() {
    const store = storage();
    if (!store)
        return [];
    try {
        const ids = [];
        for (let i = 0; i < store.length; i++) {
            const key = store.key(i);
            if (key?.startsWith(PROGRESS_PREFIX))
                ids.push(key.slice(PROGRESS_PREFIX.length));
        }
        return ids;
    }
    catch {
        return [];
    }
}
export function savedLevelCount() {
    return savedLevelIds().length;
}
export function readSandboxProgress() {
    // A failed localStorage write may leave an older value behind. The fallback
    // is newer until a successful write clears it.
    const fallback = windowNameState()[SANDBOX_PROGRESS_KEY];
    return typeof fallback === "string" ? fallback : readStoredText(SANDBOX_PROGRESS_KEY);
}
export function writeSandboxProgress(value) {
    try {
        const store = storage();
        if (store) {
            store.setItem(SANDBOX_PROGRESS_KEY, value);
            writeWindowNameProgress(null);
            return;
        }
    }
    catch {
        // Preserve the latest edit in window.name when localStorage rejects it.
    }
    writeWindowNameProgress(value);
}
export function hasSandboxProgress() {
    return Boolean(readSandboxProgress());
}
export function clearAllLevelProgress() {
    for (const id of savedLevelIds())
        clearLevelProgress(id);
}
export function clearSandboxProgress() {
    removeStoredText(SANDBOX_PROGRESS_KEY);
    writeWindowNameProgress(null);
}

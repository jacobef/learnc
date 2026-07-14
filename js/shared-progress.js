const PROGRESS_PREFIX = "cboxes-progress-v1:";
const SANDBOX_PROGRESS_KEY = "cboxes:sandbox-state:v1";
function isStoredProgress(value) {
    return ("version" in value &&
        value.version === 1 &&
        "state" in value);
}
function storage() {
    try {
        return window.localStorage;
    }
    catch {
        return null;
    }
}
export function currentLevelId() {
    const path = String(window.location.pathname || "").trim();
    const leaf = path.split("/").filter(Boolean).pop() || path || "index.html";
    return leaf || "index.html";
}
export function levelProgressKey(levelId = currentLevelId()) {
    return `${PROGRESS_PREFIX}${levelId}`;
}
export function readLevelProgress(levelId = currentLevelId()) {
    const store = storage();
    if (!store)
        return null;
    const raw = store.getItem(levelProgressKey(levelId));
    if (!raw)
        return null;
    try {
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object")
            return null;
        if (isStoredProgress(parsed))
            return parsed.state ?? null;
        return parsed;
    }
    catch {
        return null;
    }
}
export function writeLevelProgress(state, levelId = currentLevelId()) {
    const store = storage();
    if (!store)
        return;
    const payload = {
        version: 1,
        savedAt: Date.now(),
        state,
    };
    try {
        store.setItem(levelProgressKey(levelId), JSON.stringify(payload));
    }
    catch {
        // Ignore storage quota / privacy-mode failures.
    }
}
export function clearLevelProgress(levelId = currentLevelId()) {
    const store = storage();
    if (!store)
        return;
    try {
        store.removeItem(levelProgressKey(levelId));
    }
    catch {
        // Ignore storage failures.
    }
}
export function savedLevelIds() {
    const store = storage();
    if (!store)
        return [];
    const ids = [];
    try {
        for (let i = 0; i < store.length; i++) {
            const key = store.key(i);
            if (!key || !key.startsWith(PROGRESS_PREFIX))
                continue;
            ids.push(key.slice(PROGRESS_PREFIX.length));
        }
    }
    catch {
        return [];
    }
    ids.sort();
    return ids;
}
export function savedLevelCount() {
    return savedLevelIds().length;
}
export function hasSandboxProgress() {
    const store = storage();
    try {
        if (store?.getItem(SANDBOX_PROGRESS_KEY))
            return true;
    }
    catch {
        // Fall through to window.name fallback.
    }
    try {
        const parsed = JSON.parse(window.name || "{}");
        return typeof parsed[SANDBOX_PROGRESS_KEY] === "string";
    }
    catch {
        return false;
    }
}
export function clearAllLevelProgress() {
    const store = storage();
    if (!store)
        return;
    const keys = savedLevelIds().map((id) => levelProgressKey(id));
    try {
        keys.forEach((key) => store.removeItem(key));
    }
    catch {
        // Ignore storage failures.
    }
}
export function clearSandboxProgress() {
    const store = storage();
    try {
        store?.removeItem(SANDBOX_PROGRESS_KEY);
    }
    catch {
        // Ignore storage failures.
    }
    try {
        const parsed = JSON.parse(window.name || "{}");
        if (!(SANDBOX_PROGRESS_KEY in parsed))
            return;
        delete parsed[SANDBOX_PROGRESS_KEY];
        window.name = Object.keys(parsed).length ? JSON.stringify(parsed) : "";
    }
    catch {
        // Ignore window.name fallback failures.
    }
}
export function maybeRestoreLevelProgress(levelId = currentLevelId()) {
    return readLevelProgress(levelId);
}

// Run with: node scripts/test-progress.ts (after tsc -p .).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  clearAllLevelProgress,
  clearSandboxProgress,
  currentLevelId,
  hasSandboxProgress,
  readLevelProgress,
  readSandboxProgress,
  savedLevelCount,
  writeLevelProgress,
  writeSandboxProgress,
} from "../js/shared-progress.js";

const sandboxKey = "cboxes:sandbox-state:v1";

function browserStorage() {
  const values = new Map<string, string>();
  const storage = {
    get length() { return values.size; },
    key: (index: number) => [...values.keys()][index] ?? null,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value); },
    removeItem: (key: string) => { values.delete(key); },
  };
  const window = { localStorage: storage, name: "", location: { pathname: "/lessons/4-code-editing-i.html" } };
  Object.assign(globalThis, { window });
  return { window, storage, values };
}

test("lesson saves round-trip, stay scoped to lessons, and clear without removing other storage", () => {
  const { values } = browserStorage();
  assert.equal(currentLevelId(), "4-code-editing-i.html");
  // The work-in-progress copy of the site keeps its own progress.
  const location = window.location;
  Object.defineProperty(window, "location", { value: { pathname: "/wip/4-code-editing-i.html" }, configurable: true });
  assert.equal(currentLevelId(), "wip/4-code-editing-i.html");
  Object.defineProperty(window, "location", { value: { pathname: "/wip/" }, configurable: true });
  assert.equal(currentLevelId(), "wip/index.html");
  Object.defineProperty(window, "location", { value: location, configurable: true });
  writeLevelProgress({ text: "int cloud;", pass: true });
  writeLevelProgress({ boundary: 3 }, "1-assignment-i.html");
  assert.deepEqual(readLevelProgress(), { text: "int cloud;", pass: true });
  assert.equal(savedLevelCount(), 2);
  values.set("unrelated", "keep me");
  writeSandboxProgress("sandbox code");
  clearAllLevelProgress();
  assert.equal(savedLevelCount(), 0);
  assert.equal(readLevelProgress(), null);
  assert.equal(values.get("unrelated"), "keep me");
  assert.equal(readSandboxProgress(), "sandbox code");
});

test("the work-in-progress copy of the site counts and clears only its own progress", () => {
  const { values } = browserStorage();
  writeLevelProgress({ boundary: 3 }, "1-assignment-i.html");
  writeSandboxProgress("main sandbox");
  Object.defineProperty(window, "location", { value: { pathname: "/wip/4-code-editing-i.html" }, configurable: true });
  writeLevelProgress({ boundary: 1 });
  assert.equal(readSandboxProgress(), null);
  writeSandboxProgress("wip sandbox");
  assert.equal(savedLevelCount(), 1);
  clearAllLevelProgress();
  clearSandboxProgress();
  assert.equal(savedLevelCount(), 0);
  Object.defineProperty(window, "location", { value: { pathname: "/4-code-editing-i.html" }, configurable: true });
  assert.equal(savedLevelCount(), 1);
  assert.equal(readSandboxProgress(), "main sandbox");
  assert.ok(values.has("cboxes-progress-v1:1-assignment-i.html"));
});

test("unavailable storage and invalid lesson payloads do not break page initialization", () => {
  const { storage, values } = browserStorage();
  for (const value of ["null", "[]", "not JSON", '{"version":2,"state":{}}', '{"text":"old unversioned state"}']) {
    values.set("cboxes-progress-v1:4-code-editing-i.html", value);
    assert.equal(readLevelProgress(), null);
  }
  storage.getItem = () => { throw new Error("read denied"); };
  assert.equal(readLevelProgress(), null);
  Object.defineProperty(globalThis.window, "localStorage", { get() { throw new Error("storage denied"); } });
  assert.doesNotThrow(() => writeLevelProgress({ text: "ignored" }));
  assert.equal(savedLevelCount(), 0);
});

test("a newer fallback wins over stale local data and is retired when local saves recover", () => {
  const { window, storage } = browserStorage();
  storage.setItem(sandboxKey, "old");
  window.name = JSON.stringify({ unrelated: "keep me" });
  const setItem = storage.setItem;
  storage.setItem = () => { throw new Error("quota exceeded"); };
  writeSandboxProgress("new");
  assert.equal(readSandboxProgress(), "new");
  assert.equal(hasSandboxProgress(), true);
  storage.setItem = setItem;
  writeSandboxProgress("newest");
  assert.equal(readSandboxProgress(), "newest");
  assert.deepEqual(JSON.parse(window.name), { unrelated: "keep me" });
  clearSandboxProgress();
  assert.equal(readSandboxProgress(), null);
  assert.equal(hasSandboxProgress(), false);
  assert.deepEqual(JSON.parse(window.name), { unrelated: "keep me" });
});

test("sandbox fallback handles malformed, null, primitive, and array window names", () => {
  const { window, storage } = browserStorage();
  storage.setItem = () => { throw new Error("quota exceeded"); };
  for (const name of ["null", "[]", '"named window"', "broken JSON"]) {
    window.name = name;
    writeSandboxProgress("saved");
    assert.equal(readSandboxProgress(), "saved");
    clearSandboxProgress();
    assert.equal(readSandboxProgress(), null);
  }
});

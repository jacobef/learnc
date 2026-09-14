import { REPLAY_ENDPOINT } from "./shared-replay-config.js";
import { REPLAY_ATTRIBUTES, REPLAY_TAGS } from "./shared-replay-protocol.js";
import type { ReplayEvent, ReplayNode } from "./shared-replay-protocol.js";

let started = false;
const preferenceKey = "cboxes:share-level-replays";

export function replaySharingEnabled(): boolean {
  try { return localStorage.getItem(preferenceKey) !== "off"; }
  catch { return false; }
}

export function setReplaySharing(enabled: boolean): void {
  localStorage.setItem(preferenceKey, enabled ? "on" : "off");
  window.dispatchEvent(new Event("replay-preference-change"));
}

export function startLevelReplay(root: HTMLElement): void {
  const level = location.pathname.split("/").pop() || "";
  if (started || !REPLAY_ENDPOINT || !replaySharingEnabled() || !/^\d+-[a-z0-9-]+\.html$/.test(level)) return;
  started = true;
  // Setup happens after the lesson's initial synchronous render, before input.
  queueMicrotask(() => {
    try { recordLevel(root, level); } catch { /* Telemetry must never break a lesson. */ }
  });
}

function recordLevel(root: HTMLElement, level: string): void {
  const worker = new Worker(new URL("./shared-replay-worker.js", import.meta.url), { type: "module" });
  const clock = performance.now();
  let live = true;
  let nextId = 1;
  const ids = new WeakMap<Node, number>();
  const listeners = new AbortController();
  const id = (node: Node): number => {
    let value = ids.get(node);
    if (!value) { value = nextId++; ids.set(node, value); }
    return value;
  };
  const emit = (type: ReplayEvent["type"], data: ReplayEvent["data"]) => {
    if (live) worker.postMessage({ kind: "event", event: { t: Math.round(performance.now() - clock), type, data } });
  };
  const serialize = (node: Node): ReplayNode | null => {
    if (node.nodeType === Node.TEXT_NODE) return { id: id(node), text: node.textContent || "" };
    if (!(node instanceof HTMLElement) || !REPLAY_TAGS.has(node.localName) || node.closest("[data-replay-ignore]")) return null;
    const attrs: Record<string, string> = {};
    for (const name of REPLAY_ATTRIBUTES) {
      if (node.hasAttribute(name)) attrs[name] = node.getAttribute(name)!;
    }
    const result: ReplayNode = { id: id(node), tag: node.localName, attrs,
      children: [...node.childNodes].map(serialize).filter((child): child is ReplayNode => child !== null) };
    if (node instanceof HTMLTextAreaElement || node instanceof HTMLInputElement || node instanceof HTMLSelectElement) {
      if (node instanceof HTMLInputElement && node.type === "password") return null;
      result.value = node.value;
    }
    return result;
  };
  // Replace only changed subtrees. No screenshots, full-page scans per keystroke,
  // layout measurements, JSON encoding, compression, or fetch on the UI thread.
  const recordMutations = (records: MutationRecord[]) => {
    if (!live) return;
    try {
      const changed = new Set<Node>();
      for (const record of records) {
        if (root.contains(record.target)) changed.add(record.target);
      }
      const patches: ReplayNode[] = [];
      for (const node of changed) {
        let parent = node.parentNode;
        while (parent && !changed.has(parent)) parent = parent.parentNode;
        if (parent) continue;
        const patch = serialize(node);
        if (patch) patches.push(patch);
      }
      if (patches.length) emit("patch", { nodes: patches });
    } catch { stop(); }
  };
  const observer = new MutationObserver(recordMutations);
  function stop() {
    live = false;
    observer.disconnect();
    listeners.abort();
    worker.terminate();
  }
  worker.addEventListener("error", (event) => { event.preventDefault(); stop(); });
  worker.addEventListener("message", (event) => { if (event.data?.kind === "stop") stop(); });
  worker.postMessage({ kind: "init", endpoint: REPLAY_ENDPOINT, level });
  emit("start", { root: serialize(root), mobile: matchMedia("(max-width: 900px)").matches });
  observer.observe(root, { subtree: true, childList: true, characterData: true,
    attributes: true, attributeFilter: REPLAY_ATTRIBUTES });
  const listen = (target: EventTarget, name: string, fn: (event: Event) => void, capture = false) => {
    target.addEventListener(name, (event) => {
      if (!live) return;
      try { fn(event); } catch { stop(); }
    }, { capture, passive: true, signal: listeners.signal });
  };
  listen(root, "input", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement) || target.closest("[data-replay-ignore]")) return;
    if (target instanceof HTMLInputElement && target.type === "password") return;
    recordMutations(observer.takeRecords());
    const value = target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement || target instanceof HTMLSelectElement
      ? target.value : target.textContent || "";
    emit("input", { target: id(target), value });
  });
  listen(root, "click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement) || target.closest("[data-replay-ignore]")) return;
    recordMutations(observer.takeRecords());
    emit("click", { target: id(target) });
    const button = target.closest<HTMLButtonElement>("button[data-role$='-check']");
    if (!button || button.disabled) return;
    // Capture the result after the synchronous Check handler and its DOM updates.
    // A separate task then asks the Worker to upload its immutable history prefix.
    setTimeout(() => {
      if (!live) return;
      try {
        recordMutations(observer.takeRecords());
        emit("check", { target: id(button) });
        worker.postMessage({ kind: "submit" });
      } catch { stop(); }
    }, 0);
  }, true);
  listen(root, "focusin", (event) => {
    if (event.target instanceof HTMLElement) emit("focus", { target: id(event.target) });
  });
  let scrollTimer = 0;
  listen(document, "scroll", (event) => {
    if (scrollTimer) return;
    const target = event.target;
    if (target !== document && (!(target instanceof HTMLElement) || !root.contains(target))) return;
    scrollTimer = window.setTimeout(() => {
      scrollTimer = 0;
      try {
        if (target === document) emit("scroll", { target: 0, x: Math.round(scrollX), y: Math.round(scrollY) });
        else if (target instanceof HTMLElement) emit("scroll", { target: id(target), x: Math.round(target.scrollLeft), y: Math.round(target.scrollTop) });
      } catch { stop(); }
    }, 100);
  }, true);
  const mobile = matchMedia("(max-width: 900px)");
  listen(mobile, "change", () => emit("layout", { mobile: mobile.matches }));
  // No persistence or shared worker: navigating to another level destroys history.
  // Keep a BFCache page alive, but tear it down when navigation actually discards it.
  listen(window, "pagehide", (event) => { if (!(event as PageTransitionEvent).persisted) stop(); });
  const checkPreference = () => { if (!replaySharingEnabled()) stop(); };
  listen(window, "storage", checkPreference);
  listen(window, "replay-preference-change", checkPreference);
  listen(window, "pageshow", checkPreference);
}

import { REPLAY_LIMIT_BYTES, REPLAY_LIMIT_EVENTS } from "./shared-replay-protocol.js";
import type { LevelReplay, ReplayEvent } from "./shared-replay-protocol.js";

let endpoint = "";
let level = "";
let history: ReplayEvent[] = [];
let bytes = 0;
let stopped = false;
const pending = new Set<AbortController>();

async function upload(events: ReplayEvent[]) {
  const abort = new AbortController();
  // Every Check starts an upload. If a connection stalls, cancel the oldest
  // upload before starting a fourth; never accumulate an unbounded retry queue.
  if (pending.size >= 3) {
    const oldest = pending.values().next().value!;
    pending.delete(oldest);
    oldest.abort();
  }
  pending.add(abort);
  const timeout = setTimeout(() => abort.abort(), 10_000);
  try {
    const replay: LevelReplay = { version: 1, level, events };
    const json = JSON.stringify(replay);
    const body = typeof CompressionStream === "function"
      ? await new Response(new Blob([json]).stream().pipeThrough(new CompressionStream("gzip"), { signal: abort.signal })).blob()
      : new Blob([json], { type: "application/json" });
    await fetch(endpoint, {
      method: "POST", body, mode: "cors", credentials: "omit", referrerPolicy: "no-referrer",
      redirect: "error", cache: "no-store", signal: abort.signal,
      headers: { "Content-Type": typeof CompressionStream === "function" ? "application/gzip" : "application/json" },
    });
  } catch { /* Best effort; no UI feedback, retry loop, or durable upload queue. */ }
  finally { clearTimeout(timeout); pending.delete(abort); }
}

self.addEventListener("message", (message: MessageEvent) => {
  if (stopped) return;
  try {
    const data = message.data;
    if (data.kind === "init") { endpoint = data.endpoint; level = data.level; return; }
    if (data.kind === "event") {
      bytes += JSON.stringify(data.event).length * 2;
      if (bytes > REPLAY_LIMIT_BYTES || history.length >= REPLAY_LIMIT_EVENTS) {
        stopped = true;
        history = [];
        self.postMessage({ kind: "stop" });
        return;
      }
      history.push(data.event);
    } else if (data.kind === "submit" && endpoint) {
      // The copy freezes this Check's endpoint while later interaction continues.
      void upload(history.slice());
    }
  } catch {
    stopped = true;
    history = [];
    self.postMessage({ kind: "stop" });
  }
});

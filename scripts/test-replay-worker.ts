// Run after tsc -p .: node --test scripts/test-replay-worker.ts
import assert from "node:assert/strict";
import { test } from "node:test";
import { gunzipSync } from "node:zlib";
import { setImmediate } from "node:timers/promises";

let run = 0;
async function boot(fetch: typeof globalThis.fetch) {
  const messages: unknown[] = [];
  const worker = Object.assign(new EventTarget(), { postMessage: (data: unknown) => messages.push(data) });
  Object.assign(globalThis, { self: worker, fetch });
  await import(`../js/shared-replay-worker.js?test=${++run}`);
  const send = (data: unknown) => worker.dispatchEvent(new MessageEvent("message", { data }));
  send({ kind: "init", endpoint: "https://collector.example/replays", level: "2-declaration.html" });
  return { messages, send };
}
async function until(predicate: () => boolean) {
  for (let i = 0; i < 1000 && !predicate(); i++) await setImmediate();
  assert.ok(predicate(), "Worker did not finish expected background work");
}
const event = (t: number) => ({ kind: "event", event: { t, type: "check", data: { target: 1 } } });

test("each submission freezes its own full history and omits browser credentials", async () => {
  const uploads: Array<{ url: string; options: RequestInit; replay: { events: Array<{ t: number }> } }> = [];
  const { send } = await boot(async (url, options) => {
    const bytes = await (options!.body as Blob).arrayBuffer();
    uploads.push({ url: String(url), options: options!, replay: JSON.parse(gunzipSync(new Uint8Array(bytes)).toString()) });
    return new Response(null, { status: 204 });
  });
  send(event(1)); send({ kind: "submit" });
  send(event(2)); send({ kind: "submit" });
  await until(() => uploads.length === 2);
  assert.deepEqual(uploads.map(upload => upload.replay.events.map(e => e.t)), [[1], [1, 2]]);
  for (const upload of uploads) {
    assert.equal(upload.url, "https://collector.example/replays");
    assert.equal(upload.options.credentials, "omit");
    assert.equal(upload.options.referrerPolicy, "no-referrer");
    assert.equal(upload.options.redirect, "error");
    assert.equal(upload.options.cache, "no-store");
  }
});

test("failed requests are swallowed and do not prevent later Check submissions", async () => {
  let calls = 0;
  const { send } = await boot(async () => { calls++; throw new Error("offline"); });
  send(event(1)); send({ kind: "submit" });
  await until(() => calls === 1);
  send(event(2)); send({ kind: "submit" });
  await until(() => calls === 2);
});

test("stalled uploads are bounded by aborting the oldest without dropping a new Check", async () => {
  const requests: Array<{ signal: AbortSignal; resolve: (response: Response) => void }> = [];
  const { send } = await boot((_url, options) => new Promise((resolve, reject) => {
    const signal = options!.signal!;
    requests.push({ signal, resolve });
    if (signal.aborted) reject(new Error("aborted"));
    else signal.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
  }));
  for (let n = 1; n <= 3; n++) { send(event(n)); send({ kind: "submit" }); }
  await until(() => requests.length === 3);
  for (let n = 4; n <= 6; n++) { send(event(n)); send({ kind: "submit" }); }
  await until(() => requests.length === 6);
  assert.equal(requests.slice(0, 3).every(request => request.signal.aborted), true);
  assert.equal(requests.filter(request => !request.signal.aborted).length, 3);
  for (const request of requests) request.resolve(new Response(null, { status: 204 }));
});

test("excessive history stops recording instead of submitting an incomplete replay", async () => {
  let uploads = 0;
  const { send, messages } = await boot(async () => { uploads++; return new Response(null, { status: 204 }); });
  send({ kind: "event", event: { t: 1, type: "input", data: { value: "x".repeat(5 * 1024 * 1024) } } });
  send({ kind: "submit" });
  await setImmediate();
  assert.deepEqual(messages, [{ kind: "stop" }]);
  assert.equal(uploads, 0);
});

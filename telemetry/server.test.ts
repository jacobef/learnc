import assert from "node:assert/strict";
import { test } from "node:test";
import { gzipSync, gunzipSync } from "node:zlib";
import { collector } from "./server.ts";
import { validReplay } from "./validation.ts";

const key = "a".repeat(64);
const replay = () => ({ version: 1, level: "2-declaration.html", events: [
  { t: 0, type: "start", data: { mobile: true, root: { id: 1, tag: "div", attrs: { class: "main" }, children: [] } } },
  { t: 400, type: "check", data: { target: 1 } },
] });

test("schema rejects identifiers, clock times, unsafe markup and incomplete histories", () => {
  assert.equal(validReplay(replay()), true);
  for (const key of ["ip", "sessionId", "userId", "url", "userAgent", "referrer", "timestamp"]) {
    assert.equal(validReplay({ ...replay(), [key]: "private" }), false);
  }
  const unsafe = replay();
  Object.assign(unsafe.events[0].data.root!.attrs, { onclick: "alert(1)" });
  assert.equal(validReplay(unsafe), false);
  const backwards = replay();
  backwards.events[1].t = -1;
  assert.equal(validReplay(backwards), false);
  const incomplete = replay();
  incomplete.events.pop();
  assert.equal(validReplay(incomplete), false);
});

test("collector accepts gzip, drops all request metadata, rejects bad origins and oversized decompression", async () => {
  const stored: Buffer[] = [];
  const revisions: unknown[] = [];
  const server = collector(async (body, revision) => { stored.push(body); revisions.push(revision); });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address() as { port: number };
  const url = `http://127.0.0.1:${address.port}/replays`;
  try {
    const headers = { Origin: "https://www.learnc.dev", "Content-Type": "application/gzip", "X-Replay-Key": key, "X-Forwarded-For": "203.0.113.9", Cookie: "identity=private", "User-Agent": "private-browser", Referer: "https://private.example/" };
    const result = await fetch(url, { method: "POST", headers, body: gzipSync(JSON.stringify(replay())) });
    assert.equal(result.status, 204);
    assert.equal(result.headers.get("set-cookie"), null);
    assert.deepEqual(JSON.parse(gunzipSync(stored[0]).toString()), replay());
    assert.deepEqual(revisions, [{ key, level: "2-declaration.html", eventCount: 2 }]);
    assert.equal((await fetch(url, { method: "POST", headers: { ...headers, Origin: "https://other.example" }, body: gzipSync(JSON.stringify(replay())) })).status, 403);
    assert.equal((await fetch(url, { method: "POST", headers, body: gzipSync('x'.repeat(9 * 1024 * 1024)) })).status, 503);
    assert.equal((await fetch(url, { method: "POST", headers: { ...headers, "X-Replay-Key": "" }, body: gzipSync(JSON.stringify(replay())) })).status, 400);
    assert.equal(stored.length, 1);
    const preflight = await fetch(url, { method: "OPTIONS", headers });
    assert.equal(preflight.status, 204);
    assert.match(preflight.headers.get("Access-Control-Allow-Headers")!, /X-Replay-Key/);
    assert.equal((await fetch(url, { method: "GET", headers })).status, 405);
  } finally { await new Promise<void>(resolve => server.close(() => resolve())); }
});

test("storage failure returns a bounded error without exposing content", async () => {
  const server = collector(async () => { throw new Error("private storage error"); });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address() as { port: number };
  try {
    const result = await fetch(`http://127.0.0.1:${address.port}/replays`, { method: "POST", headers: { Origin: "https://learnc.dev", "Content-Type": "application/json", "X-Replay-Key": key }, body: JSON.stringify(replay()) });
    assert.equal(result.status, 503);
    assert.equal(await result.text(), "");
  } finally { await new Promise<void>(resolve => server.close(() => resolve())); }
});

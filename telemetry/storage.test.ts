import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { gzipSync, gunzipSync } from "node:zlib";
import { cloudStore, localStore, objectName } from "./storage.ts";

const revision = (eventCount: number) => ({ key: "a".repeat(64), level: "1-assignment-i.html", eventCount });
const body = (count: number) => gzipSync(JSON.stringify({ events: Array(count).fill({ type: "check" }) }));

test("local storage retains one latest replay across reordered writes and restarts", async () => {
  const directory = await mkdtemp(`${tmpdir()}/replay-storage-`);
  try {
    const store = await localStore(directory);
    await Promise.all([store(body(2), revision(2)), store(body(4), revision(4)), store(body(3), revision(3))]);
    await (await localStore(directory))(body(2), revision(2));
    assert.deepEqual(await readdir(directory), [objectName(revision(4))]);
    assert.equal(JSON.parse(gunzipSync(await readFile(`${directory}/${objectName(revision(4))}`)).toString()).events.length, 4);
    assert.notEqual(objectName(revision(4)), objectName({ ...revision(4), level: "2-declaration.html" }));
    assert.notEqual(objectName(revision(4)), objectName({ ...revision(4), key: "b".repeat(64) }));
  } finally { await rm(directory, { recursive: true }); }
});

for (const first of [2, 3]) test(`cloud replacement handles concurrent generation conflicts when revision ${first} wins first`, async () => {
  let generation = 0;
  let count = 0;
  let stored = Buffer.alloc(0);
  let release!: () => void;
  const firstWritten = new Promise<void>(resolve => { release = resolve; });
  const store = cloudStore("test-bucket", async (input, options) => {
    const url = String(input);
    if (url.startsWith("http://metadata.google.internal")) return Response.json({ access_token: "test", expires_in: 3600 });
    if (options?.method !== "POST") return generation
      ? Response.json({ generation: String(generation), metadata: { eventCount: String(count) } })
      : new Response(null, { status: 404 });
    const data = Buffer.from(options.body as Uint8Array);
    const metadata = JSON.parse(data.toString().split("\r\n\r\n")[1].split("\r\n--")[0]);
    const incoming = Number(metadata.metadata.eventCount);
    if (incoming !== first) await firstWritten;
    const match = Number(new URL(url).searchParams.get("ifGenerationMatch"));
    if (match !== generation) return new Response(null, { status: 412 });
    generation++;
    count = incoming;
    stored = data;
    release();
    return Response.json({ generation: String(generation) });
  });
  await Promise.all([store(body(2), revision(2)), store(body(3), revision(3))]);
  assert.equal(count, 3);
  const finalGeneration = generation;
  await store(body(2), revision(2));
  await store(body(3), revision(3));
  assert.equal(generation, finalGeneration, "stale and duplicate submissions do not write");
  assert.ok(stored.includes(body(3)));
  assert.ok(!stored.includes(Buffer.from(revision(3).key)), "write capability is never stored");
});

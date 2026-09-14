import { createServer } from "node:http";
import { gunzip, gzip } from "node:zlib";
import { promisify } from "node:util";
import { pathToFileURL } from "node:url";
import { cloudStore, localStore } from "./storage.ts";
import type { Store } from "./storage.ts";
import { MAX_BYTES, validReplay } from "./validation.ts";

const unzip = promisify(gunzip);
const zip = promisify(gzip);
const origins = new Set(["https://learnc.dev", "https://www.learnc.dev"]);

// This service never reads or logs IPs, User-Agent, Referer, cookies, or trace
// headers. The Cloud Run request-log exclusion must exist BEFORE deployment.
export function collector(store: Store, local = false) {
  return createServer({ requestTimeout: 15_000, headersTimeout: 10_000 }, async (req, res) => {
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("X-Content-Type-Options", "nosniff");
    const origin = req.headers.origin;
    const allowed = typeof origin === "string" && (origins.has(origin) || (local && /^http:\/\/(127\.0\.0\.1|localhost):\d+$/.test(origin)));
    if (allowed) {
      res.setHeader("Access-Control-Allow-Origin", origin!);
      res.setHeader("Vary", "Origin");
      res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
      res.setHeader("Access-Control-Allow-Headers", "Content-Type, X-Replay-Key");
      res.setHeader("Access-Control-Max-Age", "3600");
    }
    if (req.url === "/health" && req.method === "GET") { res.writeHead(204).end(); return; }
    if (req.url !== "/replays" || !allowed) { res.writeHead(403).end(); return; }
    if (req.method === "OPTIONS") { res.writeHead(204).end(); return; }
    if (req.method !== "POST") { res.writeHead(405).end(); return; }
    const key = req.headers["x-replay-key"];
    if (typeof key !== "string" || !/^[a-f0-9]{64}$/.test(key)) { res.writeHead(400).end(); return; }
    const contentType = req.headers["content-type"];
    if (contentType !== "application/gzip" && contentType !== "application/json") { res.writeHead(415).end(); return; }
    try {
      const chunks: Buffer[] = [];
      let size = 0;
      for await (const chunk of req) {
        size += chunk.length;
        if (size > MAX_BYTES) { res.writeHead(413).end(); return; }
        chunks.push(chunk);
      }
      const body = Buffer.concat(chunks);
      const decoded = contentType === "application/gzip" ? await unzip(body, { maxOutputLength: MAX_BYTES }) : body;
      let replay: unknown;
      try { replay = JSON.parse(decoded.toString("utf8")); }
      catch { res.writeHead(400).end(); return; }
      if (!validReplay(replay)) { res.writeHead(400).end(); return; }
      // The write key stays out of saved replay content and request logs.
      await store(await zip(Buffer.from(JSON.stringify(replay))), { key, level: replay.level, eventCount: replay.events.length });
      res.writeHead(204).end();
    } catch {
      // Do not log errors with request context or submitted content.
      res.writeHead(503).end();
    }
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const directory = process.env.REPLAY_LOCAL_DIR;
  const store = directory ? await localStore(directory) : cloudStore(process.env.REPLAY_BUCKET || "");
  collector(store, !!directory).listen(Number(process.env.PORT || 8080), directory ? "127.0.0.1" : "0.0.0.0");
}

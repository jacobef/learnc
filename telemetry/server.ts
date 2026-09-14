import { createServer } from "node:http";
import { randomUUID } from "node:crypto";
import { gunzip, gzip } from "node:zlib";
import { promisify } from "node:util";
import { mkdir, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { MAX_BYTES, validReplay } from "./validation.ts";

const unzip = promisify(gunzip);
const zip = promisify(gzip);
const origins = new Set(["https://learnc.dev", "https://www.learnc.dev"]);
type Store = (body: Buffer) => Promise<void>;

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
      res.setHeader("Access-Control-Allow-Headers", "Content-Type");
      res.setHeader("Access-Control-Max-Age", "3600");
    }
    if (req.url === "/health" && req.method === "GET") { res.writeHead(204).end(); return; }
    if (req.url !== "/replays" || !allowed) { res.writeHead(403).end(); return; }
    if (req.method === "OPTIONS") { res.writeHead(204).end(); return; }
    if (req.method !== "POST") { res.writeHead(405).end(); return; }
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
      // Store only validated replay content; no request envelope or received-at
      // timestamp. Filenames are independently random, never client identifiers.
      await store(await zip(Buffer.from(JSON.stringify(replay))));
      res.writeHead(204).end();
    } catch {
      // Do not log errors with request context or submitted content.
      res.writeHead(503).end();
    }
  });
}

let accessToken = "";
let tokenExpires = 0;
async function storeCloud(body: Buffer) {
  const bucket = process.env.REPLAY_BUCKET;
  if (!bucket) throw new Error("Missing bucket configuration");
  if (Date.now() >= tokenExpires) {
    const response = await fetch("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token", {
      headers: { "Metadata-Flavor": "Google" }, signal: AbortSignal.timeout(3000),
    });
    if (!response.ok) throw new Error("Service authentication failed");
    const token = await response.json() as { access_token: string; expires_in: number };
    accessToken = token.access_token;
    tokenExpires = Date.now() + Math.max(0, token.expires_in - 60) * 1000;
  }
  const objectName = `${randomUUID()}.json.gz`;
  const url = `https://storage.googleapis.com/upload/storage/v1/b/${encodeURIComponent(bucket)}/o?uploadType=media&name=${objectName}&ifGenerationMatch=0`;
  const response = await fetch(url, { method: "POST", headers: { Authorization: `Bearer ${accessToken}`, "Content-Type": "application/gzip" },
    body: new Uint8Array(body), signal: AbortSignal.timeout(8000) });
  if (!response.ok) throw new Error("Storage unavailable");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const directory = process.env.REPLAY_LOCAL_DIR;
  if (directory) await mkdir(directory, { recursive: true, mode: 0o700 });
  const store: Store = directory
    ? body => writeFile(`${directory}/${randomUUID()}.json.gz`, body, { mode: 0o600 })
    : storeCloud;
  collector(store, !!directory).listen(Number(process.env.PORT || 8080), directory ? "127.0.0.1" : "0.0.0.0");
}

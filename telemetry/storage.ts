import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile, rename } from "node:fs/promises";
import { gunzipSync } from "node:zlib";

export interface Revision { key: string; level: string; eventCount: number }
export type Store = (body: Buffer, revision: Revision) => Promise<void>;
export const objectName = ({ key, level }: Revision) => `${createHash("sha256").update(`${level}:${key}`).digest("hex")}.json.gz`;

// Local development uses one process, serializes each attempt's writes, and
// atomically replaces its file. Revision state is recovered from the file.
export async function localStore(directory: string): Promise<Store> {
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const pending = new Map<string, Promise<void>>();
  return async (body, revision) => {
    const file = `${directory}/${objectName(revision)}`;
    const task = (pending.get(file) || Promise.resolve()).catch(() => {}).then(async () => {
      try {
        const previous = JSON.parse(gunzipSync(await readFile(file)).toString());
        if (previous.events.length >= revision.eventCount) return;
      } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
      const temporary = `${file}.${randomUUID()}.tmp`;
      await writeFile(temporary, body, { mode: 0o600 });
      await rename(temporary, file);
    });
    pending.set(file, task);
    try { await task; }
    finally { if (pending.get(file) === task) pending.delete(file); }
  };
}

export function cloudStore(bucket: string, request: typeof fetch = fetch): Store {
  let accessToken = "";
  let tokenExpires = 0;
  return async (body, revision) => {
    if (!bucket) throw new Error("Missing bucket configuration");
    const signal = AbortSignal.timeout(8000);
    if (Date.now() >= tokenExpires) {
      const response = await request("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token", {
        headers: { "Metadata-Flavor": "Google" }, signal,
      });
      if (!response.ok) throw new Error("Service authentication failed");
      const token = await response.json() as { access_token: string; expires_in: number };
      accessToken = token.access_token;
      tokenExpires = Date.now() + Math.max(0, token.expires_in - 60) * 1000;
    }
    const name = objectName(revision);
    const headers = { Authorization: `Bearer ${accessToken}` };
    // Generation preconditions make the comparison atomic across instances and
    // out-of-order requests. A slow earlier Check can never replace a later one.
    for (let attempt = 0; attempt < 4; attempt++) {
      const previous = await request(`https://storage.googleapis.com/storage/v1/b/${encodeURIComponent(bucket)}/o/${name}?fields=generation,metadata`, { headers, signal });
      let generation = "0";
      if (previous.ok) {
        const state = await previous.json() as { generation: string; metadata?: { eventCount?: string } };
        const count = Number(state.metadata?.eventCount);
        if (!Number.isInteger(count) || count < 2) throw new Error("Invalid stored revision");
        if (count >= revision.eventCount) return;
        generation = state.generation;
      } else if (previous.status !== 404) throw new Error("Storage unavailable");
      const boundary = randomUUID();
      const metadata = JSON.stringify({ name, contentType: "application/gzip", cacheControl: "no-store", metadata: { eventCount: String(revision.eventCount) } });
      const multipart = Buffer.concat([
        Buffer.from(`--${boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n${metadata}\r\n--${boundary}\r\nContent-Type: application/gzip\r\n\r\n`),
        body, Buffer.from(`\r\n--${boundary}--\r\n`),
      ]);
      const response = await request(`https://storage.googleapis.com/upload/storage/v1/b/${encodeURIComponent(bucket)}/o?uploadType=multipart&ifGenerationMatch=${encodeURIComponent(generation)}`, {
        method: "POST", headers: { ...headers, "Content-Type": `multipart/related; boundary=${boundary}` }, body: new Uint8Array(multipart), signal,
      });
      if (response.ok) return;
      if (response.status !== 412) throw new Error("Storage unavailable");
    }
    throw new Error("Replay update contention");
  };
}

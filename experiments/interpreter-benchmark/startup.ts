import assert from "node:assert/strict";
import { runCFiles, wasmMetadata } from "./runtime.ts";

const samples: number[] = [];
for (let i = 0; i < 6; i++) {
  const start = performance.now();
  const result = runCFiles([{ path: "startup.c", source: "int main(void) { return 0; }" }], 4096, "", false);
  samples.push(performance.now() - start);
  assert.equal(result.kind, "ok", JSON.stringify(result));
  assert.equal(result.exitStatus, 0);
}
console.log(JSON.stringify({ ...wasmMetadata, firstCallMs: samples[0], warmMs: samples.slice(1) }));

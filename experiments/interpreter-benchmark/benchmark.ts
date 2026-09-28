// Run: node experiments/interpreter-benchmark/benchmark.ts
// Requires Clang, Cargo, and an already-generated js/ interpreter payload.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir, cpus } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { runCFiles, wasmMetadata } from "./runtime.ts";
const dir = fileURLToPath(new URL(".", import.meta.url));
const build = process.env.BENCH_BUILD_DIR ?? mkdtempSync(join(tmpdir(), "cboxes-bench-"));
const source = readFileSync(join(dir, "kernels.c"), "utf8");
const cases = [
  { name: "integer mixing", kind: 0, n: 500 },
  { name: "integer mixing", kind: 0, n: 2000 },
  { name: "array updates", kind: 1, n: 256 },
  { name: "array updates", kind: 1, n: 1024 },
  { name: "recursive calls", kind: 2, n: 10 },
  { name: "recursive calls", kind: 2, n: 13 },
];
for (const opt of process.env.BENCH_SKIP_BUILD ? [] : ["O0", "O2"]) {
  execFileSync("clang", [`-${opt}`, "-std=c11", join(dir, "native.c"), "-o", join(build, opt)]);
}
if (!process.env.BENCH_SKIP_BUILD) execFileSync("cargo", ["build", "--offline", "--release", "--manifest-path", join(dir, "Cargo.toml"), "--target-dir", join(build, "rust")], { stdio: "inherit" });
const metadata = {
  date: new Date().toISOString(), cpu: cpus()[0]?.model, platform: process.platform,
  arch: process.arch, node: process.version,
  clang: execFileSync("clang", ["--version"], { encoding: "utf8" }).split("\n")[0],
  ...wasmMetadata,
};
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const rows: object[] = [];
// Pay Wasm compilation/initialization costs before timing the steady-state runs.
const warmup = runCFiles([{ path: "warmup.c", source: "int main(void) { return 0; }" }], 4096, "", false);
assert.equal(warmup.kind, "ok", JSON.stringify(warmup));
for (const item of cases.filter(item => !process.env.BENCH_QUICK || item.n >= 1000 || item.n === 13)) {
  const input = `${item.kind} ${item.n} 12345\n`;
  const native = Object.fromEntries(["O0", "O2"].map(opt => [opt,
    JSON.parse(execFileSync(join(build, opt), [String(item.kind), String(item.n), "12345"], { encoding: "utf8" }))]));
  assert.equal(native.O0.output, native.O2.output);
  const rust = JSON.parse(execFileSync(join(build, "rust/release/interpreter-benchmark"), [join(dir, "kernels.c"), input], { encoding: "utf8" }));
  assert.equal(rust.output, native.O2.output);
  const wasm: Record<string, object> = {};
  for (const traceFunctions of process.env.BENCH_MINIMAL ? [false] : [false, true]) {
    const ms: number[] = [];
    let traceEvents = 0;
    for (let trial = 0; trial < 9; trial++) {
      const start = performance.now();
      const result = runCFiles([{ path: "benchmark.c", source }], 4096, input, false, {
        stepLimit: 2_000_000, followingTraceLimit: 256, traceFunctions,
      });
      const elapsed = performance.now() - start;
      assert.equal(result.kind, "ok", JSON.stringify(result).slice(0, 1000));
      if (result.kind !== "ok") throw new Error("benchmark failed");
      assert.equal(result.exitStatus, 0);
      assert.equal(result.executionLimit, null);
      assert.equal(Number(new TextDecoder().decode(result.stdoutBytes)), native.O2.output);
      traceEvents = result.trace.length;
      if (trial >= 2) ms.push(elapsed);
    }
    wasm[traceFunctions ? "sandboxTrace" : "mainTraceOnly"] = { ms, medianMs: median(ms), traceEvents };
  }
  const row = { ...item, native, rust: { ...rust, medianMs: median(rust.ms) }, wasm };
  rows.push(row);
  console.log(JSON.stringify(row));
}
const output = join(dir, process.env.BENCH_OUTPUT ?? "results.json");
writeFileSync(output, JSON.stringify({ metadata, rows }, null, 2) + "\n");
console.log(`Saved ${output}`);

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateRawSync } from "node:zlib";
import { cpus } from "node:os";

import { runCFiles, wasmMetadata } from "./runtime.ts";
const dir = fileURLToPath(new URL(".", import.meta.url));
const build = process.env.BENCH_BUILD_DIR ?? "/private/tmp/cboxes-real-project-bench";
mkdirSync(build, { recursive: true });
const vendor = (name: string) => readFileSync(join(dir, "vendor", name), "utf8");
const recordCount = process.env.BENCH_DOCUMENT === "large" ? 64 : 8;
const json = JSON.stringify({ service: "cBoxes", version: 17, enabled: true,
  tasks: Array.from({ length: recordCount }, (_, i) => ({ id: i, name: `job-${i}`, cost: i * 1.25, tags: ["C", "benchmark"] })),
  config: { retries: 3, timeout: 30, region: "us-west", description: "A real JSON document with nested arrays and objects." } });
const jsonLiteral = JSON.stringify(json);
const compressed = deflateRawSync(Buffer.from(json.repeat(4)));
const projects = [
  { name: "zlib-puff", version: "v1.3.1", sizes: [1, 4], source: vendor("puff.h") + vendor("puff.c").replace('#include "puff.h"', ""), driver: `
unsigned benchmark_run(unsigned n, unsigned seed) {
    const unsigned char compressed[] = {${[...compressed].join(",")}};
    unsigned char output[${Buffer.byteLength(json.repeat(4))}];
    unsigned sum = seed;
    for (unsigned i = 0; i < n; i++) {
        unsigned long outlen = sizeof output, inlen = sizeof compressed;
        if (puff(output, &outlen, compressed, &inlen) != 0 || outlen != sizeof output) abort();
        for (unsigned j = 0; j < outlen; j++) sum += output[j];
    }
    return sum;
}` },
  { name: "jsmn", version: "v1.1.0", sizes: [25, 100], source: vendor("jsmn.h"), driver: `
unsigned benchmark_run(unsigned n, unsigned seed) {
    const char *json = ${jsonLiteral};
    unsigned sum = seed;
    for (unsigned i = 0; i < n; i++) {
        jsmn_parser parser;
        jsmntok_t tokens[${Math.max(256, recordCount * 16 + 32)}];
        jsmn_init(&parser);
        int count = jsmn_parse(&parser, json, strlen(json), tokens, ${Math.max(256, recordCount * 16 + 32)});
        if (count < 0) abort();
        for (int j = 0; j < count; j++) sum += (unsigned)(tokens[j].end - tokens[j].start);
    }
    return sum;
}` },
  { name: "cJSON", version: "v1.7.18", sizes: [2, 8], source: "#define CJSON_HIDE_SYMBOLS\n" + vendor("cJSON.h") + vendor("cJSON.c").replace('#include "cJSON.h"', ""), driver: `
unsigned benchmark_run(unsigned n, unsigned seed) {
    const char *text = ${jsonLiteral};
    unsigned sum = seed;
    for (unsigned i = 0; i < n; i++) {
        cJSON *root = cJSON_Parse(text);
        if (!root) abort();
        cJSON *tasks = cJSON_GetObjectItemCaseSensitive(root, "tasks");
        sum += (unsigned)cJSON_GetArraySize(tasks);
        cJSON *task = cJSON_GetArrayItem(tasks, (int)(i % 8));
        cJSON *id = cJSON_GetObjectItemCaseSensitive(task, "id");
        sum += (unsigned)id->valueint;
        char *printed = cJSON_PrintUnformatted(root);
        if (!printed) abort();
        sum += (unsigned)strlen(printed);
        free(printed);
        cJSON_Delete(root);
    }
    return sum;
}` },
  { name: "tiny-AES-c", version: "v1.0.0", sizes: [1, 4], source: vendor("aes.h") + vendor("aes.c").replace('#include "aes.h"', ""), driver: `
unsigned benchmark_run(unsigned n, unsigned seed) {
    struct AES_ctx ctx;
    uint8_t key[16], iv[16], original[64];
    state_t blocks[4];
    uint8_t *buffer = (uint8_t *)&blocks;
    for (unsigned i = 0; i < 16; i++) { key[i] = (uint8_t)(seed + i); iv[i] = (uint8_t)i; }
    for (unsigned i = 0; i < 64; i++) original[i] = (uint8_t)(seed + i * 3);
    unsigned sum = seed;
    for (unsigned i = 0; i < n; i++) {
        memcpy(buffer, original, sizeof original);
        AES_init_ctx_iv(&ctx, key, iv);
        AES_CBC_encrypt_buffer(&ctx, buffer, sizeof original);
        for (unsigned j = 0; j < 64; j++) sum += buffer[j];
        AES_ctx_set_iv(&ctx, iv);
        AES_CBC_decrypt_buffer(&ctx, buffer, sizeof original);
        if (memcmp(buffer, original, sizeof original)) abort();
    }
    return sum;
}` },
];
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const samples = Number(process.env.BENCH_SAMPLES ?? 5);
const rows: object[] = [];
for (const project of projects.filter(p => !process.env.BENCH_PROJECT || process.env.BENCH_PROJECT.split(",").includes(p.name))) {
  const driver = process.env.BENCH_MUTABLE_INPUT
    ? project.driver.replace('const char *text =', 'char text[] =').replace('const char *json =', 'char json[] =')
    : project.driver;
  const source = project.source + '\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n' + driver + `
#ifndef NATIVE_BENCH
int main(void) {
    unsigned n, seed;
    if (scanf("%u %u", &n, &seed) != 2) return 1;
    printf("%u\\n", benchmark_run(n, seed));
    return 0;
}
#endif
`;
  const path = join(build, `${project.name}.c`);
  writeFileSync(path, source);
  const binary = join(build, project.name);
  execFileSync("clang", ["-O2", "-std=c11", `-DPROJECT_SOURCE="${path}"`, join(dir, "native.c"), "-o", binary]);
  const sizes = process.env.BENCH_ITERATIONS
    ? [Number(process.env.BENCH_ITERATIONS)]
    : project.sizes.filter((_, i) => !process.env.BENCH_QUICK || i === 1);
  for (const n of sizes) {
    assert.ok(Number.isInteger(n) && n > 0, "iterations must be a positive integer");
    const native = JSON.parse(execFileSync(binary, ["0", String(n), "12345"], { encoding: "utf8" }));
    const ms: number[] = [];
    let traceEvents = 0;
    for (let trial = 0; trial < samples + 2; trial++) {
      const start = performance.now();
      const result = runCFiles([{ path: `${project.name}.c`, source }], 4096, `${n} 12345\n`, false,
        { stepLimit: 20_000_000, followingTraceLimit: 256, traceFunctions: false });
      const elapsed = performance.now() - start;
      assert.equal(result.kind, "ok", JSON.stringify(result).slice(0, 2500));
      if (result.kind !== "ok") throw new Error("project benchmark failed");
      assert.equal(result.runtimeDiagnostic ?? null, null, JSON.stringify(result.runtimeDiagnostic ?? null));
      assert.equal(result.executionLimit, null, JSON.stringify(result.executionLimit));
      assert.equal(result.exitStatus, 0);
      assert.equal(Number(new TextDecoder().decode(result.stdoutBytes)), native.output);
      traceEvents = result.trace.length;
      if (trial >= 2) ms.push(elapsed);
    }
    const row = { name: project.name, version: project.version, n, documentRecords: recordCount, sourceSha256: createHash("sha256").update(source).digest("hex"),
      native, wasm: { ms, medianMs: median(ms), traceEvents }, slowdown: median(ms) / median(native.ms) };
    rows.push(row);
    console.log(JSON.stringify(row));
    writeFileSync(join(dir, process.env.BENCH_OUTPUT ?? "projects-results.json"), JSON.stringify({
      date: new Date().toISOString(), cpu: cpus()[0]?.model, node: process.version,
      ...wasmMetadata, rows,
    }, null, 2) + "\n");
  }
}

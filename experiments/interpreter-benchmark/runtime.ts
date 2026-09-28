import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { pathToFileURL } from "node:url";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const moduleUrl = process.env.BENCH_INTERPRETER_MODULE
  ? pathToFileURL(process.env.BENCH_INTERPRETER_MODULE)
  : new URL("../../js/shared-c-interpreter.js", import.meta.url);
const payloadPath = process.env.BENCH_WASM_DATA
  ? pathToFileURL(process.env.BENCH_WASM_DATA)
  : new URL("./shared-c-interpreter-wasm-data.js", moduleUrl);
if (process.env.BENCH_WASM_DATA) {
  registerHooks({ resolve(specifier, context, nextResolve) {
    return nextResolve(specifier === "./shared-c-interpreter-wasm-data.js"
      ? payloadPath.href : specifier, context);
  } });
}
const payload = readFileSync(payloadPath, "utf8");
const encoded = payload.match(/"([A-Za-z0-9+/=]+)"/)?.[1];
if (!encoded) throw new Error("Cannot read generated Wasm payload");
const wasm = Buffer.from(encoded, "base64");
export const wasmMetadata = {
  wasmPayloadSha256: createHash("sha256").update(payload).digest("hex"),
  wasmSha256: createHash("sha256").update(wasm).digest("hex"),
  wasmBytes: wasm.length,
};
export const { runCFiles } = await import(moduleUrl.href);

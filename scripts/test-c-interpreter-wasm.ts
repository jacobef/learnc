// Run with: node scripts/test-c-interpreter-wasm.ts
// Rebuild the Wasm payload and run tsc -p . first after interpreter changes.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const { runCProgram } = await import("../js/shared-c-interpreter.js");
const fixtures = new URL("../rust/c_interpreter/tests/standard_examples/", import.meta.url);
for (const name of readdirSync(fixtures).filter((name) => name.endsWith(".c")).sort()) {
  const source = readFileSync(new URL(name, fixtures), "utf8");
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, "ok", `${name}: ${JSON.stringify(result)}`);
  assert.equal(result.exitStatus, 0, `${name}: ${JSON.stringify(result)}`);
  console.log(`PASS ${name}`);
}
for (const call of [
  'strxfrm(NULL, "x", 1)', 'strxfrm(NULL, NULL, 0)',
  'wcsxfrm(NULL, L"x", 1)', 'wcsxfrm(NULL, NULL, 0)',
  'strncpy(NULL, "x", 0)',
]) {
  const result = runCProgram(`#include <string.h>\n#include <wchar.h>\nint main(void) { ${call}; }\n`, 4096, "");
  assert.equal(result.kind, "ub", `${call}: ${JSON.stringify(result)}`);
}
console.log("PASS invalid library calls still produce UB diagnostics");

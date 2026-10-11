// Run with: node scripts/test-c-interpreter-wasm.ts
// Rebuild the Wasm payload and run tsc -p . first after interpreter changes.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const { runCProgram, evaluateCExpressionFiles } = await import("../js/shared-c-interpreter.js");
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

for (const copy of ["memcpy", "memmove"]) {
  const result = runCProgram(`#include <stdlib.h>
#include <string.h>
struct A { int x; };
struct B { int x; };
int main(void) {
  struct A a = {123};
  void *p = malloc(sizeof a);
  ${copy}(p, &a, sizeof a);
  struct B *b = p;
  return b->x;
}\n`, 4096, "");
  assert.equal(result.kind, "ub", JSON.stringify(result));
  assert.match(result.diagnostic.message, /incompatible pointer type/);
  assert.equal(result.diagnostic.range.startLine, 9);
}
console.log("PASS copied structs reject member reads through incompatible struct types");

{
  const source = `struct S { int a; int b; };
int main(void) {
  struct S x;
  x.a = 1;
  struct S y = x;
  return y.a;
}\n`;
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, "ok", JSON.stringify(result));
  assert.equal(result.exitStatus, 1);
  assert.equal(result.state.find(box => box.name === "y.b")?.value, "");
  const unknownMember = runCProgram(source.replace("return y.a", "return y.b"), 4096, "");
  assert.equal(unknownMember.kind, "ub", JSON.stringify(unknownMember));
  assert.equal(unknownMember.diagnostic.range.startLine, 5);
}
console.log("PASS partially initialized struct copies preserve known and unknown members");

for (const [source, expected] of [
  [`#include <stdlib.h>\n#include <string.h>\nstruct A { char c; }; struct B { char c; };\nint main(void) { struct A a = {1}; void *p = malloc(sizeof a); memcpy(p, (char *)&a, sizeof a); return (0, *(struct B *)p).c; }\n`, "ub"],
  [`int f(int a[static 3]);\nint f(int *a) { return a[0]; }\nint main(void) { int a[1] = {0}; return f(a); }\n`, "ub"],
  [`#if 0xffffffff < -1\n#error wrong preprocessor integer type\n#endif\nint main(void) { return 0; }\n`, "ok"],
  [`#include <stdlib.h>\nint main(void) { return aligned_alloc(3, 12) != 0 || aligned_alloc(8, 9) != 0; }\n`, "ok"],
] as const) {
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, expected, JSON.stringify(result));
  if (result.kind === "ok") assert.equal(result.exitStatus, 0);
}
console.log("PASS defect-report fixes survive the Wasm bridge");

const expressionFiles = [
  { path: "values.h", source: "#define TWICE(x) ((x)*2)\n" },
  { path: "program.c", source: '#include <stdio.h>\n#include <limits.h>\n#include "values.h"\nint main(void) { int a=3; }\n' },
];
for (const [expression, expected] of [["stdin != 0", "1"], ["INT_MAX", "2147483647"], ["TWICE(a)", "6"]]) {
  const result = evaluateCExpressionFiles(expressionFiles, 0, expression, 4096, "", false);
  assert.equal(result.kind, "ok", `${expression}: ${JSON.stringify(result)}`);
  assert.equal(result.result.value, expected, expression);
}
console.log("PASS expression evaluation uses system and user header macros");
for (const [expression, expected] of [["3+4", "7"], ["INT_MAX", "2147483647"], ["stdin != 0", "1"]]) {
  const result = evaluateCExpressionFiles(expressionFiles, -1, expression, 4096, "", false);
  assert.equal(result.kind, "ok", `${expression}: ${JSON.stringify(result)}`);
  assert.equal(result.result.value, expected, expression);
}
assert.equal(evaluateCExpressionFiles(expressionFiles, -1, "a", 4096, "", false).kind, "compile");
const emptyExpression = evaluateCExpressionFiles([{ path: "program.c", source: "int main(void) {}\n" }], -1, "3+4", 4096, "", false);
assert.equal(emptyExpression.kind, "ok", JSON.stringify(emptyExpression));
assert.equal(emptyExpression.result.value, "7");
console.log("PASS expressions before the first statement use initial bindings");


for (const body of [
  "int i=0; while(i<10);",
  "int i=0; while(i<10) i--;",
  "int i=0; while(i<100) i=(i+1)%100;",
  "int i=0; while(1) { i+=3; if(i==10) break; }",
]) {
  const result = runCProgram(`int main(void) { ${body} }\n`, 4096, "");
  assert.equal(result.kind, "ok", JSON.stringify(result));
  assert.ok(result.executionLimit, body);
  assert.notEqual(result.executionLimit.reason, "step-limit", body);
  assert.ok(result.executionLimit.executedSteps < 2000, body);
}
const longerFinite = runCProgram("int main(void) { int i=0; while(i<5000) i++; return i!=5000; }\n", 4096, "");
assert.equal(longerFinite.kind, "ok", JSON.stringify(longerFinite));
assert.equal(longerFinite.executionLimit, null);
assert.equal(longerFinite.exitStatus, 0);
console.log("PASS early loop detection and the larger finite-program budget");

for (const [body, header] of [
  ["return INT_MAX;", "limits.h"],
  ["return fmax(1,2);", "math.h"],
  ["FILE *f;", "stdio.h"],
  ["const uint32_t n=1;", "stdint.h"],
]) {
  const result = runCProgram(`int main(void) { ${body} }\n`, 4096, "");
  assert.equal(result.kind, "compile", JSON.stringify(result));
  assert.ok(result.diagnostic.message.includes(`#include <${header}>`), JSON.stringify(result));
}
const misplacedHeader = runCProgram("int main(void) {\n#include <stdio.h>\n}\n", 4096, "");
assert.equal(misplacedHeader.kind, "compile");
assert.ok(misplacedHeader.diagnostic.message.includes("outside functions"));
console.log("PASS system-header placement and missing-header suggestions");

for (const definition of [
  "struct s {\n  int x;\n}",
  "union u {\n  int x;\n}",
  "enum e {\n  A\n}",
  "struct outer {\n  struct { int x; } inner;\n} /* definition ends here */",
]) {
  const source = `${definition}\nint main(void) {\n}\n`;
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, "compile", source);
  assert.equal(result.diagnostic.message, "Missing semicolon (;). Add ; after this closing brace to end the declaration.");
  assert.equal(result.diagnostic.range.startLine, 2);
  assert.equal(result.diagnostic.range.startCol, 0);
  const closingBrace = definition.lastIndexOf("}");
  const fixed = source.slice(0, closingBrace + 1) + ";" + source.slice(closingBrace + 1);
  assert.equal(runCProgram(fixed, 4096, "").kind, "ok", fixed);
}
console.log("PASS missing type-definition semicolons point to the closing brace with a direct fix");

const chronological = runCProgram(`#include <stdio.h>
int main(void) {
puts("first");
int n;
scanf("%d", &n);
printf("second %d", n);
fputs("error", stderr);
putchar(0xc3);
putchar(0xa9);
}`, 4096, "12 34␄");
assert.equal(chronological.kind, "ok", JSON.stringify(chronological));
const decoder = new TextDecoder();
for (const [line, stdout, stderr, inputPosition] of [
  [2, "first\n", "", 0],
  [4, "first\n", "", 2],
  [5, "first\nsecond 12", "", 2],
  [6, "first\nsecond 12", "error", 2],
  [7, "first\nsecond 12�", "error", 2],
  [8, "first\nsecond 12é", "error", 2],
] as const) {
  const event = chronological.trace.find((event) => event.startLine === line)!;
  assert.ok(event, `trace at line ${line}`);
  assert.equal(decoder.decode(chronological.stdoutBytes.subarray(0, event.stdoutLength)), stdout);
  assert.equal(decoder.decode(chronological.stderrBytes.subarray(0, event.stderrLength)), stderr);
  assert.equal(event.stdinPosition, inputPosition);
}
console.log("PASS chronological stdout, stderr, and stdin including split UTF-8 output");

for (const source of [
  "int f(int n,int a[][n]) { return a[1][n-1]; }\nint main(void) { int a[2][3]={{0},{1,2,3}}; return f(3,a)!=3; }\n",
  "#include <stdlib.h>\n#include <string.h>\nstruct S { unsigned x:3,y:5; };\nint main(void) { struct S *p=malloc(sizeof *p), q; p->y=19; memcpy(&q,p,sizeof q); int r=p->y!=19 || q.y!=19 || (p->y=35)!=3; free(p); return r; }\n",
]) {
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, "ok", JSON.stringify(result));
  assert.equal(result.exitStatus, 0, JSON.stringify(result));
}
console.log("PASS unsized multidimensional parameters and partially initialized bit-fields");

{
  const { invokeCInterpreter } = await import("../js/shared-c-wasm-bridge.js");
  const { normalizeProgramResult } = await import("../js/shared-c-result-normalization.js");
  const source = `int main(void) {
    int items[3] = {1, 2, 3};
    int *p = &items[1];
    for (int i = 0; i < 4; i++) items[1] += i;
    return *p != 8;
  }`;
  const raw = invokeCInterpreter({
    operation: "run-source", primary: source, stdin: "", syntheticAddressBase: 4096,
    executionBudget: { stepLimit: 10000, followingTraceLimit: 256, traceFunctions: true },
  }) as Record<string, any>;
  assert.equal(raw.ok, true);
  const boxes = raw.trace.flatMap((event: any) => event.state);
  assert.ok(boxes.length > raw.traceTypeInfos.length * 3);
  assert.ok(boxes.every((box: any) => Number.isInteger(box.typeInfo)));
  const result = normalizeProgramResult(raw, source);
  assert.equal(result.kind, "ok");
  if (result.kind !== "ok") throw new Error("trace decoding failed");
  assert.equal(result.exitStatus, 0);
  const pointers = result.trace.flatMap(event => event.state).filter(box => box.name === "p");
  assert.ok(pointers.length > 1);
  assert.equal(pointers[0].typeInfo.kind, "pointer");
  assert.equal(pointers[0].typeInfo.pointerDepth, 1);
  assert.equal(pointers[0].typeInfo, pointers.at(-1)!.typeInfo);
  const bad = structuredClone(raw);
  bad.trace.find((event: any) => event.state.length).state[0].typeInfo = raw.traceTypeInfos.length;
  assert.throws(() => normalizeProgramResult(bad, source), /valid trace type reference/);
}
console.log("PASS trace type pooling preserves metadata and rejects invalid references");

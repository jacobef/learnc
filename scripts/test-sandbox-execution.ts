// Run after rebuilding Wasm and running tsc -p .
import assert from "node:assert/strict";
import { test } from "node:test";
Object.assign(globalThis, { window: { location: { pathname: "/" } }, localStorage: { getItem: () => null, setItem: () => {} } });
const { SandboxExecution, sandboxStepLimit } = await import("../js/shared-sandbox-execution.js");
const { runCFiles, evaluateCExpressionFiles } = await import("../js/shared-c-interpreter.js");

test("stepping reuses execution until source, stdin, mode or budget changes", () => {
  let calls = 0;
  const execution = new SandboxExecution(100000, (...args) => { calls++; return runCFiles(...args); });
  const files = [{ path: "program.c", source: "int i=1;" }];
  const result = execution.run(files, "", true);
  for (let i=0; i<100; i++) assert.equal(execution.run(files.map(file => ({...file})), "", true), result);
  assert.equal(calls, 1);
  files[0].source = "int i=2;";
  execution.run(files, "", true);
  execution.run(files, "a", true);
  execution.run(files, "a", false);
  execution.stepLimit = 20000;
  execution.run(files, "a", false);
  execution.extend();
  execution.run(files, "a", false);
  assert.equal(calls, 6);
});

test("the large finite loop loads exact trace pages and honors a configurable step limit", () => {
  const files = [{path: "program.c", source: "int main(void) {\nint i=0;\nwhile(i<200000) {\ni++;\n}\n}\n"}];
  const execution = new SandboxExecution();
  const first = execution.run(files, "", false);
  assert.equal(first.kind, "ok");
  assert.equal(first.executionLimit?.reason, "trace-limit");
  assert.equal(first.trace.length, 512);
  assert.ok(first.executionLimit.executedSteps < 3000);
  execution.extend();
  const second = execution.run(files, "", false);
  assert.equal(second.kind, "ok");
  assert.equal(second.trace.length, 1024);
  assert.deepEqual(second.trace.slice(0, first.trace.length), first.trace);
  const eventIndex = 10;
  const expression = evaluateCExpressionFiles(files, eventIndex, "i", undefined, "", false, execution.budget());
  assert.equal(expression.kind, "ok");
  assert.equal(expression.result.value, second.trace[eventIndex].state.find(box => box.name === "i")!.value);
  execution.stepLimit = 20;
  execution.reset();
  const limited = execution.run(files, "", false);
  assert.equal(limited.kind, "ok");
  assert.equal(limited.executionLimit?.reason, "step-limit");
  assert.equal(limited.executionLimit.executedSteps, 20);
  assert.equal(limited.executionLimit.tracePosition, limited.trace.length);
});

test("configured limits are finite positive integers within the UI range", () => {
  assert.equal(sandboxStepLimit(NaN), 100000);
  assert.equal(sandboxStepLimit(0), 1);
  assert.equal(sandboxStepLimit(125.8), 125);
  assert.equal(sandboxStepLimit(2000000), 1000000);
});

test("for-init variables leave the displayed and expression scope before the next declaration", () => {
  for (const body of ["for (int i=0; i<3; i++) {}", "for (int i=0; i<0; i++) {}", "for (int i=0;; i++) { break; }", "for (int i=0; i<3; i++);"]) {
    for (const outer of ["", "int i=99;\n"]) {
      const source = `int main(void) {\n${outer}${body}\nint j;\n}\n`;
      const files = [{path: "program.c", source}];
      const result = runCFiles(files, undefined, "", false);
      assert.equal(result.kind, "ok", source);
      const next = result.trace.findIndex(event => event.state.some(box => box.name === "j"));
      assert.ok(next > 0);
      const boundary = result.trace[next - 1];
      assert.equal(boundary.kind, "block-close");
      assert.equal(boundary.state.find(box => box.name === "i")?.value, outer ? "99" : undefined, source);
      const expression = evaluateCExpressionFiles(files, next - 1, "i", undefined, "", false);
      assert.equal(expression.kind, outer ? "ok" : "compile", source);
      if (expression.kind === "ok") assert.equal(expression.result.value, "99");
      // A page ending precisely at scope exit must preserve that same state.
      const paged = runCFiles(files, undefined, "", false, {traceLimit: next, stepLimit: 100000});
      assert.equal(paged.kind, "ok");
      assert.deepEqual(paged.trace, result.trace.slice(0, next));
    }
  }
});

test("background discovery finds lines beyond the loaded page without expanding the stepping cache", async () => {
  const { discoverSandboxBoundaries } = await import("../js/shared-sandbox-discovery.js");
  const files = [{path: "program.c", source: '#include <stdio.h>\nint main(void) {\nfor(int i=0;i<200;i++) {\nprintf("%d\\n",i);\n}\nint j;\n}\n'}];
  const execution = new SandboxExecution();
  const initial = execution.run(files, "", false);
  assert.equal(initial.kind, "ok");
  assert.equal(initial.trace.length, 512);
  const discovery = discoverSandboxBoundaries(files, "", false, 100000, 1024);
  assert.equal(discovery.more, false);
  const position = discovery.boundaries.findIndex(boundary => boundary?.line === 5);
  assert.ok(position > initial.trace.length);
  assert.equal(execution.run(files, "", false), initial);
  execution.includePosition(position);
  const jumped = execution.run(files, "", false);
  assert.equal(jumped.kind, "ok");
  assert.equal(jumped.trace[position].startLine, 5);
  assert.deepEqual(jumped.trace[position - 1].state, []);
  const skipped = discoverSandboxBoundaries([{path: "program.c", source: "int main(void) {\nif (0) {\nint unreachable;\n}\nint end;\n}"}], "", false, 100000, 1024);
  assert.equal(skipped.boundaries.some(boundary => boundary?.line === 2), false);
});

test("loop navigation adds no extra execution states", async () => {
  const { boundaryTargets, executionBoundaries, defaultOccurrence } = await import("../js/shared-sandbox-navigation.js");
  const source = "int main(void) {\nint i=0;\nwhile(i<2) {\ni++;\n}\n}\n";
  const files = [{path: "program.c", source}];
  const result = new SandboxExecution().run(files, "", false);
  assert.equal(result.kind, "ok");
  assert.deepEqual(result.trace.map(event => [event.kind, event.startLine]), [
    ["statement", 1], ["loop", 2], ["statement", 3], ["block-close", 4],
    ["loop", 2], ["statement", 3], ["block-close", 4], ["loop", 2],
  ]);
  const boundaries = executionBoundaries(result.trace, result.mainClose, null, {file: "program.c", line: 6});
  const targets = boundaryTargets(boundaries, "program.c").filter(target => target.line === 2);
  assert.deepEqual(targets.map(target => target.kind), ["first", "nearest"]);
  assert.deepEqual(targets[0].positions, [1, 4, 7]);
  assert.deepEqual(targets[1].positions, [1, 4, 7]);
  assert.equal(defaultOccurrence(targets[1].positions, 3), 4);
  assert.equal(result.trace[targets[0].positions[0] - 1].state[0].value, "0");
  assert.equal(result.trace[defaultOccurrence(targets[1].positions, 3)! - 1].state[0].value, "1");
});

test("nested loops use the first occurrence across all outer iterations", async () => {
  const { boundaryTargets, executionBoundaries, defaultOccurrence } = await import("../js/shared-sandbox-navigation.js");
  const files = [{path: "program.c", source: "int main(void) {\nfor(int outer=0;outer<2;outer++) {\nfor(int inner=0;inner<2;inner++) {\nint value=inner;\n}\n}\n}"}];
  const result = new SandboxExecution().run(files, "", false);
  assert.equal(result.kind, "ok");
  assert.equal(result.trace.some(event => ["loop-entry", "loop-init", "loop-update", "loop-condition"].includes(event.kind)), false);
  const targets = boundaryTargets(executionBoundaries(result.trace, result.mainClose, null, {file: "program.c", line: 7}), "program.c").filter(target => target.line === 2);
  assert.equal(targets[0].positions.length, 6);
  assert.deepEqual(targets[0].positions, targets[1].positions);
  const inSecondInvocation = targets[1].positions[3] + 1;
  assert.equal(targets[0].positions[0], targets[1].positions[0]);
  assert.equal(defaultOccurrence(targets[1].positions, inSecondInvocation), targets[1].positions[4]);
});

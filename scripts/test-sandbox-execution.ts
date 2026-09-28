// Run after rebuilding Wasm and running tsc -p .
import assert from "node:assert/strict";
import { test } from "node:test";
Object.assign(globalThis, { window: { location: { pathname: "/" } }, localStorage: { getItem: () => null, setItem: () => {} } });
const { SandboxExecution, sandboxStepLimit, sandboxTimeline, sandboxDiagnosticAt } = await import("../js/shared-sandbox-execution.js");
const { runCFiles, evaluateCExpressionFiles } = await import("../js/shared-c-interpreter.js");

test("VLA dimension errors appear at the failing runtime step, while fixed type errors are immediate", () => {
  for (const statement of ["int (*p)[n]=a;", "int (*p)[n]=0;\np=a;"]) {
    const source = `int main(void) {\nint n=2;\nint a[2][3];\n${statement}\nreturn 0;\n}`;
    const result = new SandboxExecution().run([{path: "program.c", source}], "", false);
    assert.equal(result.kind, "ub");
    assert.match(result.diagnostic.message, /array dimensions do not match/);
    assert.match(result.diagnostic.message, /Check the values used for the array sizes/);
    const timeline = sandboxTimeline(result);
    assert.equal(timeline.kind, "ok");
    assert.ok(timeline.trace.length > 1);
    for (let position = 0; position < timeline.trace.length; position++) {
      assert.equal(sandboxDiagnosticAt(timeline, position), null);
    }
    assert.equal(sandboxDiagnosticAt(timeline, timeline.trace.length), result.diagnostic);
  }
  const result = new SandboxExecution().run([{path: "program.c", source: "int main(void) { if(0) { int a[2][3]; int (*p)[2]=a; } return 0; }"}], "", false);
  assert.equal(result.kind, "compile");
  assert.equal(sandboxDiagnosticAt(result, 0), result.diagnostic);
});

test("runtime UB preserves the exact prefix and appears only when its terminal step is attempted", () => {
  const files = [{path: "program.c", source: '#include <stdio.h>\nint x=2;\nwhile(x>=0) {\nprintf("%d",x);\nint y=10/x;\nx--;\n}\n'}];
  const result = new SandboxExecution().run(files, "", true);
  assert.equal(result.kind, "ub");
  const context = result.diagnostic.runtimeContext!;
  assert.ok(context.trace.length > 5);
  const timeline = sandboxTimeline(result);
  assert.equal(timeline.kind, "ok");
  assert.deepEqual(timeline.trace.slice(0, -1), context.trace);
  assert.equal(timeline.trace.at(-1)?.kind, "ub");
  assert.equal(timeline.trace.at(-1)?.startLine, 4); // Original line, without the implicit-main wrapper.
  assert.equal(timeline.stdout, "210");
  for (let position = 0; position < timeline.trace.length; position++) {
    assert.equal(sandboxDiagnosticAt(timeline, position), null);
  }
  assert.equal(sandboxDiagnosticAt(timeline, timeline.trace.length), result.diagnostic);
  assert.deepEqual(timeline.trace.at(-1)?.state, []); // No invented post-UB state.
  assert.equal(sandboxDiagnosticAt(timeline, timeline.trace.length - 1), null); // Back clears it.
  const snapshot = evaluateCExpressionFiles(files, 0, "x", undefined, "", true);
  assert.equal(snapshot.kind, "ok");
  assert.equal(snapshot.result.value, "2");
});

test("UB in the first operation still has a runnable first step; compile errors remain immediate", () => {
  const result = new SandboxExecution().run([{path: "program.c", source: "int x=1/0;"}], "", true);
  assert.equal(result.kind, "ub");
  const timeline = sandboxTimeline(result);
  assert.equal(timeline.kind, "ok");
  assert.equal(timeline.trace.length, 1);
  assert.equal(sandboxDiagnosticAt(timeline, 0), null);
  assert.equal(sandboxDiagnosticAt(timeline, 1)?.kind, "ub");
  const invalid = new SandboxExecution().run([{path: "program.c", source: "int x = ;"}], "", true);
  assert.equal(invalid.kind, "compile");
  assert.equal(sandboxTimeline(invalid), invalid);
  assert.equal(sandboxDiagnosticAt(invalid, 0), invalid.diagnostic);
});

test("the UB prefix preserves streams and locations across source files", () => {
  const files = [
    {path: "program.c", source: '#include <stdio.h>\nint fail(int x);\nint main(void) {\nint x;\nscanf("%d", &x);\nprintf("before");\nfprintf(stderr,"error stream");\nreturn fail(x);\n}\n'},
    {path: "helper.c", source: 'int fail(int x) {\nreturn 4/x;\n}\n'},
  ];
  const result = new SandboxExecution().run(files, "0\n", false);
  assert.equal(result.kind, "ub");
  const timeline = sandboxTimeline(result);
  assert.equal(timeline.kind, "ok");
  assert.equal(timeline.trace.at(-1)?.file, "helper.c");
  assert.equal(timeline.trace.at(-1)?.startLine, 1);
  assert.equal(timeline.stdout, "before");
  assert.equal(timeline.stderr, "error stream");
  assert.equal(timeline.stdinPosition, 1);
  assert.ok(timeline.trace.some(event => event.stdoutLength === 0));
});

test("loading another trace page preserves earlier steps when it reveals UB", () => {
  const files = [{path: "program.c", source: "int i=0;\nwhile(i<300) {\ni++;\n}\nint x=1/(300-i);\n"}];
  const execution = new SandboxExecution();
  const first = execution.run(files, "", true);
  assert.equal(first.kind, "ok");
  assert.equal(first.executionLimit?.reason, "trace-limit");
  assert.equal(first.trace.length, 512);
  execution.extend();
  const result = execution.run(files, "", true);
  assert.equal(result.kind, "ub");
  const timeline = sandboxTimeline(result);
  assert.equal(timeline.kind, "ok");
  assert.deepEqual(timeline.trace.slice(0, first.trace.length), first.trace);
  assert.equal(sandboxDiagnosticAt(timeline, first.trace.length), null);
  assert.equal(sandboxDiagnosticAt(timeline, timeline.trace.length)?.kind, "ub");
});

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
      assert.equal(boundary.kind, body.includes("break;") ? "block-close" : "loop");
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

test("for increments execute at the body close, separately from the next condition", () => {
  const files = [{path:"program.c", source:"int main(void) {\nint checks=0;\nfor(int i=0; ++checks && i<2; i++) {\nint local=i;\n}\nint done=checks;\n}"}];
  const execution = new SandboxExecution();
  const result = execution.run(files, "", false);
  assert.equal(result.kind, "ok");
  assert.deepEqual(result.trace.map(event => [event.kind, event.startLine]), [
    ["statement",1], ["loop",2], ["statement",3], ["block-close",4],
    ["loop",2], ["statement",3], ["block-close",4], ["loop",2],
    ["statement",5],
  ]);
  for (const [iteration, index] of [3,6].entries()) {
    const before = result.trace[index-1].state;
    const after = result.trace[index].state;
    assert.equal(before.find(b => b.name === "i")?.value, String(iteration));
    assert.ok(before.some(b => b.name === "local"));
    assert.equal(after.find(b => b.name === "i")?.value, String(iteration+1));
    assert.equal(after.find(b => b.name === "checks")?.value, String(iteration+1));
    assert.ok(after.every(b => b.name !== "local"));
    assert.equal(result.trace[index+1].state.find(b => b.name === "checks")?.value, String(iteration+2));
    const expression = evaluateCExpressionFiles(files, index, "i", undefined, "", false, execution.budget());
    assert.equal(expression.kind, "ok");
    assert.equal(expression.result.value, String(iteration+1));
    const page = runCFiles(files, undefined, "", false, {...execution.budget(), traceLimit:index+1});
    assert.equal(page.kind, "ok");
    assert.deepEqual(page.trace, result.trace.slice(0,index+1));
  }
});

test("continue updates at the closing brace, while break and return skip the update", () => {
  for (const control of ["continue;", "break;", "return 0;"]) {
    const files = [{path:"program.c", source:`int main(void) {\nint i=0;\nfor(; i<2; i++) {\n${control}\n}\n}`}];
    const result = new SandboxExecution().run(files, "", false);
    assert.equal(result.kind, "ok");
    assert.equal(result.state.find(b => b.name === "i")?.value, control === "continue;" ? "2" : "0");
    for (let i=1; i<result.trace.length; i++) {
      const value = result.trace[i].state.find(b => b.name === "i")?.value;
      const previous = result.trace[i-1].state.find(b => b.name === "i")?.value;
      if (value !== previous) assert.equal(result.trace[i].startLine, 4);
    }
  }
});

test("a false for condition steps past the brace and ends the loop variable scope", async () => {
  const { stepDestination, previousVisibleStepPosition, executionBoundaries } = await import("../js/shared-sandbox-navigation.js");
  for (const limit of [0, 2]) {
    const files = [{path:"program.c", source:`int main(void) {\nint i=99;\nfor(int i=0; i<${limit}; i++) {\nint body=i;\n}\nint after=i;\n}`}];
    const execution = new SandboxExecution();
    const result = execution.run(files, "", false);
    assert.equal(result.kind, "ok");
    const trace = result.trace;
    const after = trace.findIndex(e => e.startLine === 5);
    const condition = after - 1;
    assert.equal(trace[condition].kind, "loop");
    assert.equal(trace[condition].startLine, 2);
    assert.equal(stepDestination(trace, condition, "over"), after);
    assert.equal(stepDestination(trace, condition, "into"), after);
    assert.equal(previousVisibleStepPosition(trace, after), condition);
    assert.equal(executionBoundaries(trace, null, null, {file:"program.c",line:6})[after]?.line, 5);
    assert.equal(trace[condition].state.filter(b => b.name === "i").length, 1);
    assert.equal(trace[condition].state.find(b => b.name === "i")?.value, "99");
    assert.equal(trace[condition].state.find(b => b.name === "i")?.nameAccessible, true);
    assert.equal(trace.filter(e => e.startLine === 4).length, limit);
    const inspected = evaluateCExpressionFiles(files, condition, "i", undefined, "", false, execution.budget());
    assert.equal(inspected.kind, "ok");
    assert.equal(inspected.result.value, "99");
    const page = runCFiles(files, undefined, "", false, {...execution.budget(), traceLimit:after});
    assert.equal(page.kind, "ok");
    assert.deepEqual(page.trace, trace.slice(0,after));
  }
});

test("calls in a for update can be stepped into at the closing brace", async () => {
  const { stepDestination } = await import("../js/shared-sandbox-navigation.js");
  const files = [{path:"program.c", source:"int inc(int i) {\nreturn i+1;\n}\nint main(void) {\nfor(int i=0; i<2; i=inc(i)) {\nint local=i;\n}\n}"}];
  const result = new SandboxExecution().run(files, "", false);
  assert.equal(result.kind, "ok");
  const trace = result.trace;
  const call = trace.findIndex(e => e.kind === "call");
  assert.equal(trace[call].startLine, 6);
  assert.equal(trace[call-1].state.find(b => b.name === "local")?.value, "0");
  const into = stepDestination(trace, call, "into")!;
  assert.equal(trace[into].startLine, 1);
  assert.ok(trace[into-1].state.every(b => b.name !== "local"));
  const over = stepDestination(trace, call, "over")!;
  assert.equal(trace[over].startLine, 4);
  assert.equal(trace[over-1].state.find(b => b.name === "i")?.value, "1");
  assert.equal(stepDestination(trace, into, "out"), over);
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

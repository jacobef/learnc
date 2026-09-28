// Rebuild Wasm and run tsc -p . before running this file with node.
import assert from "node:assert/strict";
import { test } from "node:test";
Object.assign(globalThis, { window: { location: { pathname: "/" } }, localStorage: { getItem: () => null, setItem: () => {} } });
const { runCFiles, evaluateCExpressionFiles } = await import("../js/shared-c-interpreter.js");
const { SandboxExecution, sandboxTimeline } = await import("../js/shared-sandbox-execution.js");
const { stepDestination, stepDepth, previousVisibleStepPosition, executionBoundaries } = await import("../js/shared-sandbox-navigation.js");
const { memoryDisplayState } = await import("../js/shared-program-state.js");
const budget = { traceFunctions: true, stepLimit: 100000, followingTraceLimit: 256, traceLimit: 5000 };

test("into, over, and out follow real calls and restore caller scope across files", () => {
  const files = [
    {path:"program.c", source:"int f(int *p);\nint main(void) {\nint x=4;\nint y=f(&x);\nx++;\nreturn 0;\n}"},
    {path:"helper.c", source:"int f(int *p) {\nint x=9;\n*p=7;\nreturn x;\n}"},
  ];
  const result = runCFiles(files, 4096, "", false, budget);
  assert.equal(result.kind, "ok");
  const trace = result.trace;
  const call = trace.findIndex(e => e.kind === "call");
  assert.ok(call >= 0);
  assert.equal(trace[call].file, "program.c");
  assert.equal(stepDestination(trace, call, "into"), call + 1);
  assert.equal(trace[call + 1].file, "helper.c");
  assert.equal(stepDepth(trace, call + 1), 1);
  const over = stepDestination(trace, call, "over")!;
  assert.equal(trace[over - 1].callDepth, 0);
  assert.equal(trace[over - 1].state.find(b => b.name === "y")?.value, "9");
  const out = stepDestination(trace, call + 1, "out")!;
  assert.equal(out, over);
  assert.equal(trace[out].startLine, 4);
  assert.equal(trace[out - 1].state.find(b => b.name === "x")?.nameAccessible, true);
  const mutated = trace.find(e => e.function === "f" && e.state.some(b => b.name === "x" && b.value === "7"))!;
  const caller = mutated.state.find(b => b.name === "x" && b.scope === "main")!;
  const local = mutated.state.find(b => b.name === "x" && b.scope === "f")!;
  assert.equal(caller.value, "7");
  assert.equal(caller.nameAccessible, false);
  assert.ok(caller.aliases.includes("*p"));
  assert.equal(local.nameAccessible, true);
  assert.notEqual(caller.objectId, local.objectId);
  const expression = evaluateCExpressionFiles(files, trace.indexOf(mutated), "x", 4096, "", false, budget);
  assert.equal(expression.kind, "ok");
  assert.equal(expression.result.value, "9");
  assert.ok(trace[out - 1].state.every(b => b.objectId !== local.objectId));
});

test("running return completes the call with no stale callee or duplicate caller pauses", () => {
  for (const body of ["return value;", "{\nreturn value;\n}"]) {
    const files = [
      {path:"helper.c", source:`int worker(void) {\nint value=100;\n${body}\n}`},
      {path:"program.c", source:"int worker(void);\nint main(void) {\nint result=worker();\nresult++;\n}"},
    ];
    const result = runCFiles(files, 4096, "", false, budget);
    assert.equal(result.kind, "ok");
    const trace = result.trace;
    const exit = trace.findIndex(e => e.kind === "function-exit");
    const target = stepDestination(trace, exit, "over")!;
    assert.ok(target > exit);
    assert.equal(trace[target].file, "program.c");
    assert.equal(trace[target].startLine, 3);
    assert.equal(stepDepth(trace, target), 0);
    assert.equal(trace[target - 1].state.find(b => b.name === "result")?.value, "100");
    assert.ok(trace[target - 1].state.every(b => b.scope !== "worker"));
    assert.equal(stepDestination(trace, exit, "into"), target);
    assert.equal(stepDestination(trace, exit, "out"), target);
    assert.equal(previousVisibleStepPosition(trace, target), exit);
    const boundaries = executionBoundaries(trace, null, null, {file:"program.c", line:5});
    assert.ok(boundaries[exit]);
    for (let i=exit+1; i<target; i++) assert.equal(boundaries[i], null);
    // Every possible page split inside the return must request more trace,
    // rather than expose an intermediate frame or an unfinished assignment.
    for (let limit=exit+1; limit<target; limit++) {
      const page = runCFiles(files, 4096, "", false, {...budget, traceLimit:limit});
      assert.equal(page.kind, "ok");
      assert.equal(page.executionLimit?.reason, "trace-limit");
      assert.equal(stepDestination(page.trace, exit, "over"), null);
    }
  }
});

test("implicit returns, sibling calls, and calls in return expressions have usable boundaries", () => {
  const files = [{path:"program.c", source:"void f(int *p) {\n(*p)++;\n}\nint g(void) {\nreturn 3;\n}\nint h(void) {\nreturn g()+g();\n}\nint main(void) {\nint n=0;\nf(&n);\nn=h();\nn++;\n}"}];
  const result = runCFiles(files, 4096, "", false, budget);
  assert.equal(result.kind, "ok");
  const trace = result.trace;
  const implicitExit = trace.findIndex(e => e.function === "f" && e.kind === "function-exit");
  assert.equal(trace[implicitExit].startLine, 2);
  const afterVoid = stepDestination(trace, implicitExit, "over")!;
  assert.equal(trace[afterVoid].kind, "call");
  assert.equal(trace[afterVoid].startLine, 12);
  assert.equal(stepDepth(trace, afterVoid), 0);
  const firstExit = trace.findIndex(e => e.function === "g" && e.kind === "function-exit");
  const sibling = stepDestination(trace, firstExit, "over")!;
  assert.equal(trace[sibling].kind, "call");
  assert.equal(stepDepth(trace, sibling), 1);
  assert.equal(trace[sibling - 1].function, "h");
  const secondExit = trace.findIndex((e,i) => i>firstExit && e.function === "g" && e.kind === "function-exit");
  const done = stepDestination(trace, secondExit, "over")!;
  assert.equal(trace[done].startLine, 13);
  assert.equal(stepDepth(trace, done), 0);
  assert.equal(trace[done - 1].state.find(b => b.name === "n")?.value, "6");
});

test("shadowed arrays stay distinct and their original name is restored", () => {
  const result = runCFiles([{path:"program.c",source:"int main(void) {\nint a[2]={1,2};\n{\nint a[2]={3,4};\na[0]++;\n}\na[1]++;\n}"}],4096,"",false,budget);
  assert.equal(result.kind,"ok");
  const shadow = result.trace.find(e => e.state.filter(b => b.name === "a").length === 2)!;
  assert.ok(shadow);
  const roots = shadow.state.filter(b => b.name === "a");
  assert.equal(roots[0].nameAccessible,false);
  assert.equal(roots[1].nameAccessible,true);
  const displayed = memoryDisplayState(shadow.state);
  assert.deepEqual(memoryDisplayState(displayed), displayed);
  const hidden = displayed.find(b => b.objectId === roots[0].objectId && !b.arrayRoot)!;
  assert.match(hidden.name,/out of scope/);
  assert.equal(displayed.find(b => b.objectId === hidden.objectId && b.arrayIndices?.[0] === 1)?.arrayRoot,hidden.name);
  assert.equal(result.state.filter(b => b.name === "a").length,1);
  assert.equal(result.state.find(b => b.name === "a")?.nameAccessible,true);
});

test("recursive calls keep each live activation's locals", () => {
  const source="int f(int n) {\nint x=n;\nif(n) x+=f(n-1);\nreturn x;\n}\nint main(void) {\nint x=f(3);\n}";
  const result=runCFiles([{path:"program.c",source}],4096,"",false,budget);
  assert.equal(result.kind,"ok");
  const deepest=result.trace.find(e=>e.callDepth===4 && e.state.filter(b=>b.name==="x" && b.value!=="").length===4)!;
  assert.ok(deepest);
  const locals=deepest.state.filter(b=>b.name==="x");
  assert.equal(new Set(locals.map(b=>b.objectId)).size,5);
  assert.equal(locals.filter(b=>b.nameAccessible).length,1);
  const out=stepDestination(result.trace,result.trace.indexOf(deepest)+1,"out")!;
  assert.equal(result.trace[out-1].callDepth,3);
  assert.equal(result.state.find(b=>b.name==="x")?.value,"6");
});

test("step over spans sibling calls, trace pages, and stops at UB", () => {
  const files=[{path:"program.c",source:"int f(int x) {\nfor(int i=0;i<600;i++) x++;\nreturn x;\n}\nint main(void) {\nint n=f(1)+f(2);\n}"}];
  const execution=new SandboxExecution();
  let result=execution.run(files,"",false);
  assert.equal(result.kind,"ok");
  const start=result.trace.findIndex(e=>e.kind==="call");
  assert.equal(stepDestination(result.trace,start,"over"),null);
  while(result.executionLimit?.reason==="trace-limit") {
    execution.extend();
    result=execution.run(files,"",false);
    assert.equal(result.kind,"ok");
  }
  const over=stepDestination(result.trace,start,"over")!;
  assert.equal(result.trace[over-1].state.find(b=>b.name==="n")?.value,"1203");
  const bad=sandboxTimeline(new SandboxExecution().run([{path:"program.c",source:"int f(void) {int x=0; return 1/x;} int main(void) {int n=f();}"}],"",false));
  assert.equal(bad.kind,"ok");
  assert.equal(stepDestination(bad.trace,0,"over"),bad.trace.length);
});

test("static locals remain alive outside their function without requiring a pointer", () => {
  const result=runCFiles([{path:"program.c",source:"void f(void) {static int x=3; x++;}\nint main(void) {f(); int y=1;}"}],4096,"",false,budget);
  assert.equal(result.kind,"ok");
  const x=result.state.find(b=>b.name==="x")!;
  assert.equal(x.value,"4");
  assert.equal(x.nameAccessible,false);
});

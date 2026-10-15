import assert from "node:assert/strict";
import { test } from "node:test";

const { runCFiles, runCProgram } = await import("../js/shared-c-interpreter.js");
const { doneLinesForTrace, strikeFragmentsForTrace } = await import("../js/shared-trace-lines.js");

// The lines crossed out after each event of the program's trace.
function struckLinesByEvent(source: string): number[][] {
  const run = runCProgram(source);
  assert.equal(run.kind, "ok");
  const lines = source.split("\n");
  return run.trace.map((_, index) => [
    ...new Set(strikeFragmentsForTrace(lines, run.trace, index).map((fragment) => fragment.line)),
  ].sort((a, b) => a - b));
}

function struckAt(source: string, kind: string, line: number, occurrence: number): number[] {
  const run = runCProgram(source);
  assert.equal(run.kind, "ok");
  let seen = 0;
  const index = run.trace.findIndex((event) => event.kind === kind && event.startLine === line && seen++ === occurrence);
  assert.ok(index >= 0, `no ${kind} event ${occurrence} on line ${line}`);
  return struckLinesByEvent(source)[index]!;
}

const findMax = `int scores[4] = {7, 3, 9, 5};
int best = scores[0];
for (int i = 1; i < 4; i++) {
  if (scores[i] > best) {
    best = scores[i];
  }
}
int done = 1;
`;

test("a branch skipped inside a loop is crossed out only for that iteration", () => {
  // i = 1: 3 > 7 is false, so the body is crossed out until the loop comes round again.
  assert.deepEqual(struckAt(findMax, "branch", 3, 0), [3, 4, 5]);
  assert.deepEqual(struckAt(findMax, "block-close", 6, 0), [3, 4, 5]);
  assert.deepEqual(struckAt(findMax, "loop", 2, 1), []);
  // i = 3: 5 > 9 is false; once the loop ends, nothing stays crossed out.
  assert.deepEqual(struckAt(findMax, "branch", 3, 2), [3, 4, 5]);
  assert.deepEqual(struckAt(findMax, "statement", 7, 0), []);
});

test("a skipped branch outside any loop stays crossed out", () => {
  const source = `int x = 1;
if (x > 5) {
  x = 0;
}
for (int i = 0; i < 2; i++) {
  x++;
}
`;
  assert.deepEqual(struckAt(source, "loop", 4, 2), [1, 2, 3]);
});

test("an inner loop's iterations don't un-skip a branch in the outer loop's body", () => {
  const source = `int n = 0;
for (int i = 0; i < 2; i++) {
  if (i > 5) {
    n = 100;
  }
  for (int j = 0; j < 2; j++) {
    n++;
  }
}
`;
  assert.deepEqual(struckAt(source, "loop", 5, 1), [2, 3, 4]);
  assert.deepEqual(struckAt(source, "loop", 1, 1), []);
});

test("do-while iterations un-skip branches in their body", () => {
  const source = `int n = 0;
do {
  if (n > 1) {
    n += 10;
  }
  n++;
} while (n < 3);
int done = 1;
`;
  assert.deepEqual(struckAt(source, "statement", 5, 0), [2, 3, 4]);
  assert.deepEqual(struckAt(source, "loop", 6, 0), []);
  assert.deepEqual(struckAt(source, "statement", 7, 0), []);
});

type Run = ReturnType<typeof runCProgram>;

function doneAt(source: string, run: Run, kind: string, line: number, occurrence: number): number[] {
  assert.equal(run.kind, "ok");
  let seen = 0;
  const index = run.trace.findIndex((event) => event.kind === kind && event.startLine === line && seen++ === occurrence);
  assert.ok(index >= 0, `no ${kind} event ${occurrence} on line ${line}`);
  return [...doneLinesForTrace(source.split("\n"), run.trace, index)].sort((a, b) => a - b);
}

test("lines in a loop are done only for the current iteration, and all of it once it ends", () => {
  const run = runCProgram(findMax);
  // i = 2: 9 > 7, so the branch runs.
  assert.deepEqual(doneAt(findMax, run, "statement", 4, 0), [0, 1, 2, 3, 4]);
  // Checking the condition again starts a fresh iteration.
  assert.deepEqual(doneAt(findMax, run, "loop", 2, 2), [0, 1, 2]);
  // The last check fails, so the whole loop has finished, including the line that ran in an earlier iteration.
  assert.deepEqual(doneAt(findMax, run, "loop", 2, 3), [0, 1, 2, 3, 4, 5, 6]);
  assert.deepEqual(doneAt(findMax, run, "statement", 7, 0), [0, 1, 2, 3, 4, 5, 6, 7]);
});

test("a skipped branch outside any loop never counts as done", () => {
  const source = `int x = 1;
if (x > 5) {
  x = 0;
}
x = 2;
`;
  const run = runCProgram(source);
  assert.deepEqual(doneAt(source, run, "statement", 4, 0), [0, 1, 4]);
});

test("function declarations never run, and a function's lines start over with each call", () => {
  const source = `int cube(int n);

int main(void) {
  int a = 2;
  a = cube(a);
  a = cube(a);
  return 0;
}

int cube(int n) {
  int r = n * n * n;
  return r;
}
`;
  const run = runCFiles([{ path: "program.c", source }], undefined, "", false, {
    stepLimit: 100_000,
    followingTraceLimit: 256,
    traceFunctions: true,
  });
  // main's header counts as soon as the program starts.
  assert.deepEqual(doneAt(source, run, "statement", 3, 0), [2, 3]);
  // Entering cube: its header has run, but the line with the call hasn't finished.
  assert.deepEqual(doneAt(source, run, "call", 4, 0), [2, 3, 9]);
  assert.deepEqual(doneAt(source, run, "function-exit", 11, 0), [2, 3, 9, 10, 11]);
  assert.deepEqual(doneAt(source, run, "statement", 4, 0), [2, 3, 4, 9, 10, 11]);
  // The second call starts cube over.
  assert.deepEqual(doneAt(source, run, "call", 5, 0), [2, 3, 4, 9]);
});

test("a switch header, break, and continue each count as done once they run", () => {
  const source = `int day = 7;
int w = 0;
switch (day) {
  case 1:
    w = 1;
    break;
  case 7:
    w = 2;
    break;
}
int t = 0;
for (int i = 0; i < 3; i++) {
  if (i == 1) {
    continue;
  }
  t += i;
}
`;
  const run = runCProgram(source);
  // Jumping to case 7 runs the switch header, but none of the other cases' code.
  assert.deepEqual(doneAt(source, run, "branch", 2, 0), [0, 1, 2]);
  assert.deepEqual(doneAt(source, run, "statement", 8, 0), [0, 1, 2, 7, 8]);
  // The continue counts, and the if's closing brace it jumps past doesn't.
  assert.deepEqual(doneAt(source, run, "statement", 13, 0), [0, 1, 2, 7, 8, 10, 11, 12, 13]);
});

test("a switch crosses out the cases before the one it jumps to, and a break the rest", () => {
  const source = `int day = 2;
int w = 0;
switch (day) {
  case 1:
    w = 1;
  case 2:
    w = 2;
    break;
  case 3:
    w = 3;
}
`;
  assert.deepEqual(struckAt(source, "branch", 2, 0), [3, 4]);
  assert.deepEqual(struckAt(source, "statement", 7, 0), [3, 4, 8, 9]);
  // Nothing matching skips the whole body.
  assert.deepEqual(struckAt(source.replace("day = 2", "day = 9"), "branch", 2, 0), [3, 4, 5, 6, 7, 8, 9]);
});

test("a block's opening brace counts as done once execution is inside it", () => {
  const source = `int a = 1;
{
  int b = 2;
}
`;
  const run = runCProgram(source);
  assert.deepEqual(doneAt(source, run, "statement", 0, 0), [0]);
  assert.deepEqual(doneAt(source, run, "statement", 2, 0), [0, 1, 2]);
});

test("each call of a recursive function has its own done and skipped lines", () => {
  const source = `int f(int n) {
  if (n <= 1) {
    return 1;
  }
  return n * f(n - 1);
}

int main(void) {
  int r = f(2);
  return 0;
}
`;
  const run = runCFiles([{ path: "program.c", source }], undefined, "", false, {
    stepLimit: 100_000,
    followingTraceLimit: 256,
    traceFunctions: true,
  });
  const lines = source.split("\n");
  const at = (kind: string, line: number, occurrence: number) => {
    let seen = 0;
    return run.trace.findIndex((event) => event.kind === kind && event.startLine === line && seen++ === occurrence);
  };
  // The inner call hasn't checked its if yet, so the outer call's skip isn't shown.
  const innerCall = at("call", 4, 0);
  assert.deepEqual([...strikeFragmentsForTrace(lines, run.trace, innerCall)], []);
  assert.deepEqual(doneAt(source, run, "call", 4, 0), [0, 7]);
  // Back in the outer call, the inner call's return 1 isn't one of its lines.
  const backInOuter = at("return", 4, 0);
  const done = [...doneLinesForTrace(lines, run.trace, backInOuter)].sort((a, b) => a - b);
  assert.deepEqual(done, [0, 1, 7]);
  assert.deepEqual(new Set(strikeFragmentsForTrace(lines, run.trace, backInOuter).map((f) => f.line)), new Set([1, 2, 3]));
});

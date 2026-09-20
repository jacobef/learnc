import assert from "node:assert/strict";
import { executionBoundaries, boundaryOccurrences, canJumpToBoundary, defaultOccurrence, nearestOccurrence } from "../js/shared-sandbox-navigation.js";

const trace = [1, 2, 3, 2, 3, 2, 4].map(startLine => ({ file: "program.c", startLine, endLine: startLine }));
trace.splice(3, 0, { file: "helper.c", startLine: 2, endLine: 2 });
const end = { file: "program.c", line: 7 };
const close = { file: "program.c", line: 5 };
const boundaries = executionBoundaries(trace, close, null, end);
assert.equal(boundaries.length, trace.length + 1);
assert.deepEqual(boundaries.at(-1), close);
const occurrences = boundaryOccurrences(boundaries, "program.c");
assert.deepEqual(occurrences.get(2), [1, 4, 6]);
assert.equal(occurrences.has(0), false);
assert.deepEqual(boundaryOccurrences(boundaries, "helper.c").get(2), [3]);
for (const [current, expected] of [[0, 1], [1, 4], [3, 4], [4, 6], [6, 4], [8, 6]]) {
  assert.equal(defaultOccurrence(occurrences.get(2)!, current), expected);
}
assert.equal(nearestOccurrence([1, 4, 6], 1, "backward"), undefined);
assert.equal(nearestOccurrence([1, 4, 6], 6, "forward"), undefined);
assert.equal(defaultOccurrence([3], 3), undefined);
assert.equal(defaultOccurrence([], 0), undefined);
assert.equal(canJumpToBoundary([1, 4, 6], 4, false), false);
assert.equal(canJumpToBoundary([1, 4, 6], 4, true), true);
assert.equal(canJumpToBoundary([4], 4, true), false);
assert.equal(canJumpToBoundary([4], 4, false), false);
assert.equal(canJumpToBoundary([1, 4, 6], 3, false), true);
assert.equal(canJumpToBoundary([1, 4, 6], 3, true), true);
assert.equal(canJumpToBoundary([], 3, true), false);
assert.deepEqual(executionBoundaries(trace, close, { file: "helper.c", startLine: 9 }, end).at(-1), { file: "helper.c", line: 9 });
assert.deepEqual(executionBoundaries(trace, { file: "program.c", line: 4 }, null, end).at(-1), end);
assert.equal(executionBoundaries(trace, close, null, end, true).at(-1), null);
assert.equal(boundaryOccurrences(executionBoundaries(trace, close, null, end, true), "program.c").has(close.line), false);
console.log("PASS reachable boundaries, repeated occurrences, file isolation, and forward/backward jumps");

const { boundaryTargets } = await import("../js/shared-sandbox-navigation.js");
const loopBoundaries = [
  {file: "program.c", line: 1, kind: "statement"},
  {file: "program.c", line: 2, kind: "loop"},
  {file: "program.c", line: 3, kind: "statement"},
  {file: "program.c", line: 2, kind: "loop"},
  {file: "program.c", line: 3, kind: "statement"},
  {file: "program.c", line: 2, kind: "loop"},
];
const targets = boundaryTargets(loopBoundaries, "program.c");
const first = targets.find(target => target.line === 2 && target.kind === "first")!;
const nearest = targets.find(target => target.line === 2 && target.kind === "nearest")!;
assert.deepEqual(first.positions, [1, 3, 5]);
assert.deepEqual(nearest.positions, first.positions);
assert.equal(first.positions[0], 1);
assert.equal(defaultOccurrence(nearest.positions, 2), 3);
assert.equal(defaultOccurrence(nearest.positions, 6), 5);
assert.equal(first.paired, true);
assert.equal(nearest.paired, true);
assert.equal(targets.filter(target => target.line === 3).length, 1);
assert.equal(targets.find(target => target.line === 3)!.paired, false);
assert.equal(boundaryTargets(loopBoundaries, "other.c").length, 0);
console.log("PASS first and nearest loop controls share ordinary occurrences");

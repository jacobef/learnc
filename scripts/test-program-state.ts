// Run with: node scripts/test-program-state.ts (after building Wasm and running tsc -p .).
import assert from "node:assert/strict";
import { test } from "node:test";
import type { BoxState } from "../shared-core-utils.ts";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const { basicHintForBoxes, stateMatches, stateWithArrayRoots } =
  await import("../js/shared-program-state.js");

const variable = (name: string, value: string = ""): BoxState => ({ name, type: "int", value });

test("matching answers reject duplicate variables and use C literal conversion", () => {
  const expected = [variable("x", "10")];
  assert.equal(stateMatches([variable("x", "0xa")], expected), true);
  assert.equal(stateMatches([variable("x", "9")], expected), false);
  assert.equal(stateMatches([variable("x", "1 + 9")], expected), false);
  assert.equal(stateMatches([...expected, ...expected], expected), false);
});

test("array roots are display metadata, while every element remains part of the answer", () => {
  const elements: BoxState[] = [0, 1].map((index) => ({
    ...variable(`a[${index}]`, String(index)),
    arrayRoot: "a", arrayShape: [2], arrayIndices: [index],
  }));
  const rooted = stateWithArrayRoots(elements);
  assert.equal(rooted.length, 3);
  assert.equal(stateWithArrayRoots(rooted).length, 3);
  assert.equal(stateMatches(rooted, elements), true);
  assert.equal(stateMatches(rooted.slice(1), elements), false);
  assert.equal(stateMatches([...rooted, elements[0]], elements), false);
});

test("hints distinguish removed variables from missing new variables", () => {
  const baseline = [variable("x")];
  const expected = [...baseline, variable("y")];
  const stage = { runLine: 1, runEndLine: 1 };
  assert.deepEqual(basicHintForBoxes([], expected, baseline, stage), {
    kind: "removed", variable: "x", message: "This line shouldn't remove the $n{x} variable.",
  });
  assert.deepEqual(basicHintForBoxes(baseline, expected, baseline, stage), {
    kind: "count", variable: "y", message: "You need to add the $n{y} variable.",
  });
});

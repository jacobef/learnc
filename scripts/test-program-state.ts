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

const { normalizeChallengeInput } = await import("../js/shared-code-challenge-input.js");
const { allocateCWorkspaceObject } = await import("../js/shared-c-workspace-model.js");

test("challenge inputs preserve scalar values across internal naming changes", () => {
  for (const [literal, type, expected] of [
    [" -10 ", "int", "-10"], ["0", "int", "0"], ["2147483647", "int", "2147483647"],
    ["3.5", "double", "3.5"], ["257", "unsigned char", "1"], ["0.5", "_Bool", "1"],
  ]) {
    assert.deepEqual(normalizeChallengeInput(literal, type), { runtime: expected, literal: literal.trim() });
  }
  assert.throws(() => normalizeChallengeInput("missing", "int"), /not valid C/);
});

test("lesson objects receive distinct interpreter-backed addresses", () => {
  const boxes: BoxState[] = [];
  for (const type of ["int", "double", "char", "int *", "int"]) {
    const allocation = allocateCWorkspaceObject(boxes, type);
    assert.ok(allocation, type);
    const start = Number(allocation.address);
    assert.ok(Number.isFinite(start));
    for (const box of boxes) {
      assert.ok(start >= Number(box.address) + box.typeInfo!.size!);
    }
    boxes.push({ name: `v${boxes.length}`, type, value: "", address: allocation.address, typeInfo: allocation.typeInfo });
  }
});

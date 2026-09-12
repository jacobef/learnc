import { normalizeZeroDisplay, type BoxState } from "./shared-core-utils.js";
import { boxValueMatchesSpec } from "./shared-c-value-semantics.js";

export interface BasicProgramHint {
  kind: "count" | "removed" | "not-removed" | "name" | "type" | "value";
  variable?: string;
  message: string;
}

function isArrayRootStateBox(box: BoxState): boolean {
  if (box.arrayRoot) return false;
  const shape = Array.isArray(box.arrayShape) ? box.arrayShape : [];
  const indices = Array.isArray(box.arrayIndices) ? box.arrayIndices : [];
  if (shape.length > 0 && indices.length === 0) return true;
  return /\[\s*\d+\s*\]\s*$/.test(String(box.type || ""));
}

export function stateWithArrayRoots(boxes: BoxState[]): BoxState[] {
  const result = boxes.slice();
  const rootNames = new Set(
    result
      .filter((box) => isArrayRootStateBox(box))
      .map((box) => String(box.name || "").trim())
      .filter(Boolean),
  );
  const firstElementByRoot = new Map<string, BoxState>();
  for (const box of result) {
    const rootName = String(box.arrayRoot || "").trim();
    if (rootName && !firstElementByRoot.has(rootName)) {
      firstElementByRoot.set(rootName, box);
    }
  }
  for (const [rootName, first] of firstElementByRoot) {
    if (rootNames.has(rootName)) continue;
    const shape = Array.isArray(first.arrayShape)
      ? first.arrayShape
          .map((value) => Math.floor(Number(value)))
          .filter((value) => Number.isFinite(value) && value > 0)
      : [];
    if (!shape.length) continue;
    result.push({
      ...first,
      name: rootName,
      type: `${first.type}${shape.map((value) => `[${value}]`).join("")}`,
      value: "",
      rawValue: "",
      arrayRoot: null,
      arrayShape: shape,
      arrayIndices: [],
    });
  }
  return result;
}

function comparableStateBoxes(boxes: BoxState[]): BoxState[] {
  return boxes.filter((box) => !isArrayRootStateBox(box));
}

function comparableStateBoxKey(box: BoxState): string {
  const arrayRoot = String(box.arrayRoot || "").trim();
  if (arrayRoot) {
    const indices = Array.isArray(box.arrayIndices)
      ? box.arrayIndices.map((value) => Math.floor(Number(value))).join(",")
      : "";
    return `array:${arrayRoot}:${indices || box.name}`;
  }
  return `scalar:${box.name}`;
}

function visibleStateBoxes(boxes: BoxState[]): BoxState[] {
  return stateWithArrayRoots(boxes).filter((box) => !box.arrayRoot);
}

export function stateMatches(actual: BoxState[], expected: BoxState[]): boolean {
  const actualBoxes = comparableStateBoxes(actual);
  const actualByName = new Map(
    actualBoxes.map((box) => [comparableStateBoxKey(box), box]),
  );
  if (actualByName.size !== actualBoxes.length) return false;
  const expectedByName = new Map(
    comparableStateBoxes(expected).map((box) => [
      comparableStateBoxKey(box),
      box,
    ]),
  );
  if (actualByName.size !== expectedByName.size) return false;
  for (const [name, expectedBox] of expectedByName.entries()) {
    const actualBox = actualByName.get(name);
    if (!actualBox) return false;
    if ((actualBox.type || "").trim() !== (expectedBox.type || "").trim()) return false;
    if (!boxValueMatchesSpec(actualBox, expectedBox).ok) return false;
  }
  return true;
}

function formatNameList(names: string[]): string {
  const tokens = names.map((name) => `$n{${name}}`);
  if (tokens.length === 1) return tokens[0] || "";
  if (tokens.length === 2) return `${tokens[0]} and ${tokens[1]}`;
  return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
}

export function basicHintForBoxes(
  actual: BoxState[],
  expected: BoxState[],
  baseline: BoxState[],
  stage: { runLine: number; runEndLine: number },
): BasicProgramHint | null {
  const visibleActual = visibleStateBoxes(actual);
  const visibleExpected = visibleStateBoxes(expected);
  const visibleBaseline = visibleStateBoxes(baseline);
  const actualCount = visibleActual.length;
  const expectedCount = visibleExpected.length;
  const nameOf = (box: BoxState | null | undefined) =>
    String(box?.name || "").trim();
  const typeOf = (box: BoxState | null | undefined) =>
    String(box?.type || "").trim();
  const expectedNames = visibleExpected.map(nameOf).filter(Boolean);
  const expectedNameSet = new Set(expectedNames);
  const actualNames = visibleActual.map(nameOf);
  const actualNameSet = new Set(actualNames.filter(Boolean));
  const missingExpectedNames = expectedNames.filter(
    (name) => !actualNameSet.has(name),
  );
  const baselineNames = new Set(visibleBaseline.map(nameOf).filter(Boolean));

  const removedName = missingExpectedNames.find((name) =>
    baselineNames.has(name),
  );
  if (removedName) {
    return {
      message: `This line shouldn't remove the $n{${removedName}} variable.`,
      kind: "removed",
      variable: removedName,
    };
  }

  const extraBaselineNames = actualNames.filter(
    (name) => name && baselineNames.has(name) && !expectedNameSet.has(name),
  );
  if (extraBaselineNames.length > 0) {
    const name = extraBaselineNames[0] || "";
    if (name) {
      return {
        message: `This line should remove the $n{${name}} variable.`,
        kind: "not-removed",
        variable: name,
      };
    }
  }

  if (actualCount < expectedCount) {
    const expectedName = missingExpectedNames[0] || expectedNames[0] || "";
    if (!expectedName) {
      return { message: "You need to add a new variable.", kind: "count" };
    }
    return {
      message: `You need to add the $n{${expectedName}} variable.`,
      kind: "count",
      variable: expectedName,
    };
  }

  if (actualCount === expectedCount && missingExpectedNames.length > 0) {
    const expectedNewNames = expectedNames.filter(
      (name) => !baselineNames.has(name),
    );
    if (expectedNewNames.length > 1) {
      return {
        message: `The new variables should be named ${formatNameList(
          expectedNewNames,
        )}.`,
        kind: "name",
        variable: expectedNewNames[0],
      };
    }
    const expectedName = expectedNewNames[0] || missingExpectedNames[0] || "";
    if (!expectedName) return null;
    return {
      message: `The new variable should be named $n{${expectedName}}.`,
      kind: "name",
      variable: expectedName,
    };
  }

  if (actualCount > expectedCount) {
    const baselineCount = visibleBaseline.length;
    const expectedNew = Math.max(0, expectedCount - baselineCount);
    const actualNew = Math.max(0, actualCount - baselineCount);
    const extraCount = Math.max(0, actualNew - expectedNew);
    const start = Math.max(1, stage.runLine + 1);
    const end = Math.max(start, stage.runEndLine + 1);
    const label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
    const extraLabel = extraCount === 1 ? "variable" : "variables";
    if (expectedNew === 0) {
      return {
        message: `${label} shouldn't add any new variables. Remove the extra ${extraLabel}.`,
        kind: "count",
      };
    }
    const expectedLabel = expectedNew === 1 ? "variable" : "variables";
    return {
      message: `${label} should only add ${expectedNew} new ${expectedLabel}, but you added ${actualNew}. Remove the extra ${extraLabel}.`,
      kind: "count",
    };
  }

  const baselineByName = new Map<string, BoxState>();
  visibleBaseline.forEach((box) => {
    const name = nameOf(box);
    if (name && !baselineByName.has(name)) baselineByName.set(name, box);
  });
  const actualByName = new Map<string, BoxState>();
  visibleActual.forEach((box) => {
    const name = nameOf(box);
    if (name && !actualByName.has(name)) actualByName.set(name, box);
  });

  let deferredBe: BasicProgramHint | null = null;
  for (const expectedBox of visibleExpected) {
    const name = nameOf(expectedBox);
    if (!name) continue;
    const actualBox = actualByName.get(name);
    if (!actualBox) continue;
    const expectedType = typeOf(expectedBox);
    const actualType = typeOf(actualBox);
    if (actualType !== expectedType) {
      return {
        message: `$n{${name}}'s type should be $t{${expectedType}}.`,
        kind: "type",
        variable: name,
      };
    }
    const mismatch = !boxValueMatchesSpec(actualBox, expectedBox).ok;
    if (!mismatch) continue;
    const expectedValue = (expectedBox.value ?? "").trim();
    const label =
      expectedValue === "" ? "empty" : `$v{${normalizeZeroDisplay(expectedValue)}}`;
    const baselineBox = baselineByName.get(name);
    const shouldRemain = baselineBox
      ? boxValueMatchesSpec(baselineBox, expectedBox).ok
      : false;
    const message = `$n{${name}}'s value should ${shouldRemain ? "remain" : "be"} ${label}.`;
    if (shouldRemain) {
      return {
        message,
        kind: "value",
        variable: name,
      };
    }
    if (!deferredBe) {
      deferredBe = {
        message,
        kind: "value",
        variable: name,
      };
    }
  }

  const actualElementsByName = new Map(
    comparableStateBoxes(actual)
      .filter((box) => !!box.arrayRoot)
      .map((box) => [box.name, box]),
  );
  const baselineElementsByName = new Map(
    comparableStateBoxes(baseline)
      .filter((box) => !!box.arrayRoot)
      .map((box) => [box.name, box]),
  );
  for (const expectedBox of comparableStateBoxes(expected).filter(
    (box) => !!box.arrayRoot,
  )) {
    const name = nameOf(expectedBox);
    if (!name) continue;
    const actualBox = actualElementsByName.get(name);
    if (!actualBox) {
      return {
        message: `The $n{${name}} array element is missing.`,
        kind: "value",
        variable: name,
      };
    }
    const expectedType = typeOf(expectedBox);
    if (typeOf(actualBox) !== expectedType) {
      return {
        message: `$n{${name}}'s type should be $t{${expectedType}}.`,
        kind: "type",
        variable: name,
      };
    }
    if (boxValueMatchesSpec(actualBox, expectedBox).ok) {
      continue;
    }
    const expectedValue = (expectedBox.value ?? "").trim();
    const label =
      expectedValue === ""
        ? "empty"
        : `$v{${normalizeZeroDisplay(expectedValue)}}`;
    const baselineBox = baselineElementsByName.get(name);
    const shouldRemain = baselineBox
      ? boxValueMatchesSpec(baselineBox, expectedBox).ok
      : false;
    const message = `$n{${name}}'s value should ${shouldRemain ? "remain" : "be"} ${label}.`;
    if (shouldRemain) {
      return { message, kind: "value", variable: name };
    }
    if (!deferredBe) {
      deferredBe = { message, kind: "value", variable: name };
    }
  }

  return deferredBe;
}

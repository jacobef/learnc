import { createCodeEditorTemplate } from "./shared-code-editor.js";
import { boxesByName, firstRedeclaredName, formatNames, missingSemicolonHint } from "./shared-code-editor-hints.js";
import type { BoxState } from "./shared-core.js";

const targetState: BoxState[] = [
  { name: "apple", type: "int", value: "10", address: "<i>(any)</i>" },
  { name: "berry", type: "int", value: "5", address: "<i>(any)</i>" },
];

function missingTarget(boxes: Map<string, BoxState>): BoxState | null {
  return targetState.find((expected) => !boxes.has(expected.name)) ?? null;
}

function extraNames(boxes: BoxState[]): string[] {
  const targetNames = new Set(targetState.map((box) => box.name));
  return boxes.map((box) => box.name).filter((name) => name && !targetNames.has(name));
}

createCodeEditorTemplate({
  startCode: "",
  textareaMinLines: 4,
  targetState,
  next: "6-assignment-ii.html",
  instructions: "Write code yourself.",
  hints: (ctx) => {
    const semicolonHint = missingSemicolonHint(ctx.findMissingSemicolonLines(ctx.text));
    if (semicolonHint) return semicolonHint;

    const statements = ctx.parseStatements(ctx.text);
    const redeclared = firstRedeclaredName(statements);
    if (redeclared) return `Declare $n{${redeclared}} only once.`;

    const currentState = ctx.applyUserProgram();
    if (!currentState) {
      return "This code does not compile yet. Use simple declarations and assignments.";
    }

    const boxes = boxesByName(currentState);
    const missing = missingTarget(boxes);
    if (missing) {
      return `Create a variable named $n{${missing.name}}.`;
    }

    const extra = extraNames(currentState);
    if (extra.length) {
      return `You only need ${formatNames(targetState.map((box) => box.name))}. Remove ${formatNames(extra)}.`;
    }

    for (const expected of targetState) {
      const actual = boxes.get(expected.name)!;
      if (actual.type !== expected.type) {
        return `$n{${expected.name}} should have type $t{${expected.type}}.`;
      }
      if (actual.value === "") {
        return `$n{${expected.name}} needs value $v{${expected.value}}.`;
      }
      if (actual.value !== expected.value) {
        return `$n{${expected.name}}'s value should be $v{${expected.value}}, not $v{${actual.value}}.`;
      }
    }
  },
});

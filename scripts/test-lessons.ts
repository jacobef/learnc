// Run with: node scripts/test-lessons.ts (after building Wasm and running tsc -p .).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import type { createProgramTemplate } from "../shared-program-template.ts";
import type { createCodeEditorTemplate } from "../shared-code-editor.ts";
import type { createCodeOutputChallengeTemplate } from "../shared-code-output-template.ts";
import type { createExpressionEvalTemplate } from "../shared-expression-template.ts";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const { DEFAULT_NAV_ITEMS } = await import("../js/nav-items.js");
const { runCProgram, evaluateCExpression } = await import("../js/shared-c-interpreter.js");
const { normalizeChallengeInput } = await import("../js/shared-code-challenge-input.js");

type Lesson =
  | { kind: "program"; config: Parameters<typeof createProgramTemplate>[0] }
  | { kind: "editor"; config: Parameters<typeof createCodeEditorTemplate>[0] }
  | { kind: "challenge"; config: Parameters<typeof createCodeOutputChallengeTemplate>[0] }
  | { kind: "expression"; config: Parameters<typeof createExpressionEvalTemplate>[0] };

function run(source: string) {
  const result = runCProgram(source, 4096, "");
  assert.equal(result.kind, "ok", JSON.stringify(result));
  assert.equal(result.executionLimit, null, "Lesson must finish without loop detection or a step limit");
  assert.equal(result.blocked, null, "Lesson must not block on stdin");
  return result;
}

// Capture the actual lesson configuration without initializing its DOM template.
// This is interpreter integration coverage; browser checks cover the templates themselves.
function loadLesson(file: string): Lesson {
  const captured: Lesson[] = [];
  const source = readFileSync(new URL(`../js/${file}`, import.meta.url), "utf8");
  runInNewContext(source.replace(/^import\b[^;]*;\s*/gm, ""), {
    createProgramTemplate: (config: Parameters<typeof createProgramTemplate>[0]) => captured.push({ kind: "program", config }),
    createCodeEditorTemplate: (config: Parameters<typeof createCodeEditorTemplate>[0]) => captured.push({ kind: "editor", config }),
    createCodeOutputChallengeTemplate: (config: Parameters<typeof createCodeOutputChallengeTemplate>[0]) => captured.push({ kind: "challenge", config }),
    createExpressionEvalTemplate: (config: Parameters<typeof createExpressionEvalTemplate>[0]) => captured.push({ kind: "expression", config }),
  }, { filename: file, timeout: 1000 });
  assert.equal(captured.length, 1, "Each lesson must initialize exactly one template");
  return captured[0];
}

const lessons = DEFAULT_NAV_ITEMS.filter(item => /^\d+-.*\.html$/.test(item.href));
assert.ok(lessons.length > 0, "Navigation must contain numbered lessons");
for (const item of lessons) {
  test(`${item.label}: lesson programs run in the browser interpreter`, () => {
    const lesson = loadLesson(item.href.replace(/\.html$/, ".js"));
    switch (lesson.kind) {
      case "program":
        run(lesson.config.steps.map(step => step.code).join(""));
        break;
      case "editor":
        run(lesson.config.startCode || "");
        break;
      case "challenge": {
        const config = lesson.config;
        for (const [index, inputs] of [...config.testInputs, config.startInput].entries()) {
          assert.equal(inputs.length, config.inputs.length, `Input row ${index}`);
          const declarations = config.inputs.map((spec, i) => {
            const normalized = normalizeChallengeInput(inputs[i], spec.type);
            assert.notEqual(normalized.runtime, "", `Input ${spec.name}, row ${index}`);
            return `${spec.type} ${spec.name} = ${normalized.literal};\n`;
          }).join("");
          const result = run(declarations + config.solve);
          for (const spec of config.outputs) {
            const output = result.state.find(box => box.name === spec.name);
            assert.ok(output, `Missing output ${spec.name}, row ${index}`);
            assert.equal(output.type, spec.type);
            assert.notEqual(output.value.trim(), "", `Uninitialized output ${spec.name}, row ${index}`);
          }
        }
        break;
      }
      case "expression":
        for (const step of lesson.config.steps) {
          const source = step.setup || "0;";
          const setup = run(source);
          const result = evaluateCExpression(source, Math.max(0, setup.trace.length - 1), step.expression, 4096, "");
          assert.equal(result.kind, "ok", JSON.stringify(result));
          assert.ok(result.result.type, `Missing type for ${step.expression}`);
        }
        break;
    }
  });
}

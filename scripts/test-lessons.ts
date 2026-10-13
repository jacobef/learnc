// Run with: node scripts/test-lessons.ts (after building Wasm and running tsc -p .).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import type { createProgramTemplate } from "../shared-program-template.ts";
import type { createCodeEditorTemplate } from "../shared-code-editor.ts";
import type { createCodeOutputChallengeTemplate } from "../shared-code-output-template.ts";
import type { createExpressionEvalTemplate } from "../shared-expression-template.ts";
import type { createCompileQuizTemplate } from "../shared-compile-quiz-template.ts";
import type { createMachineChallengeTemplate, createMachineTemplate } from "../shared-machine-template.ts";

Object.assign(globalThis, {
  window: { location: { pathname: "/" } },
  localStorage: { getItem: () => null, setItem: () => {} },
});
const { DEFAULT_NAV_ITEMS } = await import("../js/nav-items.js");
const { runCProgram, runCFiles, evaluateCExpression } = await import("../js/shared-c-interpreter.js");
const { normalizeChallengeInput, runChallengeProgram } = await import("../js/shared-code-challenge-input.js");
const { REFERENCE_SECTIONS } = await import("../js/shared-reference-data.js");
const { assemble, runMachine, startState } = await import("../js/shared-toy-machine.js");

type Lesson =
  | { kind: "program"; config: Parameters<typeof createProgramTemplate>[0] }
  | { kind: "editor"; config: Parameters<typeof createCodeEditorTemplate>[0] }
  | { kind: "challenge"; config: Parameters<typeof createCodeOutputChallengeTemplate>[0] }
  | { kind: "expression"; config: Parameters<typeof createExpressionEvalTemplate>[0] }
  | { kind: "quiz"; config: Parameters<typeof createCompileQuizTemplate>[0] }
  | { kind: "machine"; config: Parameters<typeof createMachineTemplate>[0] }
  | { kind: "machine-challenge"; config: Parameters<typeof createMachineChallengeTemplate>[0] };

function run(source: string, implicitMain = true, stdin = "") {
  return checkedRun(implicitMain
    ? runCProgram(source, 4096, stdin)
    : runCFiles([{ path: "program.c", source }], 4096, stdin, false));
}

function checkedRun(result: ReturnType<typeof runCProgram>) {
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
    createCompileQuizTemplate: (config: Parameters<typeof createCompileQuizTemplate>[0]) => captured.push({ kind: "quiz", config }),
    createMachineTemplate: (config: Parameters<typeof createMachineTemplate>[0]) => captured.push({ kind: "machine", config }),
    createMachineChallengeTemplate: (config: Parameters<typeof createMachineChallengeTemplate>[0]) => captured.push({ kind: "machine-challenge", config }),
    // Page effects that only matter in a browser.
    startArraysAmbiance: () => {},
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
        run(lesson.config.steps.map(step => step.code).join(""), lesson.config.implicitMain ?? true, lesson.config.stdin);
        break;
      case "editor":
        run(lesson.config.startCode || "");
        break;
      case "challenge": {
        const config = lesson.config;
        const inputs = config.inputs ?? [];
        const shape = { ...config, inputs };
        for (const [index, row] of [...(config.testInputs ?? [[]]), config.startInput ?? []].entries()) {
          assert.equal(row.length, inputs.length, `Input row ${index}`);
          const literals = inputs.map((spec, i) => {
            const normalized = normalizeChallengeInput(row[i], spec);
            assert.notEqual(normalized.runtime, "", `Input ${spec.name}, row ${index}`);
            return normalized.literal;
          });
          const result = checkedRun(runChallengeProgram(shape, literals, config.solve));
          for (const spec of config.outputs ?? []) {
            const output = result.state.find(box => box.name === spec.name);
            assert.ok(output, `Missing output ${spec.name}, row ${index}`);
            assert.equal(output.type, spec.type);
            const valued = result.state.filter(box => box.arrayRoot === spec.name);
            for (const box of valued.length ? valued : [output]) {
              assert.notEqual(box.value.trim(), "", `Uninitialized output ${box.name}, row ${index}`);
            }
          }
        }
        break;
      }
      case "machine": {
        const program = assemble(lesson.config.steps.map(step => step.code).join(""));
        const result = runMachine(program, startState(lesson.config.start));
        assert.equal(result.error, null);
        assert.ok(result.steps.at(-1)?.op === "HALT", "Machine lessons end by halting");
        break;
      }
      case "machine-challenge": {
        const program = assemble(lesson.config.solve);
        assemble(lesson.config.startCode ?? "");
        for (const [index, start] of lesson.config.cases.entries()) {
          assert.equal(runMachine(program, startState(start)).error, null, `Case ${index}`);
        }
        break;
      }
      case "quiz":
        for (const [index, step] of lesson.config.steps.entries()) {
          if (step.choices) continue;
          const source = step.code.endsWith("\n") ? step.code : `${step.code}\n`;
          const result = lesson.config.implicitMain === false
            ? runCFiles([{ path: "program.c", source }], 4096, "", false)
            : runCProgram(source, 4096, "");
          // Every outcome is a legitimate answer; the program just has to stop.
          if (result.kind === "ok") assert.equal(result.executionLimit, null, `Step ${index} must finish`);
          if (result.kind === "ub") assert.ok(lesson.config.allowUndefinedAnswer || step.question === false, `Step ${index} has UB`);
        }
        break;
      case "expression":
        for (const step of lesson.config.steps) {
          const source = step.setup || "0;";
          const setup = run(source);
          const result = evaluateCExpression(source, Math.max(0, setup.trace.length - 1), step.expression, 4096, "");
          if (result.kind === "ub" && lesson.config.workspace?.allowUndefinedAnswer) continue;
          assert.equal(result.kind, "ok", JSON.stringify(result));
          assert.ok(result.result.type, `Missing type for ${step.expression}`);
        }
        break;
    }
  });
}

test("Quick reference entries name lessons in the navigation", () => {
  const hrefs = new Set(DEFAULT_NAV_ITEMS.map(item => item.href));
  for (const section of REFERENCE_SECTIONS) {
    const entries = [...(section.notes ?? []), ...section.groups.flatMap(group => group.entries)];
    for (const entry of entries) assert.ok(hrefs.has(entry.lesson), `${section.title}: ${entry.lesson}`);
  }
});

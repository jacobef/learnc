import { renderStatePanel } from "./shared-workspace-dom.js";
import {
  bindBtnRefPulse,
  clearNode,
  createStepper,
  flashStatus,
  setPartsContent,
} from "./shared-core.js";
import type {
  BoxState,
  Parts,
  ProgramDiagnostic,
  Stepper,
} from "./shared-core.js";
import {
  bindCodeEditorTabKey,
  ensureCodeSurfaceElements,
  updateCodeSurface,
} from "./shared-code-editor-surface.js";
import { ensureCodeLessonLayout } from "./shared-code-lesson-layout.js";
import {
  diagnosticDecorations,
  diagnosticMessageText,
  offsetDiagnosticLines,
} from "./shared-diagnostics.js";
import { runCProgram } from "./shared-c-interpreter.js";
import type { CProgramResult } from "./shared-c-interpreter-types.js";
import {
  codeRuntimeIssue,
  renderCodeDiagnostic,
  diagnosticRuntimeContextText,
  offsetCodeRuntimeIssue,
  renderCodeRuntimeIssue,
  type CodeRuntimeIssue,
} from "./shared-code-runtime-issues.js";
import { boxValueMatchesSpec } from "./shared-c-value-semantics.js";
import {
  createButtonTokenReplacer,
  createHintPresenter,
  createLevelProgressController,
  invalidTemplateConfig,
  nextLessonLabel,
  resetLevelAfterConfirmation,
} from "./shared-lesson-runtime.js";

type ChallengeParts = Parts;
type ChallengeRuntimeValue = string;
type ChallengePartsSpec =
  | ChallengeParts
  | ((ctx: CodeOutputChallengeHintContext) => ChallengeParts | null | undefined)
  | null;

type ChallengeFailureKind =
  | "compile"
  | "ub"
  | "step-limit"
  | "blocked-input"
  | "missing-output"
  | "wrong-output-type"
  | "wrong-output-value";

interface ChallengeVariableSpec {
  name: string;
  type: string;
}

interface CodeOutputChallengeConfig {
  inputs: ChallengeVariableSpec[];
  outputs: ChallengeVariableSpec[];
  testInputs: string[][];
  startInput: string[];
  solve: string;
  instructions?: string;
  startCode?: string;
  textareaMinLines?: number;
  allowNewLines?: boolean;
  hints?: ChallengePartsSpec;
  next?: string | null;
  nextLabel?: string;
}

interface ChallengeCase {
  inputValues: ChallengeRuntimeValue[];
  inputLiterals: string[];
  expectedLiterals: string[];
}

interface ChallengeCaseResult {
  ok: boolean;
  kind: "ok" | ChallengeFailureKind;
  state: BoxState[] | null;
  outputBox: BoxState | null;
  expected: BoxState | null;
  failingOutput: ChallengeVariableSpec | null;
}

interface ChallengeRunItem {
  index: number;
  testCase: ChallengeCase;
  result: ChallengeCaseResult;
}

interface ChallengeRunReport {
  pass: boolean;
  items: ChallengeRunItem[];
  firstFailure: ChallengeRunItem | null;
}

interface VisibleProgramFeedback {
  diagnostic: ProgramDiagnostic | null;
  runtimeIssue: CodeRuntimeIssue | null;
}

interface CodeOutputChallengeState {
  text: string;
  pass: boolean;
  visibleCase: ChallengeCase;
  testCases: ChallengeCase[];
  lastReport: ChallengeRunReport | null;
  pendingFailingCase: ChallengeCase | null;
  showFullShownOutput: boolean;
}

interface CodeOutputChallengeHintContext {
  text: string;
  inputs: Array<ChallengeVariableSpec & { value: ChallengeRuntimeValue }>;
  outputs: ChallengeVariableSpec[];
  currentCase: ChallengeCase;
  currentResult: ChallengeCaseResult;
  report: ChallengeRunReport;
  behavesLike: (program: string) => boolean;
}

interface CodeOutputChallengeProgress {
  text: string;
  pass: boolean;
  visibleCaseInputLiterals: string[];
  pendingFailingCaseInputLiterals: string[] | null;
  showFullShownOutput: boolean;
  hasRunChecks: boolean;
}

function createCodeOutputChallengeTemplate(
  config: CodeOutputChallengeConfig,
): void {
  const {
    inputs,
    outputs,
    testInputs,
    startInput,
    solve,
    instructions = "",
    startCode = "",
    textareaMinLines = 5,
    allowNewLines = true,
    hints = null,
    next = null,
    nextLabel,
  } = config;
  const endLabel = nextLessonLabel({
    next,
    fallback: "Next Program",
    override: nextLabel,
  });

  const ensureIdentifier = (name: string, label: string): string => {
    const trimmed = String(name || "").trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed)) {
      invalidTemplateConfig(`${label} must be a valid C identifier.`);
    }
    return trimmed;
  };

  if (!Array.isArray(inputs) || inputs.length === 0) {
    invalidTemplateConfig("inputs must be a non-empty array.");
  }
  if (!Array.isArray(outputs) || outputs.length === 0) {
    invalidTemplateConfig("outputs must be a non-empty array.");
  }
  const inputSpecs: ChallengeVariableSpec[] = inputs.map((spec, index) => ({
    name: ensureIdentifier(spec?.name || "", `Input name ${index + 1}`),
    type: String(spec?.type || "").trim(),
  }));
  const outputSpecs: ChallengeVariableSpec[] = outputs.map((spec, index) => ({
    name: ensureIdentifier(spec?.name || "", `Output name ${index + 1}`),
    type: String(spec?.type || "").trim(),
  }));
  if (!Array.isArray(testInputs)) {
    invalidTemplateConfig("testInputs must be an array.");
  }
  if (testInputs.length === 0) {
    invalidTemplateConfig("testInputs must contain at least one value.");
  }
  if (
    testInputs.some(
      (row) =>
        !Array.isArray(row) ||
        row.length !== inputSpecs.length ||
        row.some((value) => typeof value !== "string"),
    )
  ) {
    invalidTemplateConfig(
      `Each testInputs entry must be a string array of length ${inputSpecs.length}.`,
    );
  }
  if (!Array.isArray(startInput)) {
    invalidTemplateConfig("startInput is required.");
  }
  if (
    startInput.length !== inputSpecs.length ||
    startInput.some((value) => typeof value !== "string")
  ) {
    invalidTemplateConfig(`startInput must be a string array of length ${inputSpecs.length}.`);
  }
  if (typeof solve !== "string") {
    invalidTemplateConfig("solve must be a C code string.");
  }
  if (!Number.isFinite(textareaMinLines)) {
    invalidTemplateConfig("textareaMinLines must be a number.");
  }
  const solveCode = String(solve || "").replace(/\r\n/g, "\n");
  if (!solveCode.trim()) {
    invalidTemplateConfig("solve must be a non-empty C code string.");
  }

  const {
    instructionsEl,
    lockedLineNumbers,
    lockedInputLine,
    editor,
    lineNumbers,
    stage,
    status,
    diagnosticEl,
    hintPanel,
    hintBtn,
    checkBtn,
    levelResetBtn,
    rerollBtn,
    showFailBtn,
    prevBtn,
    nextBtn,
    codeRoot,
  } = ensureCodeLessonLayout({ textareaMinLines, lockedInput: true });
  const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);

  bindBtnRefPulse(codeRoot || document);

  function normalizeProgramBody(text: string): string {
    const normalized = String(text || "").replace(/\r\n/g, "\n");
    return normalized === "" || normalized.endsWith("\n")
      ? normalized
      : `${normalized}\n`;
  }

  function normalizeInputLiteral(
    value: string,
    type: string,
    label: string,
  ): { runtime: ChallengeRuntimeValue; literal: string } {
    const literal = value.trim();
    const run = runCProgram(`${type} __cboxes_input = ${literal};\n`);
    if (run.kind !== "ok") {
      return invalidTemplateConfig(`${label} is not valid C: ${diagnosticMessageText(run.diagnostic)}`);
    }
    const stored = run.state.find((box) => box.name === "__cboxes_input");
    if (!stored) return invalidTemplateConfig(`${label} did not create a scalar C value.`);
    return {
      runtime: stored.value,
      literal,
    };
  }

  const preludeLineCount = inputSpecs.length;
  const targetOutputNameSet = new Set(outputSpecs.map((spec) => spec.name));
  const outputNamesText = outputSpecs.map((spec) => spec.name).join(", ");

  function lockedInputLinesForCase(
    testCase: Pick<ChallengeCase, "inputLiterals">,
  ): string[] {
    return inputSpecs.map(
      (inputSpec, index) =>
        `${inputSpec.type} ${inputSpec.name} = ${testCase.inputLiterals[index]};`,
    );
  }

  function lockedInputTextForCase(
    testCase: Pick<ChallengeCase, "inputLiterals">,
  ): string {
    return lockedInputLinesForCase(testCase).join("\n");
  }

  function fullProgramTextForBody(
    testCase: Pick<ChallengeCase, "inputLiterals">,
    body: string,
  ): string {
    return `${lockedInputTextForCase(testCase)}\n${normalizeProgramBody(body)}`;
  }

  function expectedLiteralsFromSolve(
    testCase: Pick<ChallengeCase, "inputLiterals">,
    label: string,
  ): string[] {
    const text = fullProgramTextForBody(testCase, solveCode);
    const analyzed = runCProgram(text);
    const solvedState =
      analyzed.kind === "ok" && !analyzed.executionLimit
        ? analyzed.state
        : null;
    if (!solvedState) {
      if (analyzed.kind === "compile") {
        invalidTemplateConfig(`solve does not compile for ${label}.`);
      }
      invalidTemplateConfig(`solve has undefined behavior for ${label}.`);
      return [];
    }
    const expectedLiterals: string[] = [];
    for (const outputSpec of outputSpecs) {
      const outputBox =
        solvedState.find((box: BoxState) => box.name === outputSpec.name) || null;
      if (!outputBox) {
        invalidTemplateConfig(`solve must create ${outputSpec.name} for ${label}.`);
        return [];
      }
      if (String(outputBox.type || "").trim() !== outputSpec.type) {
        invalidTemplateConfig(
          `solve must produce ${outputSpec.name} with type ${outputSpec.type} for ${label}.`,
        );
        return [];
      }
      const literal = String(outputBox.value ?? "").trim();
      if (!literal) {
        invalidTemplateConfig(`solve leaves ${outputSpec.name} without a value for ${label}.`);
        return [];
      }
      expectedLiterals.push(literal);
    }
    return expectedLiterals;
  }

  function createChallengeCaseForInputRow(
    rawInputRow: string[],
    label: string,
  ): ChallengeCase {
    const normalizedInputs = inputSpecs.map((inputSpec, index) =>
      normalizeInputLiteral(
        rawInputRow[index]!,
        inputSpec.type,
        `${label} input ${inputSpec.name}`,
      ),
    );
    const partialCase: Pick<ChallengeCase, "inputLiterals"> = {
      inputLiterals: normalizedInputs.map((item) => item.literal),
    };
    return {
      inputValues: normalizedInputs.map((item) => item.runtime),
      inputLiterals: partialCase.inputLiterals.slice(),
      expectedLiterals: expectedLiteralsFromSolve(partialCase, label),
    };
  }

  const testCases: ChallengeCase[] = testInputs.map((row, index) =>
    createChallengeCaseForInputRow(row, `testInputs[${index}]`),
  );

  function copyCase(testCase: ChallengeCase): ChallengeCase {
    return {
      inputValues: testCase.inputValues.slice(),
      inputLiterals: testCase.inputLiterals.slice(),
      expectedLiterals: testCase.expectedLiterals.slice(),
    };
  }

  function caseInputKey(testCase: Pick<ChallengeCase, "inputLiterals">): string {
    return testCase.inputLiterals.join("\u0000");
  }

  function pickDifferentTestCase(currentCase: ChallengeCase): ChallengeCase {
    const currentKey = caseInputKey(currentCase);
    const candidates = testCases.filter(
      (testCase) => caseInputKey(testCase) !== currentKey,
    );
    if (!candidates.length) return copyCase(currentCase);
    const index = Math.floor(Math.random() * candidates.length);
    const item = candidates[index]!;
    return copyCase(item);
  }

  function restoreCase(
    inputLiterals: string[] | null | undefined,
  ): ChallengeCase | null {
    if (!Array.isArray(inputLiterals) || inputLiterals.length !== inputSpecs.length) {
      return null;
    }
    try {
      return createChallengeCaseForInputRow(inputLiterals, "saved progress");
    } catch {
      return null;
    }
  }

  const progress = createLevelProgressController<CodeOutputChallengeProgress>(
    isDefaultProgress,
  );
  const defaultText = normalizeUserCodeText(startCode);
  const defaultVisibleCase = createChallengeCaseForInputRow(startInput, "startInput");
  const restoredProgress = progress.restore();
  const restoredVisibleCase = restoreCase(
    restoredProgress?.visibleCaseInputLiterals,
  );
  const restoredPendingCase = restoreCase(
    restoredProgress?.pendingFailingCaseInputLiterals,
  );
  const restoredHadRunChecks = restoredProgress?.hasRunChecks === true;

  const state: CodeOutputChallengeState = {
    text:
      typeof restoredProgress?.text === "string"
        ? normalizeUserCodeText(restoredProgress.text)
        : defaultText,
    pass: restoredProgress?.pass === true,
    visibleCase: restoredVisibleCase || copyCase(defaultVisibleCase),
    testCases,
    lastReport: null,
    pendingFailingCase: restoredPendingCase,
    showFullShownOutput: restoredProgress?.showFullShownOutput === true,
  };

  let pager: Stepper | null = null;

  function normalizeUserCodeText(text: string): string {
    const normalized = text.replace(/\r\n/g, "\n");
    if (allowNewLines) return normalized;
    return normalized.replace(/\n/g, " ");
  }

  function adjustSelectionForCarriageReturns(
    text: string,
    pos: number,
  ): number {
    let removed = 0;
    for (let i = 0; i < pos && i < text.length; i++) {
      if (text[i] === "\r") removed += 1;
    }
    return Math.max(0, pos - removed);
  }

  function getEditorText(): string {
    return fullProgramTextForCase(state.visibleCase);
  }

  function getUserText(): string {
    const raw = editor ? editor.value : state.text || "";
    return normalizeUserCodeText(raw);
  }

  function fullProgramTextForCase(testCase: ChallengeCase): string {
    return fullProgramTextForBody(testCase, getUserText());
  }

  if (editor) {
    const lines = Math.max(1, Number(textareaMinLines));
    editor.style.minHeight = `calc(var(--code-line-height) * ${lines} + 16px)`;
    editor.value = state.text || "";
  }

  function getUserRawLines(): string[] {
    const raw = editor ? editor.value : state.text || "";
    return raw.split(/\r?\n/);
  }

  function syncEditorLinkedScroll() {
    if (!editor) return;
    if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
  }

  let visibleProgramFeedback: VisibleProgramFeedback = {
    diagnostic: null,
    runtimeIssue: null,
  };
  let visibleCaseResult: ChallengeCaseResult | null = null;

  function getVisibleProgramFeedback(
    result: CProgramResult,
  ): VisibleProgramFeedback {
    if (result.kind !== "ok") {
      const diagnostic = result.diagnostic;
      return {
        diagnostic:
          diagnostic.range.startLine < preludeLineCount
            ? null
            : offsetDiagnosticLines(diagnostic, -preludeLineCount),
        runtimeIssue: null,
      };
    }
    const issue = codeRuntimeIssue(result);
    return {
      diagnostic: null,
      runtimeIssue: issue
        ? offsetCodeRuntimeIssue(issue, -preludeLineCount)
        : null,
    };
  }

  function updateLineGutters(
    diagnostic: ProgramDiagnostic | null,
    runtimeIssue: CodeRuntimeIssue | null,
  ) {
    const lines = getUserRawLines();
    const lineNumberClasses = new Map<number, string[]>();
    const problemLine = diagnostic?.range.startLine ?? runtimeIssue?.range.startLine;
    if (problemLine !== undefined) {
      lineNumberClasses.set(problemLine, ["has-error"]);
    }
    updateCodeSurface({
      editor,
      lineNumbers,
      highlightEl,
      measureEl,
      lines,
      lineNumberStart: preludeLineCount + 1,
      decorations: diagnosticDecorations(diagnostic, lines),
      lineNumberClasses,
    });
    syncEditorLinkedScroll();
    if (diagnostic) {
      renderCodeDiagnostic(diagnosticEl, editor, diagnostic, { displayedLineOffset: preludeLineCount });
    } else {
      renderCodeRuntimeIssue(
        diagnosticEl,
        editor,
        runtimeIssue,
        preludeLineCount,
      );
    }
  }

  type ProgramBehavior =
    | {
        kind:
          | "compile"
          | "ub"
          | "step-limit"
          | "blocked-input"
          | "missing-output"
          | "wrong-output-type";
      }
    | { kind: "ok"; outputBoxes: BoxState[] };

  function validateOutputBoxes(
    finalState: BoxState[],
  ):
    | { kind: "ok"; boxes: BoxState[] }
    | {
        kind: "missing-output" | "wrong-output-type";
        outputBox: BoxState | null;
        outputSpec: ChallengeVariableSpec;
      } {
    const boxes: BoxState[] = [];
    for (const outputSpec of outputSpecs) {
      const outputBox =
        finalState.find((box: BoxState) => box.name === outputSpec.name) || null;
      if (!outputBox) {
        return { kind: "missing-output", outputBox: null, outputSpec };
      }
      if (String(outputBox.type || "").trim() !== outputSpec.type) {
        return { kind: "wrong-output-type", outputBox, outputSpec };
      }
      boxes.push(outputBox);
    }
    return { kind: "ok", boxes };
  }

  function evaluateProgramBehaviorForCase(
    body: string,
    testCase: ChallengeCase,
  ): ProgramBehavior {
    const text = fullProgramTextForBody(testCase, body);
    const analyzed = runCProgram(text);
    if (analyzed.kind !== "ok") {
      return { kind: analyzed.kind };
    }
    const runtimeIssue = codeRuntimeIssue(analyzed);
    if (runtimeIssue) {
      return {
        kind:
          runtimeIssue.kind === "execution-limit"
            ? "step-limit"
            : "blocked-input",
      };
    }
    const checked = validateOutputBoxes(analyzed.state);
    if (checked.kind !== "ok") {
      return { kind: checked.kind };
    }
    return { kind: "ok", outputBoxes: checked.boxes };
  }

  function behavesLikeProgramOnTestInputs(program: string): boolean {
    if (!testCases.length) return false;
    const candidateBody = normalizeProgramBody(program);
    if (!candidateBody.trim()) return false;
    const userBody = normalizeProgramBody(getUserText());
    for (const testCase of testCases) {
      const userBehavior = evaluateProgramBehaviorForCase(userBody, testCase);
      const candidateBehavior = evaluateProgramBehaviorForCase(
        candidateBody,
        testCase,
      );
      if (userBehavior.kind !== candidateBehavior.kind) return false;
      if (userBehavior.kind !== "ok" || candidateBehavior.kind !== "ok") {
        continue;
      }
      for (let index = 0; index < outputSpecs.length; index += 1) {
        if (
          !boxValueMatchesSpec(
            userBehavior.outputBoxes[index]!,
            candidateBehavior.outputBoxes[index]!,
          ).ok
        ) {
          return false;
        }
      }
    }
    return true;
  }

  function evaluateCase(
    testCase: ChallengeCase,
    analyzed = runCProgram(fullProgramTextForCase(testCase)),
  ): ChallengeCaseResult {
    const expectedBoxes = expectedBoxesForCase(testCase);
    const fallbackExpected = expectedBoxes[0] || null;
    const fallbackOutput = outputSpecs[0] || null;
    if (analyzed.kind !== "ok") {
      return {
        ok: false,
        kind: analyzed.kind,
        state: analyzed.diagnostic.runtimeContext?.state ?? null,
        outputBox: null,
        expected: fallbackExpected,
        failingOutput: fallbackOutput,
      };
    }
    const runtimeIssue = codeRuntimeIssue(analyzed);
    if (runtimeIssue) {
      return {
        ok: false,
        kind:
          runtimeIssue.kind === "execution-limit"
            ? "step-limit"
            : "blocked-input",
        state:
          runtimeIssue.kind === "blocked-input"
            ? analyzed.blocked?.state ?? analyzed.state
            : analyzed.state,
        outputBox: null,
        expected: fallbackExpected,
        failingOutput: fallbackOutput,
      };
    }
    const finalState = analyzed.state;
    for (let index = 0; index < outputSpecs.length; index += 1) {
      const outputSpec = outputSpecs[index]!;
      const expected = expectedBoxes[index]!;
      const actual =
        finalState.find((box: BoxState) => box.name === outputSpec.name) || null;
      if (!actual) {
        return {
          ok: false,
          kind: "missing-output",
          state: finalState,
          outputBox: null,
          expected,
          failingOutput: outputSpec,
        };
      }
      if (String(actual.type || "").trim() !== outputSpec.type) {
        return {
          ok: false,
          kind: "wrong-output-type",
          state: finalState,
          outputBox: actual,
          expected,
          failingOutput: outputSpec,
        };
      }
      if (!boxValueMatchesSpec(actual, expected).ok) {
        return {
          ok: false,
          kind: "wrong-output-value",
          state: finalState,
          outputBox: actual,
          expected,
          failingOutput: outputSpec,
        };
      }
    }
    return {
      ok: true,
      kind: "ok",
      state: finalState,
      outputBox:
        finalState.find(
          (box: BoxState) =>
            box.name === (outputSpecs[0]?.name || ""),
        ) || null,
      expected: fallbackExpected,
      failingOutput: null,
    };
  }

  function runAllCases(): ChallengeRunReport {
    const allCases = state.testCases;
    const items: ChallengeRunItem[] = allCases.map((testCase, index) => ({
      index,
      testCase,
      result: evaluateCase(testCase),
    }));
    const firstFailure = items.find((item) => !item.result.ok) || null;
    return {
      pass: !firstFailure,
      items,
      firstFailure,
    };
  }

  function expectedBoxesForCase(testCase: ChallengeCase): BoxState[] {
    return outputSpecs.map((outputSpec, index) => ({
      name: outputSpec.name,
      type: outputSpec.type,
      value: testCase.expectedLiterals[index] || "",
    }));
  }

  function expectedStateBoxesForCase(testCase: ChallengeCase): BoxState[] {
    return expectedBoxesForCase(testCase).map((box) => ({
      ...box,
      address: "<i>(any)</i>",
    }));
  }

  function progressSnapshot(): CodeOutputChallengeProgress {
    return {
      text: getUserText(),
      pass: state.pass,
      visibleCaseInputLiterals: state.visibleCase.inputLiterals.slice(),
      pendingFailingCaseInputLiterals: state.pendingFailingCase
        ? state.pendingFailingCase.inputLiterals.slice()
        : null,
      showFullShownOutput: state.showFullShownOutput,
      hasRunChecks: !!state.lastReport,
    };
  }

  function isDefaultProgress(snapshot: CodeOutputChallengeProgress): boolean {
    return (
      snapshot.text === defaultText &&
      !snapshot.pass &&
      caseInputKey({ inputLiterals: snapshot.visibleCaseInputLiterals }) ===
        caseInputKey(defaultVisibleCase) &&
      snapshot.pendingFailingCaseInputLiterals == null &&
      !snapshot.showFullShownOutput &&
      !snapshot.hasRunChecks
    );
  }

  function persistProgress() {
    progress.save(progressSnapshot());
  }

  function renderStage(currentResult: ChallengeCaseResult): void {
    if (!stage) return;
    clearNode(stage);
    const group = document.createElement("div");
    group.className = "state-group two-col";
    const shownKind: "ok" | "compile" | "ub" =
      currentResult.kind === "compile" || currentResult.kind === "ub"
        ? currentResult.kind
        : "ok";
    const fullShownState = currentResult.state || [];
    const filteredShownState = fullShownState.filter(
      (box: BoxState) => targetOutputNameSet.has(String(box.name || "")),
    );
    const hasExtraShownVars =
      shownKind === "ok" &&
      fullShownState.some(
        (box: BoxState) => !targetOutputNameSet.has(String(box.name || "")),
      );
    const shownBoxes =
      shownKind !== "ok" || state.showFullShownOutput
        ? currentResult.state
        : filteredShownState;
    const shownEmptyMessage =
      currentResult.kind === "compile"
        ? "(fix the error above to run the program)"
        : currentResult.kind === "step-limit"
        ? "(no output was produced before the program was stopped)"
        : currentResult.kind === "blocked-input"
          ? "(no output was produced before the program began waiting)"
          : shownKind === "ok" && !state.showFullShownOutput
        ? outputSpecs.length === 1
          ? `(missing variable ${outputSpecs[0]!.name})`
          : "(missing one or more output variables)"
        : "(no variables)";
    const shownTitle =
      currentResult.kind === "ub" && currentResult.state
        ? "Last recorded state before undefined behavior"
        : currentResult.kind === "step-limit"
          ? "State before the repeating section"
          : currentResult.kind === "blocked-input"
            ? "Program state when it began waiting"
            : currentResult.kind === "compile"
              ? "Program did not run"
          : "Your code's output";
    const shownControls = (() => {
      if (!hasExtraShownVars) return null;
      const toggle = document.createElement("button");
      toggle.type = "button";
      toggle.className = "state-panel-toggle";
      toggle.textContent = state.showFullShownOutput
        ? outputSpecs.length === 1
          ? "Show only output variable"
          : "Show only output variables"
        : "Show full state";
      toggle.addEventListener("click", () => {
        state.showFullShownOutput = !state.showFullShownOutput;
        renderStage(currentResult);
        persistProgress();
      });
      return toggle;
    })();
    group.appendChild(
      renderStatePanel(shownTitle, shownBoxes, {
        emptyMessage: shownEmptyMessage,
        controls: shownControls,
      }),
    );
    group.appendChild(
      renderStatePanel("Expected output", expectedStateBoxesForCase(state.visibleCase)),
    );
    stage.appendChild(group);
  }

  function buttonReplacements() {
    const backLabel = (prevBtn?.textContent || "Back ◀").trim();
    return [
      ["$checkButton", "$b{Check}"],
      ["$newInputButton", "$b{New input}"],
      ["$showFailingCaseButton", "$b{Show failing case}"],
      ["$runLineButton", "$b{Run line}"],
      ["$backButton", `$b{${backLabel}}`],
    ] as const;
  }

  const applyButtonTokens = createButtonTokenReplacer(buttonReplacements);

  function setStatus(text: string, cls: string = "muted") {
    if (!status) return;
    status.textContent = text;
    status.className = cls;
  }

  function updateLockedInputLine() {
    const lines = lockedInputLinesForCase(state.visibleCase);
    if (lockedInputLine) {
      lockedInputLine.textContent = lines.join("\n");
    }
    if (lockedLineNumbers) {
      const frag = document.createDocumentFragment();
      for (let i = 0; i < lines.length; i += 1) {
        const num = document.createElement("div");
        num.className = "code-line-number";
        num.textContent = String(i + 1);
        frag.appendChild(num);
      }
      clearNode(lockedLineNumbers);
      lockedLineNumbers.appendChild(frag);
    }
  }

  function updateInstructions() {
    if (state.pass) {
      setPartsContent(instructionsEl, "Challenge solved!");
      return;
    }
    if (instructions) {
      setPartsContent(instructionsEl, applyButtonTokens(instructions));
      return;
    }
    const inputSummary = inputSpecs
      .map((spec) => `${spec.type} ${spec.name}`)
      .join(", ");
    const outputSummary = outputSpecs
      .map((spec) => `${spec.type} ${spec.name}`)
      .join(", ");
    const lockedLineSummary =
      preludeLineCount === 1
        ? "Line 1 is the current input assignment and is locked."
        : `Lines 1-${preludeLineCount} are the current input assignments and are locked.`;
    const msg =
      `Write code that creates ${outputSummary} from ${inputSummary}. ` +
      `${lockedLineSummary} ` +
      `Press $checkButton to run all ${state.testCases.length} test input${
        state.testCases.length === 1 ? "" : "s"
      }.`;
    setPartsContent(instructionsEl, applyButtonTokens(msg));
  }

  const { hide: hideHint, show: showHint } = createHintPresenter(
    hintPanel,
    applyButtonTokens,
  );

  function defaultHint(
    current: ChallengeCaseResult,
    report?: ChallengeRunReport,
  ): string {
    if (current.kind === "compile") {
      const diagnostic = visibleProgramFeedback.diagnostic;
      return diagnostic
        ? diagnosticMessageText(diagnostic)
        : "The shown case does not compile yet. Fix syntax errors first.";
    }
    if (current.kind === "ub") {
      const diagnostic = visibleProgramFeedback.diagnostic;
      if (diagnostic) {
        return [
          diagnosticMessageText(diagnostic),
          diagnosticRuntimeContextText(diagnostic),
        ]
          .filter(Boolean)
          .join(" ");
      }
      return "The shown case has undefined behavior.";
    }
    if (current.kind === "step-limit") {
      return "The shown case did not finish. Check whether a loop condition can become false or a recursive call can reach its base case.";
    }
    if (current.kind === "blocked-input") {
      return "The shown case is waiting for input, but code-writing lessons do not provide interactive input. Remove the input operation or initialize the value directly.";
    }
    if (current.kind === "missing-output") {
      if (current.failingOutput) {
        return `Create a variable named $n{${current.failingOutput.name}}.`;
      }
      return `Create the output variable${outputSpecs.length === 1 ? "" : "s"}: $n{${outputNamesText}}.`;
    }
    if (current.kind === "wrong-output-type") {
      if (current.failingOutput) {
        return `$n{${current.failingOutput.name}} should have type $t{${current.failingOutput.type}}.`;
      }
      return `One output variable has the wrong type.`;
    }
    if (current.kind === "wrong-output-value") {
      if (current.failingOutput && current.expected) {
        return `For the shown input, $n{${current.failingOutput.name}} should be $v{${current.expected.value}}.`;
      }
      return "For the shown input, one output variable has the wrong value.";
    }
    if (report && !report.pass) {
      return "The shown input works, but at least one other test input fails. Make sure your code computes the value from the input instead of hardcoding.";
    }
    return "Looks good. Press $checkButton.";
  }

  function render() {
    const analyzed = runCProgram(fullProgramTextForCase(state.visibleCase));
    const currentResult = evaluateCase(state.visibleCase, analyzed);
    visibleCaseResult = currentResult;
    visibleProgramFeedback = getVisibleProgramFeedback(analyzed);
    renderStage(currentResult);
    updateLockedInputLine();
    updateInstructions();
    updateLineGutters(
      visibleProgramFeedback.diagnostic,
      visibleProgramFeedback.runtimeIssue,
    );
    if (state.pass) {
      setStatus("correct", "ok");
    } else if (state.lastReport && !state.lastReport.pass) {
      setStatus("incorrect", "err");
    } else {
      setStatus("", "muted");
    }
    const editable = !state.pass;
    if (checkBtn) checkBtn.classList.toggle("hidden", !editable);
    if (hintBtn) hintBtn.classList.toggle("hidden", !editable);
    const visibleCaseKey = caseInputKey(state.visibleCase);
    const hasAlternateInput = state.testCases.some(
      (testCase) => caseInputKey(testCase) !== visibleCaseKey,
    );
    if (rerollBtn) {
      rerollBtn.classList.toggle("hidden", !editable || !hasAlternateInput);
    }
    if (showFailBtn) {
      showFailBtn.classList.toggle(
        "hidden",
        !editable || !state.pendingFailingCase || !!(currentResult && !currentResult.ok),
      );
    }
    if (editor) editor.readOnly = !editable;
    if (!editable) editor?.classList.add("readonly");
    nextBtn?.classList.remove("hidden");
    pager?.update();
    if (levelResetBtn) {
      levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
    }
    persistProgress();
  }

  function buildHintContext(
    currentResult: ChallengeCaseResult,
    report: ChallengeRunReport,
  ): CodeOutputChallengeHintContext {
    return {
      text: getEditorText(),
      inputs: inputSpecs.map((inputSpec, index) => ({
        ...inputSpec,
        value: state.visibleCase.inputValues[index]!,
      })),
      outputs: outputSpecs.map((outputSpec) => ({ ...outputSpec })),
      currentCase: state.visibleCase,
      currentResult,
      report,
      behavesLike: behavesLikeProgramOnTestInputs,
    };
  }

  if (editor) {
    bindCodeEditorTabKey(editor);
    if (!allowNewLines) {
      editor.addEventListener("keydown", (event) => {
        if (event.key === "Enter") event.preventDefault();
      });
    }

    editor.addEventListener("input", () => {
      const raw = editor.value;
      const normalized = normalizeUserCodeText(raw);
      if (normalized !== raw) {
        const start = adjustSelectionForCarriageReturns(raw, editor.selectionStart);
        const end = adjustSelectionForCarriageReturns(raw, editor.selectionEnd);
        editor.value = normalized;
        const clampedStart = Math.min(normalized.length, start);
        const clampedEnd = Math.min(normalized.length, end);
        editor.setSelectionRange(clampedStart, clampedEnd);
      }
      state.text = editor.value;
      state.lastReport = null;
      state.pendingFailingCase = null;
      render();
    });
    editor.addEventListener("scroll", syncEditorLinkedScroll);
    window.addEventListener("resize", () => {
      updateLineGutters(
        visibleProgramFeedback.diagnostic,
        visibleProgramFeedback.runtimeIssue,
      );
    });
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => {
        updateLineGutters(
          visibleProgramFeedback.diagnostic,
          visibleProgramFeedback.runtimeIssue,
        );
      });
      ro.observe(editor);
    }
  }

  if (levelResetBtn) {
    levelResetBtn.addEventListener("click", () => {
      resetLevelAfterConfirmation(progress);
    });
  }

  if (rerollBtn) {
    rerollBtn.addEventListener("click", () => {
      if (state.pass) return;
      hideHint();
      state.visibleCase = pickDifferentTestCase(state.visibleCase);
      state.lastReport = null;
      state.pendingFailingCase = null;
      setStatus("", "muted");
      render();
    });
  }

  if (hintBtn) {
    hintBtn.addEventListener("click", () => {
      hideHint();
      const currentResult =
        visibleCaseResult ?? evaluateCase(state.visibleCase);
      if (
        currentResult.kind === "compile" ||
        currentResult.kind === "ub" ||
        currentResult.kind === "step-limit" ||
        currentResult.kind === "blocked-input"
      ) {
        state.pendingFailingCase = null;
        showHint(defaultHint(currentResult));
        return;
      }
      const report = runAllCases();
      if (currentResult.ok && !report.pass) {
        const failingCase = report.firstFailure?.testCase || null;
        state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
      } else {
        state.pendingFailingCase = null;
      }
      render();
      let parts: ChallengeParts | null | undefined = null;
      if (typeof hints === "function") {
        parts = hints(buildHintContext(currentResult, report));
      } else {
        parts = hints as ChallengeParts;
      }
      if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        parts = defaultHint(currentResult, report);
      }
      showHint(parts);
    });
  }

  if (showFailBtn) {
    showFailBtn.addEventListener("click", () => {
      if (state.pass || !state.pendingFailingCase) return;
      hideHint();
      state.visibleCase = copyCase(state.pendingFailingCase);
      state.pendingFailingCase = null;
      render();
    });
  }

  if (checkBtn) {
    checkBtn.addEventListener("click", () => {
      hideHint();
      const report = runAllCases();
      state.lastReport = report;
      if (!report.pass) {
        const failingCase = report.firstFailure?.testCase || null;
        state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
        render();
        const failureKind = report.firstFailure?.result.kind;
        const failureStatus =
          failureKind === "compile"
            ? "fix the error shown above"
            : failureKind === "ub"
              ? "fix the undefined behavior shown above"
              : failureKind === "step-limit"
                ? "program did not finish"
                : failureKind === "blocked-input"
                  ? "program is waiting for input"
                  : "incorrect";
        setStatus(failureStatus, "err");
        flashStatus(status);
        return;
      }
      state.pass = true;
      state.pendingFailingCase = null;
      if (editor) editor.readOnly = true;
      checkBtn?.classList.add("hidden");
      hintBtn?.classList.add("hidden");
      rerollBtn?.classList.add("hidden");
      showFailBtn?.classList.add("hidden");
      pager?.pulseNext();
      pager?.update();
      render();
      setStatus("correct", "ok");
      flashStatus(status);
    });
  }

  if (restoredHadRunChecks && !state.pass) {
    state.lastReport = runAllCases();
    if (state.lastReport.pass) {
      state.pendingFailingCase = null;
    } else if (!state.pendingFailingCase) {
      const failingCase = state.lastReport.firstFailure?.testCase || null;
      state.pendingFailingCase = failingCase ? copyCase(failingCase) : null;
    }
  }

  pager = createStepper({
    root: codeRoot || editor?.closest(".panel") || document.body,
    lines: 0,
    nextPage: next || null,
    endLabel,
    getBoundary: () => 0,
    setBoundary: () => {},
    onAfterChange: render,
    isStepLocked: () => !state.pass,
  });

  pager.update();
  render();
}

export { createCodeOutputChallengeTemplate };

import {
  applyTextTokenReplacements,
  appendStateObjects,
  bindBtnRefPulse,
  boxValueMatchesSpec,
  clearNode,
  createStepper,
  ensurePanelizedMain,
  flashStatus,
  formatValueForType,
  getNavLabelForHref,
  parseType,
  queryElement,
  queryRole,
  renderParts,
  setPartsContent,
  syncDocumentTitleFromNav,
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
  type CodeDecoration,
} from "./shared-code-editor-surface.js";
import { parseCValueLiteral, runCProgram } from "./shared-c-interpreter.js";
import {
  clearLevelProgress,
  currentLevelId,
  maybeRestoreLevelProgress,
  writeLevelProgress,
} from "./shared-progress.js";

type ChallengeParts = Parts;
type ChallengeScalar = string | number | bigint;
type ChallengeRuntimeValue = number | bigint;
type ChallengePartsSpec =
  | ChallengeParts
  | ((ctx: CodeOutputChallengeHintContext) => ChallengeParts | null | undefined)
  | null;

type ChallengeFailureKind =
  | "compile"
  | "ub"
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
  isLast?: boolean;
}

interface CodeOutputChallengeElements {
  instructionsEl: HTMLElement | null;
  lockedLineNumbers: HTMLElement | null;
  lockedInputLine: HTMLElement | null;
  editor: HTMLTextAreaElement | null;
  lineNumbers: HTMLElement | null;
  stage: HTMLElement | null;
  status: HTMLElement | null;
  diagnosticEl: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
  levelResetBtn: HTMLButtonElement | null;
  rerollBtn: HTMLButtonElement | null;
  showFailBtn: HTMLButtonElement | null;
  nextBtn: HTMLButtonElement | null;
  codeRoot: HTMLElement | null;
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

interface CodeOutputChallengeState {
  text: string;
  pass: boolean;
  allocBase: number | null;
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
  allocBase: number | null;
  visibleCaseInputLiterals: string[];
  pendingFailingCaseInputLiterals: string[] | null;
  showFullShownOutput: boolean;
  hasRunChecks: boolean;
}

const INT32_MIN = -2147483648n;
const INT32_MAX = 2147483647n;

function collectCodeOutputChallengeElements(
  root: ParentNode = document,
): CodeOutputChallengeElements {
  const role = <T extends Element>(name: string) => queryRole<T>(name, root);
  return {
    instructionsEl: role<HTMLElement>("code-instructions"),
    lockedLineNumbers: role<HTMLElement>("code-locked-line-numbers"),
    lockedInputLine: role<HTMLElement>("code-locked-input-line"),
    editor: role<HTMLTextAreaElement>("code-editor"),
    lineNumbers: role<HTMLElement>("code-line-numbers"),
    stage: role<HTMLElement>("code-stage"),
    status: role<HTMLElement>("code-status"),
    diagnosticEl: role<HTMLElement>("code-diagnostic"),
    hintPanel: role<HTMLElement>("code-hint"),
    hintBtn: role<HTMLButtonElement>("code-hint-btn"),
    checkBtn: role<HTMLButtonElement>("code-check"),
    levelResetBtn: role<HTMLButtonElement>("code-reset-level"),
    rerollBtn: role<HTMLButtonElement>("code-reroll"),
    showFailBtn: role<HTMLButtonElement>("code-show-failing-case"),
    nextBtn: queryElement<HTMLButtonElement>('button[data-stepper="next"]', root),
    codeRoot: role<HTMLElement>("code-root"),
  };
}

function ensureCodeOutputChallengeLayout({
  textareaMinLines,
}: {
  textareaMinLines: number;
}): CodeOutputChallengeElements {
  const resolvedTitle = syncDocumentTitleFromNav();
  const existing = queryRole<HTMLElement>("code-editor");
  if (existing) return collectCodeOutputChallengeElements();

  const main = ensurePanelizedMain(resolvedTitle);

  const instructionsEl = document.createElement("p");
  instructionsEl.dataset.role = "code-instructions";
  instructionsEl.className = "intro";

  const section = document.createElement("section");
  section.dataset.role = "code-root";
  section.classList.add("panel-shell");
  const actionBar = document.createElement("div");
  actionBar.className = "controls-bar controls-bar-code";
  const controlsMain = document.createElement("div");
  controlsMain.className = "controls-main panel panel-controls";
  const controlsRow = document.createElement("div");
  controlsRow.className = "controls-row controls-left";
  controlsMain.appendChild(controlsRow);
  actionBar.appendChild(controlsMain);
  section.appendChild(actionBar);
  const row = document.createElement("div");
  row.className = "row panel-row";
  section.appendChild(row);
  main.appendChild(section);

  const codePanel = document.createElement("div");
  codePanel.className = "panel code-editor-panel panel-scroll code-panel-shell";
  codePanel.dataset.role = "code-panel";
  const codeTitle = document.createElement("div");
  codeTitle.className = "panel-title code-title";
  codeTitle.textContent = "Code";
  const codePane = document.createElement("div");
  codePane.className = "codepane panel-body";
  const lockedRow = document.createElement("div");
  lockedRow.className = "codepane-row code-locked-row";
  const lockedLineNumbers = document.createElement("div");
  lockedLineNumbers.dataset.role = "code-locked-line-numbers";
  lockedLineNumbers.className = "code-gutter";
  lockedLineNumbers.setAttribute("aria-hidden", "true");
  const lockedInputLine = document.createElement("div");
  lockedInputLine.dataset.role = "code-locked-input-line";
  lockedInputLine.className = "code-locked-line";
  lockedRow.appendChild(lockedLineNumbers);
  lockedRow.appendChild(lockedInputLine);
  const codeRow = document.createElement("div");
  codeRow.className = "codepane-row";
  const lineNumbers = document.createElement("div");
  lineNumbers.dataset.role = "code-line-numbers";
  lineNumbers.className = "code-gutter";
  lineNumbers.setAttribute("aria-hidden", "true");
  const editorWrap = document.createElement("div");
  editorWrap.className = "code-editor-wrap";
  const editor = document.createElement("textarea");
  editor.dataset.role = "code-editor";
  editor.className = "code-textarea";
  editor.spellcheck = false;
  const rows = Math.max(1, Number(textareaMinLines));
  editor.setAttribute("rows", String(rows));
  editorWrap.appendChild(editor);
  codeRow.appendChild(lineNumbers);
  codeRow.appendChild(editorWrap);
  codePane.appendChild(lockedRow);
  codePane.appendChild(codeRow);
  const nextBtn = document.createElement("button");
  nextBtn.textContent = "Next Program ▶▶";
  nextBtn.dataset.stepper = "next";
  const controlsSpacer = document.createElement("span");
  controlsSpacer.className = "controls-spacer";
  controlsSpacer.setAttribute("aria-hidden", "true");
  controlsRow.appendChild(nextBtn);
  controlsRow.appendChild(controlsSpacer);
  const diagnosticEl = document.createElement("div");
  diagnosticEl.dataset.role = "code-diagnostic";
  diagnosticEl.className = "code-diagnostic hidden";
  codePanel.appendChild(codeTitle);
  codePanel.appendChild(codePane);
  codePanel.appendChild(diagnosticEl);

  const stateCol = document.createElement("div");
  stateCol.className = "code-editor-state-col";
  const stage = document.createElement("div");
  stage.dataset.role = "code-stage";
  stage.className = "code-editor-state-stage";
  const rerollBtn = document.createElement("button");
  rerollBtn.dataset.role = "code-reroll";
  rerollBtn.textContent = "New input";
  const checkBtn = document.createElement("button");
  checkBtn.dataset.role = "code-check";
  checkBtn.textContent = "Check";
  const levelResetBtn = document.createElement("button");
  levelResetBtn.dataset.role = "code-reset-level";
  levelResetBtn.textContent = "Reset level";
  const showFailBtn = document.createElement("button");
  showFailBtn.dataset.role = "code-show-failing-case";
  showFailBtn.textContent = "Show failing case";
  showFailBtn.classList.add("hidden");
  const hintBtn = document.createElement("button");
  hintBtn.dataset.role = "code-hint-btn";
  hintBtn.type = "button";
  hintBtn.className = "hint-link";
  hintBtn.textContent = "Hint";
  const status = document.createElement("span");
  status.dataset.role = "code-status";
  status.className = "muted";
  controlsRow.appendChild(rerollBtn);
  controlsRow.appendChild(levelResetBtn);
  controlsRow.appendChild(hintBtn);
  controlsRow.appendChild(checkBtn);
  controlsRow.appendChild(showFailBtn);
  controlsRow.appendChild(status);
  const hintPanel = document.createElement("div");
  hintPanel.dataset.role = "code-hint";
  hintPanel.className = "hint-inline hidden";
  actionBar.appendChild(hintPanel);
  actionBar.appendChild(instructionsEl);
  stateCol.appendChild(stage);

  row.appendChild(codePanel);
  row.appendChild(stateCol);

  return {
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
    nextBtn,
    codeRoot: section,
  };
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
    isLast = false,
  } = config;
  const endLabel = (() => {
    if (isLast) return "Finish";
    const label = getNavLabelForHref(next);
    return label ? `Next: ${label}` : "Next Program";
  })();

  const failConfig = (message: string): never => {
    alert(message);
    throw new Error(message);
  };

  const ensureIdentifier = (name: string, label: string): string => {
    const trimmed = String(name || "").trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed)) {
      failConfig(`${label} must be a valid C identifier.`);
    }
    return trimmed;
  };

  if (!Array.isArray(inputs) || inputs.length === 0) {
    failConfig("inputs must be a non-empty array.");
  }
  if (!Array.isArray(outputs) || outputs.length === 0) {
    failConfig("outputs must be a non-empty array.");
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
    failConfig("testInputs must be an array.");
  }
  if (testInputs.length === 0) {
    failConfig("testInputs must contain at least one value.");
  }
  if (
    testInputs.some(
      (row) =>
        !Array.isArray(row) ||
        row.length !== inputSpecs.length ||
        row.some((value) => typeof value !== "string"),
    )
  ) {
    failConfig(
      `Each testInputs entry must be a string array of length ${inputSpecs.length}.`,
    );
  }
  if (!Array.isArray(startInput)) {
    failConfig("startInput is required.");
  }
  if (
    startInput.length !== inputSpecs.length ||
    startInput.some((value) => typeof value !== "string")
  ) {
    failConfig(`startInput must be a string array of length ${inputSpecs.length}.`);
  }
  if (typeof solve !== "string") {
    failConfig("solve must be a C code string.");
  }
  if (!Number.isFinite(textareaMinLines)) {
    failConfig("textareaMinLines must be a number.");
  }
  const solveCode = String(solve || "").replace(/\r\n/g, "\n");
  if (!solveCode.trim()) {
    failConfig("solve must be a non-empty C code string.");
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
    nextBtn,
    codeRoot,
  } = ensureCodeOutputChallengeLayout({ textareaMinLines });
  const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);

  bindBtnRefPulse(codeRoot || document);

  function normalizeProgramBody(text: string): string {
    const normalized = String(text || "").replace(/\r\n/g, "\n");
    return normalized === "" || normalized.endsWith("\n")
      ? normalized
      : `${normalized}\n`;
  }

  function normalizeNumberLike(
    value: ChallengeScalar,
    type: string,
    label: string,
  ): { runtime: ChallengeRuntimeValue; literal: string } {
    const parsedType = parseType(type);
    const base = parsedType.base;
    if (!base || parsedType.depth !== 0) {
      failConfig(`${label} type must be int, long, or double.`);
    }
    if (base === "double") {
      let numeric: number;
      if (typeof value === "number") {
        numeric = value;
      } else if (typeof value === "bigint") {
        numeric = Number(value);
      } else {
        const parsed = Number(String(value).trim());
        if (Number.isNaN(parsed)) {
          failConfig(`${label} must be numeric for type ${type}.`);
        }
        numeric = parsed;
      }
      if (!Number.isFinite(numeric)) {
        failConfig(`${label} must be finite for type ${type}.`);
      }
      return {
        runtime: numeric,
        literal: formatValueForType(numeric, type),
      };
    }

    let asBigInt = 0n;
    if (typeof value === "bigint") {
      asBigInt = value;
    } else if (typeof value === "number") {
      if (!Number.isFinite(value) || !Number.isInteger(value)) {
        failConfig(`${label} must be an integer for type ${type}.`);
      }
      asBigInt = BigInt(Math.trunc(value));
    } else {
      const trimmed = String(value).trim();
      if (!/^[+-]?\d+$/.test(trimmed)) {
        failConfig(`${label} must be an integer literal for type ${type}.`);
      }
      try {
        asBigInt = BigInt(trimmed);
      } catch {
        failConfig(`${label} is out of range for type ${type}.`);
      }
    }

    if (base === "int") {
      if (asBigInt < INT32_MIN || asBigInt > INT32_MAX) {
        failConfig(`${label} must be within 32-bit int range.`);
      }
      const asNumber = Number(asBigInt);
      return {
        runtime: asNumber,
        literal: String(asNumber),
      };
    }

    return {
      runtime: asBigInt,
      literal: asBigInt.toString(),
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
    const solvedState = analyzed.kind === "ok" ? analyzed.state : null;
    if (!solvedState) {
      if (analyzed.kind === "compile") {
        failConfig(`solve does not compile for ${label}.`);
      }
      failConfig(`solve has undefined behavior for ${label}.`);
      return [];
    }
    const expectedLiterals: string[] = [];
    for (const outputSpec of outputSpecs) {
      const outputBox =
        solvedState.find((box: BoxState) => box.name === outputSpec.name) || null;
      if (!outputBox) {
        failConfig(`solve must create ${outputSpec.name} for ${label}.`);
        return [];
      }
      if (String(outputBox.type || "").trim() !== outputSpec.type) {
        failConfig(
          `solve must produce ${outputSpec.name} with type ${outputSpec.type} for ${label}.`,
        );
        return [];
      }
      const literal = String(outputBox.value ?? "").trim();
      if (!literal) {
        failConfig(`solve leaves ${outputSpec.name} without a value for ${label}.`);
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
      normalizeNumberLike(
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

  const levelId = currentLevelId();
  const defaultText = normalizeUserCodeText(startCode);
  const defaultVisibleCase = createChallengeCaseForInputRow(startInput, "startInput");
  const restoredProgress =
    maybeRestoreLevelProgress<CodeOutputChallengeProgress>(levelId);
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
    allocBase:
      typeof restoredProgress?.allocBase === "number"
        ? restoredProgress.allocBase
        : null,
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

  function getProgramDiagnostic(): ProgramDiagnostic | null {
    const result = runCProgram(fullProgramTextForCase(state.visibleCase));
    if (result.kind === "ok") return null;
    const diagnostic = result.diagnostic;
    if (diagnostic.range.startLine < preludeLineCount) return null;
    return {
      ...diagnostic,
      range: {
        startLine: diagnostic.range.startLine - preludeLineCount,
        startCol: diagnostic.range.startCol,
        endLine: diagnostic.range.endLine - preludeLineCount,
        endCol: diagnostic.range.endCol,
      },
    };
  }

  function diagnosticDecoration(
    diagnostic: ProgramDiagnostic | null,
  ): CodeDecoration[] {
    if (!diagnostic) return [];
    return [
      {
        line: diagnostic.range.startLine,
        startCol: diagnostic.range.startCol,
        endCol: diagnostic.range.endCol,
        className: "code-highlight-error",
      },
    ];
  }

  function renderDiagnostic(diagnostic: ProgramDiagnostic | null) {
    if (!diagnosticEl) return;
    if (!diagnostic) {
      diagnosticEl.classList.add("hidden");
      diagnosticEl.textContent = "";
      editor?.removeAttribute("aria-invalid");
      return;
    }
    diagnosticEl.classList.remove("hidden");
    diagnosticEl.replaceChildren();
    const heading = document.createElement("div");
    heading.className = "code-diagnostic-title";
    heading.textContent = `${
      diagnostic.kind === "ub" ? "Undefined behavior" : "Error"
    } on line ${diagnostic.range.startLine + preludeLineCount + 1}, column ${
      diagnostic.range.startCol + 1
    }`;
    const message = document.createElement("div");
    message.className = "code-diagnostic-message";
    message.textContent = diagnostic.message;
    diagnosticEl.append(heading, message);
    if (diagnostic.tip) {
      const tip = document.createElement("div");
      tip.className = "code-diagnostic-tip";
      tip.textContent = diagnostic.tip;
      diagnosticEl.appendChild(tip);
    }
    editor?.setAttribute("aria-invalid", "true");
  }

  function updateLineGutters(diagnostic: ProgramDiagnostic | null = null) {
    const lines = getUserRawLines();
    const lineNumberClasses = new Map<number, string[]>();
    if (diagnostic) {
      lineNumberClasses.set(diagnostic.range.startLine, ["has-error"]);
    }
    updateCodeSurface({
      editor,
      lineNumbers,
      highlightEl,
      measureEl,
      lines,
      lineNumberStart: preludeLineCount + 1,
      decorations: diagnosticDecoration(diagnostic),
      lineNumberClasses,
    });
    syncEditorLinkedScroll();
    renderDiagnostic(diagnostic);
  }

  type ProgramBehavior =
    | { kind: "compile" | "ub" | "missing-output" | "wrong-output-type" }
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
            parseCValueLiteral,
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

  function evaluateCase(testCase: ChallengeCase): ChallengeCaseResult {
    const text = fullProgramTextForCase(testCase);
    const analyzed = runCProgram(text);
    const expectedBoxes = expectedBoxesForCase(testCase);
    const fallbackExpected = expectedBoxes[0] || null;
    const fallbackOutput = outputSpecs[0] || null;
    if (analyzed.kind !== "ok") {
      return {
        ok: false,
        kind: analyzed.kind,
        state: null,
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
      if (!boxValueMatchesSpec(parseCValueLiteral, actual, expected).ok) {
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
      allocBase: state.allocBase,
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
    const snapshot = progressSnapshot();
    if (isDefaultProgress(snapshot)) {
      clearLevelProgress(levelId);
      return;
    }
    writeLevelProgress(snapshot, levelId);
  }

  function renderStatePanel(
    title: string,
    boxes: BoxState[] | null,
    opts: { emptyMessage?: string; controls?: HTMLElement | null } = {},
  ): HTMLElement {
    const { emptyMessage = "(no variables)", controls = null } = opts;
    const wrap = document.createElement("div");
    wrap.className = "state-panel state-panel-scrollable";
    const heading = document.createElement("div");
    heading.className = "panel-title state-heading";
    heading.textContent = title;
    wrap.appendChild(heading);
    if (controls) {
      const controlsWrap = document.createElement("div");
      controlsWrap.className = "state-panel-controls";
      controlsWrap.appendChild(controls);
      wrap.appendChild(controlsWrap);
    }
    const grid = document.createElement("div");
    grid.className = "grid";
    if (!boxes || boxes.length === 0) {
      const msg = document.createElement("div");
      msg.className = "muted";
      msg.style.padding = "8px";
      msg.textContent = emptyMessage;
      grid.appendChild(msg);
    } else {
      appendStateObjects(grid, boxes, {
        editable: false,
        deletable: false,
      });
    }
    const body = document.createElement("div");
    body.className = "state-panel-scroll-body";
    body.appendChild(grid);
    wrap.appendChild(body);
    return wrap;
  }

  function renderStage(): ChallengeCaseResult | null {
    if (!stage) return null;
    clearNode(stage);
    const currentResult = evaluateCase(state.visibleCase);
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
      shownKind === "ok" && !state.showFullShownOutput
        ? outputSpecs.length === 1
          ? `(missing variable ${outputSpecs[0]!.name})`
          : "(missing one or more output variables)"
        : "(no variables)";
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
        renderStage();
        persistProgress();
      });
      return toggle;
    })();
    group.appendChild(
      renderStatePanel("Your code's output", shownBoxes, {
        emptyMessage: shownEmptyMessage,
        controls: shownControls,
      }),
    );
    group.appendChild(
      renderStatePanel("Expected output", expectedStateBoxesForCase(state.visibleCase)),
    );
    stage.appendChild(group);
    return currentResult;
  }

  const buttonReplacements = [
    ["$checkButton", "$b{Check}"],
    ["$newInputButton", "$b{New input}"],
    ["$showFailingCaseButton", "$b{Show failing case}"],
    ["$runLineButton", "$b{Run line}"],
    ["$backButton", "$b{Back ◀}"],
  ] as const;

  function applyButtonTokens(parts: ChallengeParts | null): ChallengeParts | null {
    return applyTextTokenReplacements(parts, buttonReplacements) as
      | ChallengeParts
      | null;
  }

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

  function hideHint() {
    if (!hintPanel) return;
    hintPanel.textContent = "";
    hintPanel.classList.add("hidden");
  }

  function showHint(parts: ChallengeParts | null) {
    if (!hintPanel) return;
    if (!parts || (Array.isArray(parts) && parts.length === 0)) return;
    renderParts(hintPanel, applyButtonTokens(parts) || "");
    hintPanel.classList.remove("hidden");
    flashStatus(hintPanel);
  }

  function defaultHint(current: ChallengeCaseResult, report: ChallengeRunReport): string {
    if (current.kind === "compile") {
      return getProgramDiagnostic()?.message || "The shown case does not compile yet. Fix syntax errors first.";
    }
    if (current.kind === "ub") {
      return "The shown case has undefined behavior. Avoid invalid pointer/math operations.";
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
    if (!report.pass) {
      return "The shown input works, but at least one other test input fails. Make sure your code computes the value from the input instead of hardcoding.";
    }
    return "Looks good. Press $checkButton.";
  }

  function render() {
    const currentResult = renderStage();
    updateLockedInputLine();
    updateInstructions();
    updateLineGutters(getProgramDiagnostic());
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
    window.addEventListener("resize", () => updateLineGutters(getProgramDiagnostic()));
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => updateLineGutters(getProgramDiagnostic()));
      ro.observe(editor);
    }
  }

  if (levelResetBtn) {
    levelResetBtn.addEventListener("click", () => {
      const confirmed = window.confirm(
        "Reset your saved progress for this level and start over?",
      );
      if (!confirmed) return;
      clearLevelProgress(levelId);
      window.location.reload();
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
      const currentResult = evaluateCase(state.visibleCase);
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
        setStatus("incorrect", "err");
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

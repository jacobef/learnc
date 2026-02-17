import {
  applyTextTokenReplacements,
  appendStateObjects,
  bindBtnRefPulse,
  boxValueMatchesSpec,
  createSimpleSimulator,
  createStepper,
  ensureBaseLayout,
  flashStatus,
  formatValueForType,
  getNavLabelForHref,
  parseType,
  randAddr,
  renderParts,
  resolveActiveNavItem,
  setPartsContent,
  typeInfo,
} from "./shared-core.js";
import type { BoxState, LineStatus, Parts, SimpleSimulator, Stepper } from "./shared-core.js";

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
  lockedErrorGutter: HTMLElement | null;
  editor: HTMLTextAreaElement | null;
  lineNumbers: HTMLElement | null;
  errorGutter: HTMLElement | null;
  stage: HTMLElement | null;
  status: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
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

type OutputCheckStatus = "ok" | "missing" | "wrong-type" | "wrong-value";

interface ChallengeOutputCheck {
  output: ChallengeVariableSpec;
  expected: BoxState;
  actual: BoxState | null;
  status: OutputCheckStatus;
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
  showFullShownOutput: boolean;
  pendingFailingCase: ChallengeCase | null;
}

interface CodeOutputChallengeHintContext {
  text: string;
  inputs: Array<ChallengeVariableSpec & { value: ChallengeRuntimeValue }>;
  outputs: ChallengeVariableSpec[];
  outputChecks: ChallengeOutputCheck[];
  currentCase: ChallengeCase;
  currentResult: ChallengeCaseResult;
  report: ChallengeRunReport;
  tokenizeProgram: SimpleSimulator["tokenizeProgram"];
  parseStatements: SimpleSimulator["parseStatements"];
  findMissingSemicolonLines: SimpleSimulator["findMissingSemicolonLines"];
  behavesLike: (program: string) => boolean;
}

const INT32_MIN = -2147483648n;
const INT32_MAX = 2147483647n;

function collectCodeOutputChallengeElements(
  root: ParentNode = document,
): CodeOutputChallengeElements {
  return {
    instructionsEl: root.querySelector(
      '[data-role="code-instructions"]',
    ) as HTMLElement | null,
    lockedLineNumbers: root.querySelector(
      '[data-role="code-locked-line-numbers"]',
    ) as HTMLElement | null,
    lockedInputLine: root.querySelector(
      '[data-role="code-locked-input-line"]',
    ) as HTMLElement | null,
    lockedErrorGutter: root.querySelector(
      '[data-role="code-locked-error-gutter"]',
    ) as HTMLElement | null,
    editor: root.querySelector(
      '[data-role="code-editor"]',
    ) as HTMLTextAreaElement | null,
    lineNumbers: root.querySelector(
      '[data-role="code-line-numbers"]',
    ) as HTMLElement | null,
    errorGutter: root.querySelector(
      '[data-role="code-error-gutter"]',
    ) as HTMLElement | null,
    stage: root.querySelector('[data-role="code-stage"]') as HTMLElement | null,
    status: root.querySelector(
      '[data-role="code-status"]',
    ) as HTMLElement | null,
    hintPanel: root.querySelector(
      '[data-role="code-hint"]',
    ) as HTMLElement | null,
    hintBtn: root.querySelector(
      '[data-role="code-hint-btn"]',
    ) as HTMLButtonElement | null,
    checkBtn: root.querySelector(
      '[data-role="code-check"]',
    ) as HTMLButtonElement | null,
    rerollBtn: root.querySelector(
      '[data-role="code-reroll"]',
    ) as HTMLButtonElement | null,
    showFailBtn: root.querySelector(
      '[data-role="code-show-failing-case"]',
    ) as HTMLButtonElement | null,
    nextBtn: root.querySelector(
      'button[data-stepper="next"]',
    ) as HTMLButtonElement | null,
    codeRoot: root.querySelector(
      '[data-role="code-root"]',
    ) as HTMLElement | null,
  };
}

function ensureCodeOutputChallengeLayout({
  textareaMinLines,
}: {
  textareaMinLines: number;
}): CodeOutputChallengeElements {
  const activeItem = resolveActiveNavItem();
  const resolvedTitle = activeItem?.label || "";
  const nextBrowserTitle = resolvedTitle ? `C Boxes - ${resolvedTitle}` : "";
  if (nextBrowserTitle) document.title = nextBrowserTitle;
  const existing = document.querySelector('[data-role="code-editor"]');
  if (existing) return collectCodeOutputChallengeElements();

  const { main } = ensureBaseLayout();
  main.classList.add("main-panelized");
  if (resolvedTitle) {
    const heading = document.createElement("h1");
    heading.className = "page-title";
    heading.textContent = resolvedTitle;
    main.appendChild(heading);
  }

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
  const lockedErrorGutter = document.createElement("div");
  lockedErrorGutter.dataset.role = "code-locked-error-gutter";
  lockedErrorGutter.className = "code-error-gutter code-error-gutter-locked";
  lockedErrorGutter.setAttribute("aria-hidden", "true");
  lockedRow.appendChild(lockedLineNumbers);
  lockedRow.appendChild(lockedInputLine);
  lockedRow.appendChild(lockedErrorGutter);
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
  const errorGutter = document.createElement("div");
  errorGutter.dataset.role = "code-error-gutter";
  errorGutter.className = "code-error-gutter";
  errorGutter.setAttribute("aria-hidden", "true");
  codeRow.appendChild(lineNumbers);
  codeRow.appendChild(editorWrap);
  codeRow.appendChild(errorGutter);
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
  codePanel.appendChild(codeTitle);
  codePanel.appendChild(codePane);

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
    lockedErrorGutter,
    editor,
    lineNumbers,
    errorGutter,
    stage,
    status,
    hintPanel,
    hintBtn,
    checkBtn,
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
  const inputNameSet = new Set<string>();
  for (const spec of inputSpecs) {
    if (inputNameSet.has(spec.name)) {
      failConfig(`Duplicate input name: ${spec.name}.`);
    }
    inputNameSet.add(spec.name);
  }
  const outputNameSet = new Set<string>();
  for (const spec of outputSpecs) {
    if (outputNameSet.has(spec.name)) {
      failConfig(`Duplicate output name: ${spec.name}.`);
    }
    if (inputNameSet.has(spec.name)) {
      failConfig(`Input/output names must be different: ${spec.name}.`);
    }
    outputNameSet.add(spec.name);
  }
  for (const [index, spec] of inputSpecs.entries()) {
    const parsed = parseType(spec.type);
    if (
      !parsed.base ||
      parsed.depth !== 0 ||
      (parsed.base !== "int" && parsed.base !== "long" && parsed.base !== "double")
    ) {
      failConfig(`Input type ${index + 1} must be int, long, or double.`);
    }
  }
  for (const [index, spec] of outputSpecs.entries()) {
    const parsed = parseType(spec.type);
    if (
      !parsed.base ||
      parsed.depth !== 0 ||
      (parsed.base !== "int" && parsed.base !== "long" && parsed.base !== "double")
    ) {
      failConfig(`Output type ${index + 1} must be int, long, or double.`);
    }
  }
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
    lockedErrorGutter,
    editor,
    lineNumbers,
    errorGutter,
    stage,
    status,
    hintPanel,
    hintBtn,
    checkBtn,
    rerollBtn,
    showFailBtn,
    nextBtn,
    codeRoot,
  } = ensureCodeOutputChallengeLayout({ textareaMinLines });

  bindBtnRefPulse(codeRoot || document);

  const measureEl = (() => {
    if (!editor || !editor.parentElement) return null;
    const el = document.createElement("div");
    el.className = "code-textarea-measure";
    el.setAttribute("aria-hidden", "true");
    editor.parentElement.appendChild(el);
    return el;
  })();

  const simulator = createSimpleSimulator();

  function makeAllocFactory(start: number): (type?: string) => string {
    let next = Math.max(0, Math.floor(Number(start)));
    return (type = "int") => {
      const info = typeInfo(type || "int");
      const size = info.size || 4;
      const align = info.align || 1;
      if (next % align !== 0) {
        next = Math.ceil(next / align) * align;
      }
      const addr = next;
      next = addr + size;
      return String(addr);
    };
  }

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
    const tokens = simulator.tokenizeProgram(text);
    const parts = simulator.splitStatements(tokens);
    const analyzed = simulator.analyzeProgramParts(parts, {
      alloc: makeAllocFactory(4096),
    });
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

  const state: CodeOutputChallengeState = {
    text: "",
    pass: false,
    allocBase: null,
    visibleCase: createChallengeCaseForInputRow(startInput, "startInput"),
    testCases,
    lastReport: null,
    showFullShownOutput: false,
    pendingFailingCase: null,
  };

  let pager: Stepper | null = null;

  function normalizeUserCodeText(text: string): string {
    const normalized = text.replace(/\r\n/g, "\n");
    if (allowNewLines) return normalized;
    return normalized.replace(/\n/g, " ");
  }

  function adjustSelectionForCarriageReturns(
    text: string,
    pos: number | null,
  ): number | null {
    if (!Number.isFinite(pos)) return pos;
    const safePos = pos as number;
    let removed = 0;
    for (let i = 0; i < safePos && i < text.length; i++) {
      if (text[i] === "\r") removed += 1;
    }
    return Math.max(0, safePos - removed);
  }

  if (String(startCode || "") !== "") {
    state.text = normalizeUserCodeText(startCode);
  }

  function allocFactory(): (type?: string) => string {
    if (state.allocBase == null) state.allocBase = randAddr("int");
    return makeAllocFactory(state.allocBase);
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

  function classifyUserLineStatuses(): LineStatus {
    const lines = getUserRawLines();
    const fullText = fullProgramTextForCase(state.visibleCase);
    const fullLines = fullText.split(/\r?\n/);
    const fullStatus = simulator.classifyLineStatuses(fullLines, {
      alloc: allocFactory(),
    });
    const mapIndex = (line: number) => line - preludeLineCount;
    const isVisibleLine = (line: number) =>
      line >= preludeLineCount && mapIndex(line) < Math.max(lines.length, 1);
    const invalid = new Set<number>();
    const incomplete = new Set<number>();
    const errors = new Map<number, string | { text: string; html: string }>();
    const errorKinds = new Map<number, string>();
    const info = new Map<number, string | { text: string; html: string }>();
    fullStatus.invalid.forEach((line) => {
      if (!isVisibleLine(line)) return;
      invalid.add(mapIndex(line));
    });
    fullStatus.incomplete.forEach((line) => {
      if (!isVisibleLine(line)) return;
      incomplete.add(mapIndex(line));
    });
    fullStatus.errors.forEach((value, line) => {
      if (!isVisibleLine(line)) return;
      errors.set(mapIndex(line), value);
    });
    fullStatus.errorKinds.forEach((value, line) => {
      if (!isVisibleLine(line)) return;
      errorKinds.set(mapIndex(line), value);
    });
    fullStatus.info.forEach((value, line) => {
      if (!isVisibleLine(line)) return;
      info.set(mapIndex(line), value);
    });
    return { invalid, incomplete, errors, errorKinds, info };
  }

  function getLineHeightPx(): number {
    if (!editor) return 32;
    const style = window.getComputedStyle(editor);
    const lh = parseFloat(style.lineHeight);
    return Number.isFinite(lh) ? lh : 32;
  }

  function autoSizeEditor() {
    if (!editor) return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
  }

  function measureWrapCounts(lines: string[]): number[] {
    if (!editor || !measureEl) return lines.map(() => 1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(
      1,
      editor.clientWidth - paddingLeft - paddingRight,
    );
    measureEl.style.width = `${contentWidth}px`;
    measureEl.style.fontFamily = style.fontFamily;
    measureEl.style.fontSize = style.fontSize;
    measureEl.style.fontWeight = style.fontWeight;
    measureEl.style.letterSpacing = style.letterSpacing;
    measureEl.style.lineHeight = style.lineHeight;
    const lineHeight = getLineHeightPx();
    return lines.map((line) => {
      measureEl.textContent = line === "" ? " " : line;
      const h = measureEl.scrollHeight;
      return Math.max(1, Math.ceil(h / lineHeight - 0.01));
    });
  }

  function syncEditorLinkedScroll() {
    if (!editor) return;
    if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
    if (errorGutter) errorGutter.scrollTop = editor.scrollTop;
  }

  function updateLineGutters() {
    autoSizeEditor();
    const lines = getUserRawLines();
    const count = Math.max(lines.length, 1);
    const lineHeight = getLineHeightPx();
    const wraps = measureWrapCounts(lines);
    if (lineNumbers) {
      const frag = document.createDocumentFragment();
      for (let i = 1; i <= count; i++) {
        const num = document.createElement("div");
        num.className = "code-line-number";
        num.style.height = `${(wraps[i - 1] || 1) * lineHeight}px`;
        num.textContent = String(i + preludeLineCount);
        frag.appendChild(num);
      }
      lineNumbers.innerHTML = "";
      lineNumbers.appendChild(frag);
      if (editor) lineNumbers.style.height = `${editor.clientHeight}px`;
    }
    if (errorGutter) {
      const status = classifyUserLineStatuses();
      const frag = document.createDocumentFragment();
      for (let i = 0; i < count; i++) {
        const cell = document.createElement("div");
        cell.className = "code-error-line";
        cell.style.height = `${(wraps[i] || 1) * lineHeight}px`;
        if (status.invalid.has(i)) {
          cell.classList.add("is-invalid");
          const kind = status.errorKinds?.get(i) || "compile";
          cell.textContent = kind === "ub" ? "💣" : "🚫";
          cell.title =
            kind === "ub"
              ? "Line causes undefined behavior"
              : "Line does not compile";
        } else if (status.incomplete.has(i)) {
          cell.classList.add("is-incomplete");
          cell.textContent = "...";
          cell.title = "Line is incomplete";
        }
        frag.appendChild(cell);
      }
      errorGutter.innerHTML = "";
      errorGutter.appendChild(frag);
      if (editor) errorGutter.style.height = `${editor.clientHeight}px`;
    }
    syncEditorLinkedScroll();
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
    const tokens = simulator.tokenizeProgram(text);
    const parts = simulator.splitStatements(tokens);
    const analyzed = simulator.analyzeProgramParts(parts, {
      alloc: makeAllocFactory(4096),
    });
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
            simulator,
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
    const tokens = simulator.tokenizeProgram(text);
    const parts = simulator.splitStatements(tokens);
    const analyzed = simulator.analyzeProgramParts(parts, {
      alloc: allocFactory(),
    });
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
    const outputChecks = outputChecksForCase(testCase, finalState);
    const firstFailing = outputChecks.find((check) => check.status !== "ok") || null;
    if (firstFailing) {
      const kindByStatus: Record<
        Exclude<OutputCheckStatus, "ok">,
        "missing-output" | "wrong-output-type" | "wrong-output-value"
      > = {
        missing: "missing-output",
        "wrong-type": "wrong-output-type",
        "wrong-value": "wrong-output-value",
      };
      return {
        ok: false,
        kind: kindByStatus[firstFailing.status as Exclude<OutputCheckStatus, "ok">],
        state: finalState,
        outputBox: firstFailing.actual,
        expected: firstFailing.expected,
        failingOutput: firstFailing.output,
      }
    }
    return {
      ok: true,
      kind: "ok",
      state: finalState,
      outputBox:
        outputChecks.find((check) => check.actual)?.actual || outputChecks[0]?.actual || null,
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

  function outputChecksForCase(
    testCase: ChallengeCase,
    finalState: BoxState[] | null,
  ): ChallengeOutputCheck[] {
    const expectedBoxes = expectedBoxesForCase(testCase);
    return outputSpecs.map((outputSpec, index) => {
      const expected = expectedBoxes[index]!;
      const actual =
        finalState?.find((box: BoxState) => box.name === outputSpec.name) || null;
      let status: OutputCheckStatus = "ok";
      if (!actual) {
        status = "missing";
      } else if (String(actual.type || "").trim() !== outputSpec.type) {
        status = "wrong-type";
      } else if (!boxValueMatchesSpec(simulator, actual, expected).ok) {
        status = "wrong-value";
      }
      return {
        output: { ...outputSpec },
        expected,
        actual,
        status,
      };
    });
  }

  function expectedStateBoxesForCase(testCase: ChallengeCase): BoxState[] {
    return expectedBoxesForCase(testCase).map((box) => ({
      ...box,
      address: "<i>(any)</i>",
    }));
  }

  function renderStatePanel(
    title: string,
    boxes: BoxState[] | null,
    kind: "ok" | "compile" | "ub" = "ok",
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
    if (kind === "compile") {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent = "(this code is not valid)";
      grid.appendChild(msg);
    } else if (kind === "ub") {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent =
        "(undefined behavior occurred; fix the code before checking output)";
      grid.appendChild(msg);
    } else if (!boxes || boxes.length === 0) {
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
    stage.innerHTML = "";
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
      });
      return toggle;
    })();
    group.appendChild(
      renderStatePanel("Your code's output", shownBoxes, shownKind, {
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
      lockedLineNumbers.innerHTML = "";
      lockedLineNumbers.appendChild(frag);
    }
    if (lockedErrorGutter) {
      const frag = document.createDocumentFragment();
      for (let i = 0; i < lines.length; i += 1) {
        const cell = document.createElement("div");
        cell.className = "code-error-line";
        frag.appendChild(cell);
      }
      lockedErrorGutter.innerHTML = "";
      lockedErrorGutter.appendChild(frag);
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
    const missingLines = simulator.findMissingSemicolonLines(getEditorText() || "");
    if (missingLines.length) {
      const lineList = missingLines.map(String);
      let formatted = lineList[0];
      if (lineList.length === 2) {
        formatted = `${lineList[0]} and ${lineList[1]}`;
      } else if (lineList.length > 2) {
        formatted = `${lineList.slice(0, -1).join(", ")}, and ${lineList[lineList.length - 1]}`;
      }
      return `You need ${missingLines.length === 1 ? "a semicolon" : "semicolons"} at the end of line${missingLines.length === 1 ? " " : "s "}${formatted}.`;
    }
    if (current.kind === "compile") {
      return "The shown case does not compile yet. Fix syntax errors first.";
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
    updateLineGutters();
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
    if (nextBtn) nextBtn.disabled = !state.pass;
  }

  function buildHintContext(
    currentResult: ChallengeCaseResult,
    report: ChallengeRunReport,
  ): CodeOutputChallengeHintContext {
    const outputChecks = outputChecksForCase(
      state.visibleCase,
      currentResult.state || null,
    );
    return {
      text: getEditorText(),
      inputs: inputSpecs.map((inputSpec, index) => ({
        ...inputSpec,
        value: state.visibleCase.inputValues[index]!,
      })),
      outputs: outputSpecs.map((outputSpec) => ({ ...outputSpec })),
      outputChecks,
      currentCase: state.visibleCase,
      currentResult,
      report,
      tokenizeProgram: simulator.tokenizeProgram,
      parseStatements: simulator.parseStatements,
      findMissingSemicolonLines: simulator.findMissingSemicolonLines,
      behavesLike: behavesLikeProgramOnTestInputs,
    };
  }

  if (editor) {
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
        if (
          typeof start === "number" &&
          typeof end === "number" &&
          Number.isFinite(start) &&
          Number.isFinite(end) &&
          typeof editor.setSelectionRange === "function"
        ) {
          const clampedStart = Math.min(normalized.length, start);
          const clampedEnd = Math.min(normalized.length, end);
          editor.setSelectionRange(clampedStart, clampedEnd);
        }
      }
      state.text = editor.value;
      state.lastReport = null;
      state.pendingFailingCase = null;
      renderStage();
      updateLineGutters();
    });
    editor.addEventListener("scroll", syncEditorLinkedScroll);
    editor.addEventListener("mouseup", updateLineGutters);
    window.addEventListener("resize", updateLineGutters);
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => updateLineGutters());
      ro.observe(editor);
    }
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

  pager = createStepper({
    root: codeRoot || editor?.closest(".panel") || document.body,
    lines: 0,
    nextPage: next || null,
    endLabel,
    getBoundary: () => 0,
    setBoundary: () => {},
    onAfterChange: render,
    isStepLocked: () => false,
  });

  pager.update();
  render();
}

export { createCodeOutputChallengeTemplate };

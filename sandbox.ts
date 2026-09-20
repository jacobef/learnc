import { pageSyntheticAddressBase } from "./shared-c-address-space.js";
import { boundaryTargets, canJumpToBoundary, defaultOccurrence, executionBoundaries, nearestOccurrence, type LineBoundary } from "./shared-sandbox-navigation.js";
import { stdinReadPosition } from "./shared-stdin-position.js";
import { bindPanelSplit } from "./shared-panel-split.js";
import { SandboxExecution, sandboxStepLimit, DEFAULT_SANDBOX_STEP_LIMIT } from "./shared-sandbox-execution.js";
import { executionLimitExplanation, renderCodeDiagnostic } from "./shared-code-runtime-issues.js";
import { readSandboxProgress, writeSandboxProgress } from "./shared-progress.js";
import {
  applyOtherNames,
  appendStateObjects,
  clearNode,
  ensureBaseLayout,
  findArrayObjectBoxesForResult,
  queryRole,
  vbox,
} from "./shared-core-dom.js";
import type { BoxState, ProgramDiagnostic } from "./shared-core-utils.js";
import { confettiRain } from "./confetti.js";
import {
  bindCodeEditorTabKey,
  ensureCodeSurfaceElements,
  updateCodeSurface,
} from "./shared-code-editor-surface.js";
import {
  diagnosticDecorations,
  diagnosticMessageText,
} from "./shared-diagnostics.js";
import {
  evaluateCExpressionFiles,
  type CExecutionBudget,
  type CProgramBlocked,
  type CProgramExecutionLimit,
  type CSourceFile,
  type CProgramResult,
  type CProgramSourceLocation,
  type CProgramTraceEvent,
} from "./shared-c-interpreter.js";

const { main } = ensureBaseLayout();
main.classList.add("main-panelized");

function requiredRole<T extends Element>(name: string): T {
  const element = queryRole<T>(name);
  if (!element) throw new Error(`Missing required sandbox element: ${name}`);
  return element;
}

const instructions = requiredRole<HTMLElement>("sandbox-instructions");
const editor = requiredRole<HTMLTextAreaElement>("sandbox-editor");
const lineNumbers = requiredRole<HTMLElement>("sandbox-line-numbers");
const fileBar = requiredRole<HTMLElement>("sandbox-file-bar");
const terminal = requiredRole<HTMLElement>("sandbox-terminal");
const stdinInput = requiredRole<HTMLTextAreaElement>("sandbox-stdin");
const stdinHighlight = requiredRole<HTMLElement>("sandbox-stdin-highlight");
const stdinPositionLabel = requiredRole<HTMLElement>("sandbox-stdin-position");
const stdoutOutput = requiredRole<HTMLElement>("sandbox-stdout");
const stderrOutput = requiredRole<HTMLElement>("sandbox-stderr");
const stderrWrap = requiredRole<HTMLElement>("sandbox-stderr-wrap");
const simpleModeButton = requiredRole<HTMLButtonElement>("sandbox-mode-simple");
const advancedModeButton =
  requiredRole<HTMLButtonElement>("sandbox-mode-advanced");
const implicitMainControl = requiredRole<HTMLElement>(
  "sandbox-implicit-main-control",
);
const implicitMainInput =
  requiredRole<HTMLInputElement>("sandbox-implicit-main");
const askOnAmbiguousJump = requiredRole<HTMLInputElement>("sandbox-ask-jump");
const jumpDialog = requiredRole<HTMLDialogElement>("sandbox-jump-dialog");
const jumpTitle = requiredRole<HTMLElement>("sandbox-jump-title");
const jumpDescription = requiredRole<HTMLElement>("sandbox-jump-description");
const jumpChoices = requiredRole<HTMLElement>("sandbox-jump-choices");
const jumpForm = requiredRole<HTMLFormElement>("sandbox-jump-form");
const jumpOccurrence = requiredRole<HTMLInputElement>("sandbox-jump-occurrence");
let offeredJumpPositions: number[] = [];
let jumpReturnLabel = "";
const implicitMainNotice = requiredRole<HTMLElement>(
  "sandbox-implicit-main-notice",
);
const exprInput = requiredRole<HTMLInputElement>("sandbox-expr");
const exprResult = requiredRole<HTMLElement>("sandbox-expr-result");
const stage = requiredRole<HTMLElement>("sandbox-stage");
const codePane = requiredRole<HTMLElement>("sandbox-code");
const prevButtons = [requiredRole<HTMLButtonElement>("sandbox-prev")];
const nextButtons = [requiredRole<HTMLButtonElement>("sandbox-next")];
const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
bindCodeEditorTabKey(editor);

const STDIN_EOF_MARKER = "\u2404";
let finishedConfettiShown = false;

const boundaryLine = (() => {
  const row = editor.closest(".codepane-row");
  if (!(row instanceof HTMLElement)) throw new Error("Missing sandbox code editor row.");
  const el = document.createElement("div");
  el.className = "code-boundary-line";
  el.setAttribute("aria-hidden", "true");
  row.appendChild(el);
  return el;
})();

const diagnosticEl = (() => {
  const existing = codePane.querySelector<HTMLElement>(".code-diagnostic");
  if (existing) return existing;
  const el = document.createElement("div");
  el.className = "code-diagnostic hidden";
  codePane.appendChild(el);
  return el;
})();
const fileTabs = requiredRole<HTMLElement>("sandbox-file-tabs");
const addFileButton = requiredRole<HTMLButtonElement>("sandbox-file-add");
const deleteFileButton = requiredRole<HTMLButtonElement>("sandbox-file-delete");
const createFileControls = requiredRole<HTMLElement>("sandbox-file-create");
const fileNameInput = requiredRole<HTMLInputElement>("sandbox-file-name");
const confirmFileButton = requiredRole<HTMLButtonElement>("sandbox-file-confirm");
const cancelFileButton = requiredRole<HTMLButtonElement>("sandbox-file-cancel");
const deleteFileControls = requiredRole<HTMLElement>("sandbox-file-delete-confirm");
const deleteFileLabel = requiredRole<HTMLElement>("sandbox-file-delete-label");
const confirmDeleteButton = requiredRole<HTMLButtonElement>("sandbox-file-delete-yes");
const cancelDeleteButton = requiredRole<HTMLButtonElement>("sandbox-file-delete-no");

type SandboxState = {
  files: CSourceFile[];
  activePath: string;
  interfaceMode: "simple" | "advanced";
  implicitMain: boolean;
  effectiveImplicitMain: boolean;
  implicitMainNotice: string;
  stepPosition: number;
  traceLength: number;
  trace: CProgramTraceEvent[];
  mainClose: CProgramSourceLocation | null;
  blocked: CProgramBlocked | null;
  executionLimit: CProgramExecutionLimit | null;
  diagnostic: ProgramDiagnostic | null;
  otherNamesShown: Set<string>;
  lastState: BoxState[] | null;
  exprOtherNamesShown: Set<string>;
  stdin: string;
};

type StoredSandboxState = {
  stepLimit?: number;
  introductionDismissed?: boolean;
  codeShare?: number;
  askOnAmbiguousJump?: boolean;
  files?: CSourceFile[];
  activePath?: string;
  interfaceMode?: SandboxState["interfaceMode"];
  implicitMain?: boolean;
  stepPosition?: number;
  exprText?: string;
  otherNamesShown?: string[];
  exprOtherNamesShown?: string[];
  stdin?: string;
};

type StepAnchor = {
  file: string;
  kind: string;
  mappedStartLine: number;
  occurrence: number;
  text: string;
};

function validStoredFile(file: unknown): file is CSourceFile {
  if (!file || typeof file !== "object") return false;
  const candidate = file as Partial<CSourceFile>;
  return (
    typeof candidate.path === "string" &&
    candidate.path.length > 0 &&
    typeof candidate.source === "string"
  );
}

function validStringArray(value: unknown): string[] | undefined {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : undefined;
}

function validStepPosition(value: unknown): number | undefined {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  return Math.max(0, Math.floor(value));
}

function loadStoredSandboxState(): StoredSandboxState {
  try {
    const raw = readSandboxProgress();
    if (!raw) return {};
    const parsed = JSON.parse(raw) as StoredSandboxState;
    const files = Array.isArray(parsed.files)
      ? parsed.files.filter(validStoredFile)
      : undefined;
    const activePath =
      typeof parsed.activePath === "string" &&
      files?.some((file) => file.path === parsed.activePath)
        ? parsed.activePath
        : files?.[0]?.path;
    const interfaceMode =
      parsed.interfaceMode === "advanced" || parsed.interfaceMode === "simple"
        ? parsed.interfaceMode
        : undefined;
    return {
      introductionDismissed: parsed.introductionDismissed === true,
      stepLimit: typeof parsed.stepLimit === "number" ? sandboxStepLimit(parsed.stepLimit) : undefined,
      files: files && files.length > 0 ? files : undefined,
      activePath,
      interfaceMode,
      implicitMain: files && files.filter(file => file.path.endsWith(".c")).length > 1
        ? false
        : typeof parsed.implicitMain === "boolean" ? parsed.implicitMain : undefined,
      askOnAmbiguousJump: parsed.askOnAmbiguousJump === true,
      codeShare: typeof parsed.codeShare === "number" && Number.isFinite(parsed.codeShare)
        ? Math.max(0.2, Math.min(0.8, parsed.codeShare)) : undefined,
      stepPosition: validStepPosition(parsed.stepPosition),
      exprText: typeof parsed.exprText === "string" ? parsed.exprText : undefined,
      otherNamesShown: validStringArray(parsed.otherNamesShown),
      exprOtherNamesShown: validStringArray(parsed.exprOtherNamesShown),
      stdin: typeof parsed.stdin === "string" ? parsed.stdin : undefined,
    };
  } catch {
    return {};
  }
}

const storedSandbox = loadStoredSandboxState();
let introductionDismissed = storedSandbox.introductionDismissed === true;
let codeShare = storedSandbox.codeShare ?? 1 / 2.3;
let keepStepperAtEnd = storedSandbox.stepPosition === undefined;
let shouldInferInitialEndPin = storedSandbox.stepPosition !== undefined;
let pendingStepAnchor: StepAnchor | null = null;
let executionLimitNoticeDismissed = false;
const execution = new SandboxExecution(storedSandbox.stepLimit ?? DEFAULT_SANDBOX_STEP_LIMIT);
const stepLimitInput = requiredRole<HTMLInputElement>("sandbox-step-limit");
stepLimitInput.value = String(execution.stepLimit);

function executionBudget(): CExecutionBudget {
  return execution.budget();
}

function resetExecutionPaging() {
  executionLimitNoticeDismissed = false;
  execution.reset();
}

const sandbox: SandboxState = {
  files: storedSandbox.files ?? [{ path: "program.c", source: editor.value }],
  activePath: storedSandbox.activePath ?? "program.c",
  interfaceMode: storedSandbox.interfaceMode ?? "simple",
  implicitMain: storedSandbox.implicitMain ?? true,
  effectiveImplicitMain: true,
  implicitMainNotice: "",
  stepPosition: storedSandbox.stepPosition ?? 0,
  traceLength: 0,
  trace: [],
  mainClose: null,
  blocked: null,
  executionLimit: null,
  diagnostic: null,
  otherNamesShown: new Set(storedSandbox.otherNamesShown ?? []),
  lastState: null,
  exprOtherNamesShown: new Set(storedSandbox.exprOtherNamesShown ?? []),
  stdin: storedSandbox.stdin ?? stdinInput.value,
};
editor.value = activeFile().source;
exprInput.value = storedSandbox.exprText ?? exprInput.value;
stdinInput.value = sandbox.stdin;
askOnAmbiguousJump.checked = storedSandbox.askOnAmbiguousJump ?? false;
bindPanelSplit(
  requiredRole<HTMLElement>("sandbox-panels"),
  requiredRole<HTMLElement>("sandbox-panel-divider"),
  codeShare,
  share => { codeShare = share; saveSandboxState(); },
);

function saveSandboxState() {
  try {
    writeSandboxProgress(
      JSON.stringify({
        introductionDismissed,
        stepLimit: execution.stepLimit,
        askOnAmbiguousJump: askOnAmbiguousJump.checked,
        codeShare,
        files: sandbox.files,
        activePath: sandbox.activePath,
        interfaceMode: sandbox.interfaceMode,
        implicitMain: sandbox.implicitMain,
        stepPosition: sandbox.stepPosition,
        exprText: exprInput.value,
        otherNamesShown: Array.from(sandbox.otherNamesShown),
        exprOtherNamesShown: Array.from(sandbox.exprOtherNamesShown),
        stdin: sandbox.stdin,
      } satisfies StoredSandboxState),
    );
  } catch {
    // Losing local persistence should not break the sandbox itself.
  }
}

function getEditorText() {
  return editor.value;
}

function activeFile(): CSourceFile {
  return (
    sandbox.files.find((file) => file.path === sandbox.activePath) ??
    sandbox.files[0]!
  );
}

function filesForRun(): CSourceFile[] {
  return sandbox.interfaceMode === "simple" ? [activeFile()] : sandbox.files;
}

function stdinForRun(): string {
  return sandbox.interfaceMode === "advanced" ? sandbox.stdin : "";
}

function implicitMainForRun(): boolean {
  return sandbox.interfaceMode === "simple" || sandbox.implicitMain;
}

function renderInterfaceControls() {
  const simple = sandbox.interfaceMode === "simple";
  simpleModeButton.classList.toggle("is-active", simple);
  simpleModeButton.setAttribute("aria-pressed", String(simple));
  advancedModeButton.classList.toggle("is-active", !simple);
  advancedModeButton.setAttribute("aria-pressed", String(!simple));
  fileBar.classList.toggle("hidden", simple);
  terminal.classList.toggle("hidden", simple);
  requiredRole<HTMLElement>("sandbox-jump-setting").classList.toggle("hidden", simple);
  implicitMainControl.classList.toggle("hidden", simple);
  requiredRole<HTMLElement>("sandbox-step-limit-control").classList.toggle("hidden", simple);
  implicitMainInput.checked = sandbox.implicitMain;
  const multipleCFiles = sandbox.files.filter(file => file.path.endsWith(".c")).length > 1;
  implicitMainInput.disabled = multipleCFiles;
  requiredRole<HTMLElement>("sandbox-implicit-main-restriction").classList.toggle("hidden", !multipleCFiles);
  implicitMainNotice.textContent =
    simple &&
    sandbox.implicitMainNotice ===
      "This program works with implicit main off. Try turning off Implicit main."
      ? "This program works with implicit main off. Select Advanced, then turn off Implicit main."
      : sandbox.implicitMainNotice;
  implicitMainNotice.classList.toggle("hidden", !sandbox.implicitMainNotice);
}

function getRawLines() {
  return getEditorText().split(/\r?\n/);
}

function countNewlines(value: string) {
  let count = 0;
  for (let i = 0; i < value.length; i += 1) {
    if (value[i] === "\n") count += 1;
  }
  return count;
}

function diffSegments(prev: string, next: string) {
  let start = 0;
  const prevLen = prev.length;
  const nextLen = next.length;
  while (start < prevLen && start < nextLen && prev[start] === next[start]) {
    start += 1;
  }
  let endPrev = prevLen;
  let endNext = nextLen;
  while (
    endPrev > start &&
    endNext > start &&
    prev[endPrev - 1] === next[endNext - 1]
  ) {
    endPrev -= 1;
    endNext -= 1;
  }
  return {
    start,
    oldSegment: prev.slice(start, endPrev),
    newSegment: next.slice(start, endNext),
  };
}

function clampStepPosition(value: number, traceLength: number) {
  return Math.max(0, Math.min(traceLength, Math.floor(value)));
}

function sourceLinesForFile(path: string) {
  const file = sandbox.files.find((candidate) => candidate.path === path);
  return (file?.source ?? "").split(/\r?\n/);
}

function textForTraceEvent(event: CProgramTraceEvent) {
  return sourceLinesForFile(event.file)
    .slice(event.startLine, event.endLine + 1)
    .join("\n")
    .trim();
}

function mapLineThroughEdit(prevText: string, nextText: string, line: number) {
  const { start, oldSegment, newSegment } = diffSegments(prevText, nextText);
  const changeLine = countNewlines(prevText.slice(0, start));
  if (changeLine > line) return line;
  const delta = countNewlines(newSegment) - countNewlines(oldSegment);
  if (changeLine < line) return Math.max(0, line + delta);
  if (delta > 0 && oldSegment === "") return Math.max(0, line + delta);
  return line;
}

function captureStepAnchorForEdit(prevText: string, nextText: string) {
  if (keepStepperAtEnd || prevText === nextText) return null;
  const nextEvent = sandbox.trace[sandbox.stepPosition];
  if (!nextEvent || nextEvent.file !== sandbox.activePath) return null;
  const occurrence = sandbox.trace
    .slice(0, sandbox.stepPosition)
    .filter(
      (event) =>
        event.file === nextEvent.file &&
        event.kind === nextEvent.kind &&
        event.startLine === nextEvent.startLine,
    ).length;
  return {
    file: nextEvent.file,
    kind: nextEvent.kind,
    mappedStartLine: mapLineThroughEdit(
      prevText,
      nextText,
      nextEvent.startLine,
    ),
    occurrence,
    text: textForTraceEvent(nextEvent),
  };
}

function tracePositionForOccurrence(
  trace: CProgramTraceEvent[],
  occurrence: number,
  matches: (event: CProgramTraceEvent) => boolean,
): number | null {
  let seen = 0;
  let lastMatch: number | null = null;
  for (let index = 0; index < trace.length; index += 1) {
    if (!matches(trace[index]!)) continue;
    lastMatch = index;
    if (seen === occurrence) return index;
    seen += 1;
  }
  return lastMatch;
}

function positionForStepAnchor(
  anchor: StepAnchor,
  trace: CProgramTraceEvent[],
) {
  const exactLine = tracePositionForOccurrence(
    trace,
    anchor.occurrence,
    (event) =>
      event.file === anchor.file &&
      event.kind === anchor.kind &&
      event.startLine === anchor.mappedStartLine,
  );
  if (exactLine != null) return exactLine;
  if (anchor.text) {
    const textMatch = tracePositionForOccurrence(
      trace,
      anchor.occurrence,
      (event) =>
        event.file === anchor.file &&
        event.kind === anchor.kind &&
        textForTraceEvent(event) === anchor.text,
    );
    if (textMatch != null) return textMatch;
  }
  const sameFileAfterLine = trace.findIndex(
    (event) =>
      event.file === anchor.file && event.startLine > anchor.mappedStartLine,
  );
  return sameFileAfterLine >= 0 ? sameFileAfterLine : null;
}

function updateInstructions() {
  requiredRole<HTMLElement>("sandbox-introduction").classList.toggle("hidden", introductionDismissed);
  if (introductionDismissed) return;
  const message = "This is the sandbox. The program state will update as you write code.";
  const params = new URLSearchParams(window.location.search);
  const finished = params.get("finished") === "1";
  instructions.classList.toggle("sandbox-finished", finished);
  if (finished) {
    instructions.innerHTML = `You finished the tutorial as it currently exists, congrats! Many more problems will be coming later.<br><br>${message}`;
    if (!finishedConfettiShown && typeof confettiRain === "function") {
      finishedConfettiShown = true;
      confettiRain();
    }
  } else {
    instructions.textContent = message;
  }
}

requiredRole<HTMLButtonElement>("sandbox-dismiss-introduction").addEventListener("click", () => {
  introductionDismissed = true;
  saveSandboxState();
  updateInstructions();
});

function outcomeForPosition(
  result: CProgramResult,
  position: number,
  files: CSourceFile[],
) {
  if (result.kind !== "ok") {
    return {
      kind: result.kind,
      globalKind: result.kind,
      state: null as BoxState[] | null,
      trace: [] as CProgramTraceEvent[],
      stdout: "",
      stderr: "",
      stdinPosition: null,
    };
  }
  const trace = result.trace || [];
  const safePosition = clampStepPosition(position, trace.length);
  const current = safePosition > 0 ? trace[safePosition - 1] : undefined;
  const blocked = safePosition >= trace.length ? result.blocked : null;
  const executionLimit =
    !executionLimitNoticeDismissed &&
    result.executionLimit &&
    result.executionLimit.reason !== "trace-limit" &&
    safePosition === result.executionLimit.tracePosition
      ? result.executionLimit
      : null;
  return {
    kind: "ok" as const,
    globalKind: blocked
      ? ("blocked" as const)
      : executionLimit
        ? ("execution-limit" as const)
        : ("ok" as const),
    state:
      blocked?.state ??
      (current ? current.state : executionLimit ? result.state : []),
    files,
    eventIndex: current ? safePosition - 1 : null,
    implicitMain: sandbox.effectiveImplicitMain,
    trace,
    blocked,
    executionLimit,
    executionBudget: executionBudget(),
    stdout: safePosition >= trace.length && !result.executionLimit
      ? result.stdout
      : new TextDecoder().decode(result.stdoutBytes.subarray(0, current?.stdoutLength ?? 0)),
    stderr: safePosition >= trace.length && !result.executionLimit
      ? result.stderr
      : new TextDecoder().decode(result.stderrBytes.subarray(0, current?.stderrLength ?? 0)),
    stdinPosition: safePosition >= trace.length && !result.executionLimit
      ? result.stdinPosition
      : current?.stdinPosition ?? 0,
  };
}

function syncStdinHighlightScroll() {
  const style = getComputedStyle(stdinInput);
  const borders = parseFloat(style.borderLeftWidth) + parseFloat(style.borderRightWidth);
  const scrollbar = Math.max(0, Math.round(stdinInput.offsetWidth - stdinInput.clientWidth - borders));
  // Integer DOM measurements can differ by a pixel at fractional browser zoom.
  stdinHighlight.style.width = `calc(100% - ${scrollbar > 2 ? scrollbar : 0}px)`;
  stdinHighlight.scrollTop = stdinInput.scrollTop;
  stdinHighlight.scrollLeft = stdinInput.scrollLeft;
}

function renderStdinPosition(position: number | null) {
  stdinHighlight.replaceChildren();
  if (position === null) {
    stdinHighlight.textContent = stdinInput.value + "\u200b";
    stdinPositionLabel.textContent = "Read position unavailable while the program has an error.";
  } else {
    const { offset, label } = stdinReadPosition(stdinInput.value, position);
    const consumed = document.createElement("span");
    consumed.className = "sandbox-stdin-consumed";
    consumed.textContent = stdinInput.value.slice(0, offset);
    const cursor = document.createElement("span");
    cursor.className = "sandbox-stdin-cursor";
    stdinHighlight.append(consumed, cursor, stdinInput.value.slice(offset) + "\u200b");
    stdinPositionLabel.textContent = label;
  }
  syncStdinHighlightScroll();
}

function renderState(
  title: string,
  boxes: BoxState[] | null,
  status = "ok",
  blocked: CProgramBlocked | null = null,
  executionLimit: CProgramExecutionLimit | null = null,
) {
  const wrap = document.createElement("div");
  wrap.className = "state-panel";
  if (title) {
    const heading = document.createElement("div");
    heading.className = "panel-title state-heading";
    heading.textContent = title;
    wrap.appendChild(heading);
  }
  const grid = document.createElement("div");
  grid.className = "grid";
  if (status === "blocked") {
    const notice = document.createElement("div");
    notice.className = "sandbox-blocked-notice";
    notice.textContent =
      sandbox.interfaceMode === "simple"
        ? `The program is waiting for stdin in ${blocked?.function || "an input function"}. Select Advanced to enter input or send EOF.`
        : `The program is waiting for stdin in ${blocked?.function || "an input function"}. Enter more input, or press Ctrl+D in stdin to send EOF.`;
    wrap.appendChild(notice);
  }
  if (status === "execution-limit") {
    const notice = document.createElement("div");
    notice.className = "sandbox-blocked-notice";
    const location = executionLimit
      ? `${executionLimit.file}, line ${executionLimit.startLine + 1}`
      : "the repeated section";
    notice.textContent = executionLimit
      ? executionLimit.reason === "step-limit"
        ? `The configured limit of ${execution.stepLimit.toLocaleString()} interpreted steps was reached. Increase Step limit in Advanced mode to continue. This does not necessarily mean the program is in an infinite loop.`
        : `${executionLimitExplanation(executionLimit).message} See ${location}.`
      : "The interpreter paused this program. Inspect the loop's stopping condition.";
    wrap.appendChild(notice);
  }
  if (
    status !== "ok" &&
    status !== "blocked" &&
    status !== "execution-limit"
  ) {
    const msg = document.createElement("div");
    msg.className = "muted state-status";
    msg.style.padding = "8px";
    msg.textContent = "(the program does not currently run)";
    grid.appendChild(msg);
  } else if (!boxes || boxes.length === 0) {
    const msg = document.createElement("div");
    msg.className = "muted";
    msg.style.padding = "8px";
    msg.textContent = "(no variables yet)";
    grid.appendChild(msg);
  } else {
    appendStateObjects(grid, boxes, { editable: false, deletable: false });
  }
  wrap.appendChild(grid);
  return wrap;
}

function renderExpression(outcome: {
  kind: string;
  state: BoxState[] | null;
  files?: CSourceFile[];
  eventIndex?: number | null;
  implicitMain?: boolean;
  executionBudget?: CExecutionBudget;
}) {
  clearNode(exprResult);
  exprResult.classList.add("hidden");
  const expr = exprInput.value.trim();
  if (!expr) return;
  if (outcome.kind !== "ok") {
    renderExpressionError(
      "Expression unavailable",
      "Fix the program error before evaluating an expression.",
    );
    return;
  }
  const evaluated = evaluateCExpressionFiles(
    outcome.files || sandbox.files,
    outcome.eventIndex ?? -1,
    expr,
    undefined,
    stdinForRun(),
    outcome.implicitMain ?? implicitMainForRun(),
    outcome.executionBudget ?? executionBudget(),
  );
  if (evaluated.kind !== "ok") {
    renderExpressionError(
      evaluated.kind === "ub"
        ? "Invalid expression: undefined behavior"
        : "Invalid expression",
      diagnosticMessageText(evaluated.diagnostic),
    );
    return;
  }
  const { result } = evaluated;
  const arrayBoxes = findArrayObjectBoxesForResult(result, outcome.state || []);
  if (arrayBoxes && arrayBoxes.length) {
    const wrap = document.createElement("div");
    appendStateObjects(wrap, arrayBoxes, { editable: false, deletable: false });
    const arrayNode = wrap.querySelector(".arraybox") as HTMLElement | null;
    if (arrayNode) {
      exprResult.appendChild(arrayNode);
      exprResult.classList.remove("hidden");
      return;
    }
  }
  const resultName = String((result as { name?: unknown }).name ?? "").trim();
  const resultIsArray = result.typeInfo.kind === "array";
  const match =
    result.kind === "lvalue"
      ? (resultName
          ? (outcome.state || []).find((box) => box.name === resultName)
          : null) ||
        (outcome.state || []).find((box) => {
          if (String(box.address) !== String(result.address)) return false;
          const boxIsArrayRoot = !box.arrayRoot && box.typeInfo?.kind === "array";
          return resultIsArray || !boxIsArrayRoot;
        })
      : null;
  const node = match
    ? vbox({
        address: match.address ?? undefined,
        type: match.type,
        value: match.value,
        displayValue: match.displayValue,
        exactValue: match.exactValue,
        typeInfo: match.typeInfo,
        aliases: match.aliases ?? [],
        name: match.name,
        editable: false,
      })
    : vbox({
        address: result.address ? String(result.address) : "—",
        type: result.type || "int",
        value: result.value ?? "",
        displayValue: result.displayValue,
        exactValue: result.exactValue,
        typeInfo: result.typeInfo,
        name: "",
        editable: false,
      });
  if (match && (match.value ?? "") === "") {
    node.querySelector(".value")?.classList.add("placeholder", "muted");
  }
  if (!match && !result.address) node.classList.add("no-addr");
  if (!match) node.classList.add("no-name");
  exprResult.appendChild(node);
  exprResult.classList.remove("hidden");
  if (match) {
    applyOtherNames(exprResult, {
      shownAddrs: sandbox.exprOtherNamesShown,
      onToggle: () => {
        saveSandboxState();
        refreshOtherNames();
      },
      sourceBoxes: outcome.state || [],
      cleanupShownAddrs: false,
    });
  }
}

function renderExpressionError(title: string, message: string) {
  const error = document.createElement("div");
  error.className = "sandbox-expr-error";
  const heading = document.createElement("div");
  heading.className = "sandbox-expr-error-title";
  heading.textContent = title;
  const detail = document.createElement("div");
  detail.className = "sandbox-expr-error-message";
  detail.textContent = message;
  error.append(heading, detail);
  exprResult.appendChild(error);
  exprResult.classList.remove("hidden");
}

function refreshOtherNames() {
  applyOtherNames(stage, {
    shownAddrs: sandbox.otherNamesShown,
    onToggle: () => {
      saveSandboxState();
      refreshOtherNames();
    },
  });
  applyOtherNames(exprResult, {
    shownAddrs: sandbox.exprOtherNamesShown,
    onToggle: () => {
      saveSandboxState();
      refreshOtherNames();
    },
    sourceBoxes: sandbox.lastState || [],
    cleanupShownAddrs: false,
  });
}

function renderFileTabs() {
  fileTabs.replaceChildren();
  for (const file of sandbox.files) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "sandbox-file-tab";
    button.textContent = file.path;
    button.title = file.path;
    button.setAttribute("role", "tab");
    button.setAttribute("aria-selected", String(file.path === sandbox.activePath));
    button.classList.toggle("active", file.path === sandbox.activePath);
    button.addEventListener("click", () => switchToFile(file.path));
    fileTabs.appendChild(button);
  }
  deleteFileButton.disabled = sandbox.files.length <= 1;
}

function switchToFile(path: string) {
  const file = sandbox.files.find((candidate) => candidate.path === path);
  if (!file || path === sandbox.activePath) return;
  closeDeleteConfirmation();
  resetExecutionPaging();
  activeFile().source = editor.value;
  sandbox.activePath = path;
  editor.value = file.source;
  saveSandboxState();
  renderFileTabs();
  renderStage();
}

let discoveryWorker: Worker | null = null;
let discoveryKey = "";
let discoveredBoundaries: (LineBoundary | null)[] = [];

function discoverJumpTargets(files: CSourceFile[], stdin: string, implicitMain: boolean, result: CProgramResult) {
  const key = JSON.stringify([files, stdin, implicitMain, execution.stepLimit, sandbox.interfaceMode]);
  if (key === discoveryKey) return;
  discoveryKey = key;
  discoveryWorker?.terminate();
  discoveryWorker = null;
  discoveredBoundaries = [];
  if (sandbox.interfaceMode !== "advanced" || result.kind !== "ok" || result.executionLimit?.reason !== "trace-limit") return;
  const worker = new Worker(new URL("./shared-sandbox-discovery-worker.js", import.meta.url), {type: "module"});
  discoveryWorker = worker;
  worker.onmessage = ({data}: MessageEvent<{boundaries: (LineBoundary | null)[]; more: boolean}>) => {
    if (discoveryWorker !== worker) return;
    discoveredBoundaries = data.boundaries;
    updateLineGutters();
    if (!data.more) {
      worker.terminate();
      discoveryWorker = null;
    }
  };
  worker.onerror = () => {
    worker.terminate();
    if (discoveryWorker === worker) discoveryWorker = null;
  };
  worker.postMessage({files, stdin, implicitMain, stepLimit: execution.stepLimit, addressBase: pageSyntheticAddressBase});
}

function renderStage() {
  jumpDialog.close();
  offeredJumpPositions = [];
  clearNode(stage);
  const previousTraceLength = sandbox.traceLength;
  const previousStepPosition = sandbox.stepPosition;
  const runFiles = filesForRun();
  const programIsBlank = runFiles.every((file) => file.source.trim() === "");
  const implicitMainRequested = implicitMainForRun();
  const result = execution.run(runFiles, stdinForRun(), implicitMainRequested);
  discoverJumpTargets(runFiles, stdinForRun(), implicitMainRequested, result);
  sandbox.effectiveImplicitMain =
    result.implicitMainApplied ?? implicitMainRequested;
  if (result.implicitMainNotice) {
    sandbox.implicitMainNotice = result.implicitMainNotice;
  } else if (implicitMainRequested) {
    sandbox.implicitMainNotice = "";
  }
  sandbox.diagnostic = result.kind === "ok" ? null : result.diagnostic;
  if (result.kind === "ok") {
    sandbox.trace = result.trace;
    sandbox.mainClose = result.mainClose;
    sandbox.blocked = result.blocked;
    sandbox.executionLimit = result.executionLimit;
    sandbox.traceLength = sandbox.trace.length;
  } else {
    sandbox.trace = [];
    sandbox.traceLength = 0;
    sandbox.mainClose = null;
    sandbox.blocked = null;
    sandbox.executionLimit = null;
  }
  if (
    result.kind === "ok" &&
    programIsBlank &&
    previousTraceLength > 0
  ) {
    keepStepperAtEnd = previousStepPosition > 0;
  }
  if (result.kind === "ok" && result.executionLimit?.reason === "trace-limit") keepStepperAtEnd = false;
  sandbox.stepPosition = keepStepperAtEnd
    ? sandbox.traceLength
    : clampStepPosition(sandbox.stepPosition, sandbox.traceLength);
  if (!keepStepperAtEnd && result.kind === "ok" && pendingStepAnchor) {
    const anchoredPosition = positionForStepAnchor(
      pendingStepAnchor,
      sandbox.trace,
    );
    if (anchoredPosition != null) {
      sandbox.stepPosition = clampStepPosition(
        anchoredPosition,
        sandbox.traceLength,
      );
    } else if (result.executionLimit) {
      sandbox.stepPosition = result.executionLimit.tracePosition;
    }
  }
  if (
    result.kind === "ok" &&
    result.executionLimit &&
    result.executionLimit.reason !== "trace-limit" &&
    !executionLimitNoticeDismissed
  ) {
    keepStepperAtEnd = false;
    sandbox.stepPosition = clampStepPosition(
      result.executionLimit.tracePosition,
      sandbox.traceLength,
    );
  }
  if (result.kind === "ok") {
    pendingStepAnchor = null;
  }
  if (
    shouldInferInitialEndPin &&
    result.kind === "ok" &&
    sandbox.traceLength > 0
  ) {
    keepStepperAtEnd = sandbox.stepPosition >= sandbox.traceLength;
    shouldInferInitialEndPin = false;
  }
  const outcome = outcomeForPosition(result, sandbox.stepPosition, runFiles);
  stdoutOutput.textContent = outcome.stdout;
  stderrOutput.textContent = outcome.stderr;
  renderStdinPosition(outcome.stdinPosition);
  stderrWrap.classList.toggle("hidden", !outcome.stderr);
  sandbox.lastState = outcome.state;
  stage.appendChild(
    renderState(
      "",
      outcome.state,
      outcome.globalKind,
      outcome.blocked ?? null,
      outcome.executionLimit ?? null,
    ),
  );
  refreshOtherNames();
  renderExpression(outcome);
  updateStepperControls(
    sandbox.stepPosition,
    sandbox.trace,
    outcome.blocked ?? null,
    sandbox.executionLimit,
    result.kind !== "ok",
  );
  renderFileTabs();
  renderInterfaceControls();
  updateInstructions();
  updateLineGutters();
  saveSandboxState();
}

function runLabelForPosition(position: number, trace: CProgramTraceEvent[]) {
  const next = trace[position];
  if (!next) return "At end ▶";
  const filePrefix = next.file === sandbox.activePath ? "" : `${next.file}:`;
  const start = next.startLine + 1;
  const end = next.endLine + 1;
  return start === end
    ? `Run ${filePrefix}${start} ▶`
    : `Run ${filePrefix}${start}-${end} ▶`;
}

function updateStepperControls(
  position: number,
  trace: CProgramTraceEvent[],
  blocked: CProgramBlocked | null,
  executionLimit: CProgramExecutionLimit | null,
  hasError = false,
) {
  prevButtons.forEach((btn) => {
    btn.disabled = position <= 0;
  });
  nextButtons.forEach((btn) => {
    btn.textContent =
      blocked && position >= trace.length
        ? `Waiting for ${blocked.function}`
        : executionLimit &&
            position >= trace.length
          ? executionLimit.reason === "step-limit" ? "Step limit reached"
            : executionLimit.reason === "trace-limit" ? "Continue execution ▶" : "Continue loop ▶"
        : runLabelForPosition(position, trace);
    const canExtendTrace =
      !!executionLimit &&
      executionLimit.reason !== "step-limit" &&
      position >= trace.length;
    btn.disabled = hasError || (position >= trace.length && !canExtendTrace);
  });
}

function sandboxBoundaries() {
  const endFile = sandbox.files.find(file => file.path === sandbox.mainClose?.file)
    ?? filesForRun()[0]!;
  return executionBoundaries(sandbox.trace, sandbox.mainClose, sandbox.blocked, {
    file: endFile.path,
    line: endFile.source.split(/\r?\n/).length,
  }, sandbox.executionLimit !== null);
}

function jumpToPosition(position: number) {
  execution.includePosition(position);
  executionLimitNoticeDismissed = true;
  keepStepperAtEnd = false;
  sandbox.stepPosition = position;
  saveSandboxState();
  renderStage();
  const button = Array.from(lineNumbers.querySelectorAll<HTMLButtonElement>(".sandbox-line-jump"))
    .find(button => button.getAttribute("aria-label") === jumpReturnLabel);
  // The selected target may disappear after jumping. Keep keyboard arrows on
  // navigation rather than transferring them to the editor's text cursor.
  (button ?? [...nextButtons, ...prevButtons].find(control => !control.disabled))?.focus({ preventScroll: true });
  scrollEventIntoView(sandbox.trace[position] ?? sandbox.trace[position - 1]);
}

function offerBoundaryJump(label: string, positions: number[], first = false) {
  jumpReturnLabel = label;
  const target = first ? positions[0] : defaultOccurrence(positions, sandbox.stepPosition);
  if (target === undefined || target === sandbox.stepPosition) return;
  if (first || !askOnAmbiguousJump.checked || positions.length === 1) {
    jumpToPosition(target);
    return;
  }
  offeredJumpPositions = positions;
  jumpTitle.textContent = label;
  const current = positions.indexOf(sandbox.stepPosition);
  jumpDescription.textContent = `This boundary occurs ${positions.length} times in the available execution.${current >= 0 ? ` You are at occurrence ${current + 1}.` : ""}`;
  clearNode(jumpChoices);
  const choices: [string, number | undefined][] = [
    ["Nearest forward", nearestOccurrence(positions, sandbox.stepPosition, "forward")],
    ["Nearest backward", nearestOccurrence(positions, sandbox.stepPosition, "backward")],
    ["First occurrence", positions[0]],
    ["Last occurrence", positions[positions.length - 1]],
  ];
  for (const [text, position] of choices) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = text;
    button.disabled = position === undefined;
    button.addEventListener("click", () => {
      if (position !== undefined) jumpToPosition(position);
    });
    jumpChoices.appendChild(button);
  }
  jumpOccurrence.max = String(positions.length);
  jumpOccurrence.value = String(positions.indexOf(target) + 1);
  jumpDialog.showModal();
}

askOnAmbiguousJump.addEventListener("change", () => {
  saveSandboxState();
  updateLineGutters();
});
jumpForm.addEventListener("submit", event => {
  event.preventDefault();
  const position = offeredJumpPositions[jumpOccurrence.valueAsNumber - 1];
  if (position !== undefined) jumpToPosition(position);
});
requiredRole<HTMLButtonElement>("sandbox-jump-cancel").addEventListener("click", () => jumpDialog.close());
jumpDialog.addEventListener("click", event => {
  if (event.target !== jumpDialog) return;
  const rect = jumpDialog.getBoundingClientRect();
  if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) jumpDialog.close();
});

function updateLineGutters(linesOverride?: string[]) {
  const lines = linesOverride ?? getRawLines();
  const count = Math.max(lines.length, 1);
  const diagnostic = sandbox.diagnostic;
  const lineClasses = new Map<number, string[]>();
  for (const event of sandbox.trace.slice(0, sandbox.stepPosition)) {
    if (event.file !== sandbox.activePath) continue;
    const start = Math.max(0, Math.min(count - 1, event.startLine));
    const end = Math.max(start, Math.min(count - 1, event.endLine));
    for (let line = start; line <= end; line += 1) {
      lineClasses.set(line, ["is-done"]);
    }
  }
  const nextEvent = sandbox.trace[sandbox.stepPosition];
  if (!diagnostic && nextEvent?.file === sandbox.activePath) {
    for (let line = Math.max(0, nextEvent.startLine); line <= Math.min(count - 1, nextEvent.endLine); line += 1) {
      lineClasses.set(line, [...(lineClasses.get(line) ?? []), "is-next"]);
    }
  }
  const lineNumberClasses = new Map<number, string[]>();
  if (
    diagnostic &&
    (!diagnostic.file || diagnostic.file === sandbox.activePath)
  ) {
    lineNumberClasses.set(diagnostic.range.startLine, ["has-error"]);
  }
  const { wraps, lineHeight } = updateCodeSurface({
    editor,
    lineNumbers,
    highlightEl,
    measureEl,
    lines,
    lineClasses,
    lineNumberClasses,
    decorations: diagnosticDecorations(diagnostic, lines, sandbox.activePath),
  });
  renderCodeDiagnostic(diagnosticEl, editor, diagnostic, { activeFile: sandbox.activePath, showRuntimeContext: false });
  const style = window.getComputedStyle(editor);
  const paddingTop = parseFloat(style.paddingTop) || 0;
  const boundaries = sandboxBoundaries();
  const boundary = boundaries[sandbox.stepPosition];
  const boundaryLineIndex = boundary?.line ?? count;
  boundaryLine.hidden = !!diagnostic || boundary?.file !== sandbox.activePath;
  if (!diagnostic && sandbox.interfaceMode === "advanced") {
    const jumpBoundaries = discoveredBoundaries.length >= boundaries.length ? discoveredBoundaries : boundaries;
    for (const {line, positions, kind, paired} of boundaryTargets(jumpBoundaries, sandbox.activePath)) {
      const canJump = kind === "first" ? positions[0] !== sandbox.stepPosition
        : canJumpToBoundary(positions, sandbox.stepPosition, askOnAmbiguousJump.checked);
      if (line < 0 || line > count || !canJump) continue;
      const row = lineNumbers.children[Math.min(line, count - 1)];
      if (!(row instanceof HTMLElement)) continue;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "sandbox-line-jump";
      if (line === count) button.classList.add("is-end");
      const label = paired
        ? `${kind === "first" ? "First" : "Nearest"} occurrence of line ${line + 1}`
        : line === count ? "Jump to end" : `Jump before line ${line + 1}`;
      if (paired) button.classList.add(kind === "first" ? "is-loop-first" : "is-loop-nearest");
      button.setAttribute("aria-label", label);
      button.dataset.tooltip = `${label}${positions.length > 1 ? ` (${positions.length} occurrences)` : ""}`;
      button.addEventListener("click", () => offerBoundaryJump(label, positions, kind === "first"));
      row.appendChild(button);
    }
  }
  let rows = 0;
  for (let i = 0; i < Math.min(boundaryLineIndex, count); i += 1) {
    rows += wraps[i] || 1;
  }
  boundaryLine.style.top = `${Math.max(0, paddingTop + rows * lineHeight - 1)}px`;
  lineNumbers.scrollTop = editor.scrollTop;
}

function scrollEventIntoView(event: CProgramTraceEvent | undefined) {
  if (!event || event.file !== sandbox.activePath) return;
  const scroller = codePane;
  const startNode = lineNumbers.children[event.startLine] as HTMLElement | undefined;
  const endNode = lineNumbers.children[event.endLine] as HTMLElement | undefined;
  if (!startNode || !endNode) return;
  const scrollerRect = scroller.getBoundingClientRect();
  const startRect = startNode.getBoundingClientRect();
  const endRect = endNode.getBoundingClientRect();
  const top = scrollerRect.top + 8;
  const bottom = scrollerRect.bottom - 8;
  if (startRect.top < top) {
    scroller.scrollTop += startRect.top - top;
  } else if (endRect.bottom > bottom) {
    scroller.scrollTop += endRect.bottom - bottom;
  }
}

editor.addEventListener("input", () => {
  resetExecutionPaging();
  pendingStepAnchor =
    captureStepAnchorForEdit(activeFile().source, editor.value) ??
    pendingStepAnchor;
  activeFile().source = editor.value;
  saveSandboxState();
  renderStage();
});
stdinInput.addEventListener("scroll", syncStdinHighlightScroll);
if (typeof ResizeObserver !== "undefined") {
  new ResizeObserver(syncStdinHighlightScroll).observe(stdinInput);
}
stdinInput.addEventListener("input", () => {
  resetExecutionPaging();
  sandbox.stdin = stdinInput.value;
  saveSandboxState();
  renderStage();
});
stdinInput.addEventListener("keydown", (event) => {
  if (
    event.ctrlKey &&
    !event.altKey &&
    !event.metaKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === "d"
  ) {
    event.preventDefault();
    stdinInput.setRangeText(
      STDIN_EOF_MARKER,
      stdinInput.selectionStart,
      stdinInput.selectionEnd,
      "end",
    );
    resetExecutionPaging();
    sandbox.stdin = stdinInput.value;
    saveSandboxState();
    renderStage();
  }
});
editor.addEventListener("scroll", () => {
  lineNumbers.scrollTop = editor.scrollTop;
});
window.addEventListener("resize", () => updateLineGutters());
if (typeof ResizeObserver !== "undefined") {
  const ro = new ResizeObserver(() => updateLineGutters());
  ro.observe(editor);
}

prevButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    if (sandbox.executionLimit) executionLimitNoticeDismissed = true;
    keepStepperAtEnd = false;
    sandbox.stepPosition = clampStepPosition(
      sandbox.stepPosition - 1,
      sandbox.traceLength,
    );
    saveSandboxState();
    renderStage();
    scrollEventIntoView(sandbox.trace[sandbox.stepPosition]);
  });
});

nextButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    if (sandbox.executionLimit) executionLimitNoticeDismissed = true;
    const extendsLimitedTrace =
      !!sandbox.executionLimit &&
      sandbox.executionLimit.reason !== "step-limit" &&
      sandbox.stepPosition >= sandbox.traceLength;
    if (extendsLimitedTrace) {
      execution.extend();
      keepStepperAtEnd = false;
      sandbox.stepPosition += 1;
      renderStage();
      scrollEventIntoView(sandbox.trace[sandbox.stepPosition - 1]);
      return;
    }
    const event = sandbox.trace[sandbox.stepPosition];
    if (event && event.file !== sandbox.activePath) {
      const file = sandbox.files.find((candidate) => candidate.path === event.file);
      if (file) {
        activeFile().source = editor.value;
        sandbox.activePath = file.path;
        editor.value = file.source;
        saveSandboxState();
      }
    }
    sandbox.stepPosition = clampStepPosition(
      sandbox.stepPosition + 1,
      sandbox.traceLength,
    );
    keepStepperAtEnd =
      sandbox.traceLength > 0 && sandbox.stepPosition >= sandbox.traceLength;
    saveSandboxState();
    renderStage();
    scrollEventIntoView(event);
  });
});

exprInput.addEventListener("input", () => {
  saveSandboxState();
  renderExpression({
    kind: sandbox.diagnostic ? sandbox.diagnostic.kind : "ok",
    state: sandbox.lastState,
    files: filesForRun(),
    eventIndex: sandbox.stepPosition > 0 ? sandbox.stepPosition - 1 : null,
    implicitMain: sandbox.effectiveImplicitMain,
    executionBudget: executionBudget(),
  });
});

function setInterfaceMode(mode: SandboxState["interfaceMode"]) {
  if (sandbox.interfaceMode === mode) return;
  resetExecutionPaging();
  activeFile().source = editor.value;
  if (
    mode === "simple" &&
    !sandbox.activePath.toLowerCase().endsWith(".c")
  ) {
    const firstCFile = sandbox.files.find((file) =>
      file.path.toLowerCase().endsWith(".c"),
    );
    if (firstCFile) {
      sandbox.activePath = firstCFile.path;
      editor.value = firstCFile.source;
    }
  }
  sandbox.interfaceMode = mode;
  sandbox.implicitMainNotice = "";
  closeFileCreator();
  closeDeleteConfirmation();
  saveSandboxState();
  renderStage();
}

simpleModeButton.addEventListener("click", () => setInterfaceMode("simple"));
advancedModeButton.addEventListener("click", () =>
  setInterfaceMode("advanced"),
);
implicitMainInput.addEventListener("change", () => {
  resetExecutionPaging();
  sandbox.implicitMain = implicitMainInput.checked;
  sandbox.implicitMainNotice = "";
  saveSandboxState();
  renderStage();
});

stepLimitInput.addEventListener("change", () => {
  execution.stepLimit = sandboxStepLimit(stepLimitInput.valueAsNumber);
  stepLimitInput.value = String(execution.stepLimit);
  executionLimitNoticeDismissed = false;
  keepStepperAtEnd = false;
  saveSandboxState();
  renderStage();
});

function closeFileCreator() {
  createFileControls.classList.add("hidden");
  addFileButton.classList.remove("hidden");
  deleteFileButton.classList.remove("hidden");
  setFileCreationError("");
}

function setFileCreationError(message: string) {
  const error = requiredRole<HTMLElement>("sandbox-file-error");
  error.textContent = message;
  error.classList.toggle("hidden", !message);
  fileNameInput.setAttribute("aria-invalid", String(!!message));
}

function closeDeleteConfirmation() {
  deleteFileControls.classList.add("hidden");
  addFileButton.classList.remove("hidden");
  deleteFileButton.classList.remove("hidden");
}

function createNamedFile() {
  const path = fileNameInput.value.trim().replace(/\\/g, "/");
  let error = "";
  if (
    !path ||
    path.startsWith("/") ||
    path.split("/").some((part) => !part || part === "." || part === "..")
  ) {
    error = "Use a relative file name such as helper.c or include/helper.h.";
  } else if (sandbox.files.some((file) => file.path === path)) {
    error = `${path} already exists.`;
  } else if (sandbox.implicitMain && path.endsWith(".c") && sandbox.files.some(file => file.path.endsWith(".c"))) {
    error = "Turn off Implicit main before adding another .c file. Header (.h) files are allowed.";
  }
  setFileCreationError(error);
  if (error) {
    return;
  }
  activeFile().source = editor.value;
  resetExecutionPaging();
  sandbox.files.push({ path, source: "" });
  sandbox.activePath = path;
  editor.value = "";
  closeFileCreator();
  saveSandboxState();
  renderStage();
  editor.focus();
}

addFileButton.addEventListener("click", () => {
  closeDeleteConfirmation();
  const extension = sandbox.implicitMain ? "h" : "c";
  let counter = 2;
  while (sandbox.files.some((file) => file.path === `file${counter}.${extension}`)) {
    counter += 1;
  }
  fileNameInput.value = `file${counter}.${extension}`;
  addFileButton.classList.add("hidden");
  deleteFileButton.classList.add("hidden");
  createFileControls.classList.remove("hidden");
  fileNameInput.focus();
  fileNameInput.select();
});
confirmFileButton.addEventListener("click", createNamedFile);
cancelFileButton.addEventListener("click", closeFileCreator);
fileNameInput.addEventListener("input", () => setFileCreationError(""));
fileNameInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    createNamedFile();
  } else if (event.key === "Escape") {
    event.preventDefault();
    closeFileCreator();
  }
});

deleteFileButton.addEventListener("click", () => {
  if (sandbox.files.length <= 1) return;
  deleteFileLabel.textContent = `Delete ${sandbox.activePath}?`;
  addFileButton.classList.add("hidden");
  deleteFileButton.classList.add("hidden");
  deleteFileControls.classList.remove("hidden");
});
cancelDeleteButton.addEventListener("click", closeDeleteConfirmation);
confirmDeleteButton.addEventListener("click", () => {
  resetExecutionPaging();
  const path = sandbox.activePath;
  const index = sandbox.files.findIndex((file) => file.path === path);
  sandbox.files.splice(index, 1);
  const next = sandbox.files[Math.max(0, index - 1)] ?? sandbox.files[0]!;
  sandbox.activePath = next.path;
  editor.value = next.source;
  closeDeleteConfirmation();
  saveSandboxState();
  renderStage();
});

updateInstructions();
renderStage();

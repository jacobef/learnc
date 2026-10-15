import { pageSyntheticAddressBase } from "./shared-c-address-space.js";
import { memoryDisplayState } from "./shared-program-state.js";
import { markInaccessibleName } from "./shared-workspace-dom.js";
import { boundaryTargets, canJumpToBoundary, defaultOccurrence, executionBoundaries, nearestOccurrence, previousVisibleStepPosition, stepDepth, stepDestination } from "./shared-sandbox-navigation.js";
import { stdinReadPosition } from "./shared-stdin-position.js";
import { bindPanelSplit } from "./shared-panel-split.js";
import { SandboxExecution, sandboxStepLimit, sandboxTimeline, sandboxDiagnosticAt, DEFAULT_SANDBOX_STEP_LIMIT } from "./shared-sandbox-execution.js";
import { executionLimitExplanation, renderCodeDiagnostic } from "./shared-code-runtime-issues.js";
import { readSandboxProgress, writeSandboxProgress } from "./shared-progress.js";
import { applyOtherNames, appendStateObjects, clearNode, ensureBaseLayout, findArrayObjectBoxesForResult, queryRole, renderExitStatus, vbox, } from "./shared-core-dom.js";
import { confettiRain } from "./confetti.js";
import { bindCodeEditorTabKey, ensureCodeSurfaceElements, updateCodeSurface, } from "./shared-code-editor-surface.js";
import { diagnosticDecorations, diagnosticMessageText, } from "./shared-diagnostics.js";
import { evaluateCExpressionFiles, } from "./shared-c-interpreter.js";
const { main } = ensureBaseLayout();
main.classList.add("main-panelized");
function requiredRole(name) {
    const element = queryRole(name);
    if (!element)
        throw new Error(`Missing required sandbox element: ${name}`);
    return element;
}
const instructions = requiredRole("sandbox-instructions");
const editor = requiredRole("sandbox-editor");
const lineNumbers = requiredRole("sandbox-line-numbers");
const fileBar = requiredRole("sandbox-file-bar");
const terminal = requiredRole("sandbox-terminal");
const stdinInput = requiredRole("sandbox-stdin");
const stdinHighlight = requiredRole("sandbox-stdin-highlight");
const stdinPositionLabel = requiredRole("sandbox-stdin-position");
const stdoutOutput = requiredRole("sandbox-stdout");
const stderrOutput = requiredRole("sandbox-stderr");
const stderrWrap = requiredRole("sandbox-stderr-wrap");
const simpleModeButton = requiredRole("sandbox-mode-simple");
const advancedModeButton = requiredRole("sandbox-mode-advanced");
const implicitMainControl = requiredRole("sandbox-implicit-main-control");
const implicitMainInput = requiredRole("sandbox-implicit-main");
const askOnAmbiguousJump = requiredRole("sandbox-ask-jump");
const jumpDialog = requiredRole("sandbox-jump-dialog");
const jumpTitle = requiredRole("sandbox-jump-title");
const jumpDescription = requiredRole("sandbox-jump-description");
const jumpChoices = requiredRole("sandbox-jump-choices");
const jumpForm = requiredRole("sandbox-jump-form");
const jumpOccurrence = requiredRole("sandbox-jump-occurrence");
let offeredJumpPositions = [];
let jumpReturnLabel = "";
const implicitMainNotice = requiredRole("sandbox-implicit-main-notice");
const exprInput = requiredRole("sandbox-expr");
const exprResult = requiredRole("sandbox-expr-result");
const stage = requiredRole("sandbox-stage");
const codePane = requiredRole("sandbox-code");
const prevButtons = [requiredRole("sandbox-prev")];
const nextButtons = [requiredRole("sandbox-next")];
const intoButton = requiredRole("sandbox-into");
const outButton = requiredRole("sandbox-out");
let stepping = false;
const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
bindCodeEditorTabKey(editor);
const STDIN_EOF_MARKER = "\u2404";
let finishedConfettiShown = false;
const boundaryLine = (() => {
    const row = editor.closest(".codepane-row");
    if (!(row instanceof HTMLElement))
        throw new Error("Missing sandbox code editor row.");
    const el = document.createElement("div");
    el.className = "code-boundary-line";
    el.setAttribute("aria-hidden", "true");
    row.appendChild(el);
    return el;
})();
const diagnosticEl = (() => {
    const existing = codePane.querySelector(".code-diagnostic");
    if (existing)
        return existing;
    const el = document.createElement("div");
    el.className = "code-diagnostic hidden";
    codePane.appendChild(el);
    return el;
})();
const fileTabs = requiredRole("sandbox-file-tabs");
const addFileButton = requiredRole("sandbox-file-add");
const deleteFileButton = requiredRole("sandbox-file-delete");
const createFileControls = requiredRole("sandbox-file-create");
const fileNameInput = requiredRole("sandbox-file-name");
const confirmFileButton = requiredRole("sandbox-file-confirm");
const cancelFileButton = requiredRole("sandbox-file-cancel");
const deleteFileControls = requiredRole("sandbox-file-delete-confirm");
const deleteFileLabel = requiredRole("sandbox-file-delete-label");
const confirmDeleteButton = requiredRole("sandbox-file-delete-yes");
const cancelDeleteButton = requiredRole("sandbox-file-delete-no");
function validStoredFile(file) {
    if (!file || typeof file !== "object")
        return false;
    const candidate = file;
    return (typeof candidate.path === "string" &&
        candidate.path.length > 0 &&
        typeof candidate.source === "string");
}
function validStringArray(value) {
    return Array.isArray(value)
        ? value.filter((item) => typeof item === "string")
        : undefined;
}
function validStepPosition(value) {
    if (typeof value !== "number" || !Number.isFinite(value))
        return undefined;
    return Math.max(0, Math.floor(value));
}
function loadStoredSandboxState() {
    try {
        const raw = readSandboxProgress();
        if (!raw)
            return {};
        const parsed = JSON.parse(raw);
        const files = Array.isArray(parsed.files)
            ? parsed.files.filter(validStoredFile)
            : undefined;
        const activePath = typeof parsed.activePath === "string" &&
            files?.some((file) => file.path === parsed.activePath)
            ? parsed.activePath
            : files?.[0]?.path;
        const interfaceMode = parsed.interfaceMode === "advanced" || parsed.interfaceMode === "simple"
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
    }
    catch {
        return {};
    }
}
const storedSandbox = loadStoredSandboxState();
let introductionDismissed = storedSandbox.introductionDismissed === true;
let codeShare = storedSandbox.codeShare ?? 1 / 2.3;
let keepStepperAtEnd = storedSandbox.stepPosition === undefined;
let shouldInferInitialEndPin = storedSandbox.stepPosition !== undefined;
let pendingStepAnchor = null;
let executionLimitNoticeDismissed = false;
const execution = new SandboxExecution(storedSandbox.stepLimit ?? DEFAULT_SANDBOX_STEP_LIMIT);
const stepLimitInput = requiredRole("sandbox-step-limit");
stepLimitInput.value = String(execution.stepLimit);
function executionBudget() {
    return execution.budget();
}
function resetExecutionPaging() {
    executionLimitNoticeDismissed = false;
    execution.reset();
}
const sandbox = {
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
bindPanelSplit(requiredRole("sandbox-panels"), requiredRole("sandbox-panel-divider"), codeShare, share => { codeShare = share; saveSandboxState(); });
function saveSandboxState() {
    try {
        writeSandboxProgress(JSON.stringify({
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
        }));
    }
    catch {
        // Losing local persistence should not break the sandbox itself.
    }
}
function getEditorText() {
    return editor.value;
}
function activeFile() {
    return (sandbox.files.find((file) => file.path === sandbox.activePath) ??
        sandbox.files[0]);
}
function filesForRun() {
    return sandbox.interfaceMode === "simple" ? [activeFile()] : sandbox.files;
}
function stdinForRun() {
    return sandbox.interfaceMode === "advanced" ? sandbox.stdin : "";
}
function implicitMainForRun() {
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
    requiredRole("sandbox-jump-setting").classList.toggle("hidden", simple);
    implicitMainControl.classList.toggle("hidden", simple);
    requiredRole("sandbox-step-limit-control").classList.toggle("hidden", simple);
    implicitMainInput.checked = sandbox.implicitMain;
    const multipleCFiles = sandbox.files.filter(file => file.path.endsWith(".c")).length > 1;
    implicitMainInput.disabled = multipleCFiles;
    requiredRole("sandbox-implicit-main-restriction").classList.toggle("hidden", !multipleCFiles);
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
function countNewlines(value) {
    let count = 0;
    for (let i = 0; i < value.length; i += 1) {
        if (value[i] === "\n")
            count += 1;
    }
    return count;
}
function diffSegments(prev, next) {
    let start = 0;
    const prevLen = prev.length;
    const nextLen = next.length;
    while (start < prevLen && start < nextLen && prev[start] === next[start]) {
        start += 1;
    }
    let endPrev = prevLen;
    let endNext = nextLen;
    while (endPrev > start &&
        endNext > start &&
        prev[endPrev - 1] === next[endNext - 1]) {
        endPrev -= 1;
        endNext -= 1;
    }
    return {
        start,
        oldSegment: prev.slice(start, endPrev),
        newSegment: next.slice(start, endNext),
    };
}
function clampStepPosition(value, traceLength) {
    return Math.max(0, Math.min(traceLength, Math.floor(value)));
}
function sourceLinesForFile(path) {
    const file = sandbox.files.find((candidate) => candidate.path === path);
    return (file?.source ?? "").split(/\r?\n/);
}
function textForTraceEvent(event) {
    return sourceLinesForFile(event.file)
        .slice(event.startLine, event.endLine + 1)
        .join("\n")
        .trim();
}
function mapLineThroughEdit(prevText, nextText, line) {
    const { start, oldSegment, newSegment } = diffSegments(prevText, nextText);
    const changeLine = countNewlines(prevText.slice(0, start));
    if (changeLine > line)
        return line;
    const delta = countNewlines(newSegment) - countNewlines(oldSegment);
    if (changeLine < line)
        return Math.max(0, line + delta);
    if (delta > 0 && oldSegment === "")
        return Math.max(0, line + delta);
    return line;
}
function captureStepAnchorForEdit(prevText, nextText) {
    if (keepStepperAtEnd || prevText === nextText)
        return null;
    const nextEvent = sandbox.trace[sandbox.stepPosition];
    if (!nextEvent || nextEvent.file !== sandbox.activePath)
        return null;
    const occurrence = sandbox.trace
        .slice(0, sandbox.stepPosition)
        .filter((event) => event.file === nextEvent.file &&
        event.kind === nextEvent.kind &&
        event.startLine === nextEvent.startLine).length;
    return {
        file: nextEvent.file,
        kind: nextEvent.kind,
        mappedStartLine: mapLineThroughEdit(prevText, nextText, nextEvent.startLine),
        occurrence,
        text: textForTraceEvent(nextEvent),
    };
}
function tracePositionForOccurrence(trace, occurrence, matches) {
    let seen = 0;
    let lastMatch = null;
    for (let index = 0; index < trace.length; index += 1) {
        if (!matches(trace[index]))
            continue;
        lastMatch = index;
        if (seen === occurrence)
            return index;
        seen += 1;
    }
    return lastMatch;
}
function positionForStepAnchor(anchor, trace) {
    const exactLine = tracePositionForOccurrence(trace, anchor.occurrence, (event) => event.file === anchor.file &&
        event.kind === anchor.kind &&
        event.startLine === anchor.mappedStartLine);
    if (exactLine != null)
        return exactLine;
    if (anchor.text) {
        const textMatch = tracePositionForOccurrence(trace, anchor.occurrence, (event) => event.file === anchor.file &&
            event.kind === anchor.kind &&
            textForTraceEvent(event) === anchor.text);
        if (textMatch != null)
            return textMatch;
    }
    const sameFileAfterLine = trace.findIndex((event) => event.file === anchor.file && event.startLine > anchor.mappedStartLine);
    return sameFileAfterLine >= 0 ? sameFileAfterLine : null;
}
function updateInstructions() {
    requiredRole("sandbox-introduction").classList.toggle("hidden", introductionDismissed);
    if (introductionDismissed)
        return;
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
    }
    else {
        instructions.textContent = message;
    }
}
requiredRole("sandbox-dismiss-introduction").addEventListener("click", () => {
    introductionDismissed = true;
    saveSandboxState();
    updateInstructions();
});
function outcomeForPosition(result, position, files) {
    if (result.kind !== "ok") {
        return {
            kind: result.kind,
            globalKind: result.kind,
            state: null,
            trace: [],
            stdout: "",
            stderr: "",
            stdinPosition: null,
        };
    }
    const trace = result.trace || [];
    const safePosition = clampStepPosition(position, trace.length);
    const diagnostic = sandboxDiagnosticAt(result, safePosition);
    const current = safePosition > 0 ? trace[safePosition - 1] : undefined;
    const blocked = safePosition >= trace.length ? result.blocked : null;
    const executionLimit = !executionLimitNoticeDismissed &&
        result.executionLimit &&
        result.executionLimit.reason !== "trace-limit" &&
        safePosition === result.executionLimit.tracePosition
        ? result.executionLimit
        : null;
    return {
        kind: diagnostic?.kind ?? "ok",
        globalKind: diagnostic?.kind ?? (blocked
            ? "blocked"
            : executionLimit
                ? "execution-limit"
                : "ok"),
        state: diagnostic ? null : blocked?.state ??
            (current ? current.state : executionLimit ? result.state : result.initialState),
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
function renderStdinPosition(position) {
    stdinHighlight.replaceChildren();
    if (position === null) {
        stdinHighlight.textContent = stdinInput.value + "\u200b";
        stdinPositionLabel.textContent = "Read position unavailable while the program has an error.";
    }
    else {
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
function renderState(title, boxes, status = "ok", blocked = null, executionLimit = null) {
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
    if (status !== "ok" &&
        status !== "blocked" &&
        status !== "execution-limit") {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        msg.textContent = status === "ub"
            ? "Execution stopped here: undefined behavior."
            : "(the program does not currently run)";
        grid.appendChild(msg);
    }
    else if (!boxes || boxes.length === 0) {
        const msg = document.createElement("div");
        msg.className = "muted";
        msg.style.padding = "8px";
        msg.textContent = "(no variables yet)";
        grid.appendChild(msg);
    }
    else {
        appendStateObjects(grid, boxes, { editable: false, deletable: false });
    }
    wrap.appendChild(grid);
    return wrap;
}
function renderExpression(outcome) {
    clearNode(exprResult);
    exprResult.classList.add("hidden");
    const expr = exprInput.value.trim();
    if (!expr)
        return;
    if (outcome.kind !== "ok") {
        renderExpressionError("Expression unavailable", "Fix the program error before evaluating an expression.");
        return;
    }
    const evaluated = evaluateCExpressionFiles(outcome.files || sandbox.files, outcome.eventIndex ?? -1, expr, undefined, stdinForRun(), outcome.implicitMain ?? implicitMainForRun(), outcome.executionBudget ?? executionBudget());
    if (evaluated.kind !== "ok") {
        renderExpressionError(evaluated.kind === "ub"
            ? "Invalid expression: undefined behavior"
            : "Invalid expression", diagnosticMessageText(evaluated.diagnostic));
        return;
    }
    const { result } = evaluated;
    const arrayBoxes = findArrayObjectBoxesForResult(result, outcome.state || []);
    if (arrayBoxes && arrayBoxes.length) {
        const wrap = document.createElement("div");
        appendStateObjects(wrap, arrayBoxes, { editable: false, deletable: false });
        const arrayNode = wrap.querySelector(".arraybox");
        if (arrayNode) {
            exprResult.appendChild(arrayNode);
            exprResult.classList.remove("hidden");
            return;
        }
    }
    const resultName = String(result.name ?? "").trim();
    const resultIsArray = result.typeInfo.kind === "array";
    const displayedState = memoryDisplayState(outcome.state || []);
    const match = result.kind === "lvalue"
        ? displayedState.find((box) => {
            if (String(box.address) !== String(result.address))
                return false;
            const boxIsArrayRoot = !box.arrayRoot && box.typeInfo?.kind === "array";
            return resultIsArray || !boxIsArrayRoot;
        }) || displayedState.find(box => box.nameAccessible !== false && box.name === resultName)
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
    markInaccessibleName(node, match);
    if (!match && !result.address)
        node.classList.add("no-addr");
    if (!match)
        node.classList.add("no-name");
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
function renderExpressionError(title, message) {
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
function switchToFile(path) {
    const file = sandbox.files.find((candidate) => candidate.path === path);
    if (!file || path === sandbox.activePath)
        return;
    closeDeleteConfirmation();
    resetExecutionPaging();
    activeFile().source = editor.value;
    sandbox.activePath = path;
    editor.value = file.source;
    saveSandboxState();
    renderFileTabs();
    renderStage();
}
let discoveryWorker = null;
let discoveryKey = "";
let discoveredBoundaries = [];
function discoverJumpTargets(files, stdin, implicitMain, result) {
    const key = JSON.stringify([files, stdin, implicitMain, execution.stepLimit, sandbox.interfaceMode]);
    if (key === discoveryKey)
        return;
    discoveryKey = key;
    discoveryWorker?.terminate();
    discoveryWorker = null;
    discoveredBoundaries = [];
    if (sandbox.interfaceMode !== "advanced" || result.kind !== "ok" || result.executionLimit?.reason !== "trace-limit")
        return;
    const worker = new Worker(new URL("./shared-sandbox-discovery-worker.js", import.meta.url), { type: "module" });
    discoveryWorker = worker;
    worker.onmessage = ({ data }) => {
        if (discoveryWorker !== worker)
            return;
        discoveredBoundaries = data.boundaries;
        updateLineGutters();
        if (!data.more) {
            worker.terminate();
            discoveryWorker = null;
        }
    };
    worker.onerror = () => {
        worker.terminate();
        if (discoveryWorker === worker)
            discoveryWorker = null;
    };
    worker.postMessage({ files, stdin, implicitMain, stepLimit: execution.stepLimit, addressBase: pageSyntheticAddressBase });
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
    const result = sandboxTimeline(execution.run(runFiles, stdinForRun(), implicitMainRequested));
    discoverJumpTargets(runFiles, stdinForRun(), implicitMainRequested, result);
    sandbox.effectiveImplicitMain =
        result.implicitMainApplied ?? implicitMainRequested;
    if (result.implicitMainNotice) {
        sandbox.implicitMainNotice = result.implicitMainNotice;
    }
    else if (implicitMainRequested) {
        sandbox.implicitMainNotice = "";
    }
    if (result.kind === "ok") {
        sandbox.trace = result.trace;
        sandbox.mainClose = result.mainClose;
        sandbox.blocked = result.blocked;
        sandbox.executionLimit = result.executionLimit;
        sandbox.traceLength = sandbox.trace.length;
    }
    else {
        sandbox.trace = [];
        sandbox.traceLength = 0;
        sandbox.mainClose = null;
        sandbox.blocked = null;
        sandbox.executionLimit = null;
    }
    if (result.kind === "ok" &&
        programIsBlank &&
        previousTraceLength > 0) {
        keepStepperAtEnd = previousStepPosition > 0;
    }
    if (result.kind === "ok" && result.executionLimit?.reason === "trace-limit")
        keepStepperAtEnd = false;
    sandbox.stepPosition = keepStepperAtEnd
        ? sandbox.traceLength
        : clampStepPosition(sandbox.stepPosition, sandbox.traceLength);
    if (!keepStepperAtEnd && result.kind === "ok" && pendingStepAnchor) {
        const anchoredPosition = positionForStepAnchor(pendingStepAnchor, sandbox.trace);
        if (anchoredPosition != null) {
            sandbox.stepPosition = clampStepPosition(anchoredPosition, sandbox.traceLength);
        }
        else if (result.executionLimit) {
            sandbox.stepPosition = result.executionLimit.tracePosition;
        }
    }
    if (result.kind === "ok" &&
        result.executionLimit &&
        result.executionLimit.reason !== "trace-limit" &&
        !executionLimitNoticeDismissed) {
        keepStepperAtEnd = false;
        sandbox.stepPosition = clampStepPosition(result.executionLimit.tracePosition, sandbox.traceLength);
    }
    if (result.kind === "ok") {
        pendingStepAnchor = null;
        if (sandbox.stepPosition < sandbox.traceLength) {
            sandbox.stepPosition = previousVisibleStepPosition(sandbox.trace, sandbox.stepPosition + 1);
        }
    }
    if (shouldInferInitialEndPin &&
        result.kind === "ok" &&
        sandbox.traceLength > 0) {
        keepStepperAtEnd = sandbox.stepPosition >= sandbox.traceLength;
        shouldInferInitialEndPin = false;
    }
    sandbox.diagnostic = sandboxDiagnosticAt(result, sandbox.stepPosition);
    const outcome = outcomeForPosition(result, sandbox.stepPosition, runFiles);
    stdoutOutput.textContent = outcome.stdout;
    stderrOutput.textContent = outcome.stderr;
    renderStdinPosition(outcome.stdinPosition);
    stderrWrap.classList.toggle("hidden", !outcome.stderr);
    sandbox.lastState = outcome.state;
    const statePanel = renderState("", outcome.state, outcome.globalKind, outcome.blocked ?? null, outcome.executionLimit ?? null);
    // Once a program with a visible main has ended, say how.
    if (result.kind === "ok" &&
        result.endedBy &&
        outcome.globalKind === "ok" &&
        !sandbox.effectiveImplicitMain &&
        sandbox.stepPosition >= sandbox.traceLength) {
        statePanel.appendChild(renderExitStatus(result.exitStatus, result.endedBy));
    }
    stage.appendChild(statePanel);
    refreshOtherNames();
    renderExpression(outcome);
    updateStepperControls(sandbox.stepPosition, sandbox.trace, outcome.blocked ?? null, sandbox.executionLimit, !!sandbox.diagnostic);
    renderFileTabs();
    renderInterfaceControls();
    updateInstructions();
    updateLineGutters();
    saveSandboxState();
}
function runLabelForPosition(position, trace) {
    const next = trace[position];
    if (!next)
        return "At end ▶";
    const filePrefix = next.file === sandbox.activePath ? "" : `${next.file}:`;
    const start = next.startLine + 1;
    const end = next.endLine + 1;
    return start === end
        ? `Run ${filePrefix}${start} ▶`
        : `Run ${filePrefix}${start}-${end} ▶`;
}
function updateStepperControls(position, trace, blocked, executionLimit, hasError = false) {
    const entersFunction = trace[position]?.kind === "call";
    const canExtendTrace = !!executionLimit && executionLimit.reason !== "step-limit" && position >= trace.length;
    intoButton.classList.toggle("hidden", !entersFunction);
    outButton.classList.toggle("hidden", stepDepth(trace, position) === 0);
    intoButton.disabled = stepping || hasError || !entersFunction;
    outButton.disabled = stepping || hasError || (position >= trace.length && !canExtendTrace);
    prevButtons.forEach((btn) => {
        btn.disabled = stepping || position <= 0;
    });
    nextButtons.forEach((btn) => {
        btn.textContent =
            hasError && sandbox.diagnostic?.kind === "ub"
                ? "Stopped"
                : blocked && position >= trace.length
                    ? `Waiting for ${blocked.function}`
                    : executionLimit &&
                        position >= trace.length
                        ? executionLimit.reason === "step-limit" ? "Step limit reached"
                            : executionLimit.reason === "trace-limit" ? "Continue execution ▶" : "Continue loop ▶"
                        : entersFunction ? "Step over ▶" : runLabelForPosition(position, trace);
        btn.disabled = stepping || hasError || (position >= trace.length && !canExtendTrace);
    });
}
function sandboxBoundaries() {
    const endFile = sandbox.files.find(file => file.path === sandbox.mainClose?.file)
        ?? filesForRun()[0];
    return executionBoundaries(sandbox.trace, sandbox.mainClose, sandbox.blocked, {
        file: endFile.path,
        line: endFile.source.split(/\r?\n/).length,
    }, sandbox.executionLimit !== null);
}
function jumpToPosition(position) {
    execution.includePosition(position);
    executionLimitNoticeDismissed = true;
    keepStepperAtEnd = false;
    sandbox.stepPosition = position;
    saveSandboxState();
    renderStage();
    const button = Array.from(lineNumbers.querySelectorAll(".sandbox-line-jump"))
        .find(button => button.getAttribute("aria-label") === jumpReturnLabel);
    // The selected target may disappear after jumping. Keep keyboard arrows on
    // navigation rather than transferring them to the editor's text cursor.
    (button ?? [...nextButtons, ...prevButtons].find(control => !control.disabled))?.focus({ preventScroll: true });
    scrollEventIntoView(sandbox.trace[position] ?? sandbox.trace[position - 1]);
}
function offerBoundaryJump(label, positions, first = false) {
    jumpReturnLabel = label;
    const target = first ? positions[0] : defaultOccurrence(positions, sandbox.stepPosition);
    if (target === undefined || target === sandbox.stepPosition)
        return;
    if (first || !askOnAmbiguousJump.checked || positions.length === 1) {
        jumpToPosition(target);
        return;
    }
    offeredJumpPositions = positions;
    jumpTitle.textContent = label;
    const current = positions.indexOf(sandbox.stepPosition);
    jumpDescription.textContent = `This boundary occurs ${positions.length} times in the available execution.${current >= 0 ? ` You are at occurrence ${current + 1}.` : ""}`;
    clearNode(jumpChoices);
    const choices = [
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
            if (position !== undefined)
                jumpToPosition(position);
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
    if (position !== undefined)
        jumpToPosition(position);
});
requiredRole("sandbox-jump-cancel").addEventListener("click", () => jumpDialog.close());
jumpDialog.addEventListener("click", event => {
    if (event.target !== jumpDialog)
        return;
    const rect = jumpDialog.getBoundingClientRect();
    if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom)
        jumpDialog.close();
});
function updateLineGutters(linesOverride) {
    const lines = linesOverride ?? getRawLines();
    const count = Math.max(lines.length, 1);
    const diagnostic = sandbox.diagnostic;
    const lineClasses = new Map();
    for (const event of sandbox.trace.slice(0, sandbox.stepPosition)) {
        if (event.file !== sandbox.activePath)
            continue;
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
    const lineNumberClasses = new Map();
    if (diagnostic &&
        (!diagnostic.file || diagnostic.file === sandbox.activePath)) {
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
        for (const { line, positions, kind, paired } of boundaryTargets(jumpBoundaries, sandbox.activePath)) {
            const canJump = kind === "first" ? positions[0] !== sandbox.stepPosition
                : canJumpToBoundary(positions, sandbox.stepPosition, askOnAmbiguousJump.checked);
            if (line < 0 || line > count || !canJump)
                continue;
            const row = lineNumbers.children[Math.min(line, count - 1)];
            if (!(row instanceof HTMLElement))
                continue;
            const button = document.createElement("button");
            button.type = "button";
            button.className = "sandbox-line-jump";
            if (line === count)
                button.classList.add("is-end");
            const label = paired
                ? `${kind === "first" ? "First" : "Nearest"} occurrence of line ${line + 1}`
                : line === count ? "Jump to end" : `Jump before line ${line + 1}`;
            if (paired)
                button.classList.add(kind === "first" ? "is-loop-first" : "is-loop-nearest");
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
function scrollEventIntoView(event) {
    if (!event || event.file !== sandbox.activePath)
        return;
    const scroller = codePane;
    if (event.kind === "ub" && sandbox.diagnostic) {
        const scrollerRect = scroller.getBoundingClientRect();
        const diagnosticRect = diagnosticEl.getBoundingClientRect();
        scroller.scrollTop += Math.max(0, Math.min(diagnosticRect.top - scrollerRect.top - 8, diagnosticRect.bottom - scrollerRect.bottom + 8));
        return;
    }
    const startNode = lineNumbers.children[event.startLine];
    const endNode = lineNumbers.children[event.endLine];
    if (!startNode || !endNode)
        return;
    const scrollerRect = scroller.getBoundingClientRect();
    const startRect = startNode.getBoundingClientRect();
    const endRect = endNode.getBoundingClientRect();
    const top = scrollerRect.top + 8;
    const bottom = scrollerRect.bottom - 8;
    if (startRect.top < top) {
        scroller.scrollTop += startRect.top - top;
    }
    else if (endRect.bottom > bottom) {
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
    if (event.ctrlKey &&
        !event.altKey &&
        !event.metaKey &&
        !event.shiftKey &&
        event.key.toLowerCase() === "d") {
        event.preventDefault();
        stdinInput.setRangeText(STDIN_EOF_MARKER, stdinInput.selectionStart, stdinInput.selectionEnd, "end");
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
        if (sandbox.executionLimit)
            executionLimitNoticeDismissed = true;
        keepStepperAtEnd = false;
        sandbox.stepPosition = previousVisibleStepPosition(sandbox.trace, sandbox.stepPosition);
        saveSandboxState();
        renderStage();
        scrollEventIntoView(sandbox.trace[sandbox.stepPosition]);
    });
});
async function advanceStep(mode) {
    if (stepping)
        return;
    const position = sandbox.stepPosition;
    const depth = stepDepth(sandbox.trace, position);
    const sourceKey = () => JSON.stringify([filesForRun(), stdinForRun(), implicitMainForRun(), execution.stepLimit]);
    const key = sourceKey();
    stepping = true;
    keepStepperAtEnd = false;
    executionLimitNoticeDismissed = true;
    try {
        if (position >= sandbox.traceLength && sandbox.executionLimit && sandbox.executionLimit.reason !== "step-limit") {
            execution.extend();
            renderStage();
        }
        while (true) {
            const target = stepDestination(sandbox.trace, position, mode, depth);
            if (target !== null) {
                sandbox.stepPosition = target;
                break;
            }
            if (sandbox.executionLimit?.reason !== "trace-limit") {
                sandbox.stepPosition = sandbox.traceLength;
                break;
            }
            const previousLength = sandbox.traceLength;
            execution.extend();
            renderStage();
            // Keep editing responsive while a long function's trace is extended.
            await new Promise(resolve => setTimeout(resolve, 0));
            if (sourceKey() !== key)
                return;
            if (sandbox.traceLength <= previousLength)
                break;
        }
        const event = sandbox.trace[sandbox.stepPosition] ?? sandbox.trace[sandbox.stepPosition - 1];
        if (event && event.file !== sandbox.activePath) {
            const file = sandbox.files.find(candidate => candidate.path === event.file);
            if (file) {
                activeFile().source = editor.value;
                sandbox.activePath = file.path;
                editor.value = file.source;
            }
        }
        keepStepperAtEnd = sandbox.traceLength > 0 && sandbox.stepPosition >= sandbox.traceLength;
        saveSandboxState();
    }
    finally {
        stepping = false;
        renderStage();
        scrollEventIntoView(sandbox.trace[sandbox.stepPosition] ?? sandbox.trace[sandbox.stepPosition - 1]);
    }
}
nextButtons.forEach(btn => btn.addEventListener("click", () => { void advanceStep("over"); }));
intoButton.addEventListener("click", () => { void advanceStep("into"); });
outButton.addEventListener("click", () => { void advanceStep("out"); });
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
function setInterfaceMode(mode) {
    if (sandbox.interfaceMode === mode)
        return;
    resetExecutionPaging();
    activeFile().source = editor.value;
    if (mode === "simple" &&
        !sandbox.activePath.toLowerCase().endsWith(".c")) {
        const firstCFile = sandbox.files.find((file) => file.path.toLowerCase().endsWith(".c"));
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
advancedModeButton.addEventListener("click", () => setInterfaceMode("advanced"));
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
function setFileCreationError(message) {
    const error = requiredRole("sandbox-file-error");
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
    if (!path ||
        path.startsWith("/") ||
        path.split("/").some((part) => !part || part === "." || part === "..")) {
        error = "Use a relative file name such as helper.c or include/helper.h.";
    }
    else if (sandbox.files.some((file) => file.path === path)) {
        error = `${path} already exists.`;
    }
    else if (sandbox.implicitMain && path.endsWith(".c") && sandbox.files.some(file => file.path.endsWith(".c"))) {
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
    }
    else if (event.key === "Escape") {
        event.preventDefault();
        closeFileCreator();
    }
});
deleteFileButton.addEventListener("click", () => {
    if (sandbox.files.length <= 1)
        return;
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
    const next = sandbox.files[Math.max(0, index - 1)] ?? sandbox.files[0];
    sandbox.activePath = next.path;
    editor.value = next.source;
    closeDeleteConfirmation();
    saveSandboxState();
    renderStage();
});
updateInstructions();
renderStage();

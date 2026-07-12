import { applyOtherNames, appendStateObjects, clearNode, ensureBaseLayout, findArrayObjectBoxesForResult, queryRole, vbox, } from "./shared-core-dom.js";
import { formatValueForType, parseType, } from "./shared-core-utils.js";
import { confettiRain } from "./confetti.js";
import { bindCodeEditorTabKey, ensureCodeSurfaceElements, updateCodeSurface, } from "./shared-code-editor-surface.js";
import { evaluateCExpressionFiles, runCFiles, } from "./shared-c-interpreter.js";
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
const stdoutOutput = requiredRole("sandbox-stdout");
const stderrOutput = requiredRole("sandbox-stderr");
const stderrWrap = requiredRole("sandbox-stderr-wrap");
const simpleModeButton = requiredRole("sandbox-mode-simple");
const advancedModeButton = requiredRole("sandbox-mode-advanced");
const implicitMainControl = requiredRole("sandbox-implicit-main-control");
const implicitMainInput = requiredRole("sandbox-implicit-main");
const implicitMainNotice = requiredRole("sandbox-implicit-main-notice");
const exprInput = requiredRole("sandbox-expr");
const exprResult = requiredRole("sandbox-expr-result");
const stage = requiredRole("sandbox-stage");
const codePane = requiredRole("sandbox-code");
const prevButtons = [requiredRole("sandbox-prev")];
const nextButtons = [requiredRole("sandbox-next")];
const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
bindCodeEditorTabKey(editor);
const STDIN_EOF_MARKER = "\u2404";
const SANDBOX_STORAGE_KEY = "cboxes:sandbox-state:v1";
const EXECUTION_PAGE_STEP_LIMIT = 10000;
const EXECUTION_PAGE_TRACE_LIMIT = 256;
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
function readWindowNameStorage() {
    try {
        const parsed = JSON.parse(window.name || "{}");
        const value = parsed[SANDBOX_STORAGE_KEY];
        return typeof value === "string" ? value : null;
    }
    catch {
        return null;
    }
}
function writeWindowNameStorage(value) {
    try {
        let parsed = {};
        try {
            parsed = JSON.parse(window.name || "{}");
        }
        catch {
            parsed = {};
        }
        parsed[SANDBOX_STORAGE_KEY] = value;
        window.name = JSON.stringify(parsed);
    }
    catch {
        // This is only a fallback for browsers that do not expose localStorage.
    }
}
function readSandboxStorage() {
    try {
        const storage = window.localStorage;
        return storage ? storage.getItem(SANDBOX_STORAGE_KEY) : readWindowNameStorage();
    }
    catch {
        return readWindowNameStorage();
    }
}
function writeSandboxStorage(value) {
    try {
        const storage = window.localStorage;
        if (storage) {
            storage.setItem(SANDBOX_STORAGE_KEY, value);
        }
        else {
            writeWindowNameStorage(value);
        }
    }
    catch {
        writeWindowNameStorage(value);
    }
}
function loadStoredSandboxState() {
    try {
        const raw = readSandboxStorage();
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
            files: files && files.length > 0 ? files : undefined,
            activePath,
            interfaceMode,
            implicitMain: typeof parsed.implicitMain === "boolean" ? parsed.implicitMain : undefined,
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
let keepStepperAtEnd = storedSandbox.stepPosition === undefined;
let shouldInferInitialEndPin = storedSandbox.stepPosition !== undefined;
let pendingStepAnchor = null;
let executionLimitNoticeDismissed = false;
let executionLimitPage = 1;
function executionBudget() {
    return {
        stepLimit: EXECUTION_PAGE_STEP_LIMIT * executionLimitPage,
        followingTraceLimit: EXECUTION_PAGE_TRACE_LIMIT * executionLimitPage,
    };
}
function resetExecutionPaging() {
    executionLimitNoticeDismissed = false;
    executionLimitPage = 1;
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
function saveSandboxState() {
    try {
        writeSandboxStorage(JSON.stringify({
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
    implicitMainControl.classList.toggle("hidden", simple);
    implicitMainInput.checked = sandbox.implicitMain;
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
function diagnosticDecoration(diagnostic) {
    if (!diagnostic || (diagnostic.file && diagnostic.file !== sandbox.activePath))
        return [];
    return [
        {
            line: diagnostic.range.startLine,
            startCol: diagnostic.range.startCol,
            endCol: diagnostic.range.endCol,
            className: "code-highlight-error",
        },
    ];
}
function renderDiagnostic(diagnostic) {
    if (!diagnostic) {
        diagnosticEl.classList.add("hidden");
        diagnosticEl.textContent = "";
        editor.removeAttribute("aria-invalid");
        return;
    }
    diagnosticEl.classList.remove("hidden");
    diagnosticEl.replaceChildren();
    const heading = document.createElement("div");
    heading.className = "code-diagnostic-title";
    const location = diagnostic.file
        ? `${diagnostic.file}, line ${diagnostic.range.startLine + 1}, column ${diagnostic.range.startCol + 1}`
        : `line ${diagnostic.range.startLine + 1}, column ${diagnostic.range.startCol + 1}`;
    heading.textContent = `${diagnostic.kind === "ub" ? "Undefined behavior" : "Error"} at ${location}`;
    const message = document.createElement("div");
    message.className = "code-diagnostic-message";
    message.textContent = diagnostic.message;
    diagnosticEl.append(heading, message);
    if (!diagnostic.file || diagnostic.file === sandbox.activePath) {
        editor.setAttribute("aria-invalid", "true");
    }
    else {
        editor.removeAttribute("aria-invalid");
    }
}
function outcomeForPosition(result, position, files) {
    if (result.kind !== "ok") {
        return {
            kind: result.kind,
            globalKind: result.kind,
            state: null,
            trace: [],
            stdout: "",
            stderr: "",
        };
    }
    const trace = result.trace || [];
    const safePosition = clampStepPosition(position, trace.length);
    const current = safePosition > 0 ? trace[safePosition - 1] : undefined;
    const blocked = safePosition >= trace.length ? result.blocked : null;
    const executionLimit = !executionLimitNoticeDismissed &&
        result.executionLimit &&
        safePosition === result.executionLimit.tracePosition
        ? result.executionLimit
        : null;
    return {
        kind: "ok",
        globalKind: blocked
            ? "blocked"
            : executionLimit
                ? "execution-limit"
                : "ok",
        state: blocked?.state ??
            (current ? current.state : executionLimit ? result.state : []),
        files,
        eventIndex: current ? safePosition - 1 : null,
        implicitMain: sandbox.effectiveImplicitMain,
        trace,
        blocked,
        executionLimit,
        executionBudget: executionBudget(),
        stdout: result.stdout,
        stderr: result.stderr,
    };
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
        notice.textContent =
            `Execution stopped after 10,000 steps because the program appears to repeat indefinitely near ${location}. ` +
                "The stepper was moved to before that repeated section ran.";
        wrap.appendChild(notice);
    }
    if (status !== "ok" &&
        status !== "blocked" &&
        status !== "execution-limit") {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        msg.textContent = "(the program does not currently run)";
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
    if (outcome.eventIndex == null) {
        renderExpressionError("Expression unavailable", "Run at least one program step before evaluating an expression.");
        return;
    }
    const evaluated = evaluateCExpressionFiles(outcome.files || sandbox.files, outcome.eventIndex, expr, undefined, stdinForRun(), outcome.implicitMain ?? implicitMainForRun(), outcome.executionBudget ?? executionBudget());
    if (evaluated.kind !== "ok") {
        renderExpressionError(evaluated.kind === "ub"
            ? "Invalid expression: undefined behavior"
            : "Invalid expression", evaluated.diagnostic.message);
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
    const resultIsArray = !!parseType(result.type || "int").arrayDims?.length;
    const match = result.kind === "lvalue"
        ? (resultName
            ? (outcome.state || []).find((box) => box.name === resultName)
            : null) ||
            (outcome.state || []).find((box) => {
                if (String(box.address) !== String(result.address))
                    return false;
                const boxIsArrayRoot = !box.arrayRoot && !!parseType(box.type || "int").arrayDims?.length;
                return resultIsArray || !boxIsArrayRoot;
            })
        : null;
    const node = match
        ? vbox({
            address: match.address ?? undefined,
            type: match.type,
            value: match.value,
            name: match.name,
            editable: false,
        })
        : vbox({
            address: result.address ? String(result.address) : "—",
            type: result.type || "int",
            value: formatValueForType(result.value ?? "", result.type || "int", {
                nanSign: result.nanSign,
            }),
            name: "",
            editable: false,
        });
    if (match && (match.value ?? "") === "") {
        node.querySelector(".value")?.classList.add("placeholder", "muted");
    }
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
function renderStage() {
    clearNode(stage);
    const previousTraceLength = sandbox.traceLength;
    const previousStepPosition = sandbox.stepPosition;
    const runFiles = filesForRun();
    const programIsBlank = runFiles.every((file) => file.source.trim() === "");
    const implicitMainRequested = implicitMainForRun();
    const result = runCFiles(runFiles, undefined, stdinForRun(), implicitMainRequested, executionBudget());
    sandbox.effectiveImplicitMain =
        result.implicitMainApplied ?? implicitMainRequested;
    if (result.implicitMainNotice) {
        sandbox.implicitMainNotice = result.implicitMainNotice;
    }
    else if (implicitMainRequested) {
        sandbox.implicitMainNotice = "";
    }
    sandbox.diagnostic = result.kind === "ok" ? null : result.diagnostic;
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
        !executionLimitNoticeDismissed) {
        keepStepperAtEnd = false;
        sandbox.stepPosition = clampStepPosition(result.executionLimit.tracePosition, sandbox.traceLength);
    }
    if (result.kind === "ok") {
        pendingStepAnchor = null;
    }
    if (shouldInferInitialEndPin &&
        result.kind === "ok" &&
        sandbox.traceLength > 0) {
        keepStepperAtEnd = sandbox.stepPosition >= sandbox.traceLength;
        shouldInferInitialEndPin = false;
    }
    const outcome = outcomeForPosition(result, sandbox.stepPosition, runFiles);
    stdoutOutput.textContent = outcome.stdout;
    stderrOutput.textContent = outcome.stderr;
    stderrWrap.classList.toggle("hidden", !outcome.stderr);
    sandbox.lastState = outcome.state;
    stage.appendChild(renderState("", outcome.state, outcome.globalKind, outcome.blocked ?? null, outcome.executionLimit ?? null));
    refreshOtherNames();
    renderExpression(outcome);
    updateStepperControls(sandbox.stepPosition, sandbox.trace, outcome.blocked ?? null, sandbox.executionLimit, result.kind !== "ok");
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
    prevButtons.forEach((btn) => {
        btn.disabled = position <= 0;
    });
    nextButtons.forEach((btn) => {
        btn.textContent =
            blocked && position >= trace.length
                ? `Waiting for ${blocked.function}`
                : executionLimit &&
                    executionLimit.tracePosition < trace.length &&
                    position >= trace.length
                    ? "Continue loop ▶"
                    : executionLimit && position >= trace.length
                        ? "Possible infinite loop"
                        : runLabelForPosition(position, trace);
        const canExtendTrace = !!executionLimit &&
            executionLimit.tracePosition < trace.length &&
            position >= trace.length;
        btn.disabled = hasError || (position >= trace.length && !canExtendTrace);
    });
}
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
        decorations: diagnosticDecoration(diagnostic),
    });
    renderDiagnostic(diagnostic);
    const style = window.getComputedStyle(editor);
    const paddingTop = parseFloat(style.paddingTop) || 0;
    const nextEvent = sandbox.trace[sandbox.stepPosition];
    const blocked = sandbox.stepPosition >= sandbox.trace.length ? sandbox.blocked : null;
    const lastEvent = sandbox.trace[sandbox.trace.length - 1];
    const terminalMainClose = !nextEvent &&
        sandbox.mainClose?.file === sandbox.activePath &&
        (!lastEvent ||
            lastEvent.file !== sandbox.activePath ||
            sandbox.mainClose.line > lastEvent.endLine)
        ? sandbox.mainClose.line
        : null;
    const boundaryLineIndex = nextEvent?.file === sandbox.activePath
        ? nextEvent.startLine
        : blocked?.file === sandbox.activePath
            ? blocked.startLine
            : !executionLimitNoticeDismissed &&
                sandbox.executionLimit?.file === sandbox.activePath
                ? sandbox.executionLimit.startLine
                : terminalMainClose ?? count;
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
    const scroller = codePane.closest(".panel-scroll");
    const startNode = lineNumbers.children[event.startLine];
    const endNode = lineNumbers.children[event.endLine];
    if (!scroller || !startNode || !endNode)
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
        sandbox.stepPosition = clampStepPosition(sandbox.stepPosition - 1, sandbox.traceLength);
        saveSandboxState();
        renderStage();
        scrollEventIntoView(sandbox.trace[sandbox.stepPosition]);
    });
});
nextButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
        if (sandbox.executionLimit)
            executionLimitNoticeDismissed = true;
        const extendsLimitedTrace = !!sandbox.executionLimit &&
            sandbox.executionLimit.tracePosition < sandbox.traceLength &&
            sandbox.stepPosition >= sandbox.traceLength;
        if (extendsLimitedTrace) {
            executionLimitPage += 1;
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
        sandbox.stepPosition = clampStepPosition(sandbox.stepPosition + 1, sandbox.traceLength);
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
function closeFileCreator() {
    createFileControls.classList.add("hidden");
    addFileButton.classList.remove("hidden");
    deleteFileButton.classList.remove("hidden");
    fileNameInput.setCustomValidity("");
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
    fileNameInput.setCustomValidity(error);
    if (error) {
        fileNameInput.reportValidity();
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
    let counter = 2;
    while (sandbox.files.some((file) => file.path === `file${counter}.c`)) {
        counter += 1;
    }
    fileNameInput.value = `file${counter}.c`;
    addFileButton.classList.add("hidden");
    deleteFileButton.classList.add("hidden");
    createFileControls.classList.remove("hidden");
    fileNameInput.focus();
    fileNameInput.select();
});
confirmFileButton.addEventListener("click", createNamedFile);
cancelFileButton.addEventListener("click", closeFileCreator);
fileNameInput.addEventListener("input", () => fileNameInput.setCustomValidity(""));
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

import { $, applyOtherNames, createSimpleSimulator, ensureBaseLayout, formatValueForType, randAddr, updateStepperTopControls, vbox, } from "./shared-core.js";
import { confettiRain } from "./confetti.js";
ensureBaseLayout();
const instructions = $('[data-role="sandbox-instructions"]');
const codepane = $('[data-role="sandbox-code"]');
const editor = $('[data-role="sandbox-editor"]');
const lineNumbers = $('[data-role="sandbox-line-numbers"]');
const errorGutter = $('[data-role="sandbox-error-gutter"]');
const errorDetail = $('[data-role="sandbox-error-detail"]');
const exprInput = $('[data-role="sandbox-expr"]');
const exprResult = $('[data-role="sandbox-expr-result"]');
const exprError = $('[data-role="sandbox-expr-error"]');
const prevBtn = $('[data-role="sandbox-prev"]');
const nextBtn = $('[data-role="sandbox-next"]');
const prevButtons = [prevBtn].filter((btn) => !!btn);
const nextButtons = [nextBtn].filter((btn) => !!btn);
let finishedConfettiShown = false;
const highlightEl = (() => {
    if (!editor || !editor.parentElement)
        return null;
    const el = document.createElement("pre");
    el.className = "code-textarea-highlight";
    el.setAttribute("aria-hidden", "true");
    editor.parentElement.insertBefore(el, editor);
    return el;
})();
const boundaryLine = (() => {
    if (!editor)
        return null;
    const row = editor.closest(".codepane-row");
    if (!row)
        return null;
    const el = document.createElement("div");
    el.className = "code-boundary-line";
    el.setAttribute("aria-hidden", "true");
    row.appendChild(el);
    return el;
})();
const measureEl = (() => {
    if (!editor || !editor.parentElement)
        return null;
    const el = document.createElement("div");
    el.className = "code-textarea-measure";
    el.setAttribute("aria-hidden", "true");
    editor.parentElement.appendChild(el);
    return el;
})();
const sandbox = {
    text: editor ? editor.value : "",
    allocBase: null,
    errorDetailLine: null,
    errorDetailMessage: "",
    errorDetailHtml: "",
    errorDetailKind: "",
    boundary: null,
    lineCount: 0,
    otherNamesShown: new Set(),
    lastState: null,
    exprOtherNamesShown: new Set(),
};
const simulator = createSimpleSimulator({
    allowVarAssign: true,
    requireSourceValue: true,
    allowPointers: true,
});
function showExprError(message) {
    if (!exprError)
        return;
    exprError.textContent = message || "";
    exprError.classList.toggle("hidden", !message);
}
function allocFactory() {
    if (sandbox.allocBase == null)
        sandbox.allocBase = randAddr("int");
    let next = sandbox.allocBase;
    return () => {
        const addr = next;
        next += 4;
        return String(addr);
    };
}
function updateInstructions() {
    if (!instructions)
        return;
    const lines = getRawLines();
    const total = lines.length;
    const boundary = resolveBoundary(sandbox.boundary, total);
    const statementInfo = simulator.getStatementContext(lines, boundary);
    const hasCode = lines.some((line) => line.trim() !== "");
    const atEnd = boundary >= total;
    const base = "This is the sandbox. The program state will update as you write code.";
    let suffix = "";
    if (hasCode) {
        let runLabel = "";
        if (statementInfo?.midStatement &&
            isMultiLineStatement(statementInfo.currentRange)) {
            runLabel = `Finish running ${formatLineRange(statementInfo.currentRange)} ▶`;
        }
        else if (statementInfo?.atStatementStart &&
            isMultiLineStatement(statementInfo.currentRange)) {
            runLabel = `Run ${formatLineRange(statementInfo.currentRange)} ▶`;
        }
        else {
            runLabel = `Run line ${boundary + 1} ▶`;
        }
        if (atEnd) {
            suffix =
                ' Use <span class="btn-ref">Back ◀</span> to step through your program.';
        }
        else if (boundary <= 0) {
            suffix = ` Use <span class="btn-ref">${runLabel}</span> to step through your program.`;
        }
        else {
            suffix = ` Use <span class="btn-ref">Back ◀</span> and <span class="btn-ref">${runLabel}</span> to step through your program.`;
        }
    }
    const message = `${base}${suffix}`;
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
        instructions.innerHTML = message;
    }
}
function parseErrorMessage(message) {
    if (message && typeof message === "object") {
        return {
            text: String(message.text || ""),
            html: message.html ? String(message.html) : "",
        };
    }
    return { text: String(message || ""), html: "" };
}
function combineMessages(primary, secondary) {
    if (!secondary)
        return primary;
    const p = parseErrorMessage(primary);
    const s = parseErrorMessage(secondary);
    const text = [p.text, s.text].filter(Boolean).join(" ");
    const htmlParts = [p.html || p.text, s.html || s.text].filter(Boolean);
    const html = htmlParts.join(" ");
    return { text, html };
}
const GENERIC_COMPILE_ERRORS = new Set([
    "Line has an error.",
    'Declarations should look like "int name;" or "long name;" or "double name;" or "int name = value;".',
    'Declarations should look like "int name;" or "long name;" or "double name;".',
    'Assignments should look like "name = value;".',
    "Line should be a declaration or assignment.",
]);
function isGenericCompileMessage(message) {
    const parsed = parseErrorMessage(message);
    const text = (parsed.text || "").trim();
    if (!text)
        return true;
    return GENERIC_COMPILE_ERRORS.has(text);
}
function showErrorDetail(message, kind) {
    if (!errorDetail)
        return;
    const parsed = parseErrorMessage(message);
    errorDetail.innerHTML = "";
    if (kind === "ub") {
        const base = document.createElement("span");
        if (parsed.html) {
            base.innerHTML = parsed.html;
        }
        else {
            base.textContent = parsed.text;
        }
        base.appendChild(document.createTextNode(" This line causes undefined behavior. "));
        errorDetail.appendChild(base);
        const link = document.createElement("button");
        link.type = "button";
        link.className = "ub-explain-link";
        link.textContent = "[What is undefined behavior?]";
        const explain = document.createElement("div");
        explain.className = "ub-explain hidden";
        explain.textContent =
            "Undefined behavior means the C standard does not define what happens. The program might crash, act strangely, or appear to work.";
        link.addEventListener("click", (e) => {
            e.preventDefault();
            e.stopPropagation();
            explain.classList.toggle("hidden");
        });
        errorDetail.appendChild(link);
        errorDetail.appendChild(explain);
    }
    else if (parsed.html) {
        errorDetail.innerHTML = parsed.html;
    }
    else {
        errorDetail.textContent = parsed.text;
    }
    errorDetail.classList.remove("hidden");
    sandbox.errorDetailMessage = parsed.text;
    sandbox.errorDetailHtml = parsed.html;
    sandbox.errorDetailKind = kind || "compile";
}
function hideErrorDetail() {
    if (!errorDetail)
        return;
    errorDetail.textContent = "";
    errorDetail.classList.add("hidden");
    sandbox.errorDetailLine = null;
    sandbox.errorDetailMessage = "";
    sandbox.errorDetailHtml = "";
    sandbox.errorDetailKind = "";
}
function getEditorText() {
    return editor ? editor.value : sandbox.text || "";
}
function getRawLines() {
    return getEditorText().split(/\r?\n/);
}
function clampBoundary(value, total) {
    if (!Number.isFinite(value))
        return 0;
    return Math.max(0, Math.min(total, value));
}
function resolveBoundary(value, fallback) {
    return Number.isFinite(value) ? value : fallback;
}
function isMultiLineStatement(range) {
    return !!(range &&
        Number.isFinite(range.startLine) &&
        Number.isFinite(range.endLine) &&
        range.endLine > range.startLine);
}
function formatLineRange(range) {
    if (!range)
        return "";
    const start = (range.startLine ?? 0) + 1;
    const end = (range.endLine ?? 0) + 1;
    if (start === end)
        return `line ${start}`;
    return `lines ${start}-${end}`;
}
function countNewlines(value) {
    let count = 0;
    for (let i = 0; i < value.length; i++) {
        if (value[i] === "\n")
            count++;
    }
    return count;
}
function diffSegments(prev, next) {
    let start = 0;
    const prevLen = prev.length;
    const nextLen = next.length;
    while (start < prevLen && start < nextLen && prev[start] === next[start])
        start++;
    let endPrev = prevLen;
    let endNext = nextLen;
    while (endPrev > start &&
        endNext > start &&
        prev[endPrev - 1] === next[endNext - 1]) {
        endPrev--;
        endNext--;
    }
    return {
        start,
        oldSegment: prev.slice(start, endPrev),
        newSegment: next.slice(start, endNext),
    };
}
function adjustBoundaryForEdit(prevText, nextText) {
    if (prevText === nextText)
        return;
    const prevLines = prevText.split(/\r?\n/).length;
    const nextLines = nextText.split(/\r?\n/).length;
    const boundary = resolveBoundary(sandbox.boundary, prevLines);
    const { start, oldSegment, newSegment } = diffSegments(prevText, nextText);
    const beforeChange = prevText.slice(0, start);
    const changeLine = countNewlines(beforeChange);
    const lastBreak = beforeChange.lastIndexOf("\n");
    const changeCol = start - (lastBreak + 1);
    if (changeLine > boundary)
        return;
    const delta = countNewlines(newSegment) - countNewlines(oldSegment);
    if (!delta)
        return;
    if (changeLine === boundary) {
        if (changeCol !== 0 || delta < 0)
            return;
    }
    sandbox.boundary = clampBoundary(boundary + delta, nextLines);
}
function isBoundaryInsideBlockComment(lines, boundary) {
    if (!Array.isArray(lines) || boundary <= 0)
        return false;
    let inComment = false;
    const limit = Math.min(boundary, lines.length);
    for (let lineIndex = 0; lineIndex < limit; lineIndex++) {
        const line = lines[lineIndex] || "";
        for (let i = 0; i < line.length; i++) {
            const ch = line[i];
            const next = line[i + 1];
            if (!inComment && ch === "/" && next === "/")
                break;
            if (!inComment && ch === "/" && next === "*") {
                inComment = true;
                i++;
                continue;
            }
            if (inComment && ch === "*" && next === "/") {
                inComment = false;
                i++;
                continue;
            }
        }
    }
    return inComment;
}
function syncBoundaryWithLines(lines) {
    const total = lines.length;
    const prevTotal = Number.isFinite(sandbox.lineCount)
        ? sandbox.lineCount
        : total;
    const hasBoundary = Number.isFinite(sandbox.boundary);
    const wasAtEnd = !hasBoundary || sandbox.boundary >= prevTotal;
    sandbox.lineCount = total;
    if (wasAtEnd) {
        sandbox.boundary = total;
    }
    else {
        sandbox.boundary = clampBoundary(sandbox.boundary, total);
    }
    return { boundary: sandbox.boundary, total };
}
function classifyLineStatuses(lines) {
    return simulator.classifyLineStatuses(lines, { alloc: allocFactory() });
}
function summarizeStatus(lines) {
    const status = classifyLineStatuses(lines);
    let hasCompile = status.incomplete.size > 0;
    let hasUb = false;
    if (status.errorKinds) {
        for (const kind of status.errorKinds.values()) {
            if (kind === "ub")
                hasUb = true;
            else
                hasCompile = true;
        }
    }
    return { hasCompile, hasUb };
}
function applyUserProgram(lines) {
    const text = lines.join("\n");
    return simulator.applyProgram(text, { alloc: allocFactory() });
}
function getProgramOutcome(linesOverride, boundaryOverride) {
    const lines = linesOverride || getRawLines();
    const total = lines.length;
    const boundary = resolveBoundary(Number.isFinite(boundaryOverride) ? boundaryOverride : sandbox.boundary, total);
    const safeBoundary = clampBoundary(boundary, total);
    const activeLines = lines.slice(0, safeBoundary);
    const activeSummary = summarizeStatus(activeLines);
    const fullSummary = summarizeStatus(lines);
    let globalKind = "ok";
    if (fullSummary.hasCompile)
        globalKind = "compile";
    else if (fullSummary.hasUb)
        globalKind = "ub";
    if (activeSummary.hasCompile)
        return {
            kind: "compile",
            globalKind,
            state: null,
            boundary: safeBoundary,
            total,
        };
    if (activeSummary.hasUb)
        return {
            kind: "ub",
            globalKind,
            state: null,
            boundary: safeBoundary,
            total,
        };
    const state = applyUserProgram(activeLines);
    if (!state)
        return {
            kind: "compile",
            globalKind,
            state: null,
            boundary: safeBoundary,
            total,
        };
    return { kind: "ok", globalKind, state, boundary: safeBoundary, total };
}
function renderStage() {
    const stage = $('[data-role="sandbox-stage"]');
    if (!stage)
        return;
    stage.innerHTML = "";
    const lines = getRawLines();
    const { boundary, total } = syncBoundaryWithLines(lines);
    const statementInfo = simulator.getStatementContext(lines, boundary);
    const inBlockComment = isBoundaryInsideBlockComment(lines, boundary);
    let outcome = getProgramOutcome(lines, boundary);
    if (inBlockComment && outcome.kind === "compile") {
        // Close the open comment at the boundary so comments don't look invalid.
        const activeLines = lines.slice(0, boundary);
        if (activeLines.length)
            activeLines[activeLines.length - 1] += " */";
        const safeState = applyUserProgram(activeLines);
        if (safeState) {
            outcome = {
                kind: "ok",
                globalKind: outcome.globalKind,
                state: safeState,
                boundary,
                total,
            };
        }
    }
    let displayOutcome = outcome;
    if (outcome.globalKind && outcome.globalKind !== "ok") {
        displayOutcome = { kind: outcome.globalKind, state: null };
    }
    else if (statementInfo.midStatement && !inBlockComment) {
        displayOutcome = { kind: "mid", state: null };
    }
    sandbox.lastState = outcome.state;
    stage.appendChild(renderState("", displayOutcome.state, displayOutcome.kind));
    applyOtherNames(stage, {
        shownAddrs: sandbox.otherNamesShown,
        onToggle: refreshOtherNames,
    });
    renderExpression(displayOutcome);
    updateStepperControls(boundary, total, statementInfo);
    updateInstructions();
}
function renderExpression(outcome) {
    if (!exprResult || !exprInput)
        return;
    exprResult.innerHTML = "";
    showExprError("");
    const expr = exprInput.value.trim();
    if (!expr)
        return;
    if (!outcome || outcome.kind !== "ok") {
        showExprError("Fix the code before evaluating expressions.");
        return;
    }
    const evaluated = simulator.evaluateExpressionText(expr, outcome.state || []);
    if ("error" in evaluated) {
        const errorText = typeof evaluated.error === "string"
            ? evaluated.error
            : evaluated.error.text;
        showExprError(errorText);
        return;
    }
    const { result } = evaluated;
    const match = result.kind === "lvalue"
        ? (outcome.state || []).find((box) => String(box.address) === String(result.address))
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
    if (match && String(match.value ?? "") === "") {
        node.querySelector(".value")?.classList.add("placeholder", "muted");
    }
    if (!match && !result.address)
        node.classList.add("no-addr");
    if (!match)
        node.classList.add("no-name");
    exprResult.appendChild(node);
    if (match) {
        applyOtherNames(exprResult, {
            shownAddrs: sandbox.exprOtherNamesShown,
            onToggle: refreshOtherNames,
            sourceBoxes: outcome.state || [],
            cleanupShownAddrs: false,
        });
    }
}
function updateStepperControls(boundaryOverride, totalOverride, statementInfo) {
    if (!prevButtons.length && !nextButtons.length)
        return;
    const total = resolveBoundary(Number.isFinite(totalOverride) ? totalOverride : sandbox.lineCount, getRawLines().length);
    const boundary = resolveBoundary(Number.isFinite(boundaryOverride) ? boundaryOverride : sandbox.boundary, total);
    prevButtons.forEach((btn) => {
        btn.disabled = boundary <= 0;
    });
    if (nextButtons.length) {
        const atEnd = boundary >= total;
        let label = "At end ▶";
        if (!atEnd) {
            if (statementInfo?.midStatement &&
                isMultiLineStatement(statementInfo.currentRange)) {
                label = `Finish running ${formatLineRange(statementInfo.currentRange)} ▶`;
            }
            else if (statementInfo?.atStatementStart &&
                isMultiLineStatement(statementInfo.currentRange)) {
                label = `Run ${formatLineRange(statementInfo.currentRange)} ▶`;
            }
            else {
                label = `Run line ${boundary + 1} ▶`;
            }
        }
        nextButtons.forEach((btn) => {
            btn.textContent = label;
            btn.disabled = atEnd;
        });
    }
}
function renderState(title, boxes, status = "ok") {
    const wrap = document.createElement("div");
    wrap.className = "state-panel";
    if (title) {
        const heading = document.createElement("div");
        heading.className = "state-heading";
        heading.textContent = title;
        wrap.appendChild(heading);
    }
    const grid = document.createElement("div");
    grid.className = "grid";
    if (status === "mid") {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        msg.textContent = "(the program is in the middle of executing a statement)";
        grid.appendChild(msg);
    }
    else if (status === "compile") {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        msg.textContent = "(this code is not valid)";
        grid.appendChild(msg);
    }
    else if (status === "ub") {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        const label = document.createElement("span");
        label.textContent = "Kaboom! ";
        msg.appendChild(label);
        const link = document.createElement("button");
        link.type = "button";
        link.className = "ub-explain-link";
        link.textContent = "[What is undefined behavior?]";
        const explain = document.createElement("div");
        explain.className = "ub-explain hidden";
        explain.textContent =
            "Undefined behavior means the C standard does not define what happens. The program might crash, act strangely, or appear to work.";
        link.addEventListener("click", (e) => {
            e.preventDefault();
            e.stopPropagation();
            explain.classList.toggle("hidden");
        });
        msg.appendChild(link);
        msg.appendChild(explain);
        grid.appendChild(msg);
    }
    else if (boxes === null) {
        const msg = document.createElement("div");
        msg.className = "muted state-status";
        msg.style.padding = "8px";
        msg.textContent = "(this code is not valid)";
        grid.appendChild(msg);
    }
    else if (boxes.length === 0) {
        const msg = document.createElement("div");
        msg.className = "muted";
        msg.style.padding = "8px";
        msg.textContent = "(no variables yet)";
        grid.appendChild(msg);
    }
    else {
        boxes.forEach((b) => {
            const node = vbox({
                address: b.address ?? undefined,
                type: b.type,
                value: b.value,
                name: b.name,
                editable: false,
            });
            if (String(b.value ?? "") === "")
                node.querySelector(".value")?.classList.add("placeholder", "muted");
            grid.appendChild(node);
        });
    }
    wrap.appendChild(grid);
    return wrap;
}
function refreshOtherNames() {
    const stage = $('[data-role="sandbox-stage"]');
    if (stage) {
        applyOtherNames(stage, {
            shownAddrs: sandbox.otherNamesShown,
            onToggle: refreshOtherNames,
        });
    }
    if (exprResult) {
        applyOtherNames(exprResult, {
            shownAddrs: sandbox.exprOtherNamesShown,
            onToggle: refreshOtherNames,
            sourceBoxes: sandbox.lastState || [],
            cleanupShownAddrs: false,
        });
    }
}
function getLineHeightPx() {
    if (!editor)
        return 32;
    const style = window.getComputedStyle(editor);
    const lh = parseFloat(style.lineHeight);
    return Number.isFinite(lh) ? lh : 32;
}
function autoSizeEditor() {
    if (!editor)
        return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
}
function measureWrapCounts(lines) {
    if (!editor || !measureEl)
        return lines.map(() => 1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(1, editor.clientWidth - paddingLeft - paddingRight);
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
function updateLineGutters(linesOverride) {
    autoSizeEditor();
    const lines = Array.isArray(linesOverride) ? linesOverride : getRawLines();
    const count = Math.max(lines.length, 1);
    const safeBoundary = clampBoundary(sandbox.boundary ?? 0, count);
    const lineHeight = getLineHeightPx();
    const wraps = measureWrapCounts(lines);
    if (highlightEl) {
        highlightEl.innerHTML = "";
        const frag = document.createDocumentFragment();
        for (let i = 0; i < lines.length; i++) {
            const span = document.createElement("span");
            span.className = "code-highlight-line";
            if (i < safeBoundary)
                span.classList.add("is-done");
            span.textContent = lines[i] === "" ? " " : lines[i];
            frag.appendChild(span);
            if (i < lines.length - 1)
                frag.appendChild(document.createTextNode("\n"));
        }
        highlightEl.appendChild(frag);
    }
    if (boundaryLine && editor) {
        const style = window.getComputedStyle(editor);
        const paddingTop = parseFloat(style.paddingTop) || 0;
        let rows = 0;
        for (let i = 0; i < safeBoundary; i++) {
            rows += wraps[i] || 1;
        }
        const top = Math.max(0, paddingTop + rows * lineHeight - 1);
        boundaryLine.style.top = `${top}px`;
    }
    if (lineNumbers) {
        const frag = document.createDocumentFragment();
        for (let i = 1; i <= count; i++) {
            const num = document.createElement("div");
            num.className = "code-line-number";
            num.style.height = `${(wraps[i - 1] || 1) * lineHeight}px`;
            num.textContent = String(i);
            frag.appendChild(num);
        }
        lineNumbers.innerHTML = "";
        lineNumbers.appendChild(frag);
        if (editor)
            lineNumbers.style.height = `${editor.clientHeight}px`;
    }
    if (errorGutter) {
        const { invalid, incomplete, errors, errorKinds, info } = classifyLineStatuses(lines);
        const frag = document.createDocumentFragment();
        for (let i = 0; i < count; i++) {
            const cell = document.createElement("div");
            cell.className = "code-error-line";
            cell.style.height = `${(wraps[i] || 1) * lineHeight}px`;
            if (invalid.has(i)) {
                cell.classList.add("is-invalid");
                const icon = document.createElement("span");
                const kind = errorKinds?.get(i) || "compile";
                icon.textContent = kind === "ub" ? "💣" : "🚫";
                icon.title =
                    kind === "ub"
                        ? "Line causes undefined behavior"
                        : "Line does not compile";
                cell.appendChild(icon);
                const baseMessage = errors.get(i) || "Line has an error.";
                const hasInfo = info?.has(i);
                const showInfo = kind === "ub" || hasInfo || !isGenericCompileMessage(baseMessage);
                if (showInfo) {
                    const message = (errorKinds?.get(i) || "compile") === "compile" && hasInfo
                        ? combineMessages(baseMessage, info.get(i) || "")
                        : baseMessage;
                    const infoBtn = document.createElement("button");
                    infoBtn.type = "button";
                    infoBtn.className = "error-info-btn";
                    infoBtn.textContent = "i";
                    infoBtn.setAttribute("aria-label", "Explain error");
                    infoBtn.addEventListener("click", (e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        const parsed = parseErrorMessage(message);
                        if (sandbox.errorDetailLine === i &&
                            sandbox.errorDetailMessage === parsed.text &&
                            sandbox.errorDetailHtml === parsed.html &&
                            sandbox.errorDetailKind === kind) {
                            hideErrorDetail();
                        }
                        else {
                            sandbox.errorDetailLine = i;
                            showErrorDetail(message, kind);
                        }
                    });
                    cell.appendChild(infoBtn);
                }
            }
            else if (incomplete.has(i)) {
                cell.classList.add("is-incomplete");
                cell.textContent = "...";
                cell.title = "Line is incomplete";
                if (info?.has(i)) {
                    const infoMsg = info.get(i);
                    if (!infoMsg)
                        continue;
                    const parsed = parseErrorMessage(infoMsg);
                    const btn = document.createElement("button");
                    btn.type = "button";
                    btn.className = "error-info-btn";
                    btn.textContent = "i";
                    btn.setAttribute("aria-label", "Explain statement");
                    btn.addEventListener("click", (e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        if (sandbox.errorDetailLine === i &&
                            sandbox.errorDetailMessage === parsed.text &&
                            sandbox.errorDetailHtml === parsed.html &&
                            sandbox.errorDetailKind === "info") {
                            hideErrorDetail();
                        }
                        else {
                            sandbox.errorDetailLine = i;
                            showErrorDetail(infoMsg, "info");
                        }
                    });
                    cell.appendChild(btn);
                }
            }
            else if (info?.has(i)) {
                const infoMsg = info.get(i);
                if (!infoMsg)
                    continue;
                const parsed = parseErrorMessage(infoMsg);
                const btn = document.createElement("button");
                btn.type = "button";
                btn.className = "error-info-btn";
                btn.textContent = "i";
                btn.setAttribute("aria-label", "Explain statement");
                btn.addEventListener("click", (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (sandbox.errorDetailLine === i &&
                        sandbox.errorDetailMessage === parsed.text &&
                        sandbox.errorDetailHtml === parsed.html &&
                        sandbox.errorDetailKind === "info") {
                        hideErrorDetail();
                    }
                    else {
                        sandbox.errorDetailLine = i;
                        showErrorDetail(infoMsg, "info");
                    }
                });
                cell.appendChild(btn);
            }
            frag.appendChild(cell);
        }
        errorGutter.innerHTML = "";
        errorGutter.appendChild(frag);
        if (editor)
            errorGutter.style.height = `${editor.clientHeight}px`;
    }
    if (editor) {
        if (lineNumbers)
            lineNumbers.scrollTop = editor.scrollTop;
        if (errorGutter)
            errorGutter.scrollTop = editor.scrollTop;
    }
    if (codepane)
        updateStepperTopControls(codepane);
}
if (editor) {
    editor.addEventListener("input", () => {
        const nextText = editor.value;
        adjustBoundaryForEdit(sandbox.text || "", nextText);
        sandbox.text = nextText;
        hideErrorDetail();
        renderStage();
        updateLineGutters();
    });
    if (lineNumbers) {
        editor.addEventListener("scroll", () => {
            lineNumbers.scrollTop = editor.scrollTop;
        });
    }
    if (errorGutter) {
        editor.addEventListener("scroll", () => {
            errorGutter.scrollTop = editor.scrollTop;
        });
    }
    editor.addEventListener("mouseup", () => updateLineGutters());
    window.addEventListener("resize", () => updateLineGutters());
    if (typeof ResizeObserver !== "undefined") {
        const ro = new ResizeObserver(() => updateLineGutters());
        ro.observe(editor);
    }
}
prevButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
        const lines = getRawLines();
        const total = lines.length;
        const current = resolveBoundary(sandbox.boundary, total);
        if (current <= 0)
            return;
        const boundary = clampBoundary(current, total);
        const statementInfo = simulator.getStatementContext(lines, boundary);
        const prevRange = statementInfo.prevRange;
        const target = isMultiLineStatement(prevRange)
            ? prevRange.startLine
            : boundary - 1;
        sandbox.boundary = clampBoundary(target, total);
        renderStage();
        updateLineGutters();
    });
});
nextButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
        const lines = getRawLines();
        const total = lines.length;
        const current = resolveBoundary(sandbox.boundary, total);
        if (current >= total)
            return;
        const boundary = clampBoundary(current, total);
        const statementInfo = simulator.getStatementContext(lines, boundary);
        let target = boundary + 1;
        if ((statementInfo.midStatement || statementInfo.atStatementStart) &&
            statementInfo.currentRange) {
            target = statementInfo.currentRange.endLine + 1;
        }
        sandbox.boundary = clampBoundary(target, total);
        renderStage();
        updateLineGutters();
    });
});
updateInstructions();
renderStage();
updateLineGutters();
if (exprInput) {
    exprInput.addEventListener("input", () => {
        renderExpression(getProgramOutcome());
    });
}

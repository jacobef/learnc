import { applyTextTokenReplacements, appendStateObjects, bindBtnRefPulse, boxValueMatchesSpec, clearNode, createSimpleSimulator, createStepper, ensurePanelizedMain, flashStatus, getNavLabelForHref, queryElement, queryRole, randAddr, renderParts, setPartsContent, syncDocumentTitleFromNav, typeInfo, } from "./shared-core.js";
import { bindCodeEditorTabKey, ensureCodeSurfaceElements, updateCodeSurface, } from "./shared-code-editor-surface.js";
import { clearLevelProgress, currentLevelId, maybeRestoreLevelProgress, writeLevelProgress, } from "./shared-progress.js";
function collectCodeEditorElements(root = document) {
    const role = (name) => queryRole(name, root);
    return {
        instructionsEl: role("code-instructions"),
        editor: role("code-editor"),
        lineNumbers: role("code-line-numbers"),
        stage: role("code-stage"),
        status: role("code-status"),
        diagnosticEl: role("code-diagnostic"),
        hintPanel: role("code-hint"),
        hintBtn: role("code-hint-btn"),
        checkBtn: role("code-check"),
        levelResetBtn: role("code-reset-level"),
        nextBtn: queryElement('button[data-stepper="next"]', root),
        codeRoot: role("code-root"),
    };
}
function ensureCodeEditorLayout(textareaMinLines) {
    const resolvedTitle = syncDocumentTitleFromNav();
    const existing = queryRole("code-editor");
    if (existing)
        return collectCodeEditorElements();
    const main = ensurePanelizedMain(resolvedTitle);
    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "code-instructions";
    instructionsEl.className = "intro";
    const section = document.createElement("section");
    section.dataset.role = "code-root";
    section.className = "panel-shell";
    const actionBar = document.createElement("div");
    actionBar.className = "controls-bar controls-bar-code";
    const controlsMain = document.createElement("div");
    controlsMain.className = "controls-main panel panel-controls";
    const controlsRow = document.createElement("div");
    controlsRow.className = "controls-row controls-left";
    controlsMain.appendChild(controlsRow);
    actionBar.appendChild(controlsMain);
    const row = document.createElement("div");
    row.className = "row panel-row";
    section.appendChild(actionBar);
    section.appendChild(row);
    main.appendChild(section);
    const codePanel = document.createElement("div");
    codePanel.className = "panel code-editor-panel panel-scroll code-panel-shell";
    const codeTitle = document.createElement("div");
    codeTitle.className = "panel-title code-title";
    codeTitle.textContent = "Code";
    const codePane = document.createElement("div");
    codePane.className = "codepane panel-body";
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
    editor.rows = Math.max(1, Math.floor(textareaMinLines));
    editorWrap.appendChild(editor);
    codeRow.append(lineNumbers, editorWrap);
    codePane.appendChild(codeRow);
    const diagnosticEl = document.createElement("div");
    diagnosticEl.dataset.role = "code-diagnostic";
    diagnosticEl.className = "code-diagnostic hidden";
    codePanel.append(codeTitle, codePane, diagnosticEl);
    const stateCol = document.createElement("div");
    stateCol.className = "code-editor-state-col";
    const stage = document.createElement("div");
    stage.dataset.role = "code-stage";
    stage.className = "code-editor-state-stage";
    stateCol.appendChild(stage);
    const nextBtn = document.createElement("button");
    nextBtn.dataset.stepper = "next";
    nextBtn.textContent = "Next Program ▶▶";
    const hintBtn = document.createElement("button");
    hintBtn.dataset.role = "code-hint-btn";
    hintBtn.className = "hint-link";
    hintBtn.type = "button";
    hintBtn.textContent = "Hint";
    const checkBtn = document.createElement("button");
    checkBtn.dataset.role = "code-check";
    checkBtn.textContent = "Check";
    const levelResetBtn = document.createElement("button");
    levelResetBtn.dataset.role = "code-reset-level";
    levelResetBtn.textContent = "Reset level";
    const status = document.createElement("span");
    status.dataset.role = "code-status";
    status.className = "muted";
    const spacer = document.createElement("span");
    spacer.className = "controls-spacer";
    spacer.setAttribute("aria-hidden", "true");
    controlsRow.append(nextBtn, spacer, levelResetBtn, hintBtn, checkBtn, status);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "code-hint";
    hintPanel.className = "hint-inline hidden";
    actionBar.append(hintPanel, instructionsEl);
    row.append(codePanel, stateCol);
    return {
        instructionsEl,
        editor,
        lineNumbers,
        stage,
        status,
        diagnosticEl,
        hintPanel,
        hintBtn,
        checkBtn,
        levelResetBtn,
        nextBtn,
        codeRoot: section,
    };
}
function createCodeEditorTemplate(config) {
    const { startCode = "", targetState = [], textareaMinLines, allowNewLines = true, hints = null, instructions = "", next = null, isLast = false, } = config;
    const { instructionsEl, editor, lineNumbers, stage, status, diagnosticEl, hintPanel, hintBtn, checkBtn, levelResetBtn, nextBtn, codeRoot, } = ensureCodeEditorLayout(textareaMinLines);
    const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
    bindBtnRefPulse(codeRoot || document);
    const simulator = createSimpleSimulator();
    const levelId = currentLevelId();
    const defaultText = normalizeEditorText(startCode);
    const restoredProgress = maybeRestoreLevelProgress(levelId, "this level");
    const state = {
        text: typeof restoredProgress?.text === "string"
            ? normalizeEditorText(restoredProgress.text)
            : defaultText,
        pass: restoredProgress?.pass === true,
        allocBase: typeof restoredProgress?.allocBase === "number"
            ? restoredProgress.allocBase
            : null,
    };
    let pager = null;
    const endLabel = (() => {
        if (isLast)
            return "Finish";
        const label = getNavLabelForHref(next);
        return label ? `Next: ${label}` : "Next Program";
    })();
    const buttonReplacements = [
        ["$checkButton", "$b{Check}"],
        ["$resetButton", "$b{Reset}"],
        ["$newVariableButton", "$b{+ New variable}"],
        ["$runLineButton", "$b{Run line}"],
        ["$backButton", "$b{Back ◀}"],
        ["$showAliasesButton", "$b{Show aliases}"],
    ];
    function allocFactory() {
        if (state.allocBase == null)
            state.allocBase = randAddr("int");
        let nextAddr = Number(state.allocBase);
        return (type = "int") => {
            const info = typeInfo(type || "int");
            const size = info.size || 4;
            const align = info.align || 1;
            if (nextAddr % align !== 0)
                nextAddr = Math.ceil(nextAddr / align) * align;
            const addr = nextAddr;
            nextAddr += size;
            return String(addr);
        };
    }
    function normalizeEditorText(text) {
        if (allowNewLines)
            return text;
        return text.replace(/\r\n/g, "\n").replace(/\n/g, " ");
    }
    function progressSnapshot() {
        return {
            text: state.text,
            pass: state.pass,
            allocBase: state.allocBase,
        };
    }
    function persistProgress() {
        const snapshot = progressSnapshot();
        if (snapshot.text === defaultText && !snapshot.pass) {
            clearLevelProgress(levelId);
            return;
        }
        writeLevelProgress(snapshot, levelId);
    }
    function setStatus(text, cls = "muted") {
        if (!status)
            return;
        status.textContent = text;
        status.className = cls;
    }
    function getEditorText() {
        return normalizeEditorText(editor?.value || state.text || "");
    }
    function getEditorLines() {
        return getEditorText().split(/\r?\n/);
    }
    function applyUserProgram() {
        const tokens = simulator.tokenizeProgram(getEditorText());
        const parts = simulator.splitStatements(tokens);
        return simulator.applyProgramParts(parts, { alloc: allocFactory() });
    }
    function getProgramOutcome() {
        const tokens = simulator.tokenizeProgram(getEditorText());
        const parts = simulator.splitStatements(tokens);
        const result = simulator.analyzeProgramParts(parts, { alloc: allocFactory() });
        if (result.kind !== "ok")
            return { kind: result.kind, state: null };
        return { kind: "ok", state: result.state };
    }
    function getProgramDiagnostic() {
        const diagnostics = simulator.diagnoseProgram(getEditorText(), {
            alloc: allocFactory(),
        });
        return diagnostics[0] || null;
    }
    function diagnosticDecoration(diagnostic) {
        if (!diagnostic)
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
        if (!diagnosticEl)
            return;
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
        heading.textContent = `${diagnostic.kind === "ub" ? "Undefined behavior" : "Error"} on line ${diagnostic.range.startLine + 1}, column ${diagnostic.range.startCol + 1}`;
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
    function updateLineGutters(diagnostic = null) {
        const lines = getEditorLines();
        const lineNumberClasses = new Map();
        if (diagnostic) {
            lineNumberClasses.set(diagnostic.range.startLine, ["has-error"]);
        }
        updateCodeSurface({
            editor,
            lineNumbers,
            highlightEl,
            measureEl,
            lines,
            decorations: diagnosticDecoration(diagnostic),
            lineNumberClasses,
        });
        renderDiagnostic(diagnostic);
    }
    function isTargetMatch(outcome) {
        if (outcome.kind !== "ok" || !Array.isArray(outcome.state))
            return false;
        if (outcome.state.length !== targetState.length)
            return false;
        const byName = new Map(outcome.state.map((box) => [box.name, box]));
        for (const expected of targetState) {
            const actual = byName.get(expected.name);
            if (!actual)
                return false;
            if ((actual.type || "") !== (expected.type || ""))
                return false;
            if (!boxValueMatchesSpec(simulator, actual, expected).ok)
                return false;
        }
        return true;
    }
    function evaluate() {
        const outcome = getProgramOutcome();
        return { ok: isTargetMatch(outcome), outcome };
    }
    function renderState(title, boxes, kind = "ok") {
        const wrap = document.createElement("div");
        wrap.className = "state-panel state-panel-scrollable";
        const heading = document.createElement("h3");
        heading.className = "panel-title state-heading";
        heading.textContent = title;
        wrap.appendChild(heading);
        const grid = document.createElement("div");
        grid.className = "grid";
        if (!boxes || !boxes.length) {
            const msg = document.createElement("div");
            msg.className = "muted";
            msg.style.padding = "8px";
            msg.textContent = "(no variables)";
            grid.appendChild(msg);
        }
        else {
            appendStateObjects(grid, boxes, { editable: false, deletable: false });
        }
        const body = document.createElement("div");
        body.className = "state-panel-scroll-body";
        body.appendChild(grid);
        wrap.appendChild(body);
        return wrap;
    }
    function renderStage(outcome) {
        if (!stage)
            return;
        clearNode(stage);
        const group = document.createElement("div");
        group.className = "state-group two-col";
        group.appendChild(renderState("Your code's final state", outcome.state, outcome.kind));
        group.appendChild(renderState("Target final state", targetState));
        stage.appendChild(group);
    }
    function partsContext() {
        return {
            text: getEditorText(),
            targetState,
            tokenizeProgram: simulator.tokenizeProgram,
            parseStatements: simulator.parseStatements,
            findMissingSemicolonLines: simulator.findMissingSemicolonLines,
            applyUserProgram,
        };
    }
    function applyButtonTokens(parts) {
        return applyTextTokenReplacements(parts, buttonReplacements);
    }
    function hideHint() {
        if (!hintPanel)
            return;
        hintPanel.classList.add("hidden");
        hintPanel.textContent = "";
    }
    function showHint(parts) {
        if (!hintPanel)
            return;
        hintPanel.classList.remove("hidden");
        clearNode(hintPanel);
        renderParts(hintPanel, applyButtonTokens(parts) || []);
        flashStatus(hintPanel);
    }
    function handleHint() {
        if (state.pass)
            return;
        const result = evaluate();
        if (result.ok) {
            showHint("Looks good. Press $checkButton.");
            return;
        }
        if (!hints) {
            showHint("No hints for this page.");
            return;
        }
        const ctx = partsContext();
        const parts = typeof hints === "function" ? hints(ctx) : hints;
        if (!parts || (Array.isArray(parts) && parts.length === 0)) {
            showHint("No hint available for this state.");
            return;
        }
        showHint(parts);
    }
    function render() {
        const diagnostic = getProgramDiagnostic();
        updateLineGutters(diagnostic);
        const outcome = getProgramOutcome();
        renderStage(outcome);
        if (instructions)
            setPartsContent(instructionsEl, applyButtonTokens(instructions));
        else
            setPartsContent(instructionsEl, []);
        if (!state.pass) {
            setStatus("", "muted");
        }
        nextBtn?.classList.remove("hidden");
        pager?.update();
        persistProgress();
    }
    if (editor) {
        bindCodeEditorTabKey(editor);
        editor.value = state.text;
        editor.addEventListener("input", () => {
            state.text = normalizeEditorText(editor.value);
            if (!allowNewLines && editor.value !== state.text)
                editor.value = state.text;
            hideHint();
            if (!state.pass)
                setStatus("", "muted");
            render();
        });
        editor.addEventListener("scroll", () => {
            if (lineNumbers)
                lineNumbers.scrollTop = editor.scrollTop;
        });
        window.addEventListener("resize", () => updateLineGutters(getProgramDiagnostic()));
        if (typeof ResizeObserver !== "undefined") {
            const ro = new ResizeObserver(() => updateLineGutters(getProgramDiagnostic()));
            ro.observe(editor);
        }
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            hideHint();
            handleHint();
        });
    }
    if (checkBtn) {
        checkBtn.addEventListener("click", () => {
            hideHint();
            const result = evaluate();
            setStatus(result.ok ? "correct" : "incorrect", result.ok ? "ok" : "err");
            flashStatus(status);
            if (!result.ok)
                return;
            state.pass = true;
            if (editor)
                editor.readOnly = true;
            checkBtn.classList.add("hidden");
            hintBtn?.classList.add("hidden");
            pager?.pulseNext();
            render();
        });
    }
    if (levelResetBtn) {
        levelResetBtn.addEventListener("click", () => {
            const confirmed = window.confirm("Reset your saved progress for this level and start over?");
            if (!confirmed)
                return;
            clearLevelProgress(levelId);
            window.location.reload();
        });
    }
    pager = createStepper({
        root: codeRoot || editor?.closest(".panel") || document.body,
        lines: 0,
        nextPage: next || null,
        endLabel,
        getBoundary: () => 0,
        setBoundary: () => { },
        onAfterChange: render,
        isStepLocked: () => !state.pass,
    });
    pager.update();
    render();
}
export { createCodeEditorTemplate };

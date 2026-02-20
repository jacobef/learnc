import { applyTextTokenReplacements, appendStateObjects, bindBtnRefPulse, boxValueMatchesSpec, clearNode, createSimpleSimulator, createStepper, ensurePanelizedMain, flashStatus, getNavLabelForHref, queryElement, queryRole, randAddr, renderParts, setPartsContent, syncDocumentTitleFromNav, typeInfo, } from "./shared-core.js";
function collectCodeEditorElements(root = document) {
    const role = (name) => queryRole(name, root);
    return {
        instructionsEl: role("code-instructions"),
        editor: role("code-editor"),
        lineNumbers: role("code-line-numbers"),
        errorGutter: role("code-error-gutter"),
        stage: role("code-stage"),
        status: role("code-status"),
        hintPanel: role("code-hint"),
        hintBtn: role("code-hint-btn"),
        checkBtn: role("code-check"),
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
    const errorGutter = document.createElement("div");
    errorGutter.dataset.role = "code-error-gutter";
    errorGutter.className = "code-error-gutter";
    errorGutter.setAttribute("aria-hidden", "true");
    codeRow.append(lineNumbers, editorWrap, errorGutter);
    codePane.appendChild(codeRow);
    codePanel.append(codeTitle, codePane);
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
    const status = document.createElement("span");
    status.dataset.role = "code-status";
    status.className = "muted";
    const spacer = document.createElement("span");
    spacer.className = "controls-spacer";
    spacer.setAttribute("aria-hidden", "true");
    controlsRow.append(nextBtn, spacer, hintBtn, checkBtn, status);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "code-hint";
    hintPanel.className = "hint-inline hidden";
    actionBar.append(hintPanel, instructionsEl);
    row.append(codePanel, stateCol);
    return {
        instructionsEl,
        editor,
        lineNumbers,
        errorGutter,
        stage,
        status,
        hintPanel,
        hintBtn,
        checkBtn,
        nextBtn,
        codeRoot: section,
    };
}
function createCodeEditorTemplate(config) {
    const { startCode = "", targetState = [], textareaMinLines, allowNewLines = true, hints = null, instructions = "", next = null, isLast = false, } = config;
    const { instructionsEl, editor, lineNumbers, errorGutter, stage, status, hintPanel, hintBtn, checkBtn, nextBtn, codeRoot, } = ensureCodeEditorLayout(textareaMinLines);
    bindBtnRefPulse(codeRoot || document);
    const simulator = createSimpleSimulator();
    const state = { text: startCode, pass: false, allocBase: null };
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
    function updateLineGutters(statusInfo) {
        const lines = getEditorLines();
        if (lineNumbers) {
            clearNode(lineNumbers);
            const frag = document.createDocumentFragment();
            for (let i = 1; i <= lines.length; i += 1) {
                const num = document.createElement("div");
                num.className = "code-line-number";
                num.textContent = String(i);
                frag.appendChild(num);
            }
            lineNumbers.appendChild(frag);
            if (editor)
                lineNumbers.style.height = `${editor.clientHeight}px`;
        }
        if (errorGutter) {
            clearNode(errorGutter);
            const frag = document.createDocumentFragment();
            for (let i = 0; i < lines.length; i += 1) {
                const cell = document.createElement("div");
                cell.className = "code-error-line";
                if (statusInfo.invalid.has(i)) {
                    cell.classList.add("is-invalid");
                    const icon = document.createElement("span");
                    icon.className = "code-error-icon";
                    icon.textContent = "⚠";
                    icon.setAttribute("aria-hidden", "true");
                    cell.appendChild(icon);
                }
                else if (statusInfo.incomplete.has(i)) {
                    cell.classList.add("is-incomplete");
                }
                frag.appendChild(cell);
            }
            errorGutter.appendChild(frag);
            if (editor)
                errorGutter.style.height = `${editor.clientHeight}px`;
        }
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
        if (!boxes) {
            const msg = document.createElement("div");
            msg.className = "muted";
            msg.style.padding = "8px";
            msg.textContent = kind === "ub" ? "Undefined behavior." : "(compile error)";
            grid.appendChild(msg);
        }
        else if (!boxes.length) {
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
        const text = getEditorText();
        const lines = text.split(/\r?\n/);
        const statusInfo = simulator.classifyLineStatuses(lines, { alloc: allocFactory() });
        updateLineGutters(statusInfo);
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
    }
    if (editor) {
        editor.value = startCode;
        state.text = startCode;
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
            if (errorGutter)
                errorGutter.scrollTop = editor.scrollTop;
        });
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

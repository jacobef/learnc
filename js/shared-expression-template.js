import { boxValueMatchesSpec, createSimpleSimulator, createStepper, disableBoxEditing, ensureBaseLayout, flashStatus, formatValueForType, getNavLabelForHref, normalizeBoxValueForContext, readBoxState, resolveActiveNavItem, setPartsContent, vbox, } from "./shared-core.js";
function collectExpressionElements(root = document) {
    return {
        instructionsEl: root.querySelector('[data-role="expr-instructions"]'),
        continueBtn: root.querySelector('[data-role="expr-continue"]'),
        sectionEl: root.querySelector('[data-role="expr-section"]'),
        expressionEl: root.querySelector('[data-role="expr-expression"]'),
        answerSlot: root.querySelector('[data-role="expr-answer-slot"]'),
        answerResult: root.querySelector('[data-role="expr-answer-result"]'),
        toggleWrap: root.querySelector('[data-role="expr-answer-toggle"]'),
        hintPanel: root.querySelector('[data-role="expr-hint"]'),
        hintBtn: root.querySelector('[data-role="expr-hint-btn"]'),
        useSelectedBtn: root.querySelector('[data-role="expr-use-selected"]'),
        useEnteredBtn: root.querySelector('[data-role="expr-use-entered"]'),
        checkBtn: root.querySelector('[data-role="expr-check"]'),
        statusEl: root.querySelector('[data-role="expr-status"]'),
        stageEl: root.querySelector('[data-role="expr-stage"]'),
        statePanel: root.querySelector('[data-role="expr-state-panel"]'),
    };
}
function ensureExpressionLayout() {
    const activeItem = resolveActiveNavItem();
    const resolvedTitle = activeItem?.label || "";
    const nextBrowserTitle = resolvedTitle ? `C Boxes - ${resolvedTitle}` : "";
    if (nextBrowserTitle)
        document.title = nextBrowserTitle;
    const existing = document.querySelector('[data-role="expr-expression"]');
    if (existing)
        return collectExpressionElements();
    const { main } = ensureBaseLayout();
    if (resolvedTitle) {
        const heading = document.createElement("h1");
        heading.className = "page-title";
        heading.textContent = resolvedTitle;
        main.appendChild(heading);
    }
    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "expr-instructions";
    instructionsEl.className = "intro";
    main.appendChild(instructionsEl);
    const continueBtn = document.createElement("button");
    continueBtn.type = "button";
    continueBtn.dataset.role = "expr-continue";
    continueBtn.textContent = "Continue";
    main.appendChild(continueBtn);
    const section = document.createElement("section");
    section.dataset.role = "expr-section";
    const stack = document.createElement("div");
    stack.className = "expr-eval-stack";
    section.appendChild(stack);
    main.appendChild(section);
    const exprPanel = document.createElement("div");
    exprPanel.className = "panel quiz-expression-panel";
    const exprTitle = document.createElement("div");
    exprTitle.className = "panel-title";
    exprTitle.textContent = "Expression";
    const exprDisplay = document.createElement("div");
    exprDisplay.dataset.role = "expr-expression";
    exprDisplay.className = "expr-display expr-quiz-display";
    exprPanel.appendChild(exprTitle);
    exprPanel.appendChild(exprDisplay);
    const leftCol = document.createElement("div");
    leftCol.className = "expr-left-col";
    const row = document.createElement("div");
    row.className = "expr-eval-row";
    const answerPanel = document.createElement("div");
    answerPanel.className = "panel expr-answer-panel";
    const answerTitle = document.createElement("div");
    answerTitle.className = "panel-title";
    answerTitle.textContent = "Expression result";
    const answerStack = document.createElement("div");
    answerStack.className = "expr-answer-stack";
    const answerResult = document.createElement("div");
    answerResult.dataset.role = "expr-answer-result";
    answerResult.className = "expr-answer-result hidden";
    const answerSlot = document.createElement("div");
    answerSlot.dataset.role = "expr-answer-slot";
    answerSlot.className = "expr-answer-slot";
    const toggleWrap = document.createElement("div");
    toggleWrap.dataset.role = "expr-answer-toggle";
    toggleWrap.className = "expr-answer-toggle";
    const useSelectedBtn = document.createElement("button");
    useSelectedBtn.type = "button";
    useSelectedBtn.dataset.role = "expr-use-selected";
    useSelectedBtn.textContent = "From state";
    const useEnteredBtn = document.createElement("button");
    useEnteredBtn.type = "button";
    useEnteredBtn.dataset.role = "expr-use-entered";
    useEnteredBtn.textContent = "Type box";
    toggleWrap.appendChild(useSelectedBtn);
    toggleWrap.appendChild(useEnteredBtn);
    const controls = document.createElement("div");
    controls.className = "controls";
    const prevBtn = document.createElement("button");
    prevBtn.dataset.stepper = "prev";
    prevBtn.textContent = "Back ◀";
    const checkBtn = document.createElement("button");
    checkBtn.dataset.role = "expr-check";
    checkBtn.textContent = "Check";
    const controlsBreak = document.createElement("span");
    controlsBreak.className = "controls-break";
    const nextBtn = document.createElement("button");
    nextBtn.dataset.stepper = "next";
    nextBtn.textContent = "Next ▶";
    const statusEl = document.createElement("span");
    statusEl.dataset.role = "expr-status";
    statusEl.className = "muted expr-check-inline";
    controls.appendChild(checkBtn);
    const hintBtn = document.createElement("button");
    hintBtn.type = "button";
    hintBtn.dataset.role = "expr-hint-btn";
    hintBtn.textContent = "Hint";
    controls.appendChild(hintBtn);
    controls.appendChild(statusEl);
    controls.appendChild(controlsBreak);
    controls.appendChild(prevBtn);
    controls.appendChild(nextBtn);
    answerStack.appendChild(answerResult);
    answerStack.appendChild(answerSlot);
    answerStack.appendChild(toggleWrap);
    answerStack.appendChild(controls);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "expr-hint";
    hintPanel.className = "hint-inline hidden";
    answerStack.appendChild(hintPanel);
    answerPanel.appendChild(answerTitle);
    answerPanel.appendChild(answerStack);
    const statePanel = document.createElement("div");
    statePanel.className = "panel program-state-panel";
    statePanel.dataset.role = "expr-state-panel";
    const stateTitle = document.createElement("div");
    stateTitle.className = "panel-title";
    stateTitle.textContent = "Program state";
    const stageEl = document.createElement("div");
    stageEl.dataset.role = "expr-stage";
    stageEl.className = "grid";
    statePanel.appendChild(stateTitle);
    statePanel.appendChild(stageEl);
    leftCol.appendChild(exprPanel);
    leftCol.appendChild(answerPanel);
    row.appendChild(leftCol);
    row.appendChild(statePanel);
    stack.appendChild(row);
    return collectExpressionElements();
}
function createExpressionEvalTemplate(config) {
    const { steps = [], initialInstructions = "", next = null, isLast = false, } = config;
    const endLabel = (() => {
        if (isLast)
            return "Finish";
        const label = getNavLabelForHref(next);
        return label ? `Next: ${label}` : "Next Page";
    })();
    const failConfig = (message) => {
        alert(message);
        throw new Error(message);
    };
    if (!Array.isArray(steps) || !steps.length) {
        failConfig("Expression template requires at least one step.");
    }
    const { instructionsEl, continueBtn, sectionEl, expressionEl, answerSlot, answerResult, toggleWrap, hintPanel, hintBtn, useSelectedBtn, useEnteredBtn, checkBtn, statusEl, stageEl, statePanel, } = ensureExpressionLayout();
    const simulator = createSimpleSimulator({
        allowVarAssign: true,
        requireSourceValue: true,
        allowPointers: true,
    });
    const state = {
        boundary: 0,
        passes: {},
        selections: {},
        entered: {},
        modes: {},
        showIntro: !!initialInstructions,
    };
    let pager = null;
    let selectedName = null;
    let activeMode = "selected";
    function stepFor(index) {
        return steps[Math.max(0, Math.min(steps.length - 1, index))];
    }
    function setStatus(text, ok, silent = false) {
        if (!statusEl)
            return;
        statusEl.textContent = text;
        statusEl.className = `${ok ? "ok" : "err"} expr-check-inline`;
        if (!silent)
            flashStatus(statusEl);
    }
    function clearStatus() {
        if (!statusEl)
            return;
        statusEl.textContent = "";
        statusEl.className = "muted expr-check-inline";
    }
    function showHint(text) {
        if (!hintPanel)
            return;
        setPartsContent(hintPanel, text ?? null);
        hintPanel.classList.remove("hidden");
        flashStatus(hintPanel);
    }
    function hideHint() {
        if (!hintPanel)
            return;
        hintPanel.classList.add("hidden");
    }
    function setActiveMode(mode, editable) {
        activeMode = mode;
        if (useSelectedBtn) {
            useSelectedBtn.classList.toggle("is-active", mode === "selected");
            useSelectedBtn.setAttribute("aria-pressed", mode === "selected" ? "true" : "false");
            useSelectedBtn.disabled = !editable;
        }
        if (useEnteredBtn) {
            useEnteredBtn.classList.toggle("is-active", mode === "entered");
            useEnteredBtn.setAttribute("aria-pressed", mode === "entered" ? "true" : "false");
            useEnteredBtn.disabled = !editable;
        }
    }
    function updateSelectionHighlight(name, enabled) {
        if (!stageEl)
            return;
        stageEl.querySelectorAll(".vbox").forEach((node) => {
            const el = node;
            const isSelected = enabled && !!name && el.dataset.name === name;
            el.classList.toggle("quiz-selected", isSelected);
        });
    }
    function selectedBox(step) {
        if (!selectedName)
            return null;
        const boxes = step.boxes ?? [];
        return boxes.find((box) => box.name === selectedName) || null;
    }
    function getFixedMode(step) {
        if (!step.editable || !step.fixValueCategory)
            return null;
        const evaluated = evaluatedAnswer(step);
        if ("result" in evaluated && evaluated.result) {
            return evaluated.result.kind === "lvalue" ? "selected" : "entered";
        }
        return null;
    }
    function renderSelectedPreview(step) {
        if (!answerSlot)
            return;
        answerSlot.innerHTML = "";
        const box = selectedBox(step);
        if (!box) {
            const msg = document.createElement("div");
            msg.className = "expr-answer-empty muted";
            msg.textContent = "No box selected yet.";
            answerSlot.appendChild(msg);
            return;
        }
        const node = vbox({
            address: box.address ?? undefined,
            type: box.type,
            value: box.value,
            name: box.name,
            editable: false,
        });
        if (String(box.value ?? "") === "")
            node.querySelector(".value")?.classList.add("placeholder", "muted");
        answerSlot.appendChild(node);
    }
    function renderEnteredBox(saved, editable) {
        if (!answerSlot)
            return;
        answerSlot.innerHTML = "";
        const node = vbox({
            address: "—",
            type: saved?.type ?? "",
            value: saved?.rawValue ?? saved?.value ?? "",
            name: "",
            editable,
            allowNameEdit: false,
            allowTypeEdit: editable,
            showDoubleExact: saved?.showDoubleExact ?? false,
        });
        node.classList.add("no-name", "no-addr");
        answerSlot.appendChild(node);
    }
    function setSelected(name) {
        selectedName = name;
        const step = stepFor(state.boundary);
        updateSelectionHighlight(name, activeMode === "selected");
        if (activeMode === "selected")
            renderSelectedPreview(step);
    }
    function renderProgramState(step, editable, fixedMode) {
        if (!stageEl)
            return;
        stageEl.innerHTML = "";
        const boxes = step.boxes ?? [];
        const disableHover = !editable || fixedMode === "entered";
        boxes.forEach((box) => {
            const node = vbox({
                address: box.address ?? undefined,
                type: box.type,
                value: box.value,
                name: box.name,
                editable: false,
            });
            if (String(box.value ?? "") === "")
                node.querySelector(".value")?.classList.add("placeholder", "muted");
            node.dataset.name = box.name;
            node.classList.add("quiz-selectable");
            if (editable) {
                if (!disableHover) {
                    node.addEventListener("click", () => setSelected(box.name));
                }
            }
            else {
                node.classList.add("quiz-static");
            }
            if (disableHover)
                node.classList.add("quiz-static");
            stageEl.appendChild(node);
        });
        if (editable) {
            updateSelectionHighlight(selectedName, activeMode === "selected");
        }
        else {
            const evaluated = evaluatedAnswer(step);
            if ("result" in evaluated &&
                evaluated.result &&
                evaluated.result.kind === "lvalue") {
                const match = boxes.find((box) => String(box.address) === evaluated.result.address) || null;
                updateSelectionHighlight(match?.name ?? null, true);
            }
        }
    }
    function evaluatedAnswer(step) {
        const evaluated = simulator.evaluateExpressionText(step.expression, step.boxes ?? []);
        if ("error" in evaluated) {
            const error = evaluated.error ?? "That expression is not valid here.";
            return { error, kind: evaluated.kind };
        }
        return { result: evaluated.result };
    }
    function readEnteredBox() {
        const node = answerSlot?.querySelector(".vbox") || null;
        return node ? readBoxState(node) : null;
    }
    function expectedAnswer(step) {
        const evaluated = evaluatedAnswer(step);
        if ("error" in evaluated) {
            return { kind: "error", message: errorText(evaluated.error) };
        }
        if (evaluated.result.kind === "lvalue") {
            return { kind: "lvalue", address: String(evaluated.result.address || "") };
        }
        const type = evaluated.result.type || "int";
        const value = formatValueForType(evaluated.result.value ?? "", type, {
            nanSign: evaluated.result.nanSign,
        });
        return { kind: "rvalue", type, value };
    }
    function buildHintContext(step) {
        const expected = expectedAnswer(step);
        const selected = selectedBox(step);
        const entered = readEnteredBox();
        const enteredForContext = entered
            ? normalizeBoxValueForContext(simulator, entered)
            : null;
        let ok = false;
        if (expected.kind === "lvalue") {
            ok =
                activeMode === "selected" &&
                    !!selected &&
                    String(selected.address) === expected.address;
        }
        else if (expected.kind === "rvalue") {
            const expectedBox = {
                name: "",
                type: expected.type,
                value: expected.value,
            };
            ok =
                activeMode === "entered" &&
                    !!entered &&
                    String(entered.type || "").trim() === expected.type &&
                    boxValueMatchesSpec(simulator, entered, expectedBox).ok;
        }
        return {
            step,
            mode: activeMode,
            selectedName,
            selectedBox: selected,
            enteredBox: enteredForContext,
            expected,
            ok,
            hasState: Array.isArray(step.boxes),
        };
    }
    function errorText(error) {
        if (!error)
            return "That expression is not valid here.";
        return typeof error === "string" ? error : error.text;
    }
    function renderCorrectAnswer(step) {
        if (!answerResult)
            return;
        answerResult.innerHTML = "";
        const evaluated = evaluatedAnswer(step);
        if ("error" in evaluated) {
            const msg = document.createElement("div");
            msg.className = "expr-answer-empty muted";
            msg.textContent = errorText(evaluated.error);
            answerResult.appendChild(msg);
            return;
        }
        const { result } = evaluated;
        if (result.kind === "lvalue") {
            const match = (step.boxes ?? []).find((box) => String(box.address) === result.address) || null;
            const node = match
                ? vbox({
                    address: match.address ?? undefined,
                    type: match.type,
                    value: match.value,
                    name: match.name,
                    editable: false,
                })
                : vbox({
                    address: result.address || "—",
                    type: result.type,
                    value: formatValueForType(result.value ?? "", result.type, {
                        nanSign: result.nanSign,
                    }),
                    name: "",
                    editable: false,
                });
            if (match && String(match.value ?? "") === "") {
                node.querySelector(".value")?.classList.add("placeholder", "muted");
            }
            if (!match)
                node.classList.add("no-name");
            answerResult.appendChild(node);
            return;
        }
        const node = vbox({
            address: "—",
            type: result.type || "int",
            value: formatValueForType(result.value ?? "", result.type || "int", {
                nanSign: result.nanSign,
            }),
            name: "",
            editable: false,
        });
        node.classList.add("no-name", "no-addr");
        answerResult.appendChild(node);
    }
    function saveStep(index) {
        const step = stepFor(index);
        if (!step.editable)
            return;
        state.selections[index] = selectedName;
        if (activeMode === "entered" && answerSlot) {
            const node = answerSlot.querySelector(".vbox");
            state.entered[index] = node ? readBoxState(node) : null;
        }
        state.modes[index] = activeMode;
    }
    function isStepLocked(index) {
        const step = stepFor(index);
        if (!step.editable)
            return false;
        return !state.passes[index];
    }
    function render() {
        const step = stepFor(state.boundary);
        const hasState = Array.isArray(step.boxes);
        if (state.showIntro) {
            setPartsContent(instructionsEl, initialInstructions || null);
            if (continueBtn)
                continueBtn.classList.remove("hidden");
            if (sectionEl)
                sectionEl.classList.add("hidden");
            if (hintBtn)
                hintBtn.classList.add("hidden");
            hideHint();
            return;
        }
        if (continueBtn)
            continueBtn.classList.add("hidden");
        if (sectionEl)
            sectionEl.classList.remove("hidden");
        const instructionText = step.instructions ?? null;
        setPartsContent(instructionsEl, instructionText);
        if (expressionEl)
            expressionEl.textContent = step.expression;
        clearStatus();
        hideHint();
        selectedName = state.selections[state.boundary] ?? null;
        const savedEntered = state.entered[state.boundary] ?? null;
        activeMode = state.modes[state.boundary] ?? "selected";
        const fixedMode = getFixedMode(step);
        if (fixedMode) {
            activeMode = fixedMode;
            state.modes[state.boundary] = fixedMode;
        }
        const passed = !!state.passes[state.boundary];
        if (statePanel) {
            statePanel.classList.toggle("hidden", !hasState);
        }
        if (step.editable) {
            const canEdit = !passed;
            answerResult?.classList.add("hidden");
            answerSlot?.classList.remove("hidden");
            if (fixedMode)
                toggleWrap?.classList.add("hidden");
            else
                toggleWrap?.classList.remove("hidden");
            if (checkBtn) {
                if (passed)
                    checkBtn.classList.add("hidden");
                else
                    checkBtn.classList.remove("hidden");
            }
            if (hintBtn) {
                if (!passed && step.hints)
                    hintBtn.classList.remove("hidden");
                else
                    hintBtn.classList.add("hidden");
            }
            if (passed)
                setStatus("correct", true, true);
            if (hasState) {
                renderProgramState(step, canEdit, fixedMode);
            }
            if (activeMode === "selected")
                renderSelectedPreview(step);
            else
                renderEnteredBox(savedEntered, canEdit);
            setActiveMode(activeMode, canEdit);
        }
        else {
            answerSlot?.classList.add("hidden");
            toggleWrap?.classList.add("hidden");
            if (checkBtn)
                checkBtn.classList.add("hidden");
            if (hintBtn)
                hintBtn.classList.add("hidden");
            if (hasState) {
                renderProgramState(step, false, fixedMode);
            }
            answerResult?.classList.remove("hidden");
            renderCorrectAnswer(step);
        }
        if (step.editable && passed) {
            if (checkBtn)
                checkBtn.disabled = true;
        }
        else if (checkBtn) {
            checkBtn.disabled = false;
        }
        pager?.update();
    }
    function checkAnswer() {
        hideHint();
        const step = stepFor(state.boundary);
        if (!step.editable)
            return;
        const evaluation = evaluatedAnswer(step);
        if ("error" in evaluation) {
            setStatus(errorText(evaluation.error), false);
            return;
        }
        if (activeMode === "selected") {
            const chosen = selectedBox(step);
            if (!chosen) {
                setStatus("Pick a box first.", false);
                return;
            }
            if (evaluation.result.kind !== "lvalue") {
                setStatus("Incorrect.", false);
                return;
            }
            const ok = String(chosen.address) === evaluation.result.address;
            setStatus(ok ? "correct" : "incorrect", ok);
            if (ok) {
                state.passes[state.boundary] = true;
                state.selections[state.boundary] = selectedName;
                if (checkBtn)
                    checkBtn.classList.add("hidden");
                if (hintBtn)
                    hintBtn.classList.add("hidden");
                setActiveMode(activeMode, false);
                disableBoxEditing(answerSlot);
                pager?.pulseNext();
                pager?.update();
            }
            return;
        }
        const entryNode = answerSlot?.querySelector(".vbox") || null;
        const entry = entryNode ? readBoxState(entryNode) : null;
        if (!entry) {
            setStatus("Enter a box first.", false);
            return;
        }
        if (evaluation.result.kind !== "rvalue") {
            setStatus("Incorrect.", false);
            return;
        }
        const expectedType = String(evaluation.result.type || "").trim();
        const expectedValue = formatValueForType(evaluation.result.value ?? "", evaluation.result.type || "int", { nanSign: evaluation.result.nanSign });
        const entryType = String(entry.type || "").trim();
        const expectedBox = {
            name: "",
            type: expectedType,
            value: expectedValue,
        };
        const match = boxValueMatchesSpec(simulator, entry, expectedBox);
        const ok = entryType === expectedType && match.ok;
        setStatus(ok ? "correct" : "incorrect", ok);
        if (ok) {
            entry.value = match.normalized;
            entry.rawValue = match.normalized;
            state.passes[state.boundary] = true;
            state.entered[state.boundary] = entry;
            if (checkBtn)
                checkBtn.classList.add("hidden");
            if (hintBtn)
                hintBtn.classList.add("hidden");
            setActiveMode(activeMode, false);
            renderEnteredBox(entry, false);
            disableBoxEditing(answerSlot);
            pager?.pulseNext();
            pager?.update();
        }
    }
    if (useSelectedBtn) {
        useSelectedBtn.addEventListener("click", () => {
            const step = stepFor(state.boundary);
            if (!step.editable || step.fixValueCategory)
                return;
            if (activeMode === "entered") {
                const node = answerSlot?.querySelector(".vbox") || null;
                if (node)
                    state.entered[state.boundary] = readBoxState(node);
            }
            setActiveMode("selected", true);
            state.modes[state.boundary] = "selected";
            updateSelectionHighlight(selectedName, true);
            renderSelectedPreview(stepFor(state.boundary));
        });
    }
    if (useEnteredBtn) {
        useEnteredBtn.addEventListener("click", () => {
            const step = stepFor(state.boundary);
            if (!step.editable || step.fixValueCategory)
                return;
            if (activeMode === "selected") {
                state.selections[state.boundary] = selectedName;
            }
            setActiveMode("entered", true);
            state.modes[state.boundary] = "entered";
            updateSelectionHighlight(selectedName, false);
            renderEnteredBox(state.entered[state.boundary] ?? null, true);
        });
    }
    if (checkBtn) {
        checkBtn.addEventListener("click", checkAnswer);
    }
    if (continueBtn) {
        continueBtn.addEventListener("click", () => {
            state.showIntro = false;
            render();
        });
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            const step = stepFor(state.boundary);
            if (!step.editable || !step.hints)
                return;
            const ctx = buildHintContext(step);
            if (ctx.ok) {
                showHint("Looks good. Press $b{Check}.");
                return;
            }
            const parts = typeof step.hints === "function" ? step.hints(ctx) : step.hints;
            if (!parts || (Array.isArray(parts) && parts.length === 0)) {
                showHint("Your program has a problem that isn't covered by a hint, sorry.");
                return;
            }
            showHint(parts);
        });
    }
    pager = createStepper({
        root: document,
        lines: Math.max(0, steps.length - 1),
        nextPage: next,
        getBoundary: () => state.boundary,
        setBoundary: (value) => {
            state.boundary = value;
        },
        isStepLocked: (at) => isStepLocked(at),
        onBeforeChange: (current) => saveStep(current),
        onAfterChange: () => render(),
        getNextLabel: (_boundary, _total, atEnd) => {
            if (atEnd)
                return endLabel;
            return "Next";
        },
        endLabel,
    });
    render();
}
export { createExpressionEvalTemplate };

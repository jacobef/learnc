import { appendStateObjects, bindBtnRefPulse, clearNode, createStepper, disableBoxEditing, ensurePanelizedMain, isMobileViewport, findArrayObjectBoxesForResult, flashStatus, getNavLabelForHref, getPreviousNavHref, readBoxState, queryRole, setPartsContent, setStepperLabel, syncDocumentTitleFromNav, vbox, withSidebarParam, } from "./shared-core.js";
import { createSyntheticAddressBase } from "./shared-c-address-space.js";
import { evaluateCExpression, runCProgram, } from "./shared-c-interpreter.js";
import { boxValueMatchesSpec, normalizeBoxValueForContext, } from "./shared-c-value-semantics.js";
import { createButtonTokenReplacer, createHintPresenter, createLevelProgressController, invalidTemplateConfig, nextLessonLabel, resetLevelAfterConfirmation, } from "./shared-lesson-runtime.js";
function collectExpressionElements(root = document) {
    const role = (name) => queryRole(name, root);
    return {
        instructionsEl: role("expr-instructions"),
        previousBtn: root.querySelector('[data-stepper="prev"]'),
        nextBtn: root.querySelector('[data-stepper="next"]'),
        continueBtn: role("expr-continue"),
        levelResetBtn: role("expr-reset-level"),
        sectionEl: role("expr-section"),
        exerciseStack: root.querySelector(".expr-eval-stack"),
        expressionEl: role("expr-expression"),
        answerPanel: role("expr-answer-panel"),
        answerActions: role("expr-answer-actions"),
        answerSlot: role("expr-answer-slot"),
        answerResult: role("expr-answer-result"),
        toggleWrap: role("expr-answer-toggle"),
        hintPanel: role("expr-hint"),
        hintBtn: role("expr-hint-btn"),
        useSelectedBtn: role("expr-use-selected"),
        useEnteredBtn: role("expr-use-entered"),
        useUndefinedBtn: role("expr-use-undefined"),
        checkBtn: role("expr-check"),
        statusEl: role("expr-status"),
        stageEl: role("expr-stage"),
        statePanel: role("expr-state-panel"),
    };
}
function ensureExpressionLayout() {
    const resolvedTitle = syncDocumentTitleFromNav();
    const existing = queryRole("expr-expression");
    if (existing)
        return collectExpressionElements();
    const main = ensurePanelizedMain(resolvedTitle);
    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "expr-instructions";
    instructionsEl.className = "intro";
    const continueBtn = document.createElement("button");
    continueBtn.type = "button";
    continueBtn.dataset.role = "expr-continue";
    continueBtn.textContent = "Continue";
    const section = document.createElement("section");
    section.dataset.role = "expr-section";
    section.classList.add("panel-shell");
    const actionBar = document.createElement("div");
    actionBar.className = "controls-bar controls-bar-expr";
    const controlsMain = document.createElement("div");
    controlsMain.className = "controls-main panel panel-controls";
    const controlsRow = document.createElement("div");
    controlsRow.className = "controls-row controls-left";
    controlsMain.appendChild(controlsRow);
    actionBar.appendChild(controlsMain);
    section.appendChild(actionBar);
    const stack = document.createElement("div");
    stack.className = "expr-eval-stack panel-stack";
    section.appendChild(stack);
    main.appendChild(section);
    const exprPanel = document.createElement("div");
    exprPanel.className = "panel quiz-expression-panel panel-scroll";
    const exprTitle = document.createElement("div");
    exprTitle.className = "panel-title";
    exprTitle.textContent = "Expression";
    const exprDisplay = document.createElement("div");
    exprDisplay.dataset.role = "expr-expression";
    exprDisplay.className = "expr-display expr-quiz-display panel-body";
    exprPanel.appendChild(exprTitle);
    exprPanel.appendChild(exprDisplay);
    const leftCol = document.createElement("div");
    leftCol.className = "expr-left-col panel-col";
    const row = document.createElement("div");
    row.className = "expr-eval-row panel-row";
    const answerPanel = document.createElement("div");
    answerPanel.dataset.role = "expr-answer-panel";
    answerPanel.className = "panel expr-answer-panel panel-scroll";
    const answerTitle = document.createElement("div");
    answerTitle.className = "panel-title";
    answerTitle.textContent = "Expression result";
    const answerStack = document.createElement("div");
    answerStack.className = "expr-answer-stack panel-body";
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
    const useUndefinedBtn = document.createElement("button");
    useUndefinedBtn.type = "button";
    useUndefinedBtn.dataset.role = "expr-use-undefined";
    useUndefinedBtn.textContent = "Undefined";
    toggleWrap.append(useSelectedBtn, useEnteredBtn, useUndefinedBtn);
    const prevBtn = document.createElement("button");
    prevBtn.dataset.stepper = "prev";
    prevBtn.textContent = "Back ◀";
    const checkBtn = document.createElement("button");
    checkBtn.dataset.role = "expr-check";
    checkBtn.textContent = "Check";
    const nextBtn = document.createElement("button");
    nextBtn.dataset.stepper = "next";
    nextBtn.textContent = "Next ▶";
    const levelResetBtn = document.createElement("button");
    levelResetBtn.type = "button";
    levelResetBtn.dataset.role = "expr-reset-level";
    levelResetBtn.textContent = "Reset level";
    const controlsSpacer = document.createElement("span");
    controlsSpacer.className = "controls-spacer";
    controlsSpacer.setAttribute("aria-hidden", "true");
    const statusEl = document.createElement("span");
    statusEl.dataset.role = "expr-status";
    statusEl.className = "muted expr-check-inline";
    const hintBtn = document.createElement("button");
    hintBtn.type = "button";
    hintBtn.dataset.role = "expr-hint-btn";
    hintBtn.textContent = "Hint";
    controlsRow.appendChild(prevBtn);
    controlsRow.appendChild(nextBtn);
    controlsRow.appendChild(controlsSpacer);
    controlsRow.appendChild(levelResetBtn);
    controlsRow.appendChild(continueBtn);
    controlsRow.appendChild(hintBtn);
    controlsRow.appendChild(checkBtn);
    controlsRow.appendChild(statusEl);
    answerStack.appendChild(answerResult);
    answerStack.appendChild(answerSlot);
    answerStack.appendChild(toggleWrap);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "expr-hint";
    hintPanel.className = "hint-inline hidden";
    actionBar.appendChild(hintPanel);
    actionBar.appendChild(instructionsEl);
    answerPanel.appendChild(answerTitle);
    answerPanel.appendChild(answerStack);
    const statePanel = document.createElement("div");
    statePanel.className = "panel program-state-panel panel-scroll";
    statePanel.dataset.role = "expr-state-panel";
    const stateTitle = document.createElement("div");
    stateTitle.className = "panel-title";
    stateTitle.textContent = "Program state";
    const stageEl = document.createElement("div");
    stageEl.dataset.role = "expr-stage";
    stageEl.className = "grid panel-body";
    statePanel.appendChild(stateTitle);
    statePanel.appendChild(stageEl);
    leftCol.appendChild(exprPanel);
    leftCol.appendChild(answerPanel);
    row.appendChild(leftCol);
    row.appendChild(statePanel);
    stack.appendChild(row);
    const answerActions = document.createElement("div");
    answerActions.className = "controls expression-mobile-actions";
    answerActions.dataset.role = "expr-answer-actions";
    answerPanel.appendChild(answerActions);
    const placeActions = () => {
        const mobile = isMobileViewport();
        const target = mobile ? answerActions : controlsRow;
        target.append(hintBtn, checkBtn, statusEl);
        if (mobile)
            answerActions.appendChild(hintPanel);
        else
            actionBar.insertBefore(hintPanel, instructionsEl);
    };
    placeActions();
    window.matchMedia("(max-width: 900px)").addEventListener("change", placeActions);
    return collectExpressionElements();
}
function createExpressionEvalTemplate(config) {
    const { steps = [], initialInstructions = "", next = null, nextLabel, workspace = {}, } = config;
    const alwaysShowExprResult = workspace.alwaysShowExprResult !== false;
    const endLabel = nextLessonLabel({
        next,
        fallback: "Next Page",
        override: nextLabel,
    });
    if (!Array.isArray(steps) || !steps.length) {
        invalidTemplateConfig("Expression template requires at least one step.");
    }
    const normalizedSteps = steps.map((step) => ({
        ...step,
        editable: step.editable === true,
    }));
    const usedAddressBases = new Set();
    const addressBases = normalizedSteps.map(() => {
        let addressBase = createSyntheticAddressBase();
        while (usedAddressBases.has(addressBase)) {
            addressBase = createSyntheticAddressBase();
        }
        usedAddressBases.add(addressBase);
        return addressBase;
    });
    const addressBaseByStep = new Map(normalizedSteps.map((step, index) => [step, addressBases[index]]));
    const { instructionsEl, previousBtn, nextBtn, continueBtn, levelResetBtn, sectionEl, exerciseStack, expressionEl, answerPanel, answerActions, answerSlot, answerResult, toggleWrap, hintPanel, hintBtn, useSelectedBtn, useEnteredBtn, useUndefinedBtn, checkBtn, statusEl, stageEl, statePanel, } = ensureExpressionLayout();
    bindBtnRefPulse(sectionEl || document);
    let pager = null;
    let selectedName = null;
    let activeMode = "selected";
    function stepFor(index) {
        return normalizedSteps[Math.max(0, Math.min(normalizedSteps.length - 1, index))];
    }
    function addressBaseForStep(step) {
        return addressBaseByStep.get(step) ?? addressBases[0];
    }
    function clampBoundary(value) {
        return Math.max(0, Math.min(normalizedSteps.length - 1, Math.floor(value)));
    }
    function sanitizedRecord(value) {
        if (!value || typeof value !== "object")
            return {};
        const entries = Object.entries(value).filter(([key]) => /^\d+$/.test(key));
        return Object.fromEntries(entries);
    }
    function sanitizedPassedSelections(value, passes) {
        if (!value || typeof value !== "object")
            return {};
        const entries = Object.entries(value).filter(([key, selected]) => passes[Number(key)] === true && typeof selected === "string");
        return Object.fromEntries(entries);
    }
    // The answer modes offered for a step, default first. An undefined-behavior
    // answer is offered alongside the fixed category, so fixing it gives nothing away.
    function answerModes(step) {
        if (!step.editable)
            return [];
        const undefinedModes = workspace.allowUndefinedAnswer ? ["undefined"] : [];
        if (!step.fixValueCategory)
            return ["selected", "entered", ...undefinedModes];
        const evaluated = evaluatedAnswer(step);
        const lvalue = "result" in evaluated && evaluated.result?.kind === "lvalue";
        return [lvalue ? "selected" : "entered", ...undefinedModes];
    }
    function defaultModeForStep(index) {
        return answerModes(stepFor(index))[0] ?? "selected";
    }
    const progress = createLevelProgressController(isDefaultProgress);
    const defaultShowIntro = !!initialInstructions;
    const restoredProgress = progress.restore();
    const restoredPasses = sanitizedRecord(restoredProgress?.passes);
    const state = {
        boundary: clampBoundary(restoredProgress?.boundary ?? 0),
        passes: restoredPasses,
        selections: sanitizedPassedSelections(restoredProgress?.selections, restoredPasses),
        entered: sanitizedRecord(restoredProgress?.entered),
        modes: sanitizedRecord(restoredProgress?.modes),
        showIntro: typeof restoredProgress?.showIntro === "boolean"
            ? restoredProgress.showIntro
            : defaultShowIntro,
    };
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
    function setControlVisible(control, visible) {
        if (!control)
            return;
        control.classList.toggle("hidden", !visible);
    }
    function visibleButtonLabel(button, fallback) {
        return (button?.textContent || fallback).trim();
    }
    function nextButtonEl() {
        const button = document.querySelector('button[data-stepper="next"]');
        return button instanceof HTMLButtonElement ? button : null;
    }
    function previousButtonEl() {
        const button = document.querySelector('button[data-stepper="prev"]');
        return button instanceof HTMLButtonElement ? button : null;
    }
    function buttonReplacements() {
        const nextLabel = visibleButtonLabel(nextButtonEl(), "Next ▶");
        const previousLabel = visibleButtonLabel(previousButtonEl(), "Back ◀");
        return [
            ["$nextButton", `$b{${nextLabel}}`],
            ["$runLineButton", `$b{${nextLabel}}`],
            ["$backButton", `$b{${previousLabel}}`],
            ["$checkButton", "$b{Check}"],
            ["$hintButton", "$b{Hint}"],
            ["$resetButton", "$b{Reset level}"],
            ["$continueButton", "$b{Continue}"],
            ["$fromStateButton", "$b{From state}"],
            ["$typeBoxButton", "$b{Type box}"],
            ["$undefinedButton", "$b{Undefined}"],
        ];
    }
    const applyButtonTokens = createButtonTokenReplacer(buttonReplacements);
    const { hide: hideHint, show: showHint } = createHintPresenter(hintPanel, applyButtonTokens);
    function renderInstructions(parts) {
        setPartsContent(instructionsEl, applyButtonTokens(parts));
    }
    function setActiveMode(mode, editable) {
        activeMode = mode;
        const modes = answerModes(stepFor(state.boundary));
        const buttons = [
            [useSelectedBtn, "selected"],
            [useEnteredBtn, "entered"],
            [useUndefinedBtn, "undefined"],
        ];
        for (const [button, buttonMode] of buttons) {
            if (!button)
                continue;
            button.classList.toggle("hidden", !modes.includes(buttonMode));
            button.classList.toggle("is-active", mode === buttonMode);
            button.setAttribute("aria-pressed", mode === buttonMode ? "true" : "false");
            button.disabled = !editable;
        }
    }
    function updateSelectionHighlight(name, enabled) {
        for (const node of selectableNodes()) {
            node.classList.toggle("quiz-selected", enabled && !!name && node.dataset.selectName === name);
        }
    }
    function sourceForStep(step) {
        const setup = String(step.setup ?? "").trim();
        return setup ? `${setup}\n0;` : "0;";
    }
    function runtimeForStep(step) {
        const source = sourceForStep(step);
        const addressBase = addressBaseForStep(step);
        const result = runCProgram(source, addressBase);
        if (result.kind !== "ok")
            return { error: true, kind: result.kind };
        const trace = result.trace || [];
        if (!trace.length) {
            return {
                source,
                state: result.state,
                eventIndex: 0,
                addressBase,
            };
        }
        const eventIndex = trace.length - 1;
        return {
            source,
            state: trace[eventIndex]?.state ?? result.state,
            eventIndex,
            addressBase,
        };
    }
    function boxesForStep(step) {
        const runtime = runtimeForStep(step);
        return "error" in runtime ? [] : runtime.state;
    }
    function selectedBox(step) {
        if (!selectedName)
            return null;
        const boxes = boxesForStep(step);
        return boxes.find((box) => box.name === selectedName) || null;
    }
    function renderSelectedPreview(step) {
        if (!answerSlot)
            return;
        clearNode(answerSlot);
        const box = selectedBox(step);
        if (!box) {
            const msg = document.createElement("div");
            msg.className = "expr-answer-empty muted";
            msg.textContent = "No box selected yet.";
            answerSlot.appendChild(msg);
            return;
        }
        appendStateObjects(answerSlot, objectBoxes(step, box), { editable: false, deletable: false });
    }
    // A box together with its elements, if it's an array.
    function objectBoxes(step, box) {
        if (box.arrayRoot || !box.arrayShape?.length)
            return [box];
        return boxesForStep(step).filter((other) => other === box || other.arrayRoot === box.name);
    }
    function undefinedAnswerNode() {
        const node = document.createElement("div");
        node.className = "expr-answer-undefined";
        node.textContent = "Undefined behavior";
        return node;
    }
    function renderUndefinedAnswer() {
        if (!answerSlot)
            return;
        clearNode(answerSlot);
        answerSlot.appendChild(undefinedAnswerNode());
    }
    function instructionsFor(index) {
        const step = stepFor(index);
        const solved = !step.editable || !!state.passes[index];
        const parts = [step.instructions, solved ? step.explanation : undefined]
            .flatMap((part) => (part === undefined ? [] : Array.isArray(part) ? part : [part]));
        return parts.length ? parts.join("\n") : null;
    }
    function renderEnteredBox(saved, editable) {
        if (!answerSlot)
            return;
        clearNode(answerSlot);
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
    // The state's objects, plus each array's elements, can be picked as the answer.
    function selectableNodes() {
        if (!stageEl)
            return [];
        return [...stageEl.querySelectorAll(".vbox, .arraybox, .array-col-value")];
    }
    function selectionNameOf(node) {
        if (node.classList.contains("array-col-value"))
            return node.dataset.arrayName ?? "";
        if (node.classList.contains("arraybox"))
            return node.dataset.arrayRoot ?? "";
        return node.dataset.stateName || (node.querySelector(".name-text")?.textContent ?? "").trim();
    }
    function sameObject(box, address, type) {
        return String(box.address) === address && box.type === type;
    }
    function renderProgramState(step, editable) {
        if (!stageEl)
            return;
        clearNode(stageEl);
        const boxes = boxesForStep(step);
        appendStateObjects(stageEl, boxes, { editable: false, deletable: false });
        const selectable = editable && answerModes(step).includes("selected");
        for (const node of selectableNodes()) {
            node.dataset.selectName = selectionNameOf(node);
            node.classList.add("quiz-selectable");
            if (!selectable) {
                node.classList.add("quiz-static");
                continue;
            }
            node.addEventListener("click", (event) => {
                // A click on an element shouldn't also select its whole array.
                event.stopPropagation();
                if (activeMode !== "selected")
                    switchAnswerMode("selected");
                setSelected(node.dataset.selectName || null);
            });
        }
        if (editable) {
            updateSelectionHighlight(selectedName, activeMode === "selected");
        }
        else {
            const evaluated = evaluatedAnswer(step);
            if ("result" in evaluated && evaluated.result.kind === "lvalue") {
                const { address, type } = evaluated.result;
                const match = boxes.find((box) => sameObject(box, address, type)) || null;
                updateSelectionHighlight(match?.name ?? null, true);
            }
        }
    }
    function evaluatedAnswer(step) {
        const runtime = runtimeForStep(step);
        if ("error" in runtime)
            return runtime;
        const evaluated = evaluateCExpression(runtime.source, runtime.eventIndex, step.expression, runtime.addressBase);
        if (evaluated.kind !== "ok")
            return { error: true, kind: evaluated.kind };
        return { result: evaluated.result };
    }
    function readEnteredBox() {
        const node = answerSlot?.querySelector(".vbox") || null;
        return node ? readBoxState(node) : null;
    }
    function progressSnapshot() {
        const passes = Object.fromEntries(Object.entries(state.passes).filter(([, value]) => value === true));
        const selections = Object.fromEntries(Object.entries(state.selections).filter(([key, value]) => passes[Number(key)] === true && typeof value === "string"));
        const entered = Object.fromEntries(Object.entries(state.entered).filter(([, value]) => value != null));
        const modes = Object.fromEntries(Object.entries(state.modes).filter(([key, value]) => {
            const index = Number(key);
            return value !== defaultModeForStep(index);
        }));
        return {
            boundary: clampBoundary(state.boundary),
            passes,
            selections,
            entered,
            modes,
            showIntro: state.showIntro,
        };
    }
    function persistProgress() {
        progress.save(progressSnapshot());
    }
    function isDefaultProgress(snapshot) {
        return (snapshot.boundary === 0 &&
            Object.keys(snapshot.passes).length === 0 &&
            Object.keys(snapshot.selections).length === 0 &&
            Object.keys(snapshot.entered).length === 0 &&
            Object.keys(snapshot.modes).length === 0 &&
            snapshot.showIntro === defaultShowIntro);
    }
    function expectedAnswer(step) {
        const evaluated = evaluatedAnswer(step);
        if ("error" in evaluated) {
            return { kind: "error", message: errorText(evaluated.kind) };
        }
        if (evaluated.result.kind === "lvalue") {
            return { kind: "lvalue", address: String(evaluated.result.address || "") };
        }
        const type = evaluated.result.type || "int";
        return {
            kind: "rvalue",
            type,
            value: evaluated.result.value ?? "",
            typeInfo: evaluated.result.typeInfo,
        };
    }
    function shouldShowExprResultPane(step) {
        if (alwaysShowExprResult)
            return true;
        const evaluated = evaluatedAnswer(step);
        if ("error" in evaluated)
            return true;
        if (evaluated.result.kind !== "lvalue")
            return true;
        if (step.fixValueCategory)
            return false;
        if (!step.editable)
            return false;
        return true;
    }
    function buildHintContext(step) {
        const expected = expectedAnswer(step);
        const selected = selectedBox(step);
        const entered = readEnteredBox();
        const enteredForContext = entered
            ? normalizeBoxValueForContext(entered)
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
                typeInfo: expected.typeInfo,
            };
            ok =
                activeMode === "entered" &&
                    !!entered &&
                    String(entered.type || "").trim() === expected.type &&
                    boxValueMatchesSpec(entered, expectedBox).ok;
        }
        else {
            ok = activeMode === "undefined";
        }
        return {
            step,
            mode: activeMode,
            selectedName,
            selectedBox: selected,
            enteredBox: enteredForContext,
            expected,
            ok,
            hasState: boxesForStep(step).length > 0,
        };
    }
    function errorText(kind) {
        return kind === "ub"
            ? "This expression is undefined."
            : "This expression is not valid.";
    }
    function renderCorrectAnswer(step) {
        if (!answerResult)
            return;
        clearNode(answerResult);
        const evaluated = evaluatedAnswer(step);
        if ("error" in evaluated) {
            if (evaluated.kind === "ub") {
                answerResult.appendChild(undefinedAnswerNode());
                return;
            }
            const msg = document.createElement("div");
            msg.className = "expr-answer-empty muted";
            msg.textContent = errorText(evaluated.kind);
            answerResult.appendChild(msg);
            return;
        }
        const { result } = evaluated;
        const boxes = boxesForStep(step);
        const arrayBoxes = findArrayObjectBoxesForResult(result, boxes);
        if (arrayBoxes && arrayBoxes.length) {
            const wrap = document.createElement("div");
            appendStateObjects(wrap, arrayBoxes, { editable: false, deletable: false });
            const arrayNode = wrap.querySelector(".arraybox");
            if (arrayNode) {
                answerResult.appendChild(arrayNode);
                return;
            }
        }
        if (result.kind === "lvalue") {
            const match = boxes.find((box) => sameObject(box, result.address, result.type)) || null;
            if (match) {
                appendStateObjects(answerResult, objectBoxes(step, match), { editable: false, deletable: false });
                return;
            }
            const node = vbox({
                address: result.address || "—",
                type: result.type,
                value: result.value ?? "",
                displayValue: result.displayValue,
                exactValue: result.exactValue,
                typeInfo: result.typeInfo,
                name: "",
                editable: false,
            });
            node.classList.add("no-name");
            answerResult.appendChild(node);
            return;
        }
        const node = vbox({
            address: "—",
            type: result.type || "int",
            value: result.value ?? "",
            displayValue: result.displayValue,
            exactValue: result.exactValue,
            typeInfo: result.typeInfo,
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
        if (state.passes[index] && activeMode === "selected" && selectedName) {
            state.selections[index] = selectedName;
        }
        else {
            delete state.selections[index];
        }
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
        const hasState = boxesForStep(step).length > 0;
        const showExprResultPane = shouldShowExprResultPane(step);
        pager?.update();
        if (levelResetBtn) {
            levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
        }
        if (state.showIntro) {
            renderInstructions(initialInstructions || null);
            setControlVisible(sectionEl, true);
            setControlVisible(exerciseStack, false);
            // From the intro, going back means leaving for the previous lesson.
            const previousHref = getPreviousNavHref();
            const previousLabel = getNavLabelForHref(previousHref);
            setControlVisible(previousBtn, !!previousHref);
            if (previousBtn && previousLabel) {
                setStepperLabel(previousBtn, `Prev: ${previousLabel} ◀◀`, "◀ Prev");
            }
            setControlVisible(nextBtn, false);
            setControlVisible(continueBtn, true);
            setControlVisible(levelResetBtn, false);
            setControlVisible(checkBtn, false);
            setControlVisible(hintBtn, false);
            hideHint();
            persistProgress();
            return;
        }
        setControlVisible(exerciseStack, true);
        setControlVisible(previousBtn, true);
        setControlVisible(nextBtn, true);
        setControlVisible(continueBtn, false);
        setControlVisible(sectionEl, true);
        setControlVisible(levelResetBtn, true);
        setControlVisible(answerPanel, showExprResultPane);
        // Selection-only lessons hide the answer panel, but still need mobile actions.
        const actionsPanel = showExprResultPane ? answerPanel : expressionEl?.parentElement;
        if (answerActions && actionsPanel)
            actionsPanel.appendChild(answerActions);
        renderInstructions(instructionsFor(state.boundary));
        if (expressionEl)
            expressionEl.textContent = step.expression;
        clearStatus();
        hideHint();
        selectedName = state.selections[state.boundary] ?? null;
        const savedEntered = state.entered[state.boundary] ?? null;
        const modes = answerModes(step);
        const savedMode = state.modes[state.boundary];
        activeMode = savedMode && modes.includes(savedMode) ? savedMode : modes[0] ?? "selected";
        const passed = !!state.passes[state.boundary];
        if (statePanel) {
            statePanel.classList.toggle("hidden", !hasState);
        }
        if (step.editable) {
            const canEdit = !passed;
            answerResult?.classList.add("hidden");
            answerSlot?.classList.remove("hidden");
            toggleWrap?.classList.toggle("hidden", modes.length < 2);
            setControlVisible(checkBtn, !passed);
            setControlVisible(hintBtn, !passed && !!step.hints);
            if (passed)
                setStatus("correct", true, true);
            if (hasState) {
                renderProgramState(step, canEdit);
            }
            if (activeMode === "selected")
                renderSelectedPreview(step);
            else if (activeMode === "undefined")
                renderUndefinedAnswer();
            else
                renderEnteredBox(savedEntered, canEdit);
            setActiveMode(activeMode, canEdit);
        }
        else {
            answerSlot?.classList.add("hidden");
            toggleWrap?.classList.add("hidden");
            setControlVisible(checkBtn, false);
            setControlVisible(hintBtn, false);
            if (hasState) {
                renderProgramState(step, false);
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
        persistProgress();
    }
    function markCurrentStepPassed() {
        state.passes[state.boundary] = true;
        setControlVisible(checkBtn, false);
        setControlVisible(hintBtn, false);
        setActiveMode(activeMode, false);
        disableBoxEditing(answerSlot);
        pager?.pulseNext();
        pager?.update();
        renderInstructions(instructionsFor(state.boundary));
        persistProgress();
    }
    function checkAnswer() {
        hideHint();
        const step = stepFor(state.boundary);
        if (!step.editable)
            return;
        const evaluation = evaluatedAnswer(step);
        if (activeMode === "undefined") {
            const ok = "error" in evaluation && evaluation.kind === "ub";
            setStatus(ok ? "correct" : "incorrect", ok);
            if (ok)
                markCurrentStepPassed();
            return;
        }
        if ("error" in evaluation) {
            const answerable = evaluation.kind === "ub" && workspace.allowUndefinedAnswer;
            setStatus(answerable ? "incorrect" : errorText(evaluation.kind), false);
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
            const ok = sameObject(chosen, evaluation.result.address, evaluation.result.type);
            setStatus(ok ? "correct" : "incorrect", ok);
            if (ok) {
                state.selections[state.boundary] = selectedName;
                markCurrentStepPassed();
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
        const expectedValue = evaluation.result.value ?? "";
        const entryType = String(entry.type || "").trim();
        const expectedBox = {
            name: "",
            type: expectedType,
            value: expectedValue,
            typeInfo: evaluation.result.typeInfo,
        };
        const match = boxValueMatchesSpec(entry, expectedBox);
        const ok = entryType === expectedType && match.ok;
        setStatus(ok ? "correct" : "incorrect", ok);
        if (ok) {
            entry.value = match.normalized;
            entry.rawValue = match.normalized;
            state.entered[state.boundary] = entry;
            renderEnteredBox(entry, false);
            markCurrentStepPassed();
        }
    }
    function switchAnswerMode(mode) {
        const step = stepFor(state.boundary);
        if (!answerModes(step).includes(mode) || state.passes[state.boundary])
            return;
        if (activeMode === "entered") {
            const node = answerSlot?.querySelector(".vbox") || null;
            if (node)
                state.entered[state.boundary] = readBoxState(node);
        }
        setActiveMode(mode, true);
        state.modes[state.boundary] = mode;
        updateSelectionHighlight(selectedName, mode === "selected");
        if (mode === "selected")
            renderSelectedPreview(step);
        else if (mode === "undefined")
            renderUndefinedAnswer();
        else
            renderEnteredBox(state.entered[state.boundary] ?? null, true);
        persistProgress();
    }
    useSelectedBtn?.addEventListener("click", () => switchAnswerMode("selected"));
    useEnteredBtn?.addEventListener("click", () => switchAnswerMode("entered"));
    useUndefinedBtn?.addEventListener("click", () => switchAnswerMode("undefined"));
    if (answerSlot) {
        answerSlot.addEventListener("input", () => {
            saveStep(state.boundary);
            persistProgress();
        });
        answerSlot.addEventListener("click", () => {
            window.setTimeout(() => {
                saveStep(state.boundary);
                persistProgress();
            }, 0);
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
    if (levelResetBtn) {
        levelResetBtn.addEventListener("click", () => {
            resetLevelAfterConfirmation(progress);
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
            state.boundary = clampBoundary(value);
        },
        isStepLocked: (at) => isStepLocked(at),
        onBeforeChange: (current) => {
            saveStep(current);
            persistProgress();
        },
        onAfterChange: () => render(),
        getNextLabel: (_boundary, _total, atEnd) => {
            if (atEnd)
                return endLabel;
            return "Next";
        },
        endLabel,
        backToIntro: initialInstructions
            ? () => {
                if (!state.showIntro) {
                    state.showIntro = true;
                    render();
                    return;
                }
                const previousUrl = withSidebarParam(getPreviousNavHref());
                if (previousUrl)
                    window.location.href = previousUrl;
            }
            : undefined,
    });
    render();
}
export { createExpressionEvalTemplate };

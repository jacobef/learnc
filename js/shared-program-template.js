import { applyTextTokenReplacements, applyOtherNames, appendStateObjects, boxValueMatchesSpec, clearNode, cloneBoxes, createSimpleSimulator, createStepper, bindBtnRefPulse, disableBoxEditing, ensurePanelizedMain, flashStatus, getNavLabelForHref, isMobileViewport, makeAnswerBox, normalizeBoxValueForContext, normalizeZeroDisplay, queryRole, randAddr, removeBoxDeleteButtons, renderCodePane, renderParts, restoreWorkspace, serializeWorkspace, setPartsContent, syncDocumentTitleFromNav, typeInfo, } from "./shared-core.js";
import { clearLevelProgress, currentLevelId, maybeRestoreLevelProgress, writeLevelProgress, } from "./shared-progress.js";
function collectProgramElements(root = document) {
    const role = (name) => queryRole(name, root);
    return {
        instructionsEl: role("program-instructions"),
        codeEl: role("program-code"),
        codeRoot: role("program-root"),
        stageEl: role("program-stage"),
        controlsActionsEl: role("program-controls"),
        mobileActionsEl: role("program-mobile-actions"),
        statusEl: role("program-status"),
        hintPanel: role("program-hint"),
        hintBtn: role("program-hint-btn"),
        checkBtn: role("program-check"),
        levelResetBtn: role("program-reset-level"),
        addBtn: role("program-add"),
        resetBtn: role("program-reset"),
    };
}
function boxNamed(boxes, name) {
    return boxes.find((box) => box.name === name);
}
function boxesNamed(boxes, ...names) {
    const map = new Map();
    for (const box of boxes) {
        map.set(box.name, box);
    }
    return names.map((name) => map.get(name));
}
function ensureProgramLayout() {
    const resolvedTitle = syncDocumentTitleFromNav();
    const existing = queryRole("program-code");
    if (existing)
        return collectProgramElements();
    const main = ensurePanelizedMain(resolvedTitle);
    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "program-instructions";
    instructionsEl.className = "intro";
    const section = document.createElement("section");
    section.dataset.role = "program-root";
    section.classList.add("panel-shell");
    const actionBar = document.createElement("div");
    actionBar.className = "controls-bar controls-bar-program";
    const controlsMain = document.createElement("div");
    controlsMain.className = "controls-main panel panel-controls";
    const controlsRow = document.createElement("div");
    controlsRow.className = "controls-row controls-left";
    controlsRow.dataset.role = "program-controls";
    controlsMain.appendChild(controlsRow);
    actionBar.appendChild(controlsMain);
    actionBar.appendChild(instructionsEl);
    section.appendChild(actionBar);
    const row = document.createElement("div");
    row.className = "row panel-row";
    section.appendChild(row);
    main.appendChild(section);
    const codePanel = document.createElement("div");
    codePanel.className = "panel panel-scroll code-panel-shell";
    codePanel.dataset.role = "program-code-panel";
    const codeTitle = document.createElement("div");
    codeTitle.className = "panel-title code-title";
    codeTitle.textContent = "Code";
    const codeEl = document.createElement("div");
    codeEl.dataset.role = "program-code";
    codeEl.className = "codepane panel-body";
    const prevBtn = document.createElement("button");
    prevBtn.textContent = "Back ◀";
    prevBtn.dataset.stepper = "prev";
    const nextBtn = document.createElement("button");
    nextBtn.textContent = "Run line 1 ▶";
    nextBtn.dataset.stepper = "next";
    const levelResetBtn = document.createElement("button");
    levelResetBtn.dataset.role = "program-reset-level";
    levelResetBtn.textContent = "Reset level";
    const controlsSpacer = document.createElement("span");
    controlsSpacer.className = "controls-spacer";
    controlsSpacer.setAttribute("aria-hidden", "true");
    controlsRow.appendChild(prevBtn);
    controlsRow.appendChild(nextBtn);
    controlsRow.appendChild(controlsSpacer);
    controlsRow.appendChild(levelResetBtn);
    codePanel.appendChild(codeTitle);
    codePanel.appendChild(codeEl);
    const statePanel = document.createElement("div");
    statePanel.className = "panel program-state-panel panel-scroll";
    const mobileActions = document.createElement("div");
    mobileActions.className = "panel program-mobile-actions";
    mobileActions.dataset.role = "program-mobile-actions";
    const stateTitle = document.createElement("div");
    stateTitle.className = "panel-title";
    stateTitle.textContent = "Program state";
    const stageEl = document.createElement("div");
    stageEl.dataset.role = "program-stage";
    stageEl.className = "panel-body";
    const checkBtn = document.createElement("button");
    checkBtn.dataset.role = "program-check";
    checkBtn.className = "hidden";
    checkBtn.textContent = "Check";
    const hintBtn = document.createElement("button");
    hintBtn.dataset.role = "program-hint-btn";
    hintBtn.type = "button";
    hintBtn.className = "hint-link hidden";
    hintBtn.textContent = "Hint";
    const addBtn = document.createElement("button");
    addBtn.dataset.role = "program-add";
    addBtn.className = "hidden gap-wide";
    addBtn.textContent = "+ New variable";
    const resetBtn = document.createElement("button");
    resetBtn.dataset.role = "program-reset";
    resetBtn.className = "hidden reserved-button-slot";
    resetBtn.textContent = "Reset";
    const statusEl = document.createElement("span");
    statusEl.dataset.role = "program-status";
    statusEl.className = "muted";
    controlsRow.appendChild(resetBtn);
    controlsRow.appendChild(addBtn);
    controlsRow.appendChild(hintBtn);
    controlsRow.appendChild(checkBtn);
    controlsRow.appendChild(statusEl);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "program-hint";
    hintPanel.className = "hint-inline hidden";
    actionBar.insertBefore(hintPanel, instructionsEl);
    statePanel.appendChild(stateTitle);
    statePanel.appendChild(stageEl);
    row.appendChild(codePanel);
    row.appendChild(mobileActions);
    row.appendChild(statePanel);
    return {
        instructionsEl,
        codeEl,
        codeRoot: section,
        stageEl,
        controlsActionsEl: controlsRow,
        mobileActionsEl: mobileActions,
        statusEl,
        hintPanel,
        hintBtn,
        checkBtn,
        levelResetBtn,
        addBtn,
        resetBtn,
    };
}
function createProgramTemplate(config) {
    const { steps = [], initialInstructions, next = null, workspace = {}, isLast = false, } = config;
    const endLabel = (() => {
        if (isLast)
            return "Finish";
        const label = getNavLabelForHref(next);
        return label ? `Next: ${label}` : "Next Program";
    })();
    const showOtherNames = !!(workspace && workspace.showOtherNames);
    const allowVariableDeletion = !!(workspace && workspace.allowVariableDeletion);
    const failConfig = (message) => {
        alert(message);
        throw new Error(message);
    };
    const simulator = createSimpleSimulator();
    const { instructionsEl, codeEl, codeRoot, stageEl, controlsActionsEl, mobileActionsEl, statusEl, hintPanel, hintBtn, checkBtn, levelResetBtn, addBtn, resetBtn, } = ensureProgramLayout();
    function placeActionButtonsForViewport() {
        const mobileMode = isMobileViewport() && !!mobileActionsEl;
        const target = mobileMode ? mobileActionsEl : controlsActionsEl;
        if (!target)
            return;
        [levelResetBtn, resetBtn, addBtn, hintBtn, checkBtn, statusEl].forEach((node) => {
            if (node && node.parentElement !== target)
                target.appendChild(node);
        });
        if (!hintPanel)
            return;
        if (mobileMode && mobileActionsEl) {
            if (hintPanel.parentElement !== mobileActionsEl) {
                mobileActionsEl.appendChild(hintPanel);
            }
            return;
        }
        const desktopHintParent = instructionsEl?.parentElement ?? null;
        if (desktopHintParent &&
            (hintPanel.parentElement !== desktopHintParent ||
                hintPanel.nextElementSibling !== instructionsEl)) {
            desktopHintParent.insertBefore(hintPanel, instructionsEl);
        }
    }
    function updateMobileActionsVisibility() {
        if (!mobileActionsEl)
            return;
        const hasVisibleAction = [levelResetBtn, checkBtn, hintBtn, addBtn, resetBtn].some((btn) => !!btn && !btn.classList.contains("hidden"));
        mobileActionsEl.classList.toggle("hidden", !hasVisibleAction);
    }
    placeActionButtonsForViewport();
    updateMobileActionsVisibility();
    window.addEventListener("resize", placeActionButtonsForViewport);
    bindBtnRefPulse(codeRoot || document);
    if (initialInstructions !== undefined &&
        typeof initialInstructions !== "string") {
        failConfig("Program initialInstructions must be a string.");
    }
    if (!Array.isArray(steps) || steps.length === 0) {
        failConfig("Program steps must be a non-empty array.");
    }
    const stepInfos = [];
    const lineList = [];
    const isIfHeaderPart = (part) => {
        const tokens = part.tokens;
        if (tokens.length < 2)
            return false;
        const first = tokens[0];
        const second = tokens[1];
        if (first.type !== "kw" || first.value !== "if")
            return false;
        if (second.type !== "sym" || second.value !== "(")
            return false;
        let depth = 0;
        for (let i = 1; i < tokens.length; i++) {
            const tok = tokens[i];
            if (tok.type !== "sym")
                continue;
            if (tok.value === "(")
                depth += 1;
            if (tok.value === ")") {
                depth -= 1;
                if (depth === 0)
                    return i === tokens.length - 1;
                if (depth < 0)
                    return false;
            }
        }
        return false;
    };
    const isWhileHeaderPart = (part) => {
        const tokens = part.tokens;
        if (tokens.length < 2)
            return false;
        const first = tokens[0];
        const second = tokens[1];
        if (first.type !== "kw" || first.value !== "while")
            return false;
        if (second.type !== "sym" || second.value !== "(")
            return false;
        let depth = 0;
        for (let i = 1; i < tokens.length; i++) {
            const tok = tokens[i];
            if (tok.type !== "sym")
                continue;
            if (tok.value === "(")
                depth += 1;
            if (tok.value === ")") {
                depth -= 1;
                if (depth === 0)
                    return i === tokens.length - 1;
                if (depth < 0)
                    return false;
            }
        }
        return false;
    };
    const isElsePart = (part) => {
        const tokens = part.tokens;
        if (tokens.length !== 1)
            return false;
        const first = tokens[0];
        return first.type === "kw" && first.value === "else";
    };
    const isOpenBracePart = (part) => {
        if (part.tokens.length !== 1)
            return false;
        const tok = part.tokens[0];
        return tok.type === "sym" && tok.value === "{";
    };
    const isCloseBracePart = (part) => {
        if (part.tokens.length !== 1)
            return false;
        const tok = part.tokens[0];
        return tok.type === "sym" && tok.value === "}";
    };
    steps.forEach((step, index) => {
        if (!step || typeof step !== "object") {
            failConfig(`Step ${index + 1} must be an object.`);
        }
        if (typeof step.code !== "string" || !step.code.trim()) {
            failConfig(`Step ${index + 1} must include a code string.`);
        }
        const normalizedCode = step.code.endsWith("\n") ? step.code : `${step.code}\n`;
        const editable = step.editable ?? false;
        const rawLines = normalizedCode.split(/\r?\n/);
        if (rawLines[rawLines.length - 1] === "")
            rawLines.pop();
        const stepParts = simulator.splitStatements(simulator.tokenizeProgram(normalizedCode));
        const nonEmptyParts = stepParts.filter((part) => part.tokens.length > 0);
        const ifHeaderOnly = nonEmptyParts.filter((part) => isIfHeaderPart(part)).length === 1 &&
            nonEmptyParts.every((part) => isIfHeaderPart(part) ||
                isOpenBracePart(part) ||
                isCloseBracePart(part) ||
                isElsePart(part));
        const ifHeaderHasOpenBrace = ifHeaderOnly && nonEmptyParts.some((part) => isOpenBracePart(part));
        const whileHeaderOnly = nonEmptyParts.filter((part) => isWhileHeaderPart(part)).length === 1 &&
            nonEmptyParts.every((part) => isWhileHeaderPart(part) ||
                isOpenBracePart(part) ||
                isCloseBracePart(part));
        const whileHeaderHasOpenBrace = whileHeaderOnly && nonEmptyParts.some((part) => isOpenBracePart(part));
        const startLine = lineList.length;
        rawLines.forEach((line) => lineList.push(line));
        const endLine = lineList.length - 1;
        stepInfos.push({
            index,
            code: normalizedCode,
            lines: rawLines,
            startLine,
            endLine,
            boundary: endLine + 1,
            instructions: step.instructions,
            hints: step.hints,
            editable,
            canBeEditable: Array.isArray(editable)
                ? editable.some((value) => value)
                : editable === true,
            ifHeaderOnly,
            ifHeaderHasOpenBrace,
            whileHeaderOnly,
            whileHeaderHasOpenBrace,
        });
    });
    const total = lineList.length;
    const stepByLine = new Array(total).fill(null);
    stepInfos.forEach((step) => {
        for (let line = step.startLine; line <= step.endLine && line < total; line++) {
            if (line >= 0)
                stepByLine[line] = step;
        }
    });
    function stepForLine(line) {
        const safeLine = Math.max(0, Math.min(total - 1, Math.floor(line)));
        return stepByLine[safeLine] ?? null;
    }
    const levelId = currentLevelId();
    const restoredProgress = maybeRestoreLevelProgress(levelId);
    const state = {
        boundary: 0,
        executionSteps: -1,
        allocBase: null,
        workspaceEl: null,
        lastInstructionKey: null,
        lastRenderedStateCount: null,
    };
    function allocFactory() {
        if (state.allocBase == null)
            state.allocBase = randAddr("int");
        let next = Number(state.allocBase);
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
    function statementRangeEndingAt(statementMap, boundary) {
        const endLine = boundary - 1;
        return (statementMap.parts.find((part) => part.endLine === endLine && part.hasSemicolon) || null);
    }
    const statementMap = simulator.buildStatementMap(lineList);
    const parts = statementMap.parts;
    const totalLines = total;
    const ifBlocks = simulator.buildIfStatementMap(parts, {
        lastLine: Math.max(0, total - 1),
    });
    const whileBlocks = simulator.buildWhileStatementMap(parts, {
        lastLine: Math.max(0, total - 1),
    });
    let activeBranchTargets = null;
    let branchSelectionActive = false;
    let branchSelectionTarget = null;
    let branchSelectionBoundary = null;
    const groupRanges = stepInfos.map((step) => ({
        startLine: step.startLine,
        endLine: step.endLine,
    }));
    const allBoundaries = Array.from({ length: totalLines + 1 }, (_, i) => i);
    const allBoundaryTargets = new Map();
    allBoundaries.forEach((boundary) => {
        allBoundaryTargets.set(boundary, boundary);
    });
    const hasInitialInstructionsContent = typeof initialInstructions === "string" && initialInstructions.length > 0;
    const MAX_RUNTIME_TRACE_STEPS = 2000;
    const runtimeTraceByStep = [];
    const runtimeStages = [];
    let runtimeLatestSolvedStage = -1;
    const runtimeWorkspaceByStage = new Map();
    const stepStartLines = stepInfos
        .map((step) => step.startLine)
        .sort((a, b) => a - b);
    const stepReachCounts = new Array(stepInfos.length).fill(0);
    let lastRuntimeStageStepIndex = null;
    const resolveEditableForVisit = (stepInfo, visitIndex) => {
        const { editable } = stepInfo;
        if (Array.isArray(editable)) {
            return editable[visitIndex] === true;
        }
        return editable === true;
    };
    const resolveInstructionsForVisit = (stepInfo, visitIndex) => {
        const { instructions } = stepInfo;
        if (Array.isArray(instructions)) {
            return instructions[visitIndex];
        }
        return instructions;
    };
    const resolveHintsForEditableVisit = (stepInfo, editableVisitIndex) => {
        const { hints } = stepInfo;
        if (Array.isArray(hints)) {
            if (editableVisitIndex == null)
                return undefined;
            return hints[editableVisitIndex];
        }
        return hints;
    };
    const resolveStepVisitForStage = (stepInfo) => {
        if (!stepInfo) {
            lastRuntimeStageStepIndex = null;
            return {
                stepVisitIndex: null,
                editableVisitIndex: null,
                stepEditable: false,
                instructions: undefined,
                hints: undefined,
            };
        }
        const isNewVisit = lastRuntimeStageStepIndex !== stepInfo.index;
        if (isNewVisit) {
            stepReachCounts[stepInfo.index] = (stepReachCounts[stepInfo.index] || 0) + 1;
        }
        const stepVisitIndex = Math.max(0, (stepReachCounts[stepInfo.index] || 1) - 1);
        const stepEditable = resolveEditableForVisit(stepInfo, stepVisitIndex);
        let editableVisitIndex = null;
        if (stepEditable) {
            const editableVisitsBefore = Array.isArray(stepInfo.editable)
                ? stepInfo.editable
                    .slice(0, Math.max(0, stepVisitIndex))
                    .filter((value) => value).length
                : stepInfo.editable
                    ? stepVisitIndex
                    : 0;
            editableVisitIndex = editableVisitsBefore;
        }
        const instructions = resolveInstructionsForVisit(stepInfo, stepVisitIndex);
        const hints = resolveHintsForEditableVisit(stepInfo, editableVisitIndex);
        lastRuntimeStageStepIndex = stepInfo.index;
        return {
            stepVisitIndex,
            editableVisitIndex,
            stepEditable,
            instructions,
            hints,
        };
    };
    const executablePartStartLines = new Set();
    parts.forEach((part) => {
        executablePartStartLines.add(clampBoundaryLine(part.startLine));
    });
    function pushRuntimeStage(stage) {
        runtimeStages.push({
            index: runtimeStages.length,
            ...stage,
        });
    }
    function boundaryForPartIndex(partIndex) {
        if (partIndex >= parts.length)
            return totalLines;
        const part = parts[Math.max(0, Math.floor(partIndex))];
        if (!part)
            return totalLines;
        return clampBoundaryLine(part.startLine);
    }
    for (let step = 0; step <= MAX_RUNTIME_TRACE_STEPS; step++) {
        const alloc = allocFactory();
        const trace = simulator.traceProgramParts(parts, {
            alloc,
            stopSteps: step,
        });
        if (!trace) {
            failConfig(`Program trace failed at step ${step + 1}. Check the flow in this level.`);
        }
        const runtimeTrace = trace;
        runtimeTraceByStep.push(runtimeTrace);
        if (runtimeTrace.nextIndex >= parts.length)
            break;
    }
    const finalTrace = runtimeTraceByStep[runtimeTraceByStep.length - 1] || null;
    if (!finalTrace || finalTrace.nextIndex < parts.length) {
        failConfig(`Program exceeds ${MAX_RUNTIME_TRACE_STEPS} execution steps. Reduce loop iterations in this level.`);
    }
    const buildRuntimeStage = ({ traceIndex, partIndex, runLine, stepInfo, stepVisitIndex, stepEditable, instructions, hints, beforeBoundary, afterBoundary, stateBefore, stateAfter, forceStateEditable = false, }) => {
        let resolvedAfterBoundary = afterBoundary;
        const ifBlock = ifBlocks.map.get(partIndex);
        if (ifBlock) {
            const condition = simulator.evaluateCondition(ifBlock.expr, stateBefore || []);
            if (!("error" in condition)) {
                const branchBoundary = condition.value
                    ? branchEntryLine(ifBlock, "true")
                    : branchEntryLine(ifBlock, "false");
                if (branchBoundary != null) {
                    resolvedAfterBoundary = clampBoundaryLine(branchBoundary);
                }
            }
        }
        const rawRunEnd = partIndex >= 0 ? parts[partIndex].endLine : (stepInfo?.endLine ?? runLine);
        const maxLineIndex = Math.max(0, totalLines - 1);
        const runEndLine = Math.max(Math.max(0, Math.min(maxLineIndex, runLine)), Math.max(0, Math.min(maxLineIndex, rawRunEnd)));
        let editableMode = "none";
        let interactionBoundary = null;
        let expectedBoundary = null;
        let branchTargets = [];
        let expectedState = null;
        let baselineState = null;
        const stageExitsStep = !!stepInfo &&
            beforeBoundary < stepInfo.boundary &&
            resolvedAfterBoundary >= stepInfo.boundary;
        if (stepInfo && stepEditable) {
            if (partIndex >= 0 && isHeaderOnlyStep(stepInfo)) {
                editableMode = "boundary";
                interactionBoundary = stepInfo.boundary;
                expectedBoundary = resolvedAfterBoundary;
                if (stepInfo.whileHeaderOnly) {
                    const block = whileBlocks.map.get(partIndex);
                    if (block) {
                        const trueLine = lineAfterWhileHeader(block);
                        const falseLine = lineAfterWhileClose(block);
                        if (trueLine != null)
                            branchTargets.push(clampBoundaryLine(trueLine));
                        if (falseLine != null)
                            branchTargets.push(clampBoundaryLine(falseLine));
                    }
                }
                else if (stepInfo.ifHeaderOnly) {
                    const block = ifBlocks.map.get(partIndex);
                    if (block) {
                        const trueLine = branchEntryLine(block, "true");
                        const falseLine = branchEntryLine(block, "false");
                        if (trueLine != null)
                            branchTargets.push(clampBoundaryLine(trueLine));
                        if (falseLine != null)
                            branchTargets.push(clampBoundaryLine(falseLine));
                    }
                }
                branchTargets = [...new Set(branchTargets)];
                if (expectedBoundary != null && !branchTargets.includes(expectedBoundary)) {
                    branchTargets.push(expectedBoundary);
                }
            }
            else if (forceStateEditable || (partIndex >= 0 && stageExitsStep)) {
                editableMode = "state";
                interactionBoundary = resolvedAfterBoundary;
                expectedState = cloneBoxes(stateAfter || []);
                baselineState = cloneBoxes(stateBefore || []);
            }
        }
        return {
            traceIndex,
            partIndex,
            runLine,
            runEndLine,
            beforeBoundary,
            afterBoundary: resolvedAfterBoundary,
            stateAfter: cloneBoxes(stateAfter || []),
            step: stepInfo,
            stepVisitIndex,
            stepEditable,
            instructions,
            hints,
            editableMode,
            interactionBoundary,
            expectedBoundary,
            branchTargets,
            expectedState,
            baselineState,
        };
    };
    const appendNoOpStagesInRange = ({ fromBoundary, toBoundary, traceIndex, stateSnapshot, }) => {
        if (toBoundary <= fromBoundary)
            return;
        const noOpStarts = stepStartLines
            .filter((startLine) => startLine >= fromBoundary && startLine < toBoundary)
            .filter((startLine) => !executablePartStartLines.has(startLine))
            .sort((a, b) => a - b);
        for (let insertIndex = 0; insertIndex < noOpStarts.length; insertIndex++) {
            const startLine = noOpStarts[insertIndex];
            const nextBoundary = insertIndex + 1 < noOpStarts.length
                ? noOpStarts[insertIndex + 1]
                : toBoundary;
            const stepInfo = stepForLine(startLine);
            const resolvedVisit = resolveStepVisitForStage(stepInfo);
            pushRuntimeStage(buildRuntimeStage({
                traceIndex,
                partIndex: -1,
                runLine: startLine,
                stepInfo,
                stepVisitIndex: resolvedVisit.stepVisitIndex,
                stepEditable: resolvedVisit.stepEditable,
                instructions: resolvedVisit.instructions,
                hints: resolvedVisit.hints,
                beforeBoundary: startLine,
                afterBoundary: nextBoundary,
                stateBefore: stateSnapshot,
                stateAfter: stateSnapshot,
            }));
        }
    };
    const initialTrace = runtimeTraceByStep[0] || null;
    const initialBoundary = initialTrace
        ? boundaryForPartIndex(initialTrace.nextIndex)
        : 0;
    appendNoOpStagesInRange({
        fromBoundary: 0,
        toBoundary: initialBoundary,
        traceIndex: 0,
        stateSnapshot: cloneBoxes(initialTrace?.state || []),
    });
    let pendingGroupedEditable = null;
    for (let index = 0; index < runtimeTraceByStep.length - 1; index++) {
        const before = runtimeTraceByStep[index];
        const after = runtimeTraceByStep[index + 1];
        const partIndex = before.nextIndex;
        const part = parts[partIndex];
        if (!part)
            continue;
        const runLine = clampBoundaryLine(part.startLine);
        const beforeBoundary = boundaryForPartIndex(before.nextIndex);
        const rawAfterBoundary = boundaryForPartIndex(after.nextIndex);
        const isSequentialAdvance = after.nextIndex === partIndex + 1;
        const immediateAfterBoundary = clampBoundaryLine(part.endLine + 1);
        const stageAfterBoundary = isSequentialAdvance
            ? immediateAfterBoundary
            : rawAfterBoundary;
        const stepInfo = stepForLine(runLine);
        const groupedEditableStep = !!stepInfo && stepInfo.canBeEditable && !isHeaderOnlyStep(stepInfo);
        if (groupedEditableStep && stepInfo) {
            if (!pendingGroupedEditable || pendingGroupedEditable.step.index !== stepInfo.index) {
                pendingGroupedEditable = {
                    step: stepInfo,
                    entryBoundary: beforeBoundary,
                    entryState: cloneBoxes(before.state || []),
                };
            }
            const nextPart = parts[after.nextIndex];
            const nextStepInfo = nextPart
                ? stepForLine(clampBoundaryLine(nextPart.startLine))
                : null;
            const groupedStepCompleted = !nextStepInfo || nextStepInfo.index !== stepInfo.index;
            if (groupedStepCompleted) {
                const grouped = pendingGroupedEditable;
                const resolvedVisit = resolveStepVisitForStage(grouped.step);
                pushRuntimeStage(buildRuntimeStage({
                    traceIndex: index,
                    partIndex: -1,
                    runLine: grouped.step.startLine,
                    stepInfo: grouped.step,
                    stepVisitIndex: resolvedVisit.stepVisitIndex,
                    stepEditable: resolvedVisit.stepEditable,
                    instructions: resolvedVisit.instructions,
                    hints: resolvedVisit.hints,
                    beforeBoundary: grouped.entryBoundary,
                    afterBoundary: stageAfterBoundary,
                    stateBefore: cloneBoxes(grouped.entryState || []),
                    stateAfter: cloneBoxes(after.state || []),
                    forceStateEditable: true,
                }));
                if (isSequentialAdvance) {
                    const noOpStartBoundary = grouped.step.boundary;
                    appendNoOpStagesInRange({
                        fromBoundary: noOpStartBoundary,
                        toBoundary: rawAfterBoundary,
                        traceIndex: index + 1,
                        stateSnapshot: cloneBoxes(after.state || []),
                    });
                }
                pendingGroupedEditable = null;
            }
            continue;
        }
        pendingGroupedEditable = null;
        const resolvedVisit = resolveStepVisitForStage(stepInfo);
        pushRuntimeStage(buildRuntimeStage({
            traceIndex: index,
            partIndex,
            runLine,
            stepInfo,
            stepVisitIndex: resolvedVisit.stepVisitIndex,
            stepEditable: resolvedVisit.stepEditable,
            instructions: resolvedVisit.instructions,
            hints: resolvedVisit.hints,
            beforeBoundary,
            afterBoundary: stageAfterBoundary,
            stateBefore: cloneBoxes(before.state || []),
            stateAfter: cloneBoxes(after.state || []),
        }));
        if (!isSequentialAdvance)
            continue;
        const noOpStartBoundary = stepInfo ? stepInfo.boundary : runLine + 1;
        appendNoOpStagesInRange({
            fromBoundary: noOpStartBoundary,
            toBoundary: rawAfterBoundary,
            traceIndex: index + 1,
            stateSnapshot: cloneBoxes(after.state || []),
        });
    }
    function runtimeMaxStep() {
        return runtimeStages.length - 1;
    }
    function runtimeStepClamp(stepCount) {
        const safeStep = Math.floor(stepCount);
        return Math.max(-1, Math.min(runtimeMaxStep(), safeStep));
    }
    function progressSnapshot() {
        return {
            executionSteps: state.executionSteps,
            allocBase: state.allocBase,
            runtimeLatestSolvedStage,
            runtimeWorkspaceByStage: Array.from(runtimeWorkspaceByStage.entries())
                .map(([stageIndex, boxes]) => ({
                stageIndex,
                boxes: boxes ? cloneBoxes(boxes) : null,
            })),
        };
    }
    function persistProgress() {
        const snapshot = progressSnapshot();
        const isDefault = snapshot.executionSteps === -1 &&
            snapshot.runtimeLatestSolvedStage === -1 &&
            snapshot.runtimeWorkspaceByStage.length === 0;
        if (isDefault) {
            clearLevelProgress(levelId);
            return;
        }
        writeLevelProgress(snapshot, levelId);
    }
    function runtimeTraceForStage(stepCount) {
        if (!runtimeTraceByStep.length)
            return null;
        const safeStep = runtimeStepClamp(stepCount);
        if (safeStep < 0)
            return runtimeTraceByStep[0] || null;
        const stage = runtimeStageAt(safeStep);
        if (!stage)
            return runtimeTraceByStep[runtimeTraceByStep.length - 1] || null;
        const traceIndex = Math.max(0, Math.min(runtimeTraceByStep.length - 1, stage.traceIndex + 1));
        return runtimeTraceByStep[traceIndex] || null;
    }
    function runtimeStageAt(stepCount) {
        const exactStep = Math.floor(stepCount);
        if (exactStep < 0)
            return null;
        if (exactStep > runtimeMaxStep())
            return null;
        return runtimeStages[exactStep] || null;
    }
    function runtimeStageNeedsSolve(stage) {
        if (!stage || stage.editableMode === "none")
            return false;
        return stage.index > runtimeLatestSolvedStage;
    }
    function runtimeStageSolved(stage) {
        if (!stage)
            return false;
        if (stage.editableMode === "none")
            return true;
        return !runtimeStageNeedsSolve(stage);
    }
    function runtimeBoundaryForSteps(stepCount) {
        const stage = runtimeStageAt(stepCount);
        if (!stage)
            return 0;
        if (stage.editableMode === "boundary" && runtimeStageNeedsSolve(stage)) {
            return stage.interactionBoundary ?? stage.afterBoundary;
        }
        return stage.afterBoundary;
    }
    function syncBoundaryFromStage() {
        state.boundary = runtimeBoundaryForSteps(state.executionSteps);
    }
    function runtimeCurrentStage() {
        return runtimeStageAt(state.executionSteps);
    }
    function runtimePendingStage() {
        const stage = runtimeCurrentStage();
        if (!runtimeStageNeedsSolve(stage))
            return null;
        return stage;
    }
    function runtimeStateEditStageForBoundary(boundary, opts = {}) {
        const stage = runtimeCurrentStage();
        if (!stage || stage.editableMode !== "state")
            return null;
        const interactionBoundary = stage.interactionBoundary ?? stage.afterBoundary;
        if (interactionBoundary !== boundary)
            return null;
        if (!opts.includeSolved && runtimeStageSolved(stage))
            return null;
        return stage;
    }
    function runtimeBoundaryEditStageForBoundary(boundary, opts = {}) {
        const stage = runtimeCurrentStage();
        if (!stage || stage.editableMode !== "boundary")
            return null;
        if (runtimeStageSolved(stage)) {
            if (stage.afterBoundary !== boundary)
                return null;
        }
        else {
            const interactionBoundary = stage.interactionBoundary ?? stage.afterBoundary;
            if (interactionBoundary !== boundary)
                return null;
        }
        if (!opts.includeSolved && runtimeStageSolved(stage))
            return null;
        return stage;
    }
    function runtimeEditableStorageKey(boundary) {
        const stage = runtimeStateEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (!stage)
            return null;
        if (runtimeStageSolved(stage))
            return null;
        return stage.index;
    }
    function clearBranchSelection() {
        branchSelectionActive = false;
        branchSelectionBoundary = null;
        branchSelectionTarget = null;
    }
    function runtimeCurrentTrace() {
        return runtimeTraceForStage(state.executionSteps);
    }
    function runtimeIsComplete() {
        if (!runtimeStages.length)
            return true;
        if (state.executionSteps < runtimeMaxStep())
            return false;
        const current = runtimeCurrentStage();
        return !!current && !runtimeStageNeedsSolve(current);
    }
    function runtimeStageLineRange(stage) {
        if (!stage)
            return null;
        const maxLineIndex = Math.max(0, totalLines - 1);
        const start = Math.max(0, Math.min(maxLineIndex, stage.runLine));
        const end = Math.max(start, Math.min(maxLineIndex, stage.runEndLine));
        return { start, end };
    }
    function runtimeRunLabel(withArrow) {
        const labelForStage = (stage) => {
            if (!stage)
                return endLabel;
            const range = runtimeStageLineRange(stage) || {
                start: stage.runLine,
                end: stage.runLine,
            };
            const startLine = Math.max(1, Math.min(totalLines, range.start + 1));
            const endLine = Math.max(1, Math.min(totalLines, range.end + 1));
            if (stage.editableMode === "state") {
                const verb = runtimeStageNeedsSolve(stage) ? "Solve" : "Run";
                return formatRunLabel(startLine, endLine, withArrow, verb);
            }
            const isWhileHeaderStage = whileBlocks.map.has(stage.partIndex);
            const isIfHeaderStage = ifBlocks.map.has(stage.partIndex);
            if (stage.editableMode === "boundary" || isWhileHeaderStage || isIfHeaderStage) {
                return formatRunLabel(startLine, endLine, withArrow, "Branch from");
            }
            const verb = runtimeStageNeedsSolve(stage) ? "Solve" : "Run";
            return formatRunLabel(startLine, endLine, withArrow, verb);
        };
        return labelForStage(runtimeStageAt(state.executionSteps + 1));
    }
    function runtimeStepBadge() {
        const nextStage = runtimeStageAt(state.executionSteps + 1);
        return runtimeStageNeedsSolve(nextStage) ? "note" : "";
    }
    function runtimeMarkCurrentStageSolved() {
        const stage = runtimePendingStage();
        if (!stage)
            return false;
        runtimeLatestSolvedStage = Math.max(runtimeLatestSolvedStage, stage.index);
        clearBranchSelection();
        syncBoundaryFromStage();
        return true;
    }
    function runtimePendingEditableExpectedState(boundary) {
        const stage = runtimeStateEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (!stage)
            return null;
        if (!runtimeStageNeedsSolve(stage))
            return null;
        if (!stage.expectedState)
            return null;
        return cloneBoxes(stage.expectedState);
    }
    function runtimePendingEditableBaselineState(boundary) {
        const stage = runtimeStateEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (!stage)
            return null;
        if (!runtimeStageNeedsSolve(stage))
            return null;
        if (!stage.baselineState)
            return null;
        return cloneBoxes(stage.baselineState);
    }
    function clampBoundaryLine(line) {
        return Math.max(0, Math.min(totalLines, line));
    }
    function lineAfterHeader(block) {
        return clampBoundaryLine(block.headerEndLine + 1);
    }
    function lineAfterWhileHeader(block) {
        const trueLine = lineForPartIndex(block.trueTarget);
        return trueLine ?? clampBoundaryLine(block.headerEndLine + 1);
    }
    function lineAfterClose(block) {
        const closeLine = parts[block.closeIndex].endLine;
        return clampBoundaryLine(closeLine + 1);
    }
    function lineAfterWhileClose(block) {
        const afterLine = lineForPartIndex(block.afterIndex);
        if (afterLine != null)
            return afterLine;
        const closeLine = parts[block.closeIndex].endLine;
        return clampBoundaryLine(closeLine + 1);
    }
    function lineForPartIndex(partIndex) {
        const part = parts[partIndex];
        return part ? clampBoundaryLine(part.startLine) : null;
    }
    function lineForFalseBranch(block) {
        if (block.elseIndex == null)
            return lineAfterClose(block);
        const elseEntryIndex = block.elseTarget ??
            block.elseOpenIndex ??
            block.elseIndex;
        const elseEntryLine = lineForPartIndex(elseEntryIndex);
        if (elseEntryLine != null)
            return elseEntryLine;
        return lineAfterClose(block);
    }
    function branchEntryLine(block, branch) {
        if (branch === "true")
            return lineAfterHeader(block);
        return lineForFalseBranch(block);
    }
    function isHeaderOnlyStep(step) {
        return !!(step && (step.ifHeaderOnly || step.whileHeaderOnly));
    }
    function branchInfoForBoundary(boundary) {
        const stage = runtimeBoundaryEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (!stage)
            return null;
        const targets = [];
        const targetMap = new Map();
        stage.branchTargets.forEach((target) => {
            const normalized = clampBoundaryLine(target);
            if (!targets.includes(normalized))
                targets.push(normalized);
            targetMap.set(normalized, normalized);
        });
        const expected = stage.expectedBoundary == null
            ? null
            : clampBoundaryLine(stage.expectedBoundary);
        if (expected != null && !targetMap.has(expected)) {
            targets.push(expected);
            targetMap.set(expected, expected);
        }
        return {
            rangeStart: stage.runLine,
            rangeEnd: stage.runEndLine,
            targets,
            targetMap,
            expected,
        };
    }
    function groupRangeEndingAt(boundary) {
        const endLine = boundary - 1;
        return groupRanges.find((group) => group.endLine === endLine) || null;
    }
    function stateBeforePart(partIndex) {
        const safeIndex = Math.max(0, Math.min(parts.length, partIndex));
        const alloc = allocFactory();
        const result = simulator.applyProgramParts(parts, {
            alloc,
            stop: safeIndex,
        });
        return result ?? [];
    }
    function getExpectedState(boundary) {
        const pendingExpected = runtimePendingEditableExpectedState(boundary);
        if (pendingExpected)
            return pendingExpected;
        const currentStage = runtimeCurrentStage();
        if (currentStage)
            return cloneBoxes(currentStage.stateAfter || []);
        const trace = runtimeTraceByStep[0] || null;
        if (trace)
            return cloneBoxes(trace.state || []);
        return [];
    }
    function normalizeState(list) {
        return list
            .map((b) => ({
            name: (b.name || "").trim(),
            type: (b.type || "").trim(),
            value: normalizeZeroDisplay((b.value ?? "").trim()),
            address: (b.address ?? "").trim(),
        }))
            .sort((a, b) => {
            if (a.name === b.name)
                return a.address.localeCompare(b.address);
            return a.name.localeCompare(b.name);
        });
    }
    function statesEqual(a, b) {
        const na = normalizeState(a);
        const nb = normalizeState(b);
        if (na.length !== nb.length)
            return false;
        for (let i = 0; i < na.length; i++) {
            const left = na[i];
            const right = nb[i];
            if (left.name !== right.name)
                return false;
            if (left.type !== right.type)
                return false;
            if (left.value !== right.value)
                return false;
            if (left.address !== right.address)
                return false;
        }
        return true;
    }
    function getWorkspaceEl() {
        return (state.workspaceEl ||
            stageEl?.querySelector?.('[data-role="workspace"]'));
    }
    function updateResetVisibility(boundary) {
        if (!resetBtn)
            return;
        const stage = runtimeStateEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (!stage || runtimeStageSolved(stage)) {
            resetBtn.classList.add("hidden");
            return;
        }
        const baseline = stage.baselineState || null;
        const current = serializeWorkspace(getWorkspaceEl()) || [];
        const changed = !!baseline && !statesEqual(current, baseline);
        resetBtn.classList.toggle("hidden", !changed);
    }
    function attachResetWatcher(wrap, boundary) {
        if (!wrap)
            return;
        const refresh = () => {
            updateResetVisibility(boundary);
        };
        wrap.addEventListener("input", refresh);
        wrap.addEventListener("click", () => {
            setTimeout(refresh, 0);
        });
        refresh();
    }
    function nextWorkspaceAddress(wrap, type = "int") {
        if (!wrap)
            return String(randAddr(type || "int"));
        const used = new Set();
        let maxAddr = null;
        const snapshot = serializeWorkspace(wrap) || [];
        snapshot.forEach((box) => {
            const raw = box.address ?? "";
            const addrNum = Number(raw);
            if (!Number.isFinite(addrNum))
                return;
            const addrStr = String(addrNum);
            used.add(addrStr);
            if (maxAddr == null || addrNum > maxAddr)
                maxAddr = addrNum;
        });
        const { size, align } = typeInfo(type || "int");
        if (maxAddr == null)
            return String(randAddr(type || "int"));
        let next = maxAddr + size;
        if (align > 1 && next % align !== 0) {
            next = Math.ceil(next / align) * align;
        }
        while (used.has(String(next))) {
            next += size;
        }
        return String(next);
    }
    function refreshOtherNames() {
        if (!showOtherNames)
            return;
        applyOtherNames(stageEl, { onToggle: refreshOtherNames });
    }
    function boxesEqual(actual, expected) {
        const actualByName = new Map();
        for (const box of actual) {
            const name = String(box?.name || "").trim();
            if (!name || actualByName.has(name))
                return false;
            actualByName.set(name, box);
        }
        const expectedByName = new Map();
        for (const box of expected) {
            const name = String(box?.name || "").trim();
            if (!name || expectedByName.has(name))
                return false;
            expectedByName.set(name, box);
        }
        if (actualByName.size !== expectedByName.size)
            return false;
        for (const [name, exp] of expectedByName.entries()) {
            const act = actualByName.get(name);
            if (!act)
                return false;
            const actType = String(act.type || "").trim();
            const expType = String(exp.type || "").trim();
            if (actType !== expType)
                return false;
            if (!boxValueMatchesSpec(simulator, act, exp).ok)
                return false;
        }
        return true;
    }
    function defaultsForBoundary(boundary) {
        const pendingBaseline = runtimePendingEditableBaselineState(boundary);
        if (pendingBaseline)
            return cloneBoxes(pendingBaseline || []);
        return cloneBoxes(getExpectedState(boundary) || []);
    }
    function setStatus(text, cls = "muted") {
        if (!statusEl)
            return;
        statusEl.textContent = text;
        statusEl.className = cls;
    }
    function buttonReplacements(runLabel) {
        const resolvedLabel = runLabel || "Run line";
        return [
            ["$runLineButton", `$b{${resolvedLabel}}`],
            ["$backButton", "$b{Back ◀}"],
            ["$checkButton", "$b{Check}"],
            ["$resetButton", "$b{Reset}"],
            ["$newVariableButton", "$b{+ New variable}"],
            ["$showAliasesButton", "$b{Show aliases}"],
        ];
    }
    function applyButtonTokens(parts, runLabel) {
        return applyTextTokenReplacements(parts, buttonReplacements(runLabel));
    }
    function formatRunLabel(start, end, withArrow, verb = "Run") {
        if (start === end) {
            return `${verb} line ${start}${withArrow ? " ▶" : ""}`;
        }
        return `${verb} lines ${start}-${end}${withArrow ? " ▶" : ""}`;
    }
    function formatNameList(names) {
        const tokens = names.map((name) => `$n{${name}}`);
        if (tokens.length === 1)
            return tokens[0] || "";
        if (tokens.length === 2)
            return `${tokens[0]} and ${tokens[1]}`;
        return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
    }
    function baselineForBoundary(boundary) {
        const stage = runtimeStateEditStageForBoundary(boundary, {
            includeSolved: true,
        });
        if (stage?.baselineState)
            return cloneBoxes(stage.baselineState);
        return defaultsForBoundary(boundary);
    }
    function basicHintForBoxes(boxes, boundary) {
        const actual = boxes;
        const expected = getExpectedState(boundary);
        const actualCount = actual.length;
        const expectedCount = expected.length;
        const nameOf = (box) => String(box?.name || "").trim();
        const typeOf = (box) => String(box?.type || "").trim();
        const expectedNames = expected.map(nameOf).filter(Boolean);
        const expectedNameSet = new Set(expectedNames);
        const actualNames = actual.map(nameOf);
        const actualNameSet = new Set(actualNames.filter(Boolean));
        const missingExpectedNames = expectedNames.filter((name) => !actualNameSet.has(name));
        const baselineAtBoundary = baselineForBoundary(boundary);
        const baselineNames = new Set(baselineAtBoundary.map(nameOf).filter(Boolean));
        const removedName = missingExpectedNames.find((name) => baselineNames.has(name));
        if (removedName) {
            return {
                message: `This line shouldn't remove the $n{${removedName}} variable.`,
                kind: "removed",
                variable: removedName,
            };
        }
        const extraBaselineNames = actualNames.filter((name) => name && baselineNames.has(name) && !expectedNameSet.has(name));
        if (extraBaselineNames.length > 0) {
            const name = extraBaselineNames[0] || "";
            if (name) {
                return {
                    message: `This line should remove the $n{${name}} variable.`,
                    kind: "not-removed",
                    variable: name,
                };
            }
        }
        if (actualCount < expectedCount) {
            const expectedName = missingExpectedNames[0] || expectedNames[0] || "";
            if (!expectedName)
                return { message: "You need to add a new variable.", kind: "count" };
            return {
                message: `You need to add the $n{${expectedName}} variable.`,
                kind: "count",
                variable: expectedName,
            };
        }
        if (actualCount === expectedCount && missingExpectedNames.length > 0) {
            const expectedNewNames = expectedNames.filter((name) => !baselineNames.has(name));
            if (expectedNewNames.length > 1) {
                return {
                    message: `The new variables should be named ${formatNameList(expectedNewNames)}.`,
                    kind: "name",
                    variable: expectedNewNames[0],
                };
            }
            const expectedName = expectedNewNames[0] || missingExpectedNames[0] || "";
            if (!expectedName)
                return null;
            return {
                message: `The new variable should be named $n{${expectedName}}.`,
                kind: "name",
                variable: expectedName,
            };
        }
        if (actualCount > expectedCount) {
            const baselineCount = baselineAtBoundary.length;
            const expectedNew = Math.max(0, expectedCount - baselineCount);
            const actualNew = Math.max(0, actualCount - baselineCount);
            const extraCount = Math.max(0, actualNew - expectedNew);
            const groupRange = groupRangeEndingAt(boundary);
            const statementRange = statementRangeEndingAt(statementMap, boundary);
            let label = `Line ${boundary + 1}`;
            if (groupRange) {
                const start = groupRange.startLine + 1;
                const end = groupRange.endLine + 1;
                label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
            }
            else if (statementRange && statementRange.endLine > statementRange.startLine) {
                const start = statementRange.startLine + 1;
                const end = statementRange.endLine + 1;
                label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
            }
            const extraLabel = extraCount === 1 ? "variable" : "variables";
            if (expectedNew === 0) {
                return {
                    message: `${label} shouldn't add any new variables. Remove the extra ${extraLabel}.`,
                    kind: "count",
                };
            }
            const expectedLabel = expectedNew === 1 ? "variable" : "variables";
            return {
                message: `${label} should only add ${expectedNew} new ${expectedLabel}, but you added ${actualNew}. Remove the extra ${extraLabel}.`,
                kind: "count",
            };
        }
        const baselineByName = new Map();
        baselineAtBoundary.forEach((box) => {
            const name = nameOf(box);
            if (name && !baselineByName.has(name))
                baselineByName.set(name, box);
        });
        const actualByName = new Map();
        actual.forEach((box) => {
            const name = nameOf(box);
            if (name && !actualByName.has(name))
                actualByName.set(name, box);
        });
        let deferredBe = null;
        for (const exp of expected) {
            const name = nameOf(exp);
            if (!name)
                continue;
            const act = actualByName.get(name);
            if (!act)
                continue;
            const expType = typeOf(exp);
            const actType = typeOf(act);
            if (actType !== expType) {
                return {
                    message: `$n{${name}}'s type should be $t{${expType}}.`,
                    kind: "type",
                    variable: name,
                };
            }
            const mismatch = !boxValueMatchesSpec(simulator, act, exp).ok;
            if (mismatch) {
                const expVal = (exp.value ?? "").trim();
                const label = expVal === "" ? "empty" : `$v{${normalizeZeroDisplay(expVal)}}`;
                const baselineBox = baselineByName.get(name);
                const shouldRemain = baselineBox
                    ? boxValueMatchesSpec(simulator, baselineBox, exp).ok
                    : false;
                const message = `$n{${name}}'s value should ${shouldRemain ? "remain" : "be"} ${label}.`;
                if (shouldRemain) {
                    return {
                        message,
                        kind: "value",
                        variable: name,
                    };
                }
                if (!deferredBe) {
                    deferredBe = {
                        message,
                        kind: "value",
                        variable: name,
                    };
                }
            }
        }
        return deferredBe;
    }
    function partsContext({ boxes, } = {}) {
        const resolvedBoxes = boxes ?? [];
        const normalizedBoxes = resolvedBoxes.map((box) => normalizeBoxValueForContext(simulator, box));
        const topic = basicHintForBoxes(normalizedBoxes, state.boundary);
        return {
            boxes: normalizedBoxes,
            basicHint: topic?.message ?? null,
            basicHintTopicIs: (kind, variable) => !!topic &&
                topic.kind === kind &&
                (variable === undefined || topic.variable === variable),
            _basicHintTopic: topic,
            boxNamed: (name) => boxNamed(resolvedBoxes, name),
            boxesNamed: (...names) => boxesNamed(resolvedBoxes, ...names),
        };
    }
    function resolveParts(spec, ctx) {
        if (!spec)
            return null;
        const resolved = spec(ctx);
        if (!resolved)
            return null;
        return String(resolved);
    }
    function getHintParts(ctx) {
        const stage = runtimeStateEditStageForBoundary(state.boundary);
        const hintSpec = stage?.hints ?? null;
        return resolveParts(hintSpec, ctx);
    }
    function renderCodePaneForBoundary() {
        if (!codeEl)
            return;
        const key = state.boundary;
        const currentStage = runtimeCurrentStage();
        const strikeBoundary = currentStage &&
            currentStage.editableMode === "state" &&
            runtimeStageNeedsSolve(currentStage)
            ? currentStage.beforeBoundary
            : key;
        const runtimeStateStage = runtimeStateEditStageForBoundary(key, {
            includeSolved: true,
        });
        const runtimeBoundaryStage = runtimeBoundaryEditStageForBoundary(key, {
            includeSolved: true,
        });
        const runtimeStateEditable = !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
        const runtimeBranchEditable = !!runtimeBoundaryStage &&
            runtimeStageNeedsSolve(runtimeBoundaryStage) &&
            (runtimeBoundaryStage.interactionBoundary ?? runtimeBoundaryStage.afterBoundary) ===
                key;
        if (runtimeBranchEditable) {
            branchSelectionActive = true;
            branchSelectionBoundary = key;
        }
        else {
            clearBranchSelection();
        }
        let progress = runtimeStateEditable;
        let progressRange;
        let progressIndex;
        let doneBoundary;
        const branchInfo = branchInfoForBoundary(key);
        const branchSelectable = !!branchInfo && runtimeBranchEditable;
        activeBranchTargets = branchSelectable ? allBoundaryTargets : null;
        if (runtimeStateEditable && runtimeStateStage) {
            const range = runtimeStageLineRange(runtimeStateStage) || {
                start: runtimeStateStage.runLine,
                end: runtimeStateStage.runLine,
            };
            progressRange = [range.start, range.end];
            progressIndex = range.end;
            doneBoundary = range.start;
        }
        else if (runtimeBranchEditable && branchInfo) {
            progress = true;
            progressRange = [branchInfo.rangeStart, branchInfo.rangeEnd];
            progressIndex = undefined;
        }
        let strikeRanges = [];
        let strikeFragments = [];
        const elseColumnForLine = (lineIndex, block) => {
            const rawLine = lineList[lineIndex] || "";
            const elseTok = block.elseIndex == null ? null : parts[block.elseIndex].tokens[0];
            if (elseTok) {
                const col = elseTok.col;
                if (col >= 0 && col <= rawLine.length)
                    return col;
            }
            const direct = rawLine.indexOf("else");
            if (direct >= 0)
                return direct;
            const match = rawLine.search(/\belse\b/);
            if (match >= 0)
                return match;
            const braceIdx = rawLine.indexOf("}");
            if (braceIdx >= 0) {
                let col = braceIdx + 1;
                while (col < rawLine.length && /\s/.test(rawLine[col]))
                    col += 1;
                return col < rawLine.length ? col : -1;
            }
            return -1;
        };
        const headerStrikeStartForLine = (lineIndex, block) => {
            const rawLine = lineList[lineIndex] || "";
            const headerTok = parts[block.headerIndex].tokens[0];
            const ifCol = headerTok.col;
            if (ifCol > 0 && ifCol <= rawLine.length) {
                let foundElseCol = -1;
                const re = /\belse\b/g;
                let match = null;
                while ((match = re.exec(rawLine)) !== null) {
                    if (match.index >= ifCol)
                        break;
                    foundElseCol = match.index;
                }
                if (foundElseCol >= 0)
                    return foundElseCol;
            }
            return 0;
        };
        ifBlocks.map.forEach((block) => {
            if (strikeBoundary <= block.headerEndLine)
                return;
            const pendingBoundaryStage = runtimePendingStage();
            if (pendingBoundaryStage &&
                pendingBoundaryStage.editableMode === "boundary" &&
                !runtimeStageSolved(pendingBoundaryStage) &&
                pendingBoundaryStage.runLine === block.headerStartLine) {
                return;
            }
            const currentState = stateBeforePart(block.headerIndex);
            const condition = simulator.evaluateCondition(block.expr, currentState);
            if ("error" in condition)
                return;
            const headerLine = block.headerStartLine;
            const headerText = lineList[headerLine] || "";
            const blockElseLine = block.elseIndex == null ? null : parts[block.elseIndex].startLine;
            if (headerText.includes("else") && blockElseLine === headerLine) {
                const elseCol = elseColumnForLine(headerLine, block);
                if (elseCol >= 0) {
                    strikeFragments.push({
                        line: headerLine,
                        start: condition.value ? elseCol : 0,
                        end: condition.value ? headerText.length : elseCol,
                    });
                }
                return;
            }
            const ifCloseLine = parts[block.closeIndex].endLine;
            const ifOpenLine = parts[block.openIndex].startLine;
            if (!condition.value) {
                if (ifOpenLine > headerLine) {
                    const headerStart = headerStrikeStartForLine(headerLine, block);
                    strikeFragments.push({
                        line: headerLine,
                        start: headerStart,
                        end: headerText.length,
                    });
                }
                if (block.elseIndex != null &&
                    block.elseOpenIndex != null &&
                    ifCloseLine === parts[block.elseIndex].startLine) {
                    const elseCol = elseColumnForLine(ifCloseLine, block);
                    if (elseCol >= 0) {
                        strikeFragments.push({
                            line: ifCloseLine,
                            start: 0,
                            end: elseCol,
                        });
                    }
                    if (ifCloseLine > ifOpenLine) {
                        strikeRanges.push([ifOpenLine, ifCloseLine - 1]);
                    }
                }
                else {
                    strikeRanges.push([ifOpenLine, ifCloseLine]);
                }
                return;
            }
            if (block.elseIndex == null || block.elseCloseIndex == null)
                return;
            const rawElseStart = parts[block.elseOpenIndex ?? block.elseIndex].startLine;
            const elseStartLine = rawElseStart === ifCloseLine ? rawElseStart + 1 : rawElseStart;
            const elseCloseLine = parts[block.elseCloseIndex].endLine;
            if (elseStartLine <= elseCloseLine) {
                const lineHasElse = (lineList[ifCloseLine] || "").includes("else");
                const sameLine = lineHasElse || elseStartLine === ifCloseLine;
                if (sameLine) {
                    const elseCol = elseColumnForLine(ifCloseLine, block);
                    if (elseCol >= 0) {
                        strikeFragments.push({
                            line: ifCloseLine,
                            start: elseCol,
                            end: (lineList[ifCloseLine] || "").length,
                        });
                    }
                    if (elseCloseLine > ifCloseLine) {
                        strikeRanges.push([ifCloseLine + 1, elseCloseLine]);
                    }
                    return;
                }
                strikeRanges.push([elseStartLine, elseCloseLine]);
            }
        });
        renderCodePane(codeEl, lineList, key, {
            progress,
            progressRange,
            progressIndex,
            doneBoundary,
            hideBoundary: branchSelectable,
            selectableBoundaries: branchSelectable ? allBoundaries : undefined,
            selectedBoundary: branchSelectable ? branchSelectionTarget : null,
            suppressProgressMid: runtimeBranchEditable,
            boundaryTargets: branchSelectable,
            strikeRanges,
            strikeFragments,
        });
    }
    function ensureCodeLineVisible() {
        if (!codeEl)
            return;
        const activeStage = runtimePendingStage() ?? runtimeCurrentStage();
        const range = activeStage
            ? (runtimeStageLineRange(activeStage) || {
                start: activeStage.runLine,
                end: activeStage.runLine,
            })
            : {
                start: 0,
                end: 0,
            };
        if (range.start < 0 || range.start >= lineList.length)
            return;
        if (range.end < 0 || range.end >= lineList.length)
            return;
        const lines = codeEl.querySelectorAll(".line");
        const startEl = lines[range.start];
        const endEl = lines[range.end];
        if (!startEl || !endEl)
            return;
        const container = codeEl;
        const containerRect = container.getBoundingClientRect();
        const startRect = startEl.getBoundingClientRect();
        const endRect = endEl.getBoundingClientRect();
        const lineTop = startRect.top - containerRect.top + container.scrollTop;
        const lineBottom = endRect.bottom - containerRect.top + container.scrollTop;
        const bottomPadding = 24;
        const viewTop = container.scrollTop;
        const viewBottom = viewTop + container.clientHeight;
        if (lineTop < viewTop) {
            container.scrollTop = Math.max(0, Math.floor(lineTop));
            return;
        }
        if (lineBottom > viewBottom) {
            container.scrollTop = Math.max(0, Math.ceil(lineBottom - container.clientHeight + bottomPadding));
        }
    }
    function ensureNewVariableVisible(node) {
        if (!node)
            return;
        requestAnimationFrame(() => {
            const stateContainer = stageEl;
            if (stateContainer) {
                stateContainer.scrollTo({
                    top: stateContainer.scrollHeight,
                    behavior: "smooth",
                });
                return;
            }
            node.scrollIntoView({ block: "end", behavior: "smooth" });
        });
    }
    function maybeScrollStateOnGrowth(nextCount) {
        const previousCount = state.lastRenderedStateCount;
        state.lastRenderedStateCount = nextCount;
        if (previousCount == null || nextCount <= previousCount)
            return;
        requestAnimationFrame(() => {
            const stateContainer = stageEl;
            if (!stateContainer)
                return;
            stateContainer.scrollTo({
                top: stateContainer.scrollHeight,
                behavior: "smooth",
            });
        });
    }
    function renderStage() {
        if (!stageEl)
            return;
        clearNode(stageEl);
        const key = state.boundary;
        const runtimeEditableStage = runtimeStateEditStageForBoundary(key, {
            includeSolved: true,
        });
        const editable = !!runtimeEditableStage && !runtimeStageSolved(runtimeEditableStage);
        const defaults = defaultsForBoundary(key);
        const traceCount = runtimeCurrentTrace()?.state?.length ?? 0;
        state.workspaceEl = null;
        if (key <= 0 && !editable) {
            maybeScrollStateOnGrowth(traceCount);
            refreshOtherNames();
            return;
        }
        if (!editable) {
            const expected = cloneBoxes(getExpectedState(state.boundary) || []);
            const grid = document.createElement("div");
            grid.className = "grid";
            if (!expected.length) {
                const msg = document.createElement("div");
                msg.className = "muted";
                msg.style.padding = "8px";
                msg.textContent = "(no variables yet)";
                grid.appendChild(msg);
            }
            else {
                appendStateObjects(grid, expected, {
                    editable: false,
                    deletable: false,
                });
            }
            stageEl.appendChild(grid);
            maybeScrollStateOnGrowth(expected.length);
            refreshOtherNames();
            return;
        }
        const runtimeSnapshot = runtimeEditableStage != null
            ? (runtimeWorkspaceByStage.get(runtimeEditableStage.index) ?? null)
            : null;
        const wrap = restoreWorkspace(runtimeSnapshot, defaults, {
            editable,
            deletable: allowVariableDeletion,
            allowNameEdit: null,
            allowTypeEdit: null,
        });
        stageEl.appendChild(wrap);
        state.workspaceEl = wrap;
        attachResetWatcher(wrap, key);
        maybeScrollStateOnGrowth(defaults.length);
        refreshOtherNames();
    }
    if (showOtherNames && stageEl) {
        stageEl.addEventListener("input", () => {
            refreshOtherNames();
        });
        stageEl.addEventListener("click", (event) => {
            const target = event?.target;
            if (target?.closest?.(".delete")) {
                requestAnimationFrame(refreshOtherNames);
            }
        });
    }
    if (stageEl) {
        stageEl.addEventListener("input", () => {
            save();
            persistProgress();
        });
        stageEl.addEventListener("click", () => {
            window.setTimeout(() => {
                save();
                persistProgress();
            }, 0);
        });
    }
    function scrollInstructionsUpIfNeeded(instructionKey) {
        if (!instructionsEl || !instructionKey || instructionKey === state.lastInstructionKey)
            return;
        requestAnimationFrame(() => {
            const rect = instructionsEl?.getBoundingClientRect();
            if (!rect)
                return;
            const offset = 24;
            const top = Math.max(0, rect.top + window.scrollY - offset);
            if (top < window.scrollY) {
                window.scrollTo({ top, behavior: "smooth" });
            }
        });
    }
    function updateInstructions() {
        const runLabel = runtimeRunLabel(true);
        const instructionStage = runtimeCurrentStage();
        let instructionKey = null;
        let parts = null;
        if (!instructionStage && hasInitialInstructionsContent) {
            parts = initialInstructions || "";
            instructionKey = "__initial__";
        }
        else if (instructionStage?.instructions) {
            parts = String(instructionStage.instructions);
            instructionKey = `runtime-stage-${instructionStage.index}`;
        }
        parts = applyButtonTokens(parts || null, runLabel);
        setPartsContent(instructionsEl, parts);
        scrollInstructionsUpIfNeeded(instructionKey);
        state.lastInstructionKey = instructionKey;
    }
    function hideHint() {
        if (!hintPanel)
            return;
        hintPanel.textContent = "";
        hintPanel.classList.add("hidden");
    }
    function readWorkspaceBoxes() {
        const ws = getWorkspaceEl();
        if (!ws)
            return [];
        return serializeWorkspace(ws) || [];
    }
    function evaluateWorkspace(boxes) {
        const expected = getExpectedState(state.boundary);
        return { ok: boxesEqual(boxes, expected), expected };
    }
    function showHint(parts, runLabel) {
        if (!hintPanel)
            return;
        if (!parts || (Array.isArray(parts) && parts.length === 0))
            return;
        const rendered = applyButtonTokens(parts, runLabel);
        renderParts(hintPanel, rendered || "");
        hintPanel.classList.remove("hidden");
        flashStatus(hintPanel);
    }
    function isLooksGoodParts(parts) {
        if (typeof parts === "string") {
            return parts.trim() === "Looks good. Press $checkButton.";
        }
        return false;
    }
    function render() {
        if (branchSelectionActive &&
            branchSelectionBoundary !== state.boundary) {
            clearBranchSelection();
        }
        renderCodePaneForBoundary();
        renderStage();
        hideHint();
        const key = state.boundary;
        const runtimeStateStage = runtimeStateEditStageForBoundary(key, {
            includeSolved: true,
        });
        const runtimeBoundaryStage = runtimeBoundaryEditStageForBoundary(key, {
            includeSolved: true,
        });
        const branchSolved = !!runtimeBoundaryStage && runtimeStageSolved(runtimeBoundaryStage);
        const branchSelectionHere = branchSelectionActive && branchSelectionBoundary === key;
        const hasSolvedEditable = (!!runtimeStateStage && runtimeStageSolved(runtimeStateStage)) ||
            (!!runtimeBoundaryStage && runtimeStageSolved(runtimeBoundaryStage));
        const normalEditable = !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
        const branchInfo = branchInfoForBoundary(key);
        const branchEditable = branchSelectionActive &&
            branchSelectionBoundary === key &&
            !!branchInfo &&
            !!runtimeBoundaryStage &&
            !runtimeStageSolved(runtimeBoundaryStage);
        if (statusEl) {
            if (hasSolvedEditable && !branchSelectionHere) {
                setStatus("correct", "ok");
            }
            else if (normalEditable) {
                setStatus("", "muted");
            }
            else if (runtimeBoundaryStage) {
                setStatus(branchSolved ? "correct" : "", branchSolved ? "ok" : "muted");
            }
            else {
                setStatus("", "muted");
            }
        }
        if (checkBtn)
            checkBtn.classList.toggle("hidden", !normalEditable && !branchEditable);
        const branchStepActive = branchSelectionActive && branchSelectionBoundary === key;
        if (hintBtn)
            hintBtn.classList.toggle("hidden", !normalEditable || branchStepActive);
        if (addBtn)
            addBtn.classList.toggle("hidden", !normalEditable ||
                !workspace.allowVariableCreation ||
                branchStepActive);
        if (resetBtn)
            resetBtn.classList.add("hidden");
        updateMobileActionsVisibility();
        updateInstructions();
        if (normalEditable && resetBtn)
            updateResetVisibility(key);
        ensureCodeLineVisible();
        persistProgress();
    }
    function save() {
        const key = state.boundary;
        const runtimeKey = runtimeEditableStorageKey(key);
        if (runtimeKey == null)
            return;
        const snapshot = serializeWorkspace(getWorkspaceEl());
        if (Array.isArray(snapshot)) {
            runtimeWorkspaceByStage.set(runtimeKey, snapshot);
        }
    }
    if (addBtn) {
        addBtn.addEventListener("click", () => {
            const ws = getWorkspaceEl();
            if (!ws)
                return;
            const node = makeAnswerBox({
                address: nextWorkspaceAddress(ws, "int"),
                allowNameEdit: true,
                deletable: true,
            });
            node.dataset.allowDelete = "true";
            ws.appendChild(node);
            ensureNewVariableVisible(node);
            updateResetVisibility(state.boundary);
            refreshOtherNames();
            save();
            persistProgress();
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
    if (resetBtn) {
        resetBtn.addEventListener("click", () => {
            const key = state.boundary;
            const runtimeKey = runtimeEditableStorageKey(key);
            if (runtimeKey == null)
                return;
            runtimeWorkspaceByStage.delete(runtimeKey);
            if (runtimeLatestSolvedStage >= runtimeKey) {
                runtimeLatestSolvedStage = runtimeKey - 1;
            }
            syncBoundaryFromStage();
            render();
            pager.update();
        });
    }
    if (checkBtn) {
        checkBtn.addEventListener("click", () => {
            hideHint();
            const key = state.boundary;
            if (branchSelectionActive &&
                branchSelectionBoundary !== key) {
                clearBranchSelection();
            }
            const branchInfo = branchInfoForBoundary(key);
            const runtimeBranchStage = runtimeBoundaryEditStageForBoundary(key, {
                includeSolved: true,
            });
            const runtimeBranchSolved = !!runtimeBranchStage && runtimeStageSolved(runtimeBranchStage);
            const runtimeBranchEditable = branchSelectionActive &&
                branchSelectionBoundary === key &&
                !!branchInfo &&
                !!runtimeBranchStage &&
                !runtimeBranchSolved;
            const branchEditable = runtimeBranchEditable;
            if (branchEditable) {
                if (!branchSelectionActive) {
                    setStatus("Select a branch first.", "err");
                    flashStatus(statusEl);
                    return;
                }
                if (branchSelectionTarget == null) {
                    setStatus("Select a line boundary.", "err");
                    flashStatus(statusEl);
                    return;
                }
                if (branchInfo?.expected == null) {
                    setStatus("That branch can't be evaluated.", "err");
                    flashStatus(statusEl);
                    return;
                }
                if (branchSelectionTarget !== branchInfo.expected) {
                    setStatus("incorrect", "err");
                    flashStatus(statusEl);
                    return;
                }
                if (!runtimeBranchStage) {
                    setStatus("That branch can't be evaluated.", "err");
                    flashStatus(statusEl);
                    return;
                }
                runtimeMarkCurrentStageSolved();
                setStatus("correct", "ok");
                flashStatus(statusEl);
                render();
                pager.update();
                pager.pulseNext();
                return;
            }
            const runtimeStateStage = runtimeStateEditStageForBoundary(key, {
                includeSolved: true,
            });
            const runtimeStateEditable = !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
            if (!runtimeStateEditable)
                return;
            const boxes = readWorkspaceBoxes();
            const result = evaluateWorkspace(boxes);
            const ok = result.ok;
            setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
            flashStatus(statusEl);
            if (!ok)
                return;
            if (!runtimeStateStage)
                return;
            runtimeWorkspaceByStage.delete(runtimeStateStage.index);
            runtimeMarkCurrentStageSolved();
            const ws = getWorkspaceEl();
            if (ws) {
                ws
                    .querySelectorAll(".vbox, .arraybox")
                    .forEach((v) => disableBoxEditing(v));
                removeBoxDeleteButtons(ws);
            }
            if (checkBtn)
                checkBtn.classList.add("hidden");
            if (hintBtn)
                hintBtn.classList.add("hidden");
            if (addBtn)
                addBtn.classList.add("hidden");
            if (resetBtn)
                resetBtn.classList.add("hidden");
            pager.pulseNext();
            render();
            pager.update();
        });
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            const boxes = readWorkspaceBoxes();
            const result = evaluateWorkspace(boxes);
            const ctx = partsContext({ boxes });
            const runLabel = runtimeRunLabel(true);
            if (result.ok) {
                showHint("Looks good. Press $checkButton.", runLabel);
                return;
            }
            const parts = getHintParts(ctx);
            if (!parts || (Array.isArray(parts) && parts.length === 0)) {
                showHint("Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.", runLabel);
                return;
            }
            if (isLooksGoodParts(parts)) {
                showHint("Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.", runLabel);
                return;
            }
            showHint(parts, runLabel);
        });
    }
    if (restoredProgress) {
        state.executionSteps = runtimeStepClamp(restoredProgress.executionSteps);
        state.allocBase =
            typeof restoredProgress.allocBase === "number"
                ? restoredProgress.allocBase
                : null;
        const solvedStage = Math.floor(Number(restoredProgress.runtimeLatestSolvedStage));
        runtimeLatestSolvedStage = Number.isFinite(solvedStage)
            ? Math.max(-1, Math.min(runtimeMaxStep(), solvedStage))
            : -1;
        runtimeWorkspaceByStage.clear();
        if (Array.isArray(restoredProgress.runtimeWorkspaceByStage)) {
            restoredProgress.runtimeWorkspaceByStage.forEach((entry) => {
                const stageIndex = Math.floor(Number(entry?.stageIndex));
                if (!Number.isFinite(stageIndex))
                    return;
                if (stageIndex < 0 || stageIndex > runtimeMaxStep())
                    return;
                const stage = runtimeStages[stageIndex];
                if (!stage || stage.editableMode !== "state")
                    return;
                const boxes = Array.isArray(entry?.boxes) ? cloneBoxes(entry.boxes) : null;
                runtimeWorkspaceByStage.set(stageIndex, boxes);
            });
        }
    }
    syncBoundaryFromStage();
    const pager = createStepper({
        root: codeRoot || codeEl?.closest(".panel") || document.body,
        lines: runtimeStages.length + 1,
        nextPage: next || null,
        endLabel,
        getBoundary: () => state.executionSteps + 1,
        setBoundary: (val) => {
            const nextStage = runtimeStepClamp(val - 1);
            if (nextStage !== state.executionSteps) {
                state.executionSteps = nextStage;
                clearBranchSelection();
            }
            syncBoundaryFromStage();
        },
        onBeforeChange: () => {
            save();
            persistProgress();
        },
        onAfterChange: render,
        isStepLocked: () => !!runtimePendingStage(),
        getStepBadge: () => runtimeStepBadge(),
        getNextLabel: () => {
            if (runtimeIsComplete())
                return endLabel;
            const pending = runtimePendingStage();
            if (pending && pending.editableMode === "boundary")
                return "???";
            return runtimeRunLabel(false);
        },
        isAtEnd: () => runtimeIsComplete(),
    });
    if (codeEl) {
        codeEl.addEventListener("click", (event) => {
            if (!activeBranchTargets)
                return;
            const target = event?.target;
            const boundaryEl = target?.closest?.(".boundary.selectable");
            if (!boundaryEl)
                return;
            const boundaryStr = boundaryEl.dataset.boundary;
            if (!boundaryStr)
                return;
            const boundaryIndex = Number(boundaryStr);
            if (!Number.isFinite(boundaryIndex))
                return;
            if (!activeBranchTargets.has(boundaryIndex))
                return;
            branchSelectionTarget = boundaryIndex;
            render();
            pager.update();
        });
    }
    render();
    pager.update();
    return { state, pager };
}
export { createProgramTemplate };

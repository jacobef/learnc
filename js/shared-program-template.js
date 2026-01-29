import { applyOtherNames, boxValueMatchesSpec, cloneBoxes, createSimpleSimulator, createStepper, disableBoxEditing, ensureBaseLayout, flashStatus, getNavLabelForHref, makeAnswerBox, normalizeBoxValueForContext, normalizeZeroDisplay, randAddr, readBoxState, removeBoxDeleteButtons, renderCodePane, renderParts, resolveActiveNavItem, restoreWorkspace, serializeWorkspace, setPartsContent, typeInfo, vbox, } from "./shared-core.js";
function collectProgramElements(root = document) {
    return {
        instructionsEl: root.querySelector('[data-role="program-instructions"]'),
        codeEl: root.querySelector('[data-role="program-code"]'),
        codeRoot: root.querySelector('[data-role="program-code-panel"]'),
        stageEl: root.querySelector('[data-role="program-stage"]'),
        statusEl: root.querySelector('[data-role="program-status"]'),
        hintPanel: root.querySelector('[data-role="program-hint"]'),
        hintBtn: root.querySelector('[data-role="program-hint-btn"]'),
        checkBtn: root.querySelector('[data-role="program-check"]'),
        addBtn: root.querySelector('[data-role="program-add"]'),
        resetBtn: root.querySelector('[data-role="program-reset"]'),
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
    const activeItem = resolveActiveNavItem();
    const resolvedTitle = activeItem?.label || "";
    const nextBrowserTitle = resolvedTitle ? `C Boxes - ${resolvedTitle}` : "";
    if (nextBrowserTitle)
        document.title = nextBrowserTitle;
    const existing = document.querySelector('[data-role="program-code"]');
    if (existing)
        return collectProgramElements();
    const { main } = ensureBaseLayout();
    if (resolvedTitle) {
        const heading = document.createElement("h1");
        heading.className = "page-title";
        heading.textContent = resolvedTitle;
        main.appendChild(heading);
    }
    const instructionsEl = document.createElement("p");
    instructionsEl.dataset.role = "program-instructions";
    instructionsEl.className = "intro";
    main.appendChild(instructionsEl);
    const section = document.createElement("section");
    const row = document.createElement("div");
    row.className = "row";
    section.appendChild(row);
    main.appendChild(section);
    const codePanel = document.createElement("div");
    codePanel.className = "panel";
    codePanel.dataset.role = "program-code-panel";
    const codeTitle = document.createElement("div");
    codeTitle.className = "panel-title code-title";
    codeTitle.textContent = "Code";
    const codeEl = document.createElement("div");
    codeEl.dataset.role = "program-code";
    codeEl.className = "codepane";
    const codeControls = document.createElement("div");
    codeControls.className = "controls";
    const prevBtn = document.createElement("button");
    prevBtn.textContent = "Back ◀";
    prevBtn.dataset.stepper = "prev";
    const nextBtn = document.createElement("button");
    nextBtn.textContent = "Run line 1 ▶";
    nextBtn.dataset.stepper = "next";
    codeControls.appendChild(prevBtn);
    codeControls.appendChild(nextBtn);
    codePanel.appendChild(codeTitle);
    codePanel.appendChild(codeEl);
    codePanel.appendChild(codeControls);
    const statePanel = document.createElement("div");
    statePanel.className = "panel program-state-panel";
    const stateTitle = document.createElement("div");
    stateTitle.className = "panel-title";
    stateTitle.textContent = "Program state";
    const stageEl = document.createElement("div");
    stageEl.dataset.role = "program-stage";
    const stateControls = document.createElement("div");
    stateControls.className = "controls";
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
    resetBtn.className = "hidden";
    resetBtn.textContent = "Reset";
    const statusEl = document.createElement("span");
    statusEl.dataset.role = "program-status";
    statusEl.className = "muted";
    stateControls.appendChild(checkBtn);
    stateControls.appendChild(hintBtn);
    stateControls.appendChild(addBtn);
    stateControls.appendChild(resetBtn);
    stateControls.appendChild(statusEl);
    const hintPanel = document.createElement("div");
    hintPanel.dataset.role = "program-hint";
    hintPanel.className = "hint-inline hidden";
    statePanel.appendChild(stateTitle);
    statePanel.appendChild(stageEl);
    statePanel.appendChild(stateControls);
    statePanel.appendChild(hintPanel);
    row.appendChild(codePanel);
    row.appendChild(statePanel);
    return {
        instructionsEl,
        codeEl,
        codeRoot: codePanel,
        stageEl,
        statusEl,
        hintPanel,
        hintBtn,
        checkBtn,
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
    const simulator = createSimpleSimulator({
        allowVarAssign: true,
        requireSourceValue: true,
        allowPointers: true,
    });
    const { instructionsEl, codeEl, codeRoot, stageEl, statusEl, hintPanel, hintBtn, checkBtn, addBtn, resetBtn, } = ensureProgramLayout();
    if (initialInstructions !== undefined &&
        typeof initialInstructions !== "string") {
        failConfig("Program initialInstructions must be a string.");
    }
    if (!Array.isArray(steps) || steps.length === 0) {
        failConfig("Program steps must be a non-empty array.");
    }
    const stepInfos = [];
    const stepByStartLine = new Map();
    const lineList = [];
    const canRunWithAutoClosedBlocks = (text) => {
        const tokens = simulator.tokenizeProgram(text);
        let balance = 0;
        tokens.forEach((tok) => {
            if (tok.type !== "sym")
                return;
            if (tok.value === "{")
                balance += 1;
            else if (tok.value === "}")
                balance -= 1;
        });
        if (balance <= 0)
            return false;
        const closers = "}\n".repeat(balance);
        const patched = `${text}\n${closers}`;
        const result = simulator.applyProgram(patched);
        return Array.isArray(result);
    };
    const isIfHeaderPart = (part) => {
        const tokens = part?.tokens;
        if (!tokens || tokens.length < 2)
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
    const isOpenBracePart = (part) => {
        if (!part?.tokens?.length || part.tokens.length !== 1)
            return false;
        const tok = part.tokens[0];
        return tok.type === "sym" && tok.value === "{";
    };
    steps.forEach((step, index) => {
        if (!step || typeof step !== "object") {
            failConfig(`Step ${index + 1} must be an object.`);
        }
        if (typeof step.code !== "string") {
            failConfig(`Step ${index + 1} must include a code string.`);
        }
        if (!step.code.endsWith("\n")) {
            failConfig(`Step ${index + 1} code must end with a newline.`);
        }
        if (typeof step.editable !== "boolean") {
            failConfig(`Step ${index + 1} must specify editable as true or false.`);
        }
        if (step.scrollUp !== undefined && typeof step.scrollUp !== "boolean") {
            failConfig(`Step ${index + 1} scrollUp must be true or false.`);
        }
        if (step.hints && typeof step.hints !== "function") {
            failConfig(`Step ${index + 1} hints must be a function.`);
        }
        const rawLines = step.code.split(/\r?\n/);
        if (rawLines[rawLines.length - 1] === "")
            rawLines.pop();
        if (rawLines.length === 0) {
            failConfig(`Step ${index + 1} code must include at least one line.`);
        }
        const tokens = simulator.tokenizeProgram(step.code);
        const parts = simulator.splitStatements(tokens);
        const stepStartLine = lineList.length;
        const stepEndLine = stepStartLine + rawLines.length - 1;
        const ifBlocks = simulator.buildIfStatementMap(parts, {
            lastLine: Math.max(0, rawLines.length - 1),
        });
        const headerIndices = [];
        parts.forEach((part, partIndex) => {
            if (part.tokens.length && isIfHeaderPart(part)) {
                headerIndices.push(partIndex);
            }
        });
        const nonEmptyParts = parts.filter((part) => part.tokens.length > 0);
        let ifHeaderOnly = false;
        let ifHeaderHasOpenBrace = false;
        if (headerIndices.length > 0) {
            const headerOnlyCandidate = headerIndices.length === 1 &&
                nonEmptyParts.every((part) => isIfHeaderPart(part) || isOpenBracePart(part));
            if (headerOnlyCandidate) {
                ifHeaderOnly = true;
                ifHeaderHasOpenBrace = nonEmptyParts.some((part) => isOpenBracePart(part));
            }
            else {
                const incompleteHeader = headerIndices.some((idx) => {
                    const block = ifBlocks.map.get(idx);
                    if (!block)
                        return true;
                    const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
                        ? parts[block.closeIndex].endLine
                        : block.headerEndLine;
                    if (!Number.isFinite(closeLine))
                        return true;
                    return stepStartLine + closeLine > stepEndLine;
                });
                if (incompleteHeader) {
                    failConfig(`Step ${index + 1} contains an if statement that doesn't include its full block. Either include the full if statement in the step, or make the step only the if header (optionally with "{").`);
                }
            }
        }
        for (let partIndex = 0; partIndex < parts.length; partIndex++) {
            const part = parts[partIndex];
            if (part?.tokens?.length && !part.hasSemicolon) {
                if (ifBlocks.map.has(partIndex))
                    continue;
                if (isIfHeaderPart(part))
                    continue;
                failConfig(`Step ${index + 1} contains an incomplete statement. Each step must be runnable on its own.`);
            }
        }
        const startLine = lineList.length;
        rawLines.forEach((line) => lineList.push(line));
        const endLine = lineList.length - 1;
        stepInfos.push({
            index,
            code: step.code,
            lines: rawLines,
            startLine,
            endLine,
            boundary: endLine + 1,
            instructions: step.instructions,
            hints: step.hints,
            editable: step.editable,
            scrollUp: step.scrollUp,
            ifHeaderOnly,
            ifHeaderHasOpenBrace,
        });
        stepByStartLine.set(startLine, stepInfos[stepInfos.length - 1]);
    });
    if (stepInfos.some((step) => step.endLine < step.startLine)) {
        failConfig("Program steps must each contain at least one line.");
    }
    const total = lineList.length;
    const instructionMap = new Map();
    const scrollUpByBoundary = new Map();
    const hintMap = new Map();
    const editableByBoundary = new Map();
    const stepByBoundary = new Map();
    stepInfos.forEach((step) => {
        if (step.instructions !== undefined && step.instructions !== null) {
            instructionMap.set(step.boundary, step.instructions);
        }
        scrollUpByBoundary.set(step.boundary, step.scrollUp !== false);
        if (step.hints) {
            hintMap.set(step.boundary, step.hints);
        }
        if (step.editable) {
            editableByBoundary.set(step.boundary, step.startLine + 1);
        }
        stepByBoundary.set(step.boundary, step);
    });
    const state = {
        boundary: 0,
        passes: {},
        ws: {},
        baseline: {},
        branchPasses: {},
        allocBase: null,
        workspaceEl: null,
        lastInstructionKey: null,
        lastBoundary: -1,
        lastBranchCorrectBoundary: null,
        lastSkippedRanges: [],
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
        if (!Number.isFinite(endLine))
            return null;
        return (statementMap.parts.find((part) => part.endLine === endLine && part.hasSemicolon) || null);
    }
    function isMultiLineStatement(range) {
        const startLine = range?.startLine;
        const endLine = range?.endLine;
        if (typeof startLine !== "number" || typeof endLine !== "number")
            return false;
        if (!Number.isFinite(startLine) || !Number.isFinite(endLine))
            return false;
        return endLine > startLine;
    }
    const statementMap = simulator.buildStatementMap(lineList);
    const parts = statementMap.parts || [];
    const totalLines = total;
    const ifBlocks = simulator.buildIfStatementMap(parts, {
        lastLine: Math.max(0, total - 1),
    });
    let activeBranchTargets = null;
    let activeBranchExpected = null;
    let branchSelectionActive = false;
    let branchSelectionTarget = null;
    let branchSelectionBoundary = null;
    const groupRanges = stepInfos.map((step) => ({
        startLine: step.startLine,
        endLine: step.endLine,
    }));
    const editableSet = new Set(editableByBoundary.keys());
    const stepBoundaries = stepInfos.map((step) => step.boundary);
    const allBoundaries = Array.from({ length: totalLines + 1 }, (_, i) => i);
    const allBoundaryTargets = new Map();
    allBoundaries.forEach((boundary) => {
        allBoundaryTargets.set(boundary, boundary);
    });
    const hasInitialInstructionsContent = typeof initialInstructions === "string" && initialInstructions.length > 0;
    function partAt(index) {
        if (!Number.isFinite(index))
            return null;
        if (index < 0 || index >= parts.length)
            return null;
        return parts[index] || null;
    }
    function stopIndexForBoundary(boundary) {
        const target = Math.max(0, Math.min(totalLines, boundary));
        if (!parts.length)
            return 0;
        const idx = parts.findIndex((part) => {
            const end = part?.endLine;
            return Number.isFinite(end) && end >= target;
        });
        return idx === -1 ? parts.length : idx;
    }
    function headerIndexForLine(lineIndex) {
        if (!Number.isFinite(lineIndex))
            return null;
        let selected = null;
        for (let i = 0; i < parts.length; i++) {
            const block = ifBlocks.map.get(i);
            if (!block)
                continue;
            if (block.headerStartLine !== lineIndex)
                continue;
            const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
                ? parts[block.closeIndex].endLine
                : block.headerEndLine;
            if (Number.isFinite(closeLine) && closeLine > lineIndex) {
                selected = i;
            }
        }
        return selected;
    }
    function normalizedStatementRange(lineIndex) {
        const range = simulator.statementRangeForLine(statementMap, lineIndex);
        if (range &&
            typeof range.startLine === "number" &&
            typeof range.endLine === "number" &&
            Number.isFinite(range.startLine) &&
            Number.isFinite(range.endLine) &&
            range.endLine >= range.startLine) {
            return { start: range.startLine, end: range.endLine };
        }
        return null;
    }
    function runRangeForBoundary(boundary) {
        const groupRange = groupForLine(boundary);
        if (groupRange && groupRange.endLine > groupRange.startLine) {
            return { start: groupRange.startLine, end: groupRange.endLine };
        }
        const range = normalizedStatementRange(boundary);
        if (range && range.end > range.start)
            return range;
        return null;
    }
    function executedRangeForBoundary(boundary) {
        if (!Number.isFinite(boundary) || boundary <= 0)
            return null;
        const groupRange = groupRangeEndingAt(boundary);
        if (groupRange && groupRange.endLine > groupRange.startLine) {
            return { start: groupRange.startLine, end: groupRange.endLine };
        }
        const range = normalizedStatementRange(boundary - 1);
        if (range && range.end > range.start)
            return range;
        return null;
    }
    stepInfos.forEach((step) => {
        const text = lineList.slice(0, step.boundary).join("\n");
        const result = simulator.applyProgram(text);
        const allowHeaderOnly = step.ifHeaderOnly && !step.ifHeaderHasOpenBrace;
        if (!Array.isArray(result) &&
            !canRunWithAutoClosedBlocks(text) &&
            !allowHeaderOnly) {
            failConfig(`Step ${step.index + 1} cannot be run as-is. Fix the code in this step.`);
        }
    });
    editableSet.forEach((step) => {
        if (Number.isFinite(step))
            state.passes[step] = false;
    });
    function groupForLine(lineIndex) {
        if (!Number.isFinite(lineIndex))
            return null;
        return (groupRanges.find((group) => lineIndex >= group.startLine && lineIndex <= group.endLine) || null);
    }
    function stepEndingAtBoundary(boundary) {
        return stepByBoundary.get(boundary) || null;
    }
    function branchStepInfo(boundary) {
        const step = stepEndingAtBoundary(boundary);
        if (!step?.ifHeaderOnly)
            return null;
        return step;
    }
    function branchInfoForBoundary(boundary) {
        const step = stepEndingAtBoundary(boundary);
        if (!step?.ifHeaderOnly)
            return null;
        const headerIndex = headerIndexForLine(step.startLine);
        if (headerIndex == null)
            return null;
        const block = ifBlocks.map.get(headerIndex);
        if (!block)
            return null;
        const truePart = parts[block.trueTarget];
        const trueLine = Number.isFinite(truePart?.startLine)
            ? truePart.startLine
            : block.headerEndLine;
        const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
            ? parts[block.closeIndex].endLine
            : block.headerEndLine;
        let falseLine = closeLine + 1;
        const falsePart = parts[block.falseTarget];
        if (falsePart && Number.isFinite(falsePart.startLine)) {
            falseLine = falsePart.startLine;
        }
        const maxLine = Math.max(0, totalLines);
        const trueTarget = Math.max(0, Math.min(maxLine, trueLine));
        const falseTarget = Math.max(0, Math.min(maxLine, falseLine));
        const targets = [];
        const targetMap = new Map();
        const addTarget = (boundaryIndex) => {
            if (!Number.isFinite(boundaryIndex))
                return;
            if (boundaryIndex < 0 || boundaryIndex > totalLines)
                return;
            if (!targets.includes(boundaryIndex))
                targets.push(boundaryIndex);
            targetMap.set(boundaryIndex, boundaryIndex);
        };
        addTarget(trueTarget);
        addTarget(falseTarget);
        let expected = null;
        const currentState = stateBeforePart(headerIndex);
        const condition = simulator.evaluateCondition(block.expr, currentState);
        if (!("error" in condition)) {
            expected = condition.value ? trueTarget : falseTarget;
        }
        return {
            rangeStart: step.startLine,
            rangeEnd: step.endLine,
            targets,
            targetMap,
            expected,
        };
    }
    function previousStepBoundary(boundary) {
        let prev = null;
        for (const b of stepBoundaries) {
            if (b < boundary)
                prev = b;
            else
                break;
        }
        return prev;
    }
    function nextStepBoundary(boundary) {
        for (const b of stepBoundaries) {
            if (b > boundary)
                return b;
        }
        return totalLines;
    }
    function groupRangeEndingAt(boundary) {
        const endLine = boundary - 1;
        if (!Number.isFinite(endLine))
            return null;
        return groupRanges.find((group) => group.endLine === endLine) || null;
    }
    function getStatementContext(boundary) {
        return simulator.getStatementContext(lineList, boundary);
    }
    function stateBeforePart(partIndex) {
        const safeIndex = Math.max(0, Math.min(parts.length, partIndex));
        const alloc = allocFactory();
        const result = simulator.applyProgramParts(parts, {
            alloc,
            stop: safeIndex,
        });
        return Array.isArray(result) ? result : [];
    }
    function getExpectedState(boundary) {
        const stopIndex = stopIndexForBoundary(boundary);
        const alloc = allocFactory();
        const result = simulator.applyProgramParts(parts, {
            alloc,
            stop: stopIndex,
        });
        return Array.isArray(result) ? result : [];
    }
    function decorateState(boxes) {
        return cloneBoxes(boxes || []);
    }
    function normalizeState(list) {
        if (!Array.isArray(list))
            return [];
        return list
            .map((b) => ({
            name: String(b.name || "").trim(),
            type: String(b.type || "").trim(),
            value: normalizeZeroDisplay(String(b.value ?? "").trim()),
            address: String(b.address ?? "").trim(),
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
    function ensureBaseline(boundary, defaults) {
        if (!state.baseline[boundary]) {
            state.baseline[boundary] = cloneBoxes(defaults || []);
        }
        return state.baseline[boundary];
    }
    function getWorkspaceEl() {
        return (state.workspaceEl ||
            stageEl?.querySelector?.('[data-role="workspace"]'));
    }
    function updateResetVisibility(boundary) {
        if (!resetBtn)
            return;
        if (!editableSet.has(boundary) || state.passes[boundary]) {
            resetBtn.classList.add("hidden");
            return;
        }
        const baseline = state.baseline[boundary];
        const current = serializeWorkspace(getWorkspaceEl()) || [];
        const changed = Array.isArray(baseline) && !statesEqual(current, baseline);
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
        wrap.querySelectorAll(".vbox").forEach((node) => {
            const box = readBoxState(node);
            const raw = box?.address ?? "";
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
        let next = maxAddr + (Number.isFinite(size) ? size : 4);
        if (Number.isFinite(align) && align > 1 && next % align !== 0) {
            next = Math.ceil(next / align) * align;
        }
        while (used.has(String(next))) {
            next += Number.isFinite(size) ? size : 4;
        }
        return String(next);
    }
    function refreshOtherNames() {
        if (!showOtherNames)
            return;
        applyOtherNames(stageEl, { onToggle: refreshOtherNames });
    }
    function boxesEqual(actual, expected) {
        if (!Array.isArray(actual) || !Array.isArray(expected))
            return false;
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
        if (!editableSet.has(boundary))
            return decorateState(getExpectedState(boundary));
        const groupRange = groupRangeEndingAt(boundary);
        if (groupRange && Number.isFinite(groupRange.startLine)) {
            return decorateState(getExpectedState(groupRange.startLine));
        }
        const range = statementRangeEndingAt(statementMap, boundary);
        if (range && Number.isFinite(range.startLine)) {
            return decorateState(getExpectedState(range.startLine));
        }
        return decorateState(getExpectedState(Math.max(0, boundary - 1)));
    }
    function setStatus(text, cls = "muted") {
        if (!statusEl)
            return;
        statusEl.textContent = text;
        statusEl.className = cls;
    }
    function replaceButtonTokens(text, runLabel) {
        const resolvedLabel = runLabel || "Run line";
        const replacements = [
            ["$runLineButton", `$b{${resolvedLabel}}`],
            ["$backButton", "$b{Back ◀}"],
            ["$checkButton", "$b{Check}"],
            ["$resetButton", "$b{Reset}"],
            ["$newVariableButton", "$b{+ New variable}"],
            ["$showAliasesButton", "$b{Show aliases}"],
        ];
        let out = String(text ?? "");
        replacements.forEach(([needle, value]) => {
            out = out.split(needle).join(value);
        });
        return out;
    }
    function applyButtonTokens(parts, runLabel) {
        if (!parts)
            return parts;
        if (typeof parts === "string") {
            return replaceButtonTokens(parts, runLabel);
        }
        if (Array.isArray(parts)) {
            return parts.map((part) => typeof part === "string" ? replaceButtonTokens(part, runLabel) : part);
        }
        return parts;
    }
    function formatRunLabel(start, end, withArrow, verb = "Run") {
        if (start === end) {
            return `${verb} line ${start}${withArrow ? " ▶" : ""}`;
        }
        return `${verb} lines ${start}-${end}${withArrow ? " ▶" : ""}`;
    }
    function runLabelForBoundary(boundary) {
        const nextStep = nextStepBoundary(boundary);
        const needsSolve = editableSet.has(nextStep) && !state.passes[nextStep];
        const verb = needsSolve ? "Solve" : "Run";
        const branchStep = branchStepInfo(boundary);
        const branchSolved = branchStep
            ? !!state.branchPasses[branchStep.startLine]
            : false;
        const branchInfo = branchInfoForBoundary(boundary) ||
            branchInfoForBoundary(nextStep);
        if (branchInfo && !branchSolved) {
            const start = branchInfo.rangeStart + 1;
            const end = branchInfo.rangeEnd + 1;
            return formatRunLabel(start, end, true, "Branch from");
        }
        const range = runRangeForBoundary(boundary);
        if (range) {
            const start = range.start + 1;
            const end = range.end + 1;
            return formatRunLabel(start, end, true, verb);
        }
        const lineNumber = Math.max(1, Math.min(totalLines, boundary + 1));
        return `${verb} line ${lineNumber} ▶`;
    }
    function nextBoundaryForLine(current) {
        if (!Number.isFinite(current))
            return current + 1;
        if (current >= totalLines)
            return totalLines;
        const range = simulator.statementRangeForLine(statementMap, current);
        const rangeStart = typeof range?.startLine === "number" ? range.startLine : current;
        const rangeEnd = typeof range?.endLine === "number" ? range.endLine : current;
        const groupRange = groupForLine(current);
        const branchStep = !!branchStepInfo(current);
        const headerIndex = headerIndexForLine(rangeStart);
        if (headerIndex == null) {
            if (groupRange &&
                Number.isFinite(groupRange.endLine) &&
                groupRange.endLine > current) {
                return Math.min(totalLines, groupRange.endLine + 1);
            }
            if (Number.isFinite(rangeEnd) && rangeEnd > current)
                return Math.min(totalLines, rangeEnd + 1);
            return Math.min(totalLines, current + 1);
        }
        const block = ifBlocks.map.get(headerIndex);
        if (!block) {
            if (groupRange &&
                Number.isFinite(groupRange.endLine) &&
                groupRange.endLine > current) {
                return Math.min(totalLines, groupRange.endLine + 1);
            }
            if (Number.isFinite(rangeEnd) && rangeEnd > current)
                return Math.min(totalLines, rangeEnd + 1);
            return Math.min(totalLines, current + 1);
        }
        const currentState = stateBeforePart(headerIndex);
        const condition = simulator.evaluateCondition(block.expr, currentState);
        if ("error" in condition || condition.value) {
            if (branchStep) {
                const truePart = parts[block.trueTarget];
                const trueLine = Number.isFinite(truePart?.startLine)
                    ? truePart.startLine
                    : block.headerEndLine;
                if (Number.isFinite(trueLine))
                    return Math.min(totalLines, Math.max(0, trueLine));
            }
            if (groupRange &&
                Number.isFinite(groupRange.endLine) &&
                groupRange.endLine > current) {
                return Math.min(totalLines, groupRange.endLine + 1);
            }
            if (Number.isFinite(rangeEnd) && rangeEnd > current)
                return Math.min(totalLines, rangeEnd + 1);
            return Math.min(totalLines, current + 1);
        }
        const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
            ? parts[block.closeIndex].endLine
            : block.headerEndLine;
        if (!Number.isFinite(closeLine))
            return Math.min(totalLines, current + 1);
        return Math.min(totalLines, closeLine + 1);
    }
    function prevBoundaryForLine(current) {
        if (!Number.isFinite(current))
            return current - 1;
        if (current <= 0)
            return current - 1;
        let boundary = 0;
        let prev = 0;
        let guard = 0;
        while (boundary < current && guard < totalLines + 5) {
            prev = boundary;
            const next = nextBoundaryForLine(boundary);
            boundary = next === boundary ? boundary + 1 : next;
            guard += 1;
        }
        if (boundary === current)
            return prev;
        return current - 1;
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
        const baseline = state.baseline[boundary];
        if (Array.isArray(baseline))
            return baseline;
        return defaultsForBoundary(boundary);
    }
    function basicHintForBoxes(boxes, boundary) {
        const actual = Array.isArray(boxes) ? boxes : [];
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
            const baselineCount = Array.isArray(baselineAtBoundary)
                ? baselineAtBoundary.length
                : 0;
            const expectedNew = Math.max(0, expectedCount - baselineCount);
            const actualNew = Math.max(0, actualCount - baselineCount);
            const extraCount = Math.max(0, actualNew - expectedNew);
            const groupRange = groupRangeEndingAt(boundary);
            const statementRange = statementRangeEndingAt(statementMap, boundary);
            let label = `Line ${boundary + 1}`;
            if (groupRange && Number.isFinite(groupRange.startLine)) {
                const start = groupRange.startLine + 1;
                const end = groupRange.endLine + 1;
                label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
            }
            else if (statementRange && isMultiLineStatement(statementRange)) {
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
                const expVal = String(exp.value ?? "").trim();
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
    function getInstructionParts(boundary) {
        if (boundary === 0 && hasInitialInstructionsContent) {
            return initialInstructions || "";
        }
        const entry = instructionMap.get(boundary) ?? null;
        return entry ? String(entry) : null;
    }
    function getHintParts(ctx) {
        const entry = hintMap.get(state.boundary) ?? null;
        return resolveParts(entry, ctx);
    }
    function renderCodePaneForBoundary() {
        if (!codeEl)
            return;
        const key = state.boundary;
        const branchStep = branchStepInfo(key);
        const branchStepEditable = !!branchStep?.editable && !state.branchPasses[branchStep.startLine];
        let progress = editableSet.has(key) &&
            !state.passes[key] &&
            !branchStepEditable;
        let progressRange;
        let progressIndex;
        let doneBoundary;
        const branchInfo = branchInfoForBoundary(key);
        const branchSelectable = !!branchInfo && !!branchStep?.editable && branchSelectionActive;
        activeBranchTargets = branchSelectable ? allBoundaryTargets : null;
        activeBranchExpected =
            branchSelectable && branchInfo ? branchInfo.expected : null;
        if (progress) {
            const range = executedRangeForBoundary(key);
            if (range) {
                progressRange = [range.start, range.end];
                progressIndex = range.end;
                doneBoundary = range.start;
            }
        }
        else if (branchStepEditable &&
            branchSelectionActive &&
            branchSelectionBoundary === key) {
            const branchInfo = branchInfoForBoundary(key);
            if (branchInfo) {
                progress = true;
                progressRange = [branchInfo.rangeStart, branchInfo.rangeEnd];
                progressIndex = undefined;
            }
        }
        let strikeRanges = [];
        const prevBoundary = prevBoundaryForLine(key);
        if (prevBoundary >= 0 && prevBoundary < key) {
            const headerIndex = headerIndexForLine(prevBoundary);
            if (headerIndex != null) {
                const block = ifBlocks.map.get(headerIndex);
                if (block) {
                    const currentState = stateBeforePart(headerIndex);
                    const condition = simulator.evaluateCondition(block.expr, currentState);
                    if (!("error" in condition) && !condition.value) {
                        const headerStep = stepByStartLine.get(block.headerStartLine);
                        const allowStrike = !headerStep?.editable ||
                            !headerStep.ifHeaderOnly ||
                            !!state.branchPasses[headerStep.startLine];
                        if (allowStrike) {
                            const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
                                ? parts[block.closeIndex].endLine
                                : block.headerEndLine;
                            const openLine = Number.isFinite(parts[block.openIndex]?.startLine)
                                ? parts[block.openIndex].startLine
                                : block.headerEndLine;
                            if (Number.isFinite(openLine) && Number.isFinite(closeLine)) {
                                const range = [openLine, closeLine];
                                strikeRanges.push(range);
                                state.lastSkippedRanges.push(range);
                            }
                        }
                    }
                }
            }
        }
        if (state.lastSkippedRanges.length) {
            state.lastSkippedRanges = state.lastSkippedRanges.filter(([start]) => key > start);
        }
        if (state.lastSkippedRanges.length) {
            strikeRanges = strikeRanges.concat(state.lastSkippedRanges);
        }
        renderCodePane(codeEl, lineList, key, {
            progress,
            progressRange,
            progressIndex,
            doneBoundary,
            hideBoundary: branchSelectable,
            selectableBoundaries: branchSelectable ? allBoundaries : undefined,
            selectedBoundary: branchSelectable ? branchSelectionTarget : null,
            suppressProgressMid: branchStepEditable && branchSelectionActive,
            boundaryTargets: branchSelectable,
            strikeRanges,
        });
    }
    function renderStage() {
        if (!stageEl)
            return;
        stageEl.innerHTML = "";
        const key = state.boundary;
        const editStep = stepEndingAtBoundary(key);
        const editable = !!editStep?.editable &&
            !editStep.ifHeaderOnly &&
            !state.passes[key];
        const defaults = defaultsForBoundary(key);
        state.workspaceEl = null;
        if (key <= 0) {
            refreshOtherNames();
            return;
        }
        if (!editable) {
            const expected = decorateState(getExpectedState(state.boundary));
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
                expected.forEach((b) => {
                    const node = vbox({
                        address: b.address ?? "—",
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
            stageEl.appendChild(grid);
            refreshOtherNames();
            return;
        }
        const wrap = restoreWorkspace(state.ws[key], defaults, {
            editable,
            deletable: allowVariableDeletion,
            allowNameEdit: null,
            allowTypeEdit: null,
        });
        stageEl.appendChild(wrap);
        state.workspaceEl = wrap;
        attachResetWatcher(wrap, key);
        ensureBaseline(key, defaults);
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
    function updateInstructions() {
        const key = state.boundary;
        const runLabel = runLabelForBoundary(state.boundary);
        const scrollUp = scrollUpByBoundary.get(key) !== false && !!instructionsEl;
        let specKey = null;
        if (key === total && state.passes[key]) {
            setPartsContent(instructionsEl, "Program solved!");
            specKey = "__solved__";
            if (scrollUp && specKey !== state.lastInstructionKey) {
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
            state.lastInstructionKey = specKey;
            return;
        }
        let parts = getInstructionParts(key);
        if (Array.isArray(parts) && parts.length === 0)
            parts = null;
        if (!parts) {
            const step = stepByStartLine.get(key);
            if (step?.instructions) {
                if (step.ifHeaderOnly) {
                    if (branchSelectionActive && branchSelectionBoundary === key) {
                        parts = String(step.instructions);
                    }
                }
                else {
                    const nextBoundary = nextBoundaryForLine(key);
                    if (nextBoundary > step.boundary) {
                        parts = String(step.instructions);
                    }
                }
            }
        }
        if (key === 0) {
            if (parts) {
                parts = parts;
            }
            else {
                parts = null;
            }
            specKey = "__initial__";
        }
        else {
            const spec = instructionMap.get(key) ?? null;
            specKey = spec ? String(spec) : null;
        }
        if (!parts) {
            parts = null;
        }
        parts = applyButtonTokens(parts, runLabel);
        setPartsContent(instructionsEl, parts);
        if (scrollUp && specKey && specKey !== state.lastInstructionKey) {
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
        state.lastInstructionKey = specKey;
        state.lastBoundary = key;
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
        return [...ws.querySelectorAll(".vbox")]
            .map((v) => readBoxState(v))
            .filter(Boolean);
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
            branchSelectionActive = false;
            branchSelectionTarget = null;
            branchSelectionBoundary = null;
        }
        renderCodePaneForBoundary();
        renderStage();
        hideHint();
        const key = state.boundary;
        const branchStep = branchStepInfo(key);
        const branchSolved = branchStep
            ? !!state.branchPasses[branchStep.startLine]
            : false;
        const editStep = stepEndingAtBoundary(key);
        const editPass = !!editStep?.editable && !!state.passes[key];
        const branchSelectionHere = branchSelectionActive && branchSelectionBoundary === key;
        const hasSolvedEditable = !!editStep?.editable && editPass;
        const normalEditable = !!editStep?.editable && !editPass && !editStep?.ifHeaderOnly;
        const branchInfo = branchInfoForBoundary(key);
        const branchEditable = branchSelectionActive &&
            branchSelectionBoundary === key &&
            !!branchInfo &&
            !!branchStep?.editable &&
            !branchSolved;
        if (statusEl) {
            if (hasSolvedEditable && !branchSelectionHere) {
                setStatus("correct", "ok");
            }
            else if (normalEditable) {
                setStatus("", "muted");
            }
            else if (branchStep) {
                setStatus(branchSolved ? "correct" : "", branchSolved ? "ok" : "muted");
            }
            else if (editableSet.has(key)) {
                setStatus("correct", "ok");
            }
            else {
                setStatus("", "muted");
            }
        }
        if (state.lastBranchCorrectBoundary !== null &&
            state.lastBranchCorrectBoundary === key &&
            statusEl) {
            setStatus("correct", "ok");
            state.lastBranchCorrectBoundary = null;
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
        updateInstructions();
        if (normalEditable && resetBtn)
            updateResetVisibility(key);
    }
    function save() {
        const key = state.boundary;
        if (!editableSet.has(key))
            return;
        const snapshot = serializeWorkspace(getWorkspaceEl());
        if (Array.isArray(snapshot)) {
            state.ws[key] = snapshot;
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
            updateResetVisibility(state.boundary);
            refreshOtherNames();
        });
    }
    if (resetBtn) {
        resetBtn.addEventListener("click", () => {
            const key = state.boundary;
            if (!editableSet.has(key))
                return;
            state.ws[key] = null;
            state.passes[key] = false;
            state.baseline[key] = null;
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
                branchSelectionActive = false;
                branchSelectionBoundary = null;
                branchSelectionTarget = null;
            }
            const branchInfo = branchInfoForBoundary(key);
            const branchStep = branchStepInfo(key);
            const branchSolved = branchStep
                ? !!state.branchPasses[branchStep.startLine]
                : false;
            const branchEditable = branchSelectionActive &&
                branchSelectionBoundary === key &&
                !!branchInfo &&
                !!branchStep?.editable &&
                !branchSolved;
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
                if (branchStep) {
                    state.branchPasses[branchStep.startLine] = true;
                    state.passes[key] = true;
                }
                setStatus("correct", "ok");
                flashStatus(statusEl);
                branchSelectionActive = false;
                branchSelectionBoundary = null;
                branchSelectionTarget = null;
                state.lastBranchCorrectBoundary = Math.max(0, Math.min(totalLines, branchInfo.expected));
                state.boundary = Math.max(0, Math.min(totalLines, branchInfo.expected));
                render();
                pager.update();
                pager.pulseNext();
                return;
            }
            if (!editableSet.has(key))
                return;
            const boxes = readWorkspaceBoxes();
            const result = evaluateWorkspace(boxes);
            const ok = result.ok;
            setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
            flashStatus(statusEl);
            if (!ok)
                return;
            state.passes[key] = true;
            state.ws[key] = boxes;
            const ws = getWorkspaceEl();
            if (ws) {
                ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
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
            updateInstructions();
            renderCodePaneForBoundary();
            renderStage();
            pager.update();
        });
    }
    if (hintBtn) {
        hintBtn.addEventListener("click", () => {
            const boxes = readWorkspaceBoxes();
            const result = evaluateWorkspace(boxes);
            const ctx = partsContext({ boxes });
            const runLabel = runLabelForBoundary(state.boundary);
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
    const pager = createStepper({
        root: codeRoot || codeEl?.closest(".panel") || document.body,
        lines: totalLines,
        nextPage: next || null,
        getBoundary: () => state.boundary,
        setBoundary: (val) => {
            state.boundary = val;
        },
        onBeforeChange: save,
        onAfterChange: render,
        isStepLocked: (boundary) => {
            const editStep = stepEndingAtBoundary(boundary);
            if (editStep?.ifHeaderOnly && editStep.editable) {
                const solved = !!state.branchPasses[editStep.startLine];
                if (solved)
                    return false;
                return branchSelectionActive && branchSelectionBoundary === boundary;
            }
            if (editStep?.editable) {
                return !state.passes[boundary];
            }
            return false;
        },
        getStepBadge: () => {
            const nextBoundary = nextStepBoundary(state.boundary);
            if (!editableSet.has(nextBoundary))
                return "";
            return state.passes[nextBoundary] ? "check" : "note";
        },
        getNextLabel: (boundary) => {
            const atEnd = boundary >= totalLines;
            if (atEnd)
                return endLabel;
            if (branchSelectionActive &&
                branchSelectionBoundary === boundary) {
                return "???";
            }
            const nextBoundary = nextStepBoundary(boundary);
            const needsSolve = editableSet.has(nextBoundary) && !state.passes[nextBoundary];
            const verb = needsSolve ? "Solve" : "Run";
            const branchStep = branchStepInfo(nextBoundary);
            const branchSolved = branchStep
                ? !!state.branchPasses[branchStep.startLine]
                : false;
            const branchInfo = branchInfoForBoundary(nextBoundary);
            if (branchInfo && !branchSolved) {
                const start = branchInfo.rangeStart + 1;
                const end = branchInfo.rangeEnd + 1;
                return formatRunLabel(start, end, false, "Branch from");
            }
            const range = runRangeForBoundary(boundary);
            if (range) {
                const start = range.start + 1;
                const end = range.end + 1;
                return formatRunLabel(start, end, false, verb);
            }
            const lineNumber = Math.max(1, Math.min(totalLines, boundary + 1));
            return `${verb} line ${lineNumber}`;
        },
        getNextBoundary: (current) => {
            const stepNext = nextStepBoundary(current);
            const nextStep = stepEndingAtBoundary(stepNext);
            if (nextStep?.ifHeaderOnly && nextStep.editable) {
                const solved = !!state.branchPasses[nextStep.startLine];
                if (!solved) {
                    const branchInfo = branchInfoForBoundary(stepNext);
                    if (branchInfo && !branchSelectionActive) {
                        branchSelectionActive = true;
                        branchSelectionBoundary = stepNext;
                        branchSelectionTarget = null;
                    }
                    return stepNext;
                }
            }
            const next = nextBoundaryForLine(current);
            const editStep = stepEndingAtBoundary(current);
            if (editStep?.ifHeaderOnly && editStep.editable) {
                const solved = !!state.branchPasses[editStep.startLine];
                if (!solved) {
                    const branchInfo = branchInfoForBoundary(current);
                    if (branchInfo && !branchSelectionActive) {
                        branchSelectionActive = true;
                        branchSelectionBoundary = current;
                        branchSelectionTarget = null;
                    }
                    return current;
                }
            }
            return next;
        },
        getPrevBoundary: (current) => {
            if (branchSelectionActive &&
                branchSelectionBoundary === current) {
                branchSelectionActive = false;
                branchSelectionBoundary = null;
                branchSelectionTarget = null;
                return prevBoundaryForLine(current);
            }
            return prevBoundaryForLine(current);
        },
        allowSameBoundary: true,
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

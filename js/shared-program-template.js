"use strict";
/// <reference path="./shared-core.ts" />
(function (global) {
    const MB = (global.MB || {});
    const { createSimpleSimulator, randAddr, typeInfo, getPointerDepth, normalizeZeroDisplay, cloneBoxes, readBoxState, serializeWorkspace, restoreWorkspace, vbox, makeAnswerBox, applyOtherNames, renderCodePane, renderParts, setPartsContent, createStepper, flashStatus, disableBoxEditing, removeBoxDeleteButtons, isMobileViewport, isStepperTopVisible, ensureBaseLayout, resolveActiveNavItem, } = MB;
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
        const { steps = [], initialInstructions, next = null, workspace = {} } = config;
        const showOtherNames = !!(workspace && workspace.showOtherNames);
        const failConfig = (message) => {
            alert(message);
            throw new Error(message);
        };
        const simulator = createSimpleSimulator({
            allowVarAssign: true,
            allowDeclAssign: true,
            allowDeclAssignVar: true,
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
        const lineList = [];
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
            for (const part of parts) {
                if (part?.tokens?.length && !part.hasSemicolon) {
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
            });
        });
        if (stepInfos.some((step) => step.endLine < step.startLine)) {
            failConfig("Program steps must each contain at least one line.");
        }
        const total = lineList.length;
        const instructionMap = new Map();
        const hintMap = new Map();
        const editableByBoundary = new Map();
        stepInfos.forEach((step) => {
            if (step.instructions !== undefined && step.instructions !== null) {
                instructionMap.set(step.boundary, step.instructions);
            }
            if (step.hints) {
                hintMap.set(step.boundary, step.hints);
            }
            if (step.editable) {
                editableByBoundary.set(step.boundary, step.startLine + 1);
            }
        });
        const state = {
            boundary: 0,
            passes: {},
            ws: {},
            baseline: {},
            allocBase: null,
            workspaceEl: null,
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
        function buildStatementMap(linesForMap) {
            const text = linesForMap.join("\n");
            const tokens = simulator.tokenizeProgram(text);
            const parts = simulator.splitStatements(tokens);
            const byLine = new Array(linesForMap.length).fill(null);
            parts.forEach((part) => {
                if (!Number.isFinite(part.startLine) || !Number.isFinite(part.endLine))
                    return;
                const range = {
                    startLine: part.startLine,
                    endLine: part.endLine,
                    hasSemicolon: !!part.hasSemicolon,
                };
                for (let i = range.startLine; i <= range.endLine; i++) {
                    if (i >= 0 && i < byLine.length && !byLine[i])
                        byLine[i] = range;
                }
            });
            return { parts, byLine };
        }
        function statementRangeForLine(statementMap, lineIndex) {
            if (!statementMap || !Array.isArray(statementMap.byLine))
                return null;
            if (!Number.isFinite(lineIndex))
                return null;
            if (lineIndex < 0 || lineIndex >= statementMap.byLine.length)
                return null;
            return statementMap.byLine[lineIndex];
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
        const statementMap = buildStatementMap(lineList);
        const groupRanges = stepInfos.map((step) => ({
            startLine: step.startLine,
            endLine: step.endLine,
        }));
        const editableSet = new Set(editableByBoundary.keys());
        const hasInitialInstructionsContent = typeof initialInstructions === "string" && initialInstructions.length > 0;
        stepInfos.forEach((step) => {
            const text = lineList.slice(0, step.boundary).join("\n");
            const result = simulator.applyProgram(text);
            if (!Array.isArray(result)) {
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
        function groupRangeEndingAt(boundary) {
            const endLine = boundary - 1;
            if (!Number.isFinite(endLine))
                return null;
            return groupRanges.find((group) => group.endLine === endLine) || null;
        }
        function getStatementContext(boundary) {
            const currentRange = statementRangeForLine(statementMap, boundary);
            const prevRange = statementRangeForLine(statementMap, boundary - 1);
            const currentIsMulti = isMultiLineStatement(currentRange);
            const currentStart = currentRange?.startLine;
            const currentEnd = currentRange?.endLine;
            const midStatement = currentIsMulti &&
                typeof currentStart === "number" &&
                typeof currentEnd === "number" &&
                Number.isFinite(currentStart) &&
                Number.isFinite(currentEnd) &&
                boundary > currentStart &&
                boundary <= currentEnd;
            const atStatementStart = currentIsMulti &&
                typeof currentStart === "number" &&
                Number.isFinite(currentStart) &&
                boundary === currentStart;
            return { currentRange, prevRange, midStatement, atStatementStart };
        }
        function getExpectedState(boundary) {
            const safeBoundary = Math.max(0, Math.min(total, boundary));
            const text = lineList.slice(0, safeBoundary).join("\n");
            const alloc = allocFactory();
            const result = simulator.applyProgram(text, { alloc });
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
                const depth = getPointerDepth(actType);
                const isPtr = depth != null && Number.isFinite(depth) && depth > 0;
                const actVal = String(act.value ?? "").trim();
                const expVal = String(exp.value ?? "").trim();
                if (isPtr) {
                    if (actVal !== expVal)
                        return false;
                }
                else {
                    if (actVal === "" && expVal === "")
                        continue;
                    if (normalizeZeroDisplay(actVal) !== normalizeZeroDisplay(expVal))
                        return false;
                }
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
        function topNoticeParts() {
            return "This one is long, so I've placed the $backButton and $runLineButton buttons on the top as well as the bottom.";
        }
        function withTopNotice(parts) {
            if (isMobileViewport())
                return parts;
            if (!isStepperTopVisible(codeEl))
                return parts;
            const notice = topNoticeParts();
            if (typeof parts === "string") {
                return `${notice}\n${parts}`;
            }
            if (Array.isArray(parts)) {
                return [notice, "\n", ...parts];
            }
            return [notice, "\n", parts];
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
                return parts.map((part) => typeof part === "string"
                    ? replaceButtonTokens(part, runLabel)
                    : part);
            }
            return parts;
        }
        function formatRunLabel(start, end, withArrow) {
            if (start === end) {
                return `Run line ${start}${withArrow ? " ▶" : ""}`;
            }
            return `Run lines ${start}-${end}${withArrow ? " ▶" : ""}`;
        }
        function runLabelForBoundary(boundary) {
            const group = groupForLine(boundary);
            if (group) {
                const start = group.startLine + 1;
                const end = group.endLine + 1;
                return formatRunLabel(start, end, true);
            }
            const range = statementRangeForLine(statementMap, boundary);
            if (range && isMultiLineStatement(range)) {
                const start = range.startLine + 1;
                const end = range.endLine + 1;
                return formatRunLabel(start, end, true);
            }
            return `Run line ${boundary + 1} ▶`;
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
                const baseline = baselineForBoundary(boundary);
                const baselineNames = new Set(baseline.map(nameOf).filter(Boolean));
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
                const baseline = baselineForBoundary(boundary);
                const baselineCount = Array.isArray(baseline) ? baseline.length : 0;
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
            const baseline = baselineForBoundary(boundary);
            const baselineByName = new Map();
            baseline.forEach((box) => {
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
                const depth = getPointerDepth(expType);
                const isPtr = depth != null && Number.isFinite(depth) && depth > 0;
                let mismatch = false;
                const actVal = String(act.value ?? "").trim();
                const expVal = String(exp.value ?? "").trim();
                if (isPtr) {
                    mismatch = actVal !== expVal;
                }
                else if (!(actVal === "" && expVal === "")) {
                    mismatch = normalizeZeroDisplay(actVal) !== normalizeZeroDisplay(expVal);
                }
                if (mismatch) {
                    const expVal = String(exp.value ?? "").trim();
                    const label = expVal === "" ? "empty" : `$v{${normalizeZeroDisplay(expVal)}}`;
                    const baselineBox = baselineByName.get(name);
                    let shouldRemain = false;
                    if (baselineBox) {
                        const baseVal = String(baselineBox.value ?? "").trim();
                        if (isPtr) {
                            shouldRemain = baseVal === expVal;
                        }
                        else if (baseVal === "" && expVal === "") {
                            shouldRemain = true;
                        }
                        else {
                            shouldRemain =
                                normalizeZeroDisplay(baseVal) === normalizeZeroDisplay(expVal);
                        }
                    }
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
            const topic = basicHintForBoxes(resolvedBoxes, state.boundary);
            return {
                boxes: resolvedBoxes,
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
            const progress = editableSet.has(state.boundary) && !state.passes[state.boundary];
            let progressRange;
            let progressIndex;
            let doneBoundary;
            if (progress) {
                const groupRange = groupRangeEndingAt(state.boundary);
                const range = groupRange || statementRangeEndingAt(statementMap, state.boundary);
                if (range && isMultiLineStatement(range)) {
                    progressRange = [range.startLine, range.endLine];
                    progressIndex = range.startLine;
                    doneBoundary = range.startLine;
                }
            }
            renderCodePane(codeEl, lineList, state.boundary, {
                progress,
                progressRange,
                progressIndex,
                doneBoundary,
            });
        }
        function renderStage() {
            if (!stageEl)
                return;
            stageEl.innerHTML = "";
            const editable = editableSet.has(state.boundary) && !state.passes[state.boundary];
            const defaults = defaultsForBoundary(state.boundary);
            state.workspaceEl = null;
            if (state.boundary <= 0) {
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
                            node
                                .querySelector(".value")
                                ?.classList.add("placeholder", "muted");
                        grid.appendChild(node);
                    });
                }
                stageEl.appendChild(grid);
                refreshOtherNames();
                return;
            }
            const wrap = restoreWorkspace(state.ws[state.boundary], defaults, {
                editable,
                deletable: false,
                allowNameEdit: null,
                allowTypeEdit: null,
            });
            stageEl.appendChild(wrap);
            state.workspaceEl = wrap;
            attachResetWatcher(wrap, state.boundary);
            ensureBaseline(state.boundary, defaults);
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
            const runLabel = runLabelForBoundary(state.boundary);
            if (state.boundary === total && state.passes[state.boundary]) {
                setPartsContent(instructionsEl, "Program solved!");
                return;
            }
            let parts = getInstructionParts(state.boundary);
            if (Array.isArray(parts) && parts.length === 0)
                parts = null;
            if (state.boundary === 0) {
                if (parts) {
                    parts = withTopNotice(parts);
                }
                else {
                    parts = topNoticeParts();
                }
            }
            if (!parts) {
                parts = null;
            }
            parts = applyButtonTokens(parts, runLabel);
            setPartsContent(instructionsEl, parts);
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
            renderCodePaneForBoundary();
            renderStage();
            hideHint();
            const editable = editableSet.has(state.boundary) && !state.passes[state.boundary];
            if (editable && statusEl) {
                setStatus("", "muted");
            }
            else if (editableSet.has(state.boundary) && statusEl) {
                setStatus("correct", "ok");
            }
            else if (statusEl) {
                setStatus("", "muted");
            }
            if (checkBtn)
                checkBtn.classList.toggle("hidden", !editable);
            if (hintBtn)
                hintBtn.classList.toggle("hidden", !editable);
            if (addBtn)
                addBtn.classList.toggle("hidden", !editable || !workspace.allowVariableCreation);
            if (resetBtn)
                resetBtn.classList.add("hidden");
            updateInstructions();
            if (editable && resetBtn)
                updateResetVisibility(state.boundary);
        }
        function save() {
            if (!editableSet.has(state.boundary))
                return;
            const snapshot = serializeWorkspace(getWorkspaceEl());
            if (Array.isArray(snapshot)) {
                state.ws[state.boundary] = snapshot;
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
                if (!editableSet.has(state.boundary))
                    return;
                state.ws[state.boundary] = null;
                state.passes[state.boundary] = false;
                state.baseline[state.boundary] = null;
                render();
                pager.update();
            });
        }
        if (checkBtn) {
            checkBtn.addEventListener("click", () => {
                hideHint();
                if (!editableSet.has(state.boundary))
                    return;
                const boxes = readWorkspaceBoxes();
                const result = evaluateWorkspace(boxes);
                const ok = result.ok;
                setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
                flashStatus(statusEl);
                if (!ok)
                    return;
                state.passes[state.boundary] = true;
                state.ws[state.boundary] = boxes;
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
            lines: lineList,
            nextPage: next || null,
            getBoundary: () => state.boundary,
            setBoundary: (val) => {
                state.boundary = val;
            },
            onBeforeChange: save,
            onAfterChange: render,
            isStepLocked: (boundary) => editableSet.has(boundary) && !state.passes[boundary],
            getStepBadge: (step) => {
                if (!editableSet.has(step))
                    return "";
                return state.passes[step] ? "check" : "note";
            },
            getNextLabel: (boundary) => {
                const atEnd = boundary >= total;
                if (atEnd)
                    return "Next Program";
                const group = groupForLine(boundary);
                if (group) {
                    const start = group.startLine + 1;
                    const end = group.endLine + 1;
                    return formatRunLabel(start, end, false);
                }
                const range = statementRangeForLine(statementMap, boundary);
                if (range && isMultiLineStatement(range)) {
                    const start = range.startLine + 1;
                    const end = range.endLine + 1;
                    return formatRunLabel(start, end, false);
                }
                return `Run line ${boundary + 1}`;
            },
            getNextBoundary: (current) => {
                const group = groupForLine(current);
                if (group) {
                    return group.endLine + 1;
                }
                const ctx = getStatementContext(current);
                if (ctx?.currentRange && isMultiLineStatement(ctx.currentRange)) {
                    if (ctx.midStatement || ctx.atStatementStart) {
                        return ctx.currentRange.endLine + 1;
                    }
                }
                return current + 1;
            },
            getPrevBoundary: (current) => {
                const prevGroup = groupForLine(current - 1);
                if (prevGroup) {
                    return prevGroup.startLine;
                }
                const ctx = getStatementContext(current);
                if (ctx?.prevRange && isMultiLineStatement(ctx.prevRange)) {
                    return ctx.prevRange.startLine;
                }
                return current - 1;
            },
        });
        render();
        pager.update();
        return { state, pager };
    }
    MB.createProgramTemplate = createProgramTemplate;
    global.MB = MB;
})(window);

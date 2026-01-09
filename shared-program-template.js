(function (global) {
  const MB = global.MB || {};
  const {
    createSimpleSimulator,
    randAddr,
    typeInfo,
    getPointerDepth,
    normalizeZeroDisplay,
    cloneBoxes,
    disableAutoText,
    readBoxState,
    serializeWorkspace,
    restoreWorkspace,
    vbox,
    makeAnswerBox,
    applyOtherNames,
    renderCodePane,
    renderParts,
    setPartsContent,
    createStepper,
    flashStatus,
    disableBoxEditing,
    removeBoxDeleteButtons,
    isMobileViewport,
    isStepperTopVisible,
    ensureBaseLayout,
    resolveActiveNavItem,
    stepperInstructionParts,
  } = MB;

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

  function ensureProgramLayout({
    pageTitle,
    browserTitle,
    navItems,
    activeHref,
  } = {}) {
    const activeItem = resolveActiveNavItem(navItems, activeHref);
    const resolvedTitle = pageTitle || activeItem?.label || "";
    const nextBrowserTitle =
      browserTitle || (resolvedTitle ? `C Boxes - ${resolvedTitle}` : "");
    if (nextBrowserTitle) document.title = nextBrowserTitle;
    const existing = document.querySelector('[data-role="program-code"]');
    if (existing) return collectProgramElements();

    const { main } = ensureBaseLayout({ navItems, activeHref });
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

  function createProgramTemplate(config = {}) {
    const {
      lines = [],
      editableSteps = [],
      hints = {},
      instructions = null,
      next = null,
      workspace = {},
      runGroups = null,
      stepperFallback = false,
      pageTitle = null,
      browserTitle = null,
      navItems = null,
      activeHref = null,
    } = config;
    const showOtherNames = !!(workspace && workspace.showOtherNames);

    const {
      instructionsEl,
      codeEl,
      codeRoot,
      stageEl,
      statusEl,
      hintPanel,
      hintBtn,
      checkBtn,
      addBtn,
      resetBtn,
    } = ensureProgramLayout({
      pageTitle,
      browserTitle,
      navItems,
      activeHref,
    });

    const lineList = Array.isArray(lines) ? lines.map((l) => String(l)) : [];
    const total = lineList.length;
    const editableStarts = (editableSteps || [])
      .map((step) => Number(step))
      .filter((step) => Number.isFinite(step));

    const state = {
      boundary: 0,
      passes: {},
      ws: {},
      baseline: {},
      allocBase: null,
      workspaceEl: null,
    };
    const simulator = createSimpleSimulator({
      allowVarAssign: true,
      allowDeclAssign: true,
      allowDeclAssignVar: true,
      requireSourceValue: true,
      allowPointers: true,
    });

    function allocFactory() {
      if (state.allocBase == null) state.allocBase = randAddr("int");
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
          if (i >= 0 && i < byLine.length && !byLine[i]) byLine[i] = range;
        }
      });
      return { parts, byLine };
    }

    function statementRangeForLine(statementMap, lineIndex) {
      if (!statementMap || !Array.isArray(statementMap.byLine)) return null;
      if (!Number.isFinite(lineIndex)) return null;
      if (lineIndex < 0 || lineIndex >= statementMap.byLine.length) return null;
      return statementMap.byLine[lineIndex];
    }

    function statementRangeEndingAt(statementMap, boundary) {
      const endLine = boundary - 1;
      if (!Number.isFinite(endLine)) return null;
      return statementMap.parts.find(
        (part) => part.endLine === endLine && part.hasSemicolon,
      );
    }

    function isMultiLineStatement(range) {
      return (
        range &&
        Number.isFinite(range.startLine) &&
        Number.isFinite(range.endLine) &&
        range.endLine > range.startLine
      );
    }

    const statementMap = buildStatementMap(lineList);
    const groupRanges = normalizeRunGroups(runGroups, lineList.length);
    const editableByBoundary = buildEditableBoundaries(
      editableStarts,
      statementMap,
      groupRanges,
    );
    const editableSet = new Set(editableByBoundary.keys());

    editableSet.forEach((step) => {
      if (Number.isFinite(step)) state.passes[step] = false;
    });

    function normalizeRunGroups(groups, totalLines) {
      if (!Array.isArray(groups)) return [];
      const normalized = [];
      groups.forEach((group) => {
        if (!group) return;
        const rawStart =
          group.start ?? group.from ?? group.begin ?? group[0] ?? group.line;
        const rawEnd =
          group.end ?? group.to ?? group.finish ?? group[1] ?? rawStart;
        let start = Number(rawStart);
        let end = Number(rawEnd);
        if (!Number.isFinite(start) || !Number.isFinite(end)) return;
        start = Math.max(1, Math.min(totalLines, Math.round(start)));
        end = Math.max(1, Math.min(totalLines, Math.round(end)));
        if (end < start) [start, end] = [end, start];
        normalized.push({ startLine: start - 1, endLine: end - 1 });
      });
      normalized.sort((a, b) => a.startLine - b.startLine);
      return normalized;
    }

    function groupForLine(lineIndex) {
      if (!Number.isFinite(lineIndex)) return null;
      return groupRanges.find(
        (group) => lineIndex >= group.startLine && lineIndex <= group.endLine,
      );
    }

    function groupRangeEndingAt(boundary) {
      const endLine = boundary - 1;
      if (!Number.isFinite(endLine)) return null;
      return groupRanges.find((group) => group.endLine === endLine);
    }

    function buildEditableBoundaries(starts, stmtMap, groups) {
      const result = new Map();
      const totalLines = lineList.length;
      (starts || []).forEach((start) => {
        let startLine = Math.round(start) - 1;
        if (!Number.isFinite(startLine)) return;
        if (startLine < 0 || startLine >= totalLines) return;
        let endLine = startLine;
        const group = groups.find((g) => g.startLine === startLine);
        if (group) {
          endLine = group.endLine;
        } else {
          const range = statementRangeForLine(stmtMap, startLine);
          if (range && Number.isFinite(range.endLine)) {
            endLine = range.endLine;
          }
        }
        const boundary = endLine + 1;
        if (!result.has(boundary)) result.set(boundary, startLine + 1);
      });
      return result;
    }

    function stepStartForBoundary(boundary) {
      const mapped = editableByBoundary.get(boundary);
      if (mapped) return mapped;
      const group = groupRangeEndingAt(boundary);
      if (group) return group.startLine + 1;
      const range = statementRangeEndingAt(statementMap, boundary);
      if (range && Number.isFinite(range.startLine)) {
        return range.startLine + 1;
      }
      return boundary;
    }

    function getStatementContext(boundary) {
      const currentRange = statementRangeForLine(statementMap, boundary);
      const prevRange = statementRangeForLine(statementMap, boundary - 1);
      const currentIsMulti = isMultiLineStatement(currentRange);
      const midStatement =
        currentIsMulti &&
        boundary > currentRange.startLine &&
        boundary <= currentRange.endLine;
      const atStatementStart =
        currentIsMulti && boundary === currentRange.startLine;
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
      if (!Array.isArray(list)) return [];
      return list
        .map((b) => ({
          name: String(b.name || "").trim(),
          type: String(b.type || "").trim(),
          value: normalizeZeroDisplay(String(b.value ?? "").trim()),
          address: String(b.address ?? "").trim(),
        }))
        .sort((a, b) => {
          if (a.name === b.name) return a.address.localeCompare(b.address);
          return a.name.localeCompare(b.name);
        });
    }

    function statesEqual(a, b) {
      const na = normalizeState(a);
      const nb = normalizeState(b);
      if (na.length !== nb.length) return false;
      for (let i = 0; i < na.length; i++) {
        const left = na[i];
        const right = nb[i];
        if (left.name !== right.name) return false;
        if (left.type !== right.type) return false;
        if (left.value !== right.value) return false;
        if (left.address !== right.address) return false;
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
      return (
        state.workspaceEl || stageEl?.querySelector?.('[data-role="workspace"]')
      );
    }

    function updateResetVisibility(boundary) {
      if (!resetBtn) return;
      if (!editableSet.has(boundary) || state.passes[boundary]) {
        resetBtn.classList.add("hidden");
        return;
      }
      const baseline = state.baseline[boundary];
      const current = serializeWorkspace(getWorkspaceEl()) || [];
      const changed =
        Array.isArray(baseline) && !statesEqual(current, baseline);
      resetBtn.classList.toggle("hidden", !changed);
    }

    function attachResetWatcher(wrap, boundary) {
      if (!wrap) return;
      const refresh = () => {
        updateResetVisibility(boundary);
      };
      wrap.addEventListener("input", refresh);
      wrap.addEventListener("click", (event) => {
        const target = event?.target;
        setTimeout(refresh, 0);
      });
      refresh();
    }

    function nextWorkspaceAddress(wrap, type = "int") {
      if (!wrap) return String(randAddr(type || "int"));
      const used = new Set();
      let maxAddr = null;
      wrap.querySelectorAll(".vbox").forEach((node) => {
        const box = readBoxState(node);
        const raw = box?.address ?? "";
        const addrNum = Number(raw);
        if (!Number.isFinite(addrNum)) return;
        const addrStr = String(addrNum);
        used.add(addrStr);
        if (maxAddr == null || addrNum > maxAddr) maxAddr = addrNum;
      });
      const { size, align } = typeInfo(type || "int");
      if (maxAddr == null) return String(randAddr(type || "int"));
      let next = maxAddr + (Number.isFinite(size) ? size : 4);
      if (Number.isFinite(align) && align > 1 && next % align !== 0) {
        next = Math.ceil(next / align) * align;
      }
      while (used.has(String(next))) {
        next += Number.isFinite(size) ? size : 4;
      }
      return String(next);
    }

    function pointerTargetName(box, boxes) {
      if (!box) return "";
      const depth = getPointerDepth(box.type);
      if (!Number.isFinite(depth) || depth < 1) return "";
      const raw = String(box.value ?? "").trim();
      if (raw === "") return "";
      const target = (boxes || []).find(
        (b) => String(b.address ?? "") === raw,
      );
      return target?.name || "";
    }

    function refreshOtherNames() {
      if (!showOtherNames) return;
      applyOtherNames(stageEl, { onToggle: refreshOtherNames });
    }

    function boxesEqual(actual, expected) {
      if (!Array.isArray(actual) || !Array.isArray(expected)) return false;
      const actualByName = new Map();
      for (const box of actual) {
        const name = String(box?.name || "").trim();
        if (!name || actualByName.has(name)) return false;
        actualByName.set(name, box);
      }
      const expectedByName = new Map();
      for (const box of expected) {
        const name = String(box?.name || "").trim();
        if (!name || expectedByName.has(name)) return false;
        expectedByName.set(name, box);
      }
      if (actualByName.size !== expectedByName.size) return false;
      for (const [name, exp] of expectedByName.entries()) {
        const act = actualByName.get(name);
        if (!act) return false;
        const actType = String(act.type || "").trim();
        const expType = String(exp.type || "").trim();
        if (actType !== expType) return false;
        const isPtr = getPointerDepth(actType) > 0;
        if (isPtr) {
          if (
            pointerTargetName(act, actual) !== pointerTargetName(exp, expected)
          ) {
            return false;
          }
        } else {
          const actVal = String(act.value ?? "").trim();
          const expVal = String(exp.value ?? "").trim();
          if (actVal === "" && expVal === "") continue;
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
      if (!statusEl) return;
      statusEl.textContent = text;
      statusEl.className = cls;
    }

    function makeToken(role, text) {
      return { kind: "tok", role, text: String(text ?? "") };
    }

    function topNoticeParts() {
      return "This one is long, so I've placed the $b{Back ◀} and $b{Run line 1 ▶} buttons on the top as well as the bottom.";
    }

    function withTopNotice(parts, ctx) {
      if (isMobileViewport()) return parts;
      if (!isStepperTopVisible(codeEl)) return parts;
      const notice = topNoticeParts(ctx);
      if (typeof parts === "string") {
        return `${notice}\n${parts}`;
      }
      if (Array.isArray(parts)) {
        return [notice, "\n", ...parts];
      }
      return [notice, "\n", parts];
    }

    function runLabelForBoundary(boundary) {
      const group = groupForLine(boundary);
      if (group) {
        const start = group.startLine + 1;
        const end = group.endLine + 1;
        return `Run lines ${start}-${end} ▶`;
      }
      const range = statementRangeForLine(statementMap, boundary);
      if (range && isMultiLineStatement(range)) {
        const start = range.startLine + 1;
        const end = range.endLine + 1;
        return `Run lines ${start}-${end} ▶`;
      }
      return `Run line ${boundary + 1} ▶`;
    }

    function partsContext({ boxes, expected, boundaryOverride } = {}) {
      const boundary = Number.isFinite(boundaryOverride)
        ? boundaryOverride
        : state.boundary;
      const runLabel = runLabelForBoundary(boundary);
      const stepStart = stepStartForBoundary(boundary);
      const ctx = {
        boundary,
        stepStart,
        total,
        runLabel,
        passed: !!state.passes[state.boundary],
        boxes: boxes || [],
        expected: expected || [],
        byName: Object.fromEntries((boxes || []).map((b) => [b.name, b])),
        expectedByName: Object.fromEntries(
          (expected || []).map((b) => [b.name, b]),
        ),
        nameStyle: (text) => makeToken("name", text),
        typeStyle: (text) => makeToken("type", text),
        valueStyle: (text) => makeToken("value", text),
        addrStyle: (text) => makeToken("addr", text),
        codeStyle: (text) => makeToken("code", text),
        btnStyle: (text) => makeToken("btn", text),
        pointerTargetName: (box, list) =>
          pointerTargetName(box, list || boxes || []),
      };
      ctx.withTopNotice = (parts) => withTopNotice(parts, ctx);
      return ctx;
    }

    function resolveParts(spec, ctx) {
      if (!spec) return null;
      if (typeof spec === "function") return spec(ctx);
      if (Array.isArray(spec)) return spec;
      return [String(spec)];
    }

    function getInstructionParts(ctx) {
      if (!instructions) return null;
      if (typeof instructions === "function") return instructions(ctx);
      if (instructions && typeof instructions === "object") {
        const entry =
          instructions[ctx.stepStart] ??
          instructions[ctx.boundary] ??
          instructions.default ??
          null;
        return resolveParts(entry, ctx);
      }
      return null;
    }

    function getHintParts(ctx) {
      if (!hints) return null;
      if (typeof hints === "function") return hints(ctx);
      if (hints && typeof hints === "object") {
        const entry = hints[ctx.stepStart] ?? hints[state.boundary] ?? null;
        return resolveParts(entry, ctx);
      }
      return null;
    }

    function renderCodePaneForBoundary() {
      if (!codeEl) return;
      const progress =
        editableSet.has(state.boundary) && !state.passes[state.boundary];
      let progressRange;
      let progressIndex;
      let doneBoundary;
      if (progress) {
        const groupRange = groupRangeEndingAt(state.boundary);
        const range =
          groupRange || statementRangeEndingAt(statementMap, state.boundary);
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
      if (!stageEl) return;
      stageEl.innerHTML = "";

      const editable =
        editableSet.has(state.boundary) && !state.passes[state.boundary];
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
        } else {
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
      const expected = getExpectedState(state.boundary);
      const ctx = partsContext({ expected });
      if (state.boundary === total && state.passes[state.boundary]) {
        setPartsContent(instructionsEl, "Program solved!");
        return;
      }
      let parts = getInstructionParts(ctx);
      if (Array.isArray(parts) && parts.length === 0) parts = null;
      if (parts && state.boundary === 0) {
        parts = withTopNotice(parts, ctx);
      }
      const fallbackMode =
        stepperFallback === "buttons" ? "buttons" : "default";
      if (
        !parts &&
        stepperFallback &&
        state.boundary < firstInstructionBoundary()
      ) {
        parts = stepperInstructionParts(ctx, {
          atStart: state.boundary === 0,
          mode: fallbackMode,
        });
        if (parts && state.boundary === 0) {
          parts = withTopNotice(parts, ctx);
        }
      }
      if (!parts) {
        parts = null;
      }
      setPartsContent(instructionsEl, parts);
    }

    let cachedFirstInstructionBoundary = null;
    function firstInstructionBoundary() {
      if (cachedFirstInstructionBoundary !== null)
        return cachedFirstInstructionBoundary;
      if (!instructions) {
        cachedFirstInstructionBoundary = Infinity;
        return cachedFirstInstructionBoundary;
      }
      for (let b = 0; b <= total; b++) {
        const expected = getExpectedState(b);
        const ctx = partsContext({ expected, boundaryOverride: b });
        let parts = getInstructionParts(ctx);
        if (Array.isArray(parts) && parts.length === 0) parts = null;
        if (parts) {
          cachedFirstInstructionBoundary = b;
          return cachedFirstInstructionBoundary;
        }
      }
      cachedFirstInstructionBoundary = Infinity;
      return cachedFirstInstructionBoundary;
    }

    function hideHint() {
      if (!hintPanel) return;
      hintPanel.textContent = "";
      hintPanel.classList.add("hidden");
    }

    function readWorkspaceBoxes() {
      const ws = getWorkspaceEl();
      if (!ws) return [];
      return [...ws.querySelectorAll(".vbox")].map((v) => readBoxState(v));
    }

    function evaluateWorkspace(boxes) {
      const expected = getExpectedState(state.boundary);
      return { ok: boxesEqual(boxes, expected), expected };
    }

    function showHint(parts) {
      if (!hintPanel) return;
      if (!parts || (Array.isArray(parts) && parts.length === 0)) return;
      renderParts(hintPanel, parts);
      hintPanel.classList.remove("hidden");
      flashStatus(hintPanel);
    }

    function isLooksGoodParts(parts) {
      if (typeof parts === "string") {
        return parts.trim() === "Looks good. Press $b{Check}.";
      }
      if (!Array.isArray(parts) || parts.length !== 3) return false;
      const [lead, mid, tail] = parts;
      if (String(lead) !== "Looks good. Press ") return false;
      if (!mid || typeof mid !== "object") return false;
      if (mid.kind !== "tok" || mid.role !== "btn") return false;
      if (String(mid.text || "") !== "Check") return false;
      if (String(tail) !== ".") return false;
      return true;
    }

    function render() {
      renderCodePaneForBoundary();
      renderStage();
      hideHint();
      const editable =
        editableSet.has(state.boundary) && !state.passes[state.boundary];
      if (editable && statusEl) {
        setStatus("", "muted");
      } else if (editableSet.has(state.boundary) && statusEl) {
        setStatus("correct", "ok");
      } else if (statusEl) {
        setStatus("", "muted");
      }
      if (checkBtn) checkBtn.classList.toggle("hidden", !editable);
      if (hintBtn) hintBtn.classList.toggle("hidden", !editable);
      if (addBtn)
        addBtn.classList.toggle(
          "hidden",
          !editable || !workspace.allowAddAndDelete,
        );
      if (resetBtn) resetBtn.classList.add("hidden");
      updateInstructions();
      if (editable && resetBtn) updateResetVisibility(state.boundary);
    }

    function save() {
      if (!editableSet.has(state.boundary)) return;
      const snapshot = serializeWorkspace(getWorkspaceEl());
      if (Array.isArray(snapshot)) {
        state.ws[state.boundary] = snapshot;
      }
    }

    if (addBtn) {
      addBtn.addEventListener("click", () => {
        const ws = getWorkspaceEl();
        if (!ws) return;
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
        if (!editableSet.has(state.boundary)) return;
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
        if (!editableSet.has(state.boundary)) return;
        const boxes = readWorkspaceBoxes();
        const result = evaluateWorkspace(boxes);
        const ok = result.ok;
        setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
        flashStatus(statusEl);
        if (!ok) return;
        state.passes[state.boundary] = true;
        state.ws[state.boundary] = boxes;
        const ws = getWorkspaceEl();
        if (ws) {
          ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
          removeBoxDeleteButtons(ws);
        }
        if (checkBtn) checkBtn.classList.add("hidden");
        if (hintBtn) hintBtn.classList.add("hidden");
        if (addBtn) addBtn.classList.add("hidden");
        if (resetBtn) resetBtn.classList.add("hidden");
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
        const ctx = partsContext({ boxes, expected: result.expected });
        if (result.ok) {
          showHint("Looks good. Press $b{Check}.");
          return;
        }
        const parts = getHintParts(ctx);
        if (!parts || (Array.isArray(parts) && parts.length === 0)) {
          showHint(
            "Your program has a problem that isn't covered by a hint, sorry. You can click $b{Reset} to undo all of your changes for this step.",
          );
          return;
        }
        if (isLooksGoodParts(parts)) {
          showHint(
            "Your program has a problem that isn't covered by a hint, sorry. You can click $b{Reset} to undo all of your changes for this step.",
          );
          return;
        }
        showHint(parts);
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
      isStepLocked: (boundary) =>
        editableSet.has(boundary) && !state.passes[boundary],
      getStepBadge: (step) => {
        if (!editableSet.has(step)) return "";
        return state.passes[step] ? "check" : "note";
      },
      getNextLabel: (boundary, totalCount, atEnd) => {
        if (atEnd) return "Next Program";
        const group = groupForLine(boundary);
        if (group) {
          const start = group.startLine + 1;
          const end = group.endLine + 1;
          return `Run lines ${start}-${end}`;
        }
        const range = statementRangeForLine(statementMap, boundary);
        if (range && isMultiLineStatement(range)) {
          const start = range.startLine + 1;
          const end = range.endLine + 1;
          return `Run lines ${start}-${end}`;
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

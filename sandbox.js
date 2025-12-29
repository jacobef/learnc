(function (MB) {
  const { $, randAddr, vbox, isEmptyVal, createSimpleSimulator } = MB;

  const instructions = $("#sandbox-instructions");
  const editor = $("#sandbox-editor");
  const lineNumbers = $("#sandbox-line-numbers");
  const errorGutter = $("#sandbox-error-gutter");
  const errorDetail = $("#sandbox-error-detail");
  const exprInput = $("#sandbox-expr");
  const exprResult = $("#sandbox-expr-result");
  const exprError = $("#sandbox-expr-error");
  const prevBtn = $("#sandbox-prev");
  const nextBtn = $("#sandbox-next");
  const prevButtons = [
    prevBtn,
    ...document.querySelectorAll(
      '[data-stepper="prev"][data-prefix="sandbox"]',
    ),
  ].filter(Boolean);
  const nextButtons = [
    nextBtn,
    ...document.querySelectorAll(
      '[data-stepper="next"][data-prefix="sandbox"]',
    ),
  ].filter(Boolean);
  const highlightEl = (() => {
    if (!editor || !editor.parentElement) return null;
    const el = document.createElement("pre");
    el.className = "code-textarea-highlight";
    el.setAttribute("aria-hidden", "true");
    editor.parentElement.insertBefore(el, editor);
    return el;
  })();
  const boundaryLine = (() => {
    if (!editor) return null;
    const row = editor.closest(".codepane-row");
    if (!row) return null;
    const el = document.createElement("div");
    el.className = "code-boundary-line";
    el.setAttribute("aria-hidden", "true");
    row.appendChild(el);
    return el;
  })();
  const measureEl = (() => {
    if (!editor || !editor.parentElement) return null;
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
  };

  const simulator = createSimpleSimulator({
    allowVarAssign: true,
    allowDeclAssign: true,
    allowDeclAssignVar: true,
    requireSourceValue: true,
    allowPointers: true,
  });

  const INT32_MIN = -2147483648n;
  const INT32_MAX = 2147483647n;
  const INT64_MIN = -9223372036854775808n;
  const INT64_MAX = 9223372036854775807n;

  function classifyNumericLiteral(value) {
    try {
      const n = BigInt(String(value));
      if (n < INT64_MIN || n > INT64_MAX) return "compile";
      if (n < INT32_MIN || n > INT32_MAX) return "ub";
      return "ok";
    } catch {
      return "compile";
    }
  }

  function literalTypeFor(value) {
    const status = classifyNumericLiteral(value);
    if (status === "compile")
      return { error: "That number is too large to represent.", kind: "compile" };
    if (status === "ub") return { type: "long" };
    return { type: "int" };
  }

  function typeInfo(type) {
    const cleaned = String(type || "").trim();
    const match = cleaned.match(/^(.*?)(\*+)$/);
    if (!match) return { base: cleaned, depth: 0 };
    return { base: match[1], depth: match[2].length };
  }

  function makePointerType(base, depth) {
    if (!Number.isFinite(depth) || depth < 0) return null;
    if (depth === 0) return base;
    return `${base}${"*".repeat(depth)}`;
  }

  function parseExpressionTokens(src = "") {
    const tokens = [];
    let i = 0;
    while (i < src.length) {
      const ch = src[i];
      if (/\s/.test(ch)) {
        i++;
        continue;
      }
      if (ch === "*" || ch === "&") {
        tokens.push({ type: "sym", value: ch });
        i++;
        continue;
      }
      if (/[A-Za-z_]/.test(ch)) {
        let j = i + 1;
        while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j++;
        tokens.push({ type: "ident", value: src.slice(i, j) });
        i = j;
        continue;
      }
      if (ch === "-" || /[0-9]/.test(ch)) {
        let j = i;
        if (src[j] === "-") {
          if (!/[0-9]/.test(src[j + 1] || "")) {
            return { error: "That line has a character that does not belong in an expression." };
          }
          j++;
        }
        while (j < src.length && /[0-9]/.test(src[j])) j++;
        tokens.push({ type: "number", value: src.slice(i, j) });
        i = j;
        continue;
      }
      return { error: "That line has a character that does not belong in an expression." };
    }
    return { tokens };
  }

  function parseExpression(src = "") {
    const { tokens, error } = parseExpressionTokens(src);
    if (error) return { error };
    if (!tokens.length) return { error: "Enter an expression to evaluate." };
    const ops = [];
    let idx = 0;
    while (
      idx < tokens.length &&
      tokens[idx].type === "sym" &&
      (tokens[idx].value === "*" || tokens[idx].value === "&")
    ) {
      ops.push(tokens[idx].value);
      idx++;
    }
    if (idx >= tokens.length) return { error: "Expression needs a value." };
    const primary = tokens[idx];
    idx++;
    if (idx < tokens.length)
      return { error: "Only unary * and & are supported in this sandbox." };
    if (primary.type !== "ident" && primary.type !== "number")
      return { error: "Expression needs a variable name or number." };
    return { ops, primary };
  }

  function showExprError(message) {
    if (!exprError) return;
    exprError.textContent = message || "";
    exprError.classList.toggle("hidden", !message);
  }

  function evaluateExpression(expr, state) {
    const parsed = parseExpression(expr);
    if (parsed.error) return { error: parsed.error, kind: "compile" };
    const { ops, primary } = parsed;
    const byName = new Map();
    state.forEach((b) => {
      if (b.name) byName.set(b.name, b);
      if (Array.isArray(b.names)) {
        b.names.forEach((n) => {
          if (n) byName.set(n, b);
        });
      }
    });
    let current;
    let label = primary.value;
    if (primary.type === "ident") {
      const box = byName.get(primary.value);
      if (!box)
        return { error: `Unknown variable ${primary.value}.`, kind: "compile" };
      current = {
        kind: "lvalue",
        type: box.type,
        value: box.value,
        address: box.address ?? box.addr ?? "",
      };
    } else {
      const status = literalTypeFor(primary.value);
      if (status.error) return { error: status.error, kind: status.kind };
      current = {
        kind: "rvalue",
        type: status.type || "int",
        value: primary.value,
        address: "",
      };
    }

    for (let i = ops.length - 1; i >= 0; i--) {
      const op = ops[i];
      if (op === "&") {
        const nextLabel = `&${label}`;
        if (current.kind !== "lvalue" || !current.address) {
          return { error: `${nextLabel} is not valid here.`, kind: "compile" };
        }
        const { base, depth } = typeInfo(current.type);
        current = {
          kind: "rvalue",
          type: makePointerType(base || "int", depth + 1),
          value: String(current.address),
          address: "",
        };
        label = nextLabel;
      } else if (op === "*") {
        const nextLabel = `*${label}`;
        const { base, depth } = typeInfo(current.type);
        if (!Number.isFinite(depth) || depth < 1) {
          return {
            error: `${nextLabel} is not a valid dereference.`,
            kind: "compile",
          };
        }
        const ptrVal = String(current.value ?? "").trim();
        if (!ptrVal || /^empty$/i.test(ptrVal)) {
          return {
            error: `${label} doesn't have a value yet, so it can't be dereferenced.`,
            kind: "ub",
          };
        }
        const target = state.find(
          (b) => String(b.address ?? b.addr ?? "") === String(ptrVal),
        );
        if (!target) {
          return {
            error: `${nextLabel} doesn't point to a known variable.`,
            kind: "ub",
          };
        }
        current = {
          kind: "lvalue",
          type: makePointerType(base || "int", depth - 1),
          value: target.value,
          address: target.address ?? target.addr ?? "",
        };
        label = nextLabel;
      }
    }

    return { result: current, label };
  }

  function allocFactory() {
    if (sandbox.allocBase == null) sandbox.allocBase = randAddr("int");
    let next = sandbox.allocBase;
    return () => {
      const addr = next;
      next += 4;
      return String(addr);
    };
  }

  function updateInstructions() {
    if (!instructions) return;
    const lines = getRawLines();
    const total = lines.length;
    const boundary = Number.isFinite(sandbox.boundary) ? sandbox.boundary : total;
    const statementInfo = getStatementContext(lines, boundary);
    const hasCode = lines.some((line) => line.trim() !== "");
    const atEnd = boundary >= total;
    const base =
      "This is the sandbox. The program state will update as you write code.";
    let suffix = "";
    if (hasCode) {
      let runLabel = "";
      if (
        statementInfo?.midStatement &&
        isMultiLineStatement(statementInfo.currentRange)
      ) {
        runLabel = `Finish running ${formatLineRange(
          statementInfo.currentRange,
        )} ▶`;
      } else if (
        statementInfo?.atStatementStart &&
        isMultiLineStatement(statementInfo.currentRange)
      ) {
        runLabel = `Run ${formatLineRange(statementInfo.currentRange)} ▶`;
      } else {
        runLabel = `Run line ${boundary + 1} ▶`;
      }
      if (atEnd) {
        suffix = ' Use <span class="btn-ref">Back ◀</span> to step through your program.';
      } else if (boundary <= 0) {
        suffix = ` Use <span class="btn-ref">${runLabel}</span> to step through your program.`;
      } else {
        suffix = ` Use <span class="btn-ref">Back ◀</span> and <span class="btn-ref">${runLabel}</span> to step through your program.`;
      }
    }
    const message = `${base}${suffix}`;
    const params = new URLSearchParams(window.location.search);
    const finished = params.get("finished") === "1";
    instructions.classList.toggle("sandbox-finished", finished);
    if (finished) {
      instructions.innerHTML = `You finished the tutorial as it currently exists, congrats! Many more problems will be coming later.<br><br>${message}`;
    } else {
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
    if (!secondary) return primary;
    const p = parseErrorMessage(primary);
    const s = parseErrorMessage(secondary);
    const text = [p.text, s.text].filter(Boolean).join(" ");
    const htmlParts = [p.html || p.text, s.html || s.text].filter(Boolean);
    const html = htmlParts.join(" ");
    return { text, html };
  }

  function showErrorDetail(message, kind) {
    if (!errorDetail) return;
    const parsed = parseErrorMessage(message);
    errorDetail.innerHTML = "";
    if (kind === "ub") {
      const base = document.createElement("span");
      if (parsed.html) {
        base.innerHTML = parsed.html;
      } else {
        base.textContent = parsed.text;
      }
      base.appendChild(
        document.createTextNode(" This line causes undefined behavior. "),
      );
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
    } else if (parsed.html) {
      errorDetail.innerHTML = parsed.html;
    } else {
      errorDetail.textContent = parsed.text;
    }
    errorDetail.classList.remove("hidden");
    sandbox.errorDetailMessage = parsed.text;
    sandbox.errorDetailHtml = parsed.html;
    sandbox.errorDetailKind = kind || "compile";
  }

  function hideErrorDetail() {
    if (!errorDetail) return;
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
    if (!Number.isFinite(value)) return 0;
    return Math.max(0, Math.min(total, value));
  }

  function isMultiLineStatement(range) {
    return (
      range &&
      Number.isFinite(range.startLine) &&
      Number.isFinite(range.endLine) &&
      range.endLine > range.startLine
    );
  }

  function formatLineRange(range) {
    if (!range) return "";
    const start = (range.startLine ?? 0) + 1;
    const end = (range.endLine ?? 0) + 1;
    if (start === end) return `line ${start}`;
    return `lines ${start}-${end}`;
  }

  function countNewlines(value) {
    let count = 0;
    for (let i = 0; i < value.length; i++) {
      if (value[i] === "\n") count++;
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
    while (
      endPrev > start &&
      endNext > start &&
      prev[endPrev - 1] === next[endNext - 1]
    ) {
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
    if (prevText === nextText) return;
    const prevLines = prevText.split(/\r?\n/).length;
    const nextLines = nextText.split(/\r?\n/).length;
    const boundary = Number.isFinite(sandbox.boundary)
      ? sandbox.boundary
      : prevLines;
    if (!Number.isFinite(boundary)) return;
    const { start, oldSegment, newSegment } = diffSegments(prevText, nextText);
    const beforeChange = prevText.slice(0, start);
    const changeLine = countNewlines(beforeChange);
    const lastBreak = beforeChange.lastIndexOf("\n");
    const changeCol = start - (lastBreak + 1);
    if (changeLine > boundary) return;
    const delta = countNewlines(newSegment) - countNewlines(oldSegment);
    if (!delta) return;
    if (changeLine === boundary) {
      if (changeCol !== 0 || delta < 0) return;
    }
    sandbox.boundary = clampBoundary(boundary + delta, nextLines);
  }

  function buildStatementMap(lines) {
    const text = lines.join("\n");
    const tokens = simulator.tokenizeProgram(text);
    const parts = simulator.splitStatements(tokens);
    const byLine = new Array(lines.length).fill(null);
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

  function getStatementContext(lines, boundary) {
    const statementMap = buildStatementMap(lines);
    const currentRange = statementRangeForLine(statementMap, boundary);
    const prevRange = statementRangeForLine(statementMap, boundary - 1);
    const currentIsMulti = isMultiLineStatement(currentRange);
    const midStatement =
      currentIsMulti &&
      boundary > currentRange.startLine &&
      boundary <= currentRange.endLine;
    const atStatementStart =
      currentIsMulti && boundary === currentRange.startLine;
    return {
      statementMap,
      currentRange,
      prevRange,
      midStatement,
      atStatementStart,
    };
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
    } else {
      sandbox.boundary = clampBoundary(sandbox.boundary, total);
    }
    return { boundary: sandbox.boundary, total };
  }

  function classifyLineStatuses(lines) {
    return simulator.classifyLineStatuses(lines, { alloc: allocFactory() });
  }

  function applyUserProgram(lines) {
    const text = lines.join("\n");
    return simulator.applyProgram(text, { alloc: allocFactory() });
  }

  function getProgramOutcome(linesOverride, boundaryOverride) {
    const lines = linesOverride || getRawLines();
    const total = lines.length;
    const boundary = Number.isFinite(boundaryOverride)
      ? boundaryOverride
      : Number.isFinite(sandbox.boundary)
        ? sandbox.boundary
        : total;
    const safeBoundary = clampBoundary(boundary, total);
    const activeLines = lines.slice(0, safeBoundary);
    const status = classifyLineStatuses(activeLines);
    let hasCompile = status.incomplete.size > 0;
    let hasUb = false;
    if (status.errorKinds) {
      for (const kind of status.errorKinds.values()) {
        if (kind === "ub") hasUb = true;
        else hasCompile = true;
      }
    }
    if (hasCompile) return { kind: "compile", state: null, boundary: safeBoundary, total };
    if (hasUb) return { kind: "ub", state: null, boundary: safeBoundary, total };
    const state = applyUserProgram(activeLines);
    if (!state) return { kind: "compile", state: null, boundary: safeBoundary, total };
    return { kind: "ok", state, boundary: safeBoundary, total };
  }

  function renderStage() {
    const stage = $("#sandbox-stage");
    stage.innerHTML = "";
    const lines = getRawLines();
    const { boundary, total } = syncBoundaryWithLines(lines);
    const statementInfo = getStatementContext(lines, boundary);
    const outcome = getProgramOutcome(lines, boundary);
    const displayOutcome = statementInfo.midStatement
      ? { kind: "mid", state: null }
      : outcome;
    stage.appendChild(renderState("", displayOutcome.state, displayOutcome.kind));
    renderExpression(displayOutcome);
    updateStepperControls(boundary, total, statementInfo);
    updateInstructions();
  }

  function renderExpression(outcome) {
    if (!exprResult || !exprInput) return;
    exprResult.innerHTML = "";
    showExprError("");
    const expr = exprInput.value.trim();
    if (!expr) return;
    if (!outcome || outcome.kind !== "ok") {
      showExprError("Fix the code before evaluating expressions.");
      return;
    }
    const evaluated = evaluateExpression(expr, outcome.state || []);
    if (evaluated.error) {
      showExprError(evaluated.error);
      return;
    }
    const { result } = evaluated;
    const node = vbox({
      addr: result.address ? String(result.address) : "—",
      type: result.type || "int",
      value: result.value ?? "empty",
      name: null,
      editable: false,
    });
    if (!result.address) node.classList.add("no-addr");
    node.classList.add("no-name");
    exprResult.appendChild(node);
  }

  function updateStepperControls(
    boundaryOverride,
    totalOverride,
    statementInfo,
  ) {
    if (!prevButtons.length && !nextButtons.length) return;
    const total = Number.isFinite(totalOverride)
      ? totalOverride
      : Number.isFinite(sandbox.lineCount)
        ? sandbox.lineCount
        : getRawLines().length;
    const boundary = Number.isFinite(boundaryOverride)
      ? boundaryOverride
      : Number.isFinite(sandbox.boundary)
        ? sandbox.boundary
        : total;
    prevButtons.forEach((btn) => {
      btn.disabled = boundary <= 0;
    });
    if (nextButtons.length) {
      const atEnd = boundary >= total;
      let label = "At end ▶";
      if (!atEnd) {
        if (
          statementInfo?.midStatement &&
          isMultiLineStatement(statementInfo.currentRange)
        ) {
          label = `Finish running ${formatLineRange(
            statementInfo.currentRange,
          )} ▶`;
        } else if (
          statementInfo?.atStatementStart &&
          isMultiLineStatement(statementInfo.currentRange)
        ) {
          label = `Run ${formatLineRange(statementInfo.currentRange)} ▶`;
        } else {
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
    } else if (status === "compile") {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent = "(this code is not valid)";
      grid.appendChild(msg);
    } else if (status === "ub") {
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
    } else if (boxes === null) {
      const msg = document.createElement("div");
      msg.className = "muted state-status";
      msg.style.padding = "8px";
      msg.textContent = "(this code is not valid)";
      grid.appendChild(msg);
    } else if (boxes.length === 0) {
      const msg = document.createElement("div");
      msg.className = "muted";
      msg.style.padding = "8px";
      msg.textContent = "(no variables yet)";
      grid.appendChild(msg);
    } else {
      boxes.forEach((b) => {
        const node = vbox({
          addr: b.address,
          type: b.type,
          value: b.value,
          name: b.name,
          names: b.names,
          editable: false,
          allowNameToggle: true,
        });
        if (isEmptyVal(b.value || ""))
          node.querySelector(".value").classList.add("placeholder", "muted");
        grid.appendChild(node);
      });
    }
    wrap.appendChild(grid);
    return wrap;
  }

  function getLineHeightPx() {
    if (!editor) return 32;
    const style = window.getComputedStyle(editor);
    const lh = parseFloat(style.lineHeight);
    return Number.isFinite(lh) ? lh : 32;
  }

  function autoSizeEditor() {
    if (!editor) return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
  }

  function measureWrapCounts(lines) {
    if (!editor || !measureEl) return lines.map(() => 1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(
      1,
      editor.clientWidth - paddingLeft - paddingRight,
    );
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
    const lines = linesOverride || getRawLines();
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
        if (i < safeBoundary) span.classList.add("is-done");
        span.textContent = lines[i] === "" ? " " : lines[i];
        frag.appendChild(span);
        if (i < lines.length - 1) frag.appendChild(document.createTextNode("\n"));
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
      if (editor) lineNumbers.style.height = `${editor.clientHeight}px`;
    }
    if (errorGutter) {
      const { invalid, incomplete, errors, errorKinds, info } =
        classifyLineStatuses(lines);
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
          const message =
            (errorKinds?.get(i) || "compile") === "compile" && info?.has(i)
              ? combineMessages(baseMessage, info.get(i))
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
            if (
              sandbox.errorDetailLine === i &&
              sandbox.errorDetailMessage === parsed.text &&
              sandbox.errorDetailHtml === parsed.html &&
              sandbox.errorDetailKind === kind
            ) {
              hideErrorDetail();
            } else {
              sandbox.errorDetailLine = i;
              showErrorDetail(message, kind);
            }
          });
          cell.appendChild(infoBtn);
        } else if (incomplete.has(i)) {
          cell.classList.add("is-incomplete");
          cell.textContent = "...";
          cell.title = "Line is incomplete";
          if (info?.has(i)) {
            const infoMsg = info.get(i);
            const parsed = parseErrorMessage(infoMsg);
            const btn = document.createElement("button");
            btn.type = "button";
            btn.className = "error-info-btn";
            btn.textContent = "i";
            btn.setAttribute("aria-label", "Explain statement");
            btn.addEventListener("click", (e) => {
              e.preventDefault();
              e.stopPropagation();
              if (
                sandbox.errorDetailLine === i &&
                sandbox.errorDetailMessage === parsed.text &&
                sandbox.errorDetailHtml === parsed.html &&
                sandbox.errorDetailKind === "info"
              ) {
                hideErrorDetail();
              } else {
                sandbox.errorDetailLine = i;
                showErrorDetail(infoMsg, "info");
              }
            });
            cell.appendChild(btn);
          }
        } else if (info?.has(i)) {
          const infoMsg = info.get(i);
          const parsed = parseErrorMessage(infoMsg);
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "error-info-btn";
          btn.textContent = "i";
          btn.setAttribute("aria-label", "Explain statement");
          btn.addEventListener("click", (e) => {
            e.preventDefault();
            e.stopPropagation();
            if (
              sandbox.errorDetailLine === i &&
              sandbox.errorDetailMessage === parsed.text &&
              sandbox.errorDetailHtml === parsed.html &&
              sandbox.errorDetailKind === "info"
            ) {
              hideErrorDetail();
            } else {
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
      if (editor) errorGutter.style.height = `${editor.clientHeight}px`;
    }
    if (editor) {
      if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
      if (errorGutter) errorGutter.scrollTop = editor.scrollTop;
    }
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
    editor.addEventListener("mouseup", updateLineGutters);
    window.addEventListener("resize", updateLineGutters);
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => updateLineGutters());
      ro.observe(editor);
    }
  }

  prevButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      const lines = getRawLines();
      const total = lines.length;
      const current = Number.isFinite(sandbox.boundary)
        ? sandbox.boundary
        : total;
      if (current <= 0) return;
      const boundary = clampBoundary(current, total);
      const statementInfo = getStatementContext(lines, boundary);
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
      const current = Number.isFinite(sandbox.boundary)
        ? sandbox.boundary
        : total;
      if (current >= total) return;
      const boundary = clampBoundary(current, total);
      const statementInfo = getStatementContext(lines, boundary);
      let target = boundary + 1;
      if (
        (statementInfo.midStatement || statementInfo.atStatementStart) &&
        statementInfo.currentRange
      ) {
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
})(window.MB);

import {
  $,
  applyOtherNames,
  createSimpleSimulator,
  ensureBaseLayout,
  formatValueForType,
  randAddr,
  vbox,
} from "./shared-core.js";
import type {
  BoxState,
  StatementPart,
  IfBlockMap,
} from "./shared-core.js";
import { confettiRain } from "./confetti.js";

const { main } = ensureBaseLayout();
main.classList.add("main-panelized");

const instructions = $(
  '[data-role="sandbox-instructions"]',
) as HTMLElement | null;
const editor = $('[data-role="sandbox-editor"]') as HTMLTextAreaElement | null;
const lineNumbers = $(
  '[data-role="sandbox-line-numbers"]',
) as HTMLElement | null;
const errorGutter = $(
  '[data-role="sandbox-error-gutter"]',
) as HTMLElement | null;
const errorDetail = $(
  '[data-role="sandbox-error-detail"]',
) as HTMLElement | null;
const exprInput = $('[data-role="sandbox-expr"]') as HTMLInputElement | null;
const exprResult = $('[data-role="sandbox-expr-result"]') as HTMLElement | null;
const exprError = $('[data-role="sandbox-expr-error"]') as HTMLElement | null;
const prevBtn = $('[data-role="sandbox-prev"]') as HTMLButtonElement | null;
const nextBtn = $('[data-role="sandbox-next"]') as HTMLButtonElement | null;
const prevButtons = [prevBtn].filter((btn): btn is HTMLButtonElement => !!btn);
const nextButtons = [nextBtn].filter((btn): btn is HTMLButtonElement => !!btn);
let finishedConfettiShown = false;
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

type SandboxState = {
  text: string;
  allocBase: number | null;
  errorDetailLine: number | null;
  errorDetailMessage: string;
  errorDetailHtml: string;
  errorDetailKind: string;
  boundary: number | null;
  lineCount: number;
  otherNamesShown: Set<string>;
  lastState: BoxState[] | null;
  exprOtherNamesShown: Set<string>;
};

const sandbox: SandboxState = {
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
});

function showExprError(message: string) {
  if (!exprError) return;
  exprError.textContent = message || "";
  exprError.classList.toggle("hidden", !message);
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
  const base =
    "This is the sandbox. The program state will update as you write code.";
  const message = base;
  const params = new URLSearchParams(window.location.search);
  const finished = params.get("finished") === "1";
  instructions.classList.toggle("sandbox-finished", finished);
  if (finished) {
    instructions.innerHTML = `You finished the tutorial as it currently exists, congrats! Many more problems will be coming later.<br><br>${message}`;
    if (!finishedConfettiShown && typeof confettiRain === "function") {
      finishedConfettiShown = true;
      confettiRain();
    }
  } else {
    instructions.innerHTML = message;
  }
}

type ParsedMessage = { text: string; html: string };

function parseErrorMessage(message: string | ParsedMessage) {
  if (message && typeof message === "object") {
    return {
      text: String(message.text || ""),
      html: message.html ? String(message.html) : "",
    };
  }
  return { text: String(message || ""), html: "" };
}

function combineMessages(
  primary: string | ParsedMessage,
  secondary: string | ParsedMessage,
) {
  if (!secondary) return primary;
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

function isGenericCompileMessage(message: string | ParsedMessage) {
  const parsed = parseErrorMessage(message);
  const text = (parsed.text || "").trim();
  if (!text) return true;
  return GENERIC_COMPILE_ERRORS.has(text);
}

function showErrorDetail(message: string | ParsedMessage, kind: string) {
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

function clampBoundary(value: number, total: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(total, value));
}

function resolveBoundary(value: number | null | undefined, fallback: number) {
  return Number.isFinite(value) ? (value as number) : fallback;
}

function stopIndexForBoundary(
  parts: StatementPart[],
  boundary: number,
  totalLines: number,
) {
  const target = Math.max(0, Math.min(totalLines, boundary));
  if (!parts.length) return 0;
  const idx = parts.findIndex((part) => {
    const end = part?.endLine;
    return Number.isFinite(end) && end >= target;
  });
  return idx === -1 ? parts.length : idx;
}

function headerIndexForLine(
  parts: StatementPart[],
  ifBlocks: IfBlockMap,
  lineIndex: number,
) {
  if (!Number.isFinite(lineIndex)) return null;
  let selected: number | null = null;
  for (let i = 0; i < parts.length; i++) {
    const block = ifBlocks.map.get(i);
    if (!block) continue;
    if (block.headerStartLine !== lineIndex) continue;
    const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
      ? parts[block.closeIndex]!.endLine
      : block.headerEndLine;
    if (Number.isFinite(closeLine) && closeLine > lineIndex) {
      selected = i;
    }
  }
  return selected;
}

function countNewlines(value: string) {
  let count = 0;
  for (let i = 0; i < value.length; i++) {
    if (value[i] === "\n") count++;
  }
  return count;
}

function diffSegments(prev: string, next: string) {
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

function adjustBoundaryForEdit(prevText: string, nextText: string) {
  if (prevText === nextText) return;
  const prevLines = prevText.split(/\r?\n/).length;
  const nextLines = nextText.split(/\r?\n/).length;
  const boundary = resolveBoundary(sandbox.boundary, prevLines);
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

function isBoundaryInsideBlockComment(lines: string[], boundary: number) {
  if (!Array.isArray(lines) || boundary <= 0) return false;
  let inComment = false;
  const limit = Math.min(boundary, lines.length);
  for (let lineIndex = 0; lineIndex < limit; lineIndex++) {
    const line = lines[lineIndex] || "";
    for (let i = 0; i < line.length; i++) {
      const ch = line[i];
      const next = line[i + 1];
      if (!inComment && ch === "/" && next === "/") break;
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

function syncBoundaryWithLines(lines: string[]) {
  const total = lines.length;
  const prevTotal = Number.isFinite(sandbox.lineCount)
    ? sandbox.lineCount
    : total;
  const hasBoundary = Number.isFinite(sandbox.boundary);
  const wasAtEnd = !hasBoundary || sandbox.boundary! >= prevTotal;
  sandbox.lineCount = total;
  if (wasAtEnd) {
    sandbox.boundary = total;
  } else {
    sandbox.boundary = clampBoundary(sandbox.boundary as number, total);
  }
  return { boundary: sandbox.boundary as number, total };
}

function classifyLineStatuses(lines: string[]) {
  return simulator.classifyLineStatuses(lines, { alloc: allocFactory() });
}

function getProgramParts(lines: string[]) {
  const statementMap = simulator.buildStatementMap(lines);
  const parts = statementMap.parts || [];
  const ifBlocks = simulator.buildIfStatementMap(parts, {
    lastLine: Math.max(0, lines.length - 1),
  });
  return { parts, ifBlocks, statementMap };
}

function summarizeStatus(lines: string[], fullLines?: string[]) {
  const status = classifyLineStatuses(lines);
  let hasCompile = status.incomplete.size > 0;
  if (hasCompile && fullLines && fullLines.length >= lines.length) {
    const activeTokens = simulator.tokenizeProgram(lines.join("\n"));
    const fullTokens = simulator.tokenizeProgram(fullLines.join("\n"));
    const activeParts = simulator.splitStatements(activeTokens);
    const fullParts = simulator.splitStatements(fullTokens);
    const activeIfs = simulator.buildIfStatementMap(activeParts, {
      lastLine: Math.max(0, lines.length - 1),
    });
    const fullIfs = simulator.buildIfStatementMap(fullParts, {
      lastLine: Math.max(0, fullLines.length - 1),
    });
    const ignoreIncomplete = new Set<number>();
    for (let i = 0; i < activeParts.length; i++) {
      if (fullIfs.map.has(i) && !activeIfs.map.has(i)) {
        const endLine = activeParts[i]?.endLine;
        if (Number.isFinite(endLine)) ignoreIncomplete.add(endLine);
      }
    }
    if (ignoreIncomplete.size) {
      const remaining = [...status.incomplete].filter(
        (line) => !ignoreIncomplete.has(line),
      );
      hasCompile = remaining.length > 0;
    }
  }
  let hasUb = false;
  if (status.errorKinds) {
    for (const kind of status.errorKinds.values()) {
      if (kind === "ub") hasUb = true;
      else hasCompile = true;
    }
  }
  return { hasCompile, hasUb };
}

function runLabelForBoundary(
  boundary: number,
  totalLines: number,
  statementMap: ReturnType<typeof simulator.buildStatementMap>,
) {
  const range = simulator.statementRangeForLine(statementMap, boundary);
  if (
    range &&
    typeof range.startLine === "number" &&
    typeof range.endLine === "number" &&
    range.endLine > range.startLine
  ) {
    const start = range.startLine + 1;
    const end = range.endLine + 1;
    return `Run lines ${start}-${end} ▶`;
  }
  const lineNumber = Math.max(1, Math.min(totalLines, boundary + 1));
  return `Run line ${lineNumber} ▶`;
}

function stateBeforePart(parts: StatementPart[], partIndex: number): BoxState[] {
  const alloc = allocFactory();
  const result = simulator.applyProgramParts(parts, {
    alloc,
    stop: Math.max(0, Math.min(parts.length, partIndex)),
  });
  return Array.isArray(result) ? result : [];
}

function nextBoundaryForLine(
  current: number,
  parts: StatementPart[],
  ifBlocks: IfBlockMap,
  statementMap: ReturnType<typeof simulator.buildStatementMap>,
  totalLines: number,
): number {
  if (!Number.isFinite(current)) return current + 1;
  if (current >= totalLines) return totalLines;
  const range = simulator.statementRangeForLine(statementMap, current);
  const rangeStart =
    typeof range?.startLine === "number" ? range.startLine : current;
  const rangeEnd = typeof range?.endLine === "number" ? range.endLine : current;
  const headerIndex = headerIndexForLine(parts, ifBlocks, rangeStart);
  if (headerIndex == null) {
    if (Number.isFinite(rangeEnd) && rangeEnd > current)
      return Math.min(totalLines, rangeEnd + 1);
    return Math.min(totalLines, current + 1);
  }
  const block = ifBlocks.map.get(headerIndex);
  if (!block) {
    if (Number.isFinite(rangeEnd) && rangeEnd > current)
      return Math.min(totalLines, rangeEnd + 1);
    return Math.min(totalLines, current + 1);
  }
  const currentState = stateBeforePart(parts, headerIndex);
  const condition = simulator.evaluateCondition(block.expr, currentState);
  if ("error" in condition || condition.value) {
    if (Number.isFinite(rangeEnd) && rangeEnd > current)
      return Math.min(totalLines, rangeEnd + 1);
    return Math.min(totalLines, current + 1);
  }
  const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
    ? parts[block.closeIndex]!.endLine
    : block.headerEndLine;
  if (!Number.isFinite(closeLine)) return Math.min(totalLines, current + 1);
  return Math.min(totalLines, closeLine + 1);
}

function prevBoundaryForLine(
  current: number,
  parts: StatementPart[],
  ifBlocks: IfBlockMap,
  statementMap: ReturnType<typeof simulator.buildStatementMap>,
  totalLines: number,
): number {
  if (!Number.isFinite(current)) return current - 1;
  if (current <= 0) return current - 1;
  let boundary = 0;
  let prev = 0;
  let guard = 0;
  while (boundary < current && guard < totalLines + 5) {
    prev = boundary;
    const next = nextBoundaryForLine(
      boundary,
      parts,
      ifBlocks,
      statementMap,
      totalLines,
    );
    boundary = next === boundary ? boundary + 1 : next;
    guard += 1;
  }
  if (boundary === current) return prev;
  return current - 1;
}

function applyUserProgram(lines: string[]) {
  const text = lines.join("\n");
  return simulator.applyProgram(text, { alloc: allocFactory() });
}

type ProgramOutcome = {
  kind: "ok" | "compile" | "ub";
  globalKind?: "ok" | "compile" | "ub";
  state: BoxState[] | null;
  boundary: number;
  total: number;
};

function getProgramOutcome(
  linesOverride?: string[],
  partsOverride?: StatementPart[],
  boundaryOverride?: number,
): ProgramOutcome {
  const lines = linesOverride || getRawLines();
  const { parts } = Array.isArray(partsOverride)
    ? { parts: partsOverride }
    : getProgramParts(lines);
  const total = lines.length;
  const boundary = resolveBoundary(
    Number.isFinite(boundaryOverride) ? boundaryOverride : sandbox.boundary,
    total,
  );
  const safeBoundary = clampBoundary(boundary, total);
  const fullSummary = summarizeStatus(lines);
  let globalKind: "ok" | "compile" | "ub" = "ok";
  if (fullSummary.hasCompile) globalKind = "compile";
  else if (fullSummary.hasUb) globalKind = "ub";
  const stopIndex = stopIndexForBoundary(parts, safeBoundary, total);
  const activeResult = simulator.analyzeProgramParts(parts, {
    alloc: allocFactory(),
    stop: stopIndex,
  });
  if (activeResult.kind !== "ok")
    return {
      kind: activeResult.kind,
      globalKind,
      state: null,
      boundary: safeBoundary,
      total,
    };
  return {
    kind: "ok",
    globalKind,
    state: activeResult.state,
    boundary: safeBoundary,
    total,
  };
}

function renderStage() {
  const stage = $('[data-role="sandbox-stage"]') as HTMLElement | null;
  if (!stage) return;
  stage.innerHTML = "";
  const lines = getRawLines();
  const { parts } = getProgramParts(lines);
  const { boundary, total } = syncBoundaryWithLines(lines);
  const inBlockComment = isBoundaryInsideBlockComment(lines, boundary);
  let outcome = getProgramOutcome(lines, parts, boundary);
  if (inBlockComment && outcome.kind === "compile") {
    // Close the open comment at the boundary so comments don't look invalid.
    const activeLines = lines.slice(0, boundary);
    if (activeLines.length) activeLines[activeLines.length - 1] += " */";
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
  let displayOutcome: { kind: string; state: BoxState[] | null } = outcome;
  if (outcome.globalKind && outcome.globalKind !== "ok") {
    displayOutcome = { kind: outcome.globalKind, state: null };
  }
  sandbox.lastState = outcome.state;
  stage.appendChild(renderState("", displayOutcome.state, displayOutcome.kind));
  applyOtherNames(stage, {
    shownAddrs: sandbox.otherNamesShown,
    onToggle: refreshOtherNames,
  });
  renderExpression(displayOutcome);
  updateStepperControls(boundary, total);
  updateInstructions();
}

function renderExpression(outcome: { kind: string; state: BoxState[] | null }) {
  if (!exprResult || !exprInput) return;
  exprResult.innerHTML = "";
  exprResult.classList.add("hidden");
  showExprError("");
  const expr = exprInput.value.trim();
  if (!expr) return;
  if (!outcome || outcome.kind !== "ok") {
    showExprError("Fix the code before evaluating expressions.");
    return;
  }
  const evaluated = simulator.evaluateExpressionText(expr, outcome.state || []);
  if ("error" in evaluated) {
    const errorText =
      typeof evaluated.error === "string"
        ? evaluated.error
        : evaluated.error.text;
    showExprError(errorText);
    return;
  }
  const { result } = evaluated;
  const match =
    result.kind === "lvalue"
      ? (outcome.state || []).find(
          (box) => String(box.address) === String(result.address),
        )
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
  if (match && (match.value ?? "") === "") {
    node.querySelector(".value")?.classList.add("placeholder", "muted");
  }
  if (!match && !result.address) node.classList.add("no-addr");
  if (!match) node.classList.add("no-name");
  exprResult.appendChild(node);
  exprResult.classList.remove("hidden");
  if (match) {
    applyOtherNames(exprResult, {
      shownAddrs: sandbox.exprOtherNamesShown,
      onToggle: refreshOtherNames,
      sourceBoxes: outcome.state || [],
      cleanupShownAddrs: false,
    });
  }
}

function updateStepperControls(
  boundaryOverride: number,
  totalOverride: number,
) {
  if (!prevButtons.length && !nextButtons.length) return;
  const totalLines = resolveBoundary(
    Number.isFinite(totalOverride) ? totalOverride : sandbox.lineCount,
    getRawLines().length,
  );
  const boundary = resolveBoundary(
    Number.isFinite(boundaryOverride) ? boundaryOverride : sandbox.boundary,
    totalLines,
  );
  prevButtons.forEach((btn) => {
    btn.disabled = boundary <= 0;
  });
  if (nextButtons.length) {
    const atEnd = boundary >= totalLines;
    let label = "At end ▶";
    if (!atEnd) {
      const { statementMap } = getProgramParts(getRawLines());
      label = runLabelForBoundary(boundary, totalLines, statementMap);
    }
    nextButtons.forEach((btn) => {
      btn.textContent = label;
      btn.disabled = atEnd;
    });
  }
}

function renderState(
  title: string,
  boxes: BoxState[] | null,
  status: string = "ok",
) {
  const wrap = document.createElement("div");
  wrap.className = "state-panel";
  if (title) {
    const heading = document.createElement("div");
    heading.className = "panel-title state-heading";
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
        address: b.address ?? undefined,
        type: b.type,
        value: b.value,
        name: b.name,
        editable: false,
      });
      if ((b.value ?? "") === "")
        node.querySelector(".value")?.classList.add("placeholder", "muted");
      grid.appendChild(node);
    });
  }
  const body = document.createElement("div");
  body.className = "state-panel-scroll-body";
  body.appendChild(grid);
  wrap.appendChild(body);
  return wrap;
}

function refreshOtherNames() {
  const stage = $('[data-role="sandbox-stage"]') as HTMLElement | null;
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

function measureWrapCounts(lines: string[]) {
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

function updateLineGutters(linesOverride?: string[]) {
  autoSizeEditor();
  const lines = Array.isArray(linesOverride) ? linesOverride : getRawLines();
  const count = Math.max(lines.length, 1);
  const boundary = resolveBoundary(sandbox.boundary, count);
  const safeBoundary = clampBoundary(boundary, count);
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
        const hasInfo = info?.has(i);
        const showInfo =
          kind === "ub" || hasInfo || !isGenericCompileMessage(baseMessage);
        if (showInfo) {
          const message =
            (errorKinds?.get(i) || "compile") === "compile" && hasInfo
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
        }
      } else if (incomplete.has(i)) {
        cell.classList.add("is-incomplete");
        cell.textContent = "...";
        cell.title = "Line is incomplete";
        if (info?.has(i)) {
          const infoMsg = info.get(i);
          if (!infoMsg) continue;
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
        if (!infoMsg) continue;
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
    const { parts, ifBlocks, statementMap } = getProgramParts(lines);
    const total = lines.length;
    const current = resolveBoundary(sandbox.boundary, total);
    if (current <= 0) return;
    const boundary = clampBoundary(current, total);
    const target = prevBoundaryForLine(
      boundary,
      parts,
      ifBlocks,
      statementMap,
      total,
    );
    sandbox.boundary = clampBoundary(target, total);
    renderStage();
    updateLineGutters();
  });
});

nextButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    const lines = getRawLines();
    const { parts, ifBlocks, statementMap } = getProgramParts(lines);
    const total = lines.length;
    const current = resolveBoundary(sandbox.boundary, total);
    if (current >= total) return;
    const boundary = clampBoundary(current, total);
    const target = nextBoundaryForLine(
      boundary,
      parts,
      ifBlocks,
      statementMap,
      total,
    );
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

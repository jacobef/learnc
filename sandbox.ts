import {
  applyOtherNames,
  appendStateObjects,
  clearNode,
  createSimpleSimulator,
  ensureBaseLayout,
  findArrayObjectBoxesForResult,
  formatValueForType,
  queryRole,
  randAddr,
  typeInfo,
  vbox,
} from "./shared-core.js";
import type {
  BoxState,
  ProgramDiagnostic,
  StatementPart,
  IfBlockMap,
} from "./shared-core.js";
import { confettiRain } from "./confetti.js";
import {
  bindCodeEditorTabKey,
  ensureCodeSurfaceElements,
  updateCodeSurface,
  type CodeDecoration,
} from "./shared-code-editor-surface.js";

const { main } = ensureBaseLayout();
main.classList.add("main-panelized");

const role = <T extends Element>(name: string): T | null => queryRole<T>(name);

const instructions = role<HTMLElement>("sandbox-instructions");
const editor = role<HTMLTextAreaElement>("sandbox-editor");
const lineNumbers = role<HTMLElement>("sandbox-line-numbers");
const exprInput = role<HTMLInputElement>("sandbox-expr");
const exprResult = role<HTMLElement>("sandbox-expr-result");
const prevBtn = role<HTMLButtonElement>("sandbox-prev");
const nextBtn = role<HTMLButtonElement>("sandbox-next");
const prevButtons = [prevBtn].filter((btn): btn is HTMLButtonElement => !!btn);
const nextButtons = [nextBtn].filter((btn): btn is HTMLButtonElement => !!btn);
let finishedConfettiShown = false;
const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);
bindCodeEditorTabKey(editor);
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
const diagnosticEl = (() => {
  const codePane = role<HTMLElement>("sandbox-code");
  if (!codePane) return null;
  const existing = codePane.querySelector<HTMLElement>(".code-diagnostic");
  if (existing) return existing;
  const el = document.createElement("div");
  el.className = "code-diagnostic hidden";
  codePane.appendChild(el);
  return el;
})();

type SandboxState = {
  text: string;
  allocBase: number | null;
  boundary: number | null;
  lineCount: number;
  otherNamesShown: Set<string>;
  lastState: BoxState[] | null;
  exprOtherNamesShown: Set<string>;
};

const sandbox: SandboxState = {
  text: editor ? editor.value : "",
  allocBase: null,
  boundary: null,
  lineCount: 0,
  otherNamesShown: new Set(),
  lastState: null,
  exprOtherNamesShown: new Set(),
};

const simulator = createSimpleSimulator();

function allocFactory() {
  if (sandbox.allocBase == null) sandbox.allocBase = randAddr("int");
  let next = Number(sandbox.allocBase);
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

function getProgramParts(lines: string[]) {
  const statementMap = simulator.buildStatementMap(lines);
  const parts = statementMap.parts || [];
  const ifBlocks = simulator.buildIfStatementMap(parts, {
    lastLine: Math.max(0, lines.length - 1),
  });
  return { parts, ifBlocks, statementMap };
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
  const fullResult = simulator.analyzeProgramParts(parts, {
    alloc: allocFactory(),
  });
  const globalKind = fullResult.kind;
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

function getProgramDiagnostic(): ProgramDiagnostic | null {
  const diagnostics = simulator.diagnoseProgram(getEditorText(), {
    alloc: allocFactory(),
  });
  return diagnostics[0] || null;
}

function diagnosticDecoration(
  diagnostic: ProgramDiagnostic | null,
): CodeDecoration[] {
  if (!diagnostic) return [];
  return [
    {
      line: diagnostic.range.startLine,
      startCol: diagnostic.range.startCol,
      endCol: diagnostic.range.endCol,
      className: "code-highlight-error",
    },
  ];
}

function renderDiagnostic(diagnostic: ProgramDiagnostic | null) {
  if (!diagnosticEl) return;
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
  heading.textContent = `${
    diagnostic.kind === "ub" ? "Undefined behavior" : "Error"
  } on line ${diagnostic.range.startLine + 1}, column ${
    diagnostic.range.startCol + 1
  }`;
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

function renderStage() {
  const stage = role<HTMLElement>("sandbox-stage");
  if (!stage) return;
  clearNode(stage);
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
  clearNode(exprResult);
  exprResult.classList.add("hidden");
  const expr = exprInput.value.trim();
  if (!expr) return;
  if (!outcome || outcome.kind !== "ok") return;
  const evaluated = simulator.evaluateExpressionText(expr, outcome.state || [], {
    allowSideEffects: false,
  });
  if ("error" in evaluated) return;
  const { result } = evaluated;
  const arrayBoxes = findArrayObjectBoxesForResult(result, outcome.state || []);
  if (arrayBoxes && arrayBoxes.length) {
    const wrap = document.createElement("div");
    appendStateObjects(wrap, arrayBoxes, { editable: false, deletable: false });
    const arrayNode = wrap.querySelector(".arraybox") as HTMLElement | null;
    if (arrayNode) {
      exprResult.appendChild(arrayNode);
      exprResult.classList.remove("hidden");
      return;
    }
  }
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
  } else if (!boxes || boxes.length === 0) {
    const msg = document.createElement("div");
    msg.className = "muted";
    msg.style.padding = "8px";
    msg.textContent = "(no variables yet)";
    grid.appendChild(msg);
  } else {
    appendStateObjects(grid, boxes, {
      editable: false,
      deletable: false,
    });
  }
  wrap.appendChild(grid);
  return wrap;
}

function refreshOtherNames() {
  const stage = role<HTMLElement>("sandbox-stage");
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
  const lines = Array.isArray(linesOverride) ? linesOverride : getRawLines();
  const count = Math.max(lines.length, 1);
  const boundary = resolveBoundary(sandbox.boundary, count);
  const safeBoundary = clampBoundary(boundary, count);
  const diagnostic = getProgramDiagnostic();
  const lineClasses = new Map<number, string[]>();
  for (let i = 0; i < count; i++) {
    if (i < safeBoundary) lineClasses.set(i, ["is-done"]);
  }
  const lineNumberClasses = new Map<number, string[]>();
  if (diagnostic) {
    lineNumberClasses.set(diagnostic.range.startLine, ["has-error"]);
  }
  const { wraps, lineHeight } = updateCodeSurface({
    editor,
    lineNumbers,
    highlightEl,
    measureEl,
    lines,
    lineClasses,
    lineNumberClasses,
    decorations: diagnosticDecoration(diagnostic),
  });
  renderDiagnostic(diagnostic);
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
  if (editor) {
    if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
  }
}

if (editor) {
  editor.addEventListener("input", () => {
    const nextText = editor.value;
    adjustBoundaryForEdit(sandbox.text || "", nextText);
    sandbox.text = nextText;
    renderStage();
    updateLineGutters();
  });
  if (lineNumbers) {
    editor.addEventListener("scroll", () => {
      lineNumbers.scrollTop = editor.scrollTop;
    });
  }
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

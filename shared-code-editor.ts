import {
  applyTextTokenReplacements,
  appendStateObjects,
  bindBtnRefPulse,
  boxValueMatchesSpec,
  clearNode,
  createStepper,
  ensurePanelizedMain,
  flashStatus,
  getNavLabelForHref,
  queryElement,
  queryRole,
  renderParts,
  setPartsContent,
  syncDocumentTitleFromNav,
} from "./shared-core.js";
import type {
  BoxState,
  Parts,
  ProgramDiagnostic,
  Stepper,
} from "./shared-core.js";
import {
  bindCodeEditorTabKey,
  ensureCodeSurfaceElements,
  updateCodeSurface,
  type CodeDecoration,
} from "./shared-code-editor-surface.js";
import { parseCValueLiteral, runCProgram } from "./shared-c-interpreter.js";
import {
  clearLevelProgress,
  currentLevelId,
  maybeRestoreLevelProgress,
  writeLevelProgress,
} from "./shared-progress.js";

type CodeEditorParts = Parts;
type CodeEditorPartsSpec =
  | CodeEditorParts
  | ((ctx: CodeEditorContext) => CodeEditorParts | null | undefined)
  | null;

interface CodeEditorElements {
  instructionsEl: HTMLElement | null;
  editor: HTMLTextAreaElement | null;
  lineNumbers: HTMLElement | null;
  stage: HTMLElement | null;
  status: HTMLElement | null;
  diagnosticEl: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
  levelResetBtn: HTMLButtonElement | null;
  nextBtn: HTMLButtonElement | null;
  codeRoot: HTMLElement | null;
}

interface CodeEditorOutcome {
  kind: "ok" | "compile" | "ub";
  state: BoxState[] | null;
}

interface CodeEditorContext {
  text: string;
  targetState: BoxState[];
  diagnostic: ProgramDiagnostic | null;
  applyUserProgram: () => BoxState[] | null;
}

interface CodeEditorConfig {
  startCode?: string;
  targetState?: BoxState[];
  textareaMinLines: number;
  allowNewLines?: boolean;
  hints?: CodeEditorPartsSpec;
  instructions?: string;
  next?: string | null;
  isLast?: boolean;
}

interface CodeEditorState {
  text: string;
  pass: boolean;
  allocBase: number | null;
}

interface CodeEditorResult {
  ok: boolean;
  outcome: CodeEditorOutcome;
}

interface CodeEditorProgress {
  text: string;
  pass: boolean;
  allocBase: number | null;
}

function collectCodeEditorElements(
  root: ParentNode = document,
): CodeEditorElements {
  const role = <T extends Element>(name: string) => queryRole<T>(name, root);
  return {
    instructionsEl: role<HTMLElement>("code-instructions"),
    editor: role<HTMLTextAreaElement>("code-editor"),
    lineNumbers: role<HTMLElement>("code-line-numbers"),
    stage: role<HTMLElement>("code-stage"),
    status: role<HTMLElement>("code-status"),
    diagnosticEl: role<HTMLElement>("code-diagnostic"),
    hintPanel: role<HTMLElement>("code-hint"),
    hintBtn: role<HTMLButtonElement>("code-hint-btn"),
    checkBtn: role<HTMLButtonElement>("code-check"),
    levelResetBtn: role<HTMLButtonElement>("code-reset-level"),
    nextBtn: queryElement<HTMLButtonElement>('button[data-stepper="next"]', root),
    codeRoot: role<HTMLElement>("code-root"),
  };
}

function ensureCodeEditorLayout(
  textareaMinLines: number,
): CodeEditorElements {
  const resolvedTitle = syncDocumentTitleFromNav();
  const existing = queryRole<HTMLElement>("code-editor");
  if (existing) return collectCodeEditorElements();

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
  codeRow.append(lineNumbers, editorWrap);
  codePane.appendChild(codeRow);
  const diagnosticEl = document.createElement("div");
  diagnosticEl.dataset.role = "code-diagnostic";
  diagnosticEl.className = "code-diagnostic hidden";
  codePanel.append(codeTitle, codePane, diagnosticEl);

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
  const levelResetBtn = document.createElement("button");
  levelResetBtn.dataset.role = "code-reset-level";
  levelResetBtn.textContent = "Reset level";
  const status = document.createElement("span");
  status.dataset.role = "code-status";
  status.className = "muted";
  const spacer = document.createElement("span");
  spacer.className = "controls-spacer";
  spacer.setAttribute("aria-hidden", "true");
  controlsRow.append(nextBtn, spacer, levelResetBtn, hintBtn, checkBtn, status);

  const hintPanel = document.createElement("div");
  hintPanel.dataset.role = "code-hint";
  hintPanel.className = "hint-inline hidden";
  actionBar.append(hintPanel, instructionsEl);

  row.append(codePanel, stateCol);
  return {
    instructionsEl,
    editor,
    lineNumbers,
    stage,
    status,
    diagnosticEl,
    hintPanel,
    hintBtn,
    checkBtn,
    levelResetBtn,
    nextBtn,
    codeRoot: section,
  };
}

function createCodeEditorTemplate(config: CodeEditorConfig): void {
  const {
    startCode = "",
    targetState = [],
    textareaMinLines,
    allowNewLines = true,
    hints = null,
    instructions = "",
    next = null,
    isLast = false,
  } = config;

  const {
    instructionsEl,
    editor,
    lineNumbers,
    stage,
    status,
    diagnosticEl,
    hintPanel,
    hintBtn,
    checkBtn,
    levelResetBtn,
    nextBtn,
    codeRoot,
  } = ensureCodeEditorLayout(textareaMinLines);
  const { highlightEl, measureEl } = ensureCodeSurfaceElements(editor);

  bindBtnRefPulse(codeRoot || document);
  const levelId = currentLevelId();
  const defaultText = normalizeEditorText(startCode);
  const restoredProgress = maybeRestoreLevelProgress<CodeEditorProgress>(levelId);
  const state: CodeEditorState = {
    text:
      typeof restoredProgress?.text === "string"
        ? normalizeEditorText(restoredProgress.text)
        : defaultText,
    pass: restoredProgress?.pass === true,
    allocBase:
      typeof restoredProgress?.allocBase === "number"
        ? restoredProgress.allocBase
        : null,
  };
  let pager: Stepper | null = null;

  const endLabel = (() => {
    if (isLast) return "Finish";
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
  ] as const;

  function normalizeEditorText(text: string): string {
    if (allowNewLines) return text;
    return text.replace(/\r\n/g, "\n").replace(/\n/g, " ");
  }

  function progressSnapshot(): CodeEditorProgress {
    return {
      text: state.text,
      pass: state.pass,
      allocBase: state.allocBase,
    };
  }

  function isDefaultProgress(snapshot: CodeEditorProgress): boolean {
    return snapshot.text === defaultText && !snapshot.pass;
  }

  function persistProgress() {
    const snapshot = progressSnapshot();
    if (isDefaultProgress(snapshot)) {
      clearLevelProgress(levelId);
      return;
    }
    writeLevelProgress(snapshot, levelId);
  }

  function setStatus(text: string, cls = "muted") {
    if (!status) return;
    status.textContent = text;
    status.className = cls;
  }

  function getEditorText() {
    return normalizeEditorText(editor?.value || state.text || "");
  }

  function getEditorLines() {
    return getEditorText().split(/\r?\n/);
  }

  function applyUserProgram(): BoxState[] | null {
    const result = runCProgram(getEditorText());
    return result.kind === "ok" ? result.state : null;
  }

  function getProgramOutcome(): CodeEditorOutcome {
    const result = runCProgram(getEditorText());
    if (result.kind !== "ok") return { kind: result.kind, state: null };
    return { kind: "ok", state: result.state };
  }

  function getProgramDiagnostic(): ProgramDiagnostic | null {
    const result = runCProgram(getEditorText());
    return result.kind === "ok" ? null : result.diagnostic;
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

  function updateLineGutters(diagnostic: ProgramDiagnostic | null = null) {
    const lines = getEditorLines();
    const lineNumberClasses = new Map<number, string[]>();
    if (diagnostic) {
      lineNumberClasses.set(diagnostic.range.startLine, ["has-error"]);
    }
    updateCodeSurface({
      editor,
      lineNumbers,
      highlightEl,
      measureEl,
      lines,
      decorations: diagnosticDecoration(diagnostic),
      lineNumberClasses,
    });
    renderDiagnostic(diagnostic);
  }

  function isTargetMatch(outcome: CodeEditorOutcome): boolean {
    if (outcome.kind !== "ok" || !outcome.state) return false;
    if (outcome.state.length !== targetState.length) return false;
    const byName = new Map(outcome.state.map((box) => [box.name, box] as const));
    for (const expected of targetState) {
      const actual = byName.get(expected.name);
      if (!actual) return false;
      if ((actual.type || "") !== (expected.type || "")) return false;
      if (!boxValueMatchesSpec(parseCValueLiteral, actual, expected).ok) return false;
    }
    return true;
  }

  function evaluate(): CodeEditorResult {
    const outcome = getProgramOutcome();
    return { ok: isTargetMatch(outcome), outcome };
  }

  function renderState(title: string, boxes: BoxState[] | null) {
    const wrap = document.createElement("div");
    wrap.className = "state-panel state-panel-scrollable";
    const heading = document.createElement("h3");
    heading.className = "panel-title state-heading";
    heading.textContent = title;
    wrap.appendChild(heading);
    const grid = document.createElement("div");
    grid.className = "grid";
    if (!boxes || !boxes.length) {
      const msg = document.createElement("div");
      msg.className = "muted";
      msg.style.padding = "8px";
      msg.textContent = "(no variables)";
      grid.appendChild(msg);
    } else {
      appendStateObjects(grid, boxes, { editable: false, deletable: false });
    }
    const body = document.createElement("div");
    body.className = "state-panel-scroll-body";
    body.appendChild(grid);
    wrap.appendChild(body);
    return wrap;
  }

  function renderStage(outcome: CodeEditorOutcome) {
    if (!stage) return;
    clearNode(stage);
    const group = document.createElement("div");
    group.className = "state-group two-col";
    group.appendChild(renderState("Your code's final state", outcome.state));
    group.appendChild(renderState("Target final state", targetState));
    stage.appendChild(group);
  }

  function partsContext(): CodeEditorContext {
    return {
      text: getEditorText(),
      targetState,
      diagnostic: getProgramDiagnostic(),
      applyUserProgram,
    };
  }

  function applyButtonTokens(parts: CodeEditorParts | null): CodeEditorParts | null {
    return applyTextTokenReplacements(parts, buttonReplacements) as CodeEditorParts | null;
  }

  function hideHint() {
    if (!hintPanel) return;
    hintPanel.classList.add("hidden");
    hintPanel.textContent = "";
  }

  function showHint(parts: CodeEditorParts) {
    if (!hintPanel) return;
    hintPanel.classList.remove("hidden");
    clearNode(hintPanel);
    renderParts(hintPanel, applyButtonTokens(parts) || []);
    flashStatus(hintPanel);
  }

  function handleHint() {
    if (state.pass) return;
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
    const diagnostic = getProgramDiagnostic();
    updateLineGutters(diagnostic);
    const outcome = getProgramOutcome();
    renderStage(outcome);
    if (instructions) setPartsContent(instructionsEl, applyButtonTokens(instructions));
    else setPartsContent(instructionsEl, []);
    if (!state.pass) {
      setStatus("", "muted");
    }
    nextBtn?.classList.remove("hidden");
    pager?.update();
    if (levelResetBtn) {
      levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
    }
    persistProgress();
  }

  if (editor) {
    bindCodeEditorTabKey(editor);
    editor.value = state.text;
    editor.addEventListener("input", () => {
      state.text = normalizeEditorText(editor.value);
      if (!allowNewLines && editor.value !== state.text) editor.value = state.text;
      hideHint();
      if (!state.pass) setStatus("", "muted");
      render();
    });
    editor.addEventListener("scroll", () => {
      if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
    });
    window.addEventListener("resize", () => updateLineGutters(getProgramDiagnostic()));
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => updateLineGutters(getProgramDiagnostic()));
      ro.observe(editor);
    }
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
      if (!result.ok) return;
      state.pass = true;
      if (editor) editor.readOnly = true;
      checkBtn.classList.add("hidden");
      hintBtn?.classList.add("hidden");
      pager?.pulseNext();
      render();
    });
  }

  if (levelResetBtn) {
    levelResetBtn.addEventListener("click", () => {
      const confirmed = window.confirm(
        "Reset your saved progress for this level and start over?",
      );
      if (!confirmed) return;
      clearLevelProgress(levelId);
      window.location.reload();
    });
  }

  pager = createStepper({
    root: codeRoot || editor?.closest(".panel") || document.body,
    lines: 0,
    nextPage: next || null,
    endLabel,
    getBoundary: () => 0,
    setBoundary: () => {},
    onAfterChange: render,
    isStepLocked: () => !state.pass,
  });

  pager.update();
  render();
}

export { createCodeEditorTemplate };

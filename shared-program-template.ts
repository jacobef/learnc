import {
  applyTextTokenReplacements,
  applyOtherNames,
  boxValueMatchesSpec,
  cloneBoxes,
  createSimpleSimulator,
  createStepper,
  bindBtnRefPulse,
  disableBoxEditing,
  ensureBaseLayout,
  flashStatus,
  getNavLabelForHref,
  isMobileViewport,
  makeAnswerBox,
  normalizeBoxValueForContext,
  normalizeZeroDisplay,
  randAddr,
  readBoxState,
  removeBoxDeleteButtons,
  renderCodePane,
  renderParts,
  resolveActiveNavItem,
  restoreWorkspace,
  serializeWorkspace,
  setPartsContent,
  typeInfo,
  vbox,
} from "./shared-core.js";
import type {
  BoxState,
  IfBlock,
  Part,
  Parts,
  StatementMap,
  StatementPart,
  StatementRange,
  Stepper,
} from "./shared-core.js";

type ProgramParts = Parts;
type ProgramHint = (ctx: ProgramContext) => Part | null | undefined;

interface ProgramWorkspaceConfig {
  showOtherNames?: boolean;
  allowVariableCreation?: boolean;
  allowVariableDeletion?: boolean;
}

interface NormalizedRunGroup {
  startLine: number;
  endLine: number;
}

interface ProgramElements {
  instructionsEl: HTMLElement | null;
  codeEl: HTMLElement | null;
  codeRoot: HTMLElement | null;
  stageEl: HTMLElement | null;
  controlsActionsEl: HTMLElement | null;
  mobileActionsEl: HTMLElement | null;
  statusEl: HTMLElement | null;
  hintPanel: HTMLElement | null;
  hintBtn: HTMLButtonElement | null;
  checkBtn: HTMLButtonElement | null;
  addBtn: HTMLButtonElement | null;
  resetBtn: HTMLButtonElement | null;
}

interface ProgramContext {
  boxes: BoxState[];
  basicHint: string | null;
  basicHintTopicIs: (
    kind: "count" | "removed" | "not-removed" | "name" | "type" | "value",
    variable?: string,
  ) => boolean;
  _basicHintTopic?: {
    kind: "count" | "removed" | "not-removed" | "name" | "type" | "value";
    variable?: string;
  } | null;
  boxNamed: (name: string) => BoxState | undefined;
  boxesNamed: (...names: string[]) => Array<BoxState | undefined>;
}

interface ProgramStep {
  code: string;
  instructions?: Part;
  hints?: ProgramHint;
  editable?: boolean;
  scrollUp?: boolean;
}

interface ProgramTemplateConfig {
  steps: ProgramStep[];
  initialInstructions?: string;
  next: string | null;
  workspace: ProgramWorkspaceConfig;
  isLast?: boolean;
}

interface ProgramTemplateState {
  boundary: number;
  passes: Record<number, boolean>;
  ws: Record<number, BoxState[] | null>;
  baseline: Record<number, BoxState[] | null>;
  branchPasses: Record<number, boolean>;
  allocBase: number | null;
  workspaceEl: HTMLElement | null;
  lastInstructionKey: string | null;
  lastBranchCorrectBoundary: number | null;
}

interface ProgramTemplateResult {
  state: ProgramTemplateState;
  pager: Stepper;
}

function collectProgramElements(root: ParentNode = document): ProgramElements {
  return {
    instructionsEl: root.querySelector(
      '[data-role="program-instructions"]',
    ) as HTMLElement | null,
    codeEl: root.querySelector(
      '[data-role="program-code"]',
    ) as HTMLElement | null,
    codeRoot: root.querySelector(
      '[data-role="program-root"]',
    ) as HTMLElement | null,
    stageEl: root.querySelector(
      '[data-role="program-stage"]',
    ) as HTMLElement | null,
    controlsActionsEl: root.querySelector(
      '[data-role="program-controls"]',
    ) as HTMLElement | null,
    mobileActionsEl: root.querySelector(
      '[data-role="program-mobile-actions"]',
    ) as HTMLElement | null,
    statusEl: root.querySelector(
      '[data-role="program-status"]',
    ) as HTMLElement | null,
    hintPanel: root.querySelector(
      '[data-role="program-hint"]',
    ) as HTMLElement | null,
    hintBtn: root.querySelector(
      '[data-role="program-hint-btn"]',
    ) as HTMLButtonElement | null,
    checkBtn: root.querySelector(
      '[data-role="program-check"]',
    ) as HTMLButtonElement | null,
    addBtn: root.querySelector(
      '[data-role="program-add"]',
    ) as HTMLButtonElement | null,
    resetBtn: root.querySelector(
      '[data-role="program-reset"]',
    ) as HTMLButtonElement | null,
  };
}

function boxNamed(boxes: BoxState[], name: string): BoxState | undefined {
  return boxes.find((box) => box.name === name);
}

function boxesNamed(
  boxes: BoxState[],
  ...names: string[]
): Array<BoxState | undefined> {
  const map = new Map<string, BoxState>();
  for (const box of boxes) {
    map.set(box.name, box);
  }
  return names.map((name) => map.get(name));
}

function ensureProgramLayout(): ProgramElements {
  const activeItem = resolveActiveNavItem();
  const resolvedTitle = activeItem?.label || "";
  const nextBrowserTitle = resolvedTitle ? `C Boxes - ${resolvedTitle}` : "";
  if (nextBrowserTitle) document.title = nextBrowserTitle;
  const existing = document.querySelector('[data-role="program-code"]');
  if (existing) return collectProgramElements();

  const { main } = ensureBaseLayout();
  main.classList.add("main-panelized");
  if (resolvedTitle) {
    const heading = document.createElement("h1");
    heading.className = "page-title";
    heading.textContent = resolvedTitle;
    main.appendChild(heading);
  }

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
  const controlsSpacer = document.createElement("span");
  controlsSpacer.className = "controls-spacer";
  controlsSpacer.setAttribute("aria-hidden", "true");
  controlsRow.appendChild(prevBtn);
  controlsRow.appendChild(nextBtn);
  controlsRow.appendChild(controlsSpacer);
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
  resetBtn.className = "hidden";
  resetBtn.textContent = "Reset";
  const statusEl = document.createElement("span");
  statusEl.dataset.role = "program-status";
  statusEl.className = "muted";
  controlsRow.appendChild(addBtn);
  controlsRow.appendChild(resetBtn);
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
    addBtn,
    resetBtn,
  };
}

function createProgramTemplate(
  config: ProgramTemplateConfig,
): ProgramTemplateResult {
  const {
    steps = [],
    initialInstructions,
    next = null,
    workspace = {},
    isLast = false,
  } = config;
  const endLabel = (() => {
    if (isLast) return "Finish";
    const label = getNavLabelForHref(next);
    return label ? `Next: ${label}` : "Next Program";
  })();
  const showOtherNames = !!(workspace && workspace.showOtherNames);
  const allowVariableDeletion = !!(
    workspace && workspace.allowVariableDeletion
  );
  const failConfig = (message: string): never => {
    alert(message);
    throw new Error(message);
  };
  const simulator = createSimpleSimulator({
    allowVarAssign: true,
    requireSourceValue: true,
  });

  const {
    instructionsEl,
    codeEl,
    codeRoot,
    stageEl,
    controlsActionsEl,
    mobileActionsEl,
    statusEl,
    hintPanel,
    hintBtn,
    checkBtn,
    addBtn,
    resetBtn,
  } = ensureProgramLayout();

  function placeActionButtonsForViewport() {
    const mobileMode = isMobileViewport() && !!mobileActionsEl;
    const target = mobileMode ? mobileActionsEl : controlsActionsEl;
    if (!target) return;
    [addBtn, resetBtn, hintBtn, checkBtn, statusEl].forEach((node) => {
      if (node && node.parentElement !== target) target.appendChild(node);
    });
  }

  function updateMobileActionsVisibility() {
    if (!mobileActionsEl) return;
    const hasVisibleAction = [checkBtn, hintBtn, addBtn, resetBtn].some(
      (btn) => !!btn && !btn.classList.contains("hidden"),
    );
    mobileActionsEl.classList.toggle("hidden", !hasVisibleAction);
  }
  placeActionButtonsForViewport();
  updateMobileActionsVisibility();
  window.addEventListener("resize", placeActionButtonsForViewport);

  bindBtnRefPulse(codeRoot || document);
  if (
    initialInstructions !== undefined &&
    typeof initialInstructions !== "string"
  ) {
    failConfig("Program initialInstructions must be a string.");
  }

  if (!Array.isArray(steps) || steps.length === 0) {
    failConfig("Program steps must be a non-empty array.");
  }

  type StepInfo = {
    index: number;
    code: string;
    lines: string[];
    startLine: number;
    endLine: number;
    boundary: number;
    instructions?: Part;
    hints?: ProgramHint;
    editable: boolean;
    scrollUp?: boolean;
    ifHeaderOnly?: boolean;
    ifHeaderHasOpenBrace?: boolean;
  };

  const stepInfos: StepInfo[] = [];
  const stepByStartLine = new Map<number, StepInfo>();
  const lineList: string[] = [];
  const canRunWithAutoClosedBlocks = (text: string): boolean => {
    const tokens = simulator.tokenizeProgram(text);
    let balance = 0;
    tokens.forEach((tok) => {
      if (tok.type !== "sym") return;
      if (tok.value === "{") balance += 1;
      else if (tok.value === "}") balance -= 1;
    });
    if (balance <= 0) return false;
    const closers = "}\n".repeat(balance);
    const patched = `${text}\n${closers}`;
    const result = simulator.applyProgram(patched);
    return Array.isArray(result);
  };
  const isIfHeaderPart = (part: StatementPart | null): boolean => {
    const tokens = part?.tokens;
    if (!tokens || tokens.length < 2) return false;
    const first = tokens[0];
    const second = tokens[1];
    if (first.type !== "kw" || first.value !== "if") return false;
    if (second.type !== "sym" || second.value !== "(") return false;
    let depth = 0;
    for (let i = 1; i < tokens.length; i++) {
      const tok = tokens[i];
      if (tok.type !== "sym") continue;
      if (tok.value === "(") depth += 1;
      if (tok.value === ")") {
        depth -= 1;
        if (depth === 0) return i === tokens.length - 1;
        if (depth < 0) return false;
      }
    }
    return false;
  };
  const isElsePart = (part: StatementPart | null): boolean => {
    const tokens = part?.tokens;
    if (!tokens || tokens.length !== 1) return false;
    const first = tokens[0];
    return first.type === "kw" && first.value === "else";
  };
  const isOpenBracePart = (part: StatementPart | null): boolean => {
    if (!part?.tokens?.length || part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    return tok.type === "sym" && tok.value === "{";
  };
  const isCloseBracePart = (part: StatementPart | null): boolean => {
    if (!part?.tokens?.length || part.tokens.length !== 1) return false;
    const tok = part.tokens[0];
    return tok.type === "sym" && tok.value === "}";
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
    const editable = step.editable === true;
    if (step.scrollUp !== undefined && typeof step.scrollUp !== "boolean") {
      failConfig(`Step ${index + 1} scrollUp must be true or false.`);
    }
    if (step.hints && typeof step.hints !== "function") {
      failConfig(`Step ${index + 1} hints must be a function.`);
    }
    const rawLines = step.code.split(/\r?\n/);
    if (rawLines[rawLines.length - 1] === "") rawLines.pop();
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
    const headerIndices: number[] = [];
    parts.forEach((part, partIndex) => {
      if (part.tokens.length && isIfHeaderPart(part)) {
        headerIndices.push(partIndex);
      }
    });
    const nonEmptyParts = parts.filter((part) => part.tokens.length > 0);
    let ifHeaderOnly = false;
    let ifHeaderHasOpenBrace = false;
    if (headerIndices.length > 0) {
      const hasElseBeforeHeader = (() => {
        const headerPartIndex = nonEmptyParts.findIndex((part) =>
          isIfHeaderPart(part),
        );
        if (headerPartIndex < 0) return false;
        const elsePartIndex = nonEmptyParts.findIndex((part) =>
          isElsePart(part),
        );
        return elsePartIndex >= 0 && elsePartIndex < headerPartIndex;
      })();
      const headerOnlyCandidate =
        headerIndices.length === 1 &&
        nonEmptyParts.every(
          (part) =>
            isIfHeaderPart(part) ||
            isOpenBracePart(part) ||
            isCloseBracePart(part) ||
            isElsePart(part),
        );
      if (headerOnlyCandidate) {
        if (
          nonEmptyParts.some((part) => isElsePart(part)) &&
          !hasElseBeforeHeader
        ) {
          failConfig(
            `Step ${index + 1} has an else-if style header that must keep $c{else} before $c{if}.`,
          );
        }
        ifHeaderOnly = true;
        ifHeaderHasOpenBrace = nonEmptyParts.some((part) =>
          isOpenBracePart(part),
        );
      } else {
        const incompleteHeader = headerIndices.some((idx) => {
          const block = ifBlocks.map.get(idx);
          if (!block) return true;
          const closeIndex =
            block.elseCloseIndex != null ? block.elseCloseIndex : block.closeIndex;
          const closeLine = Number.isFinite(parts[closeIndex]?.endLine)
            ? parts[closeIndex]!.endLine
            : block.headerEndLine;
          if (!Number.isFinite(closeLine)) return true;
          return stepStartLine + closeLine > stepEndLine;
        });
        if (incompleteHeader) {
          failConfig(
            `Step ${index + 1} contains an if statement that doesn't include its full block. Either include the full if statement in the step, or make the step only the if header (optionally with "{").`,
          );
        }
      }
    }
    for (let partIndex = 0; partIndex < parts.length; partIndex++) {
      const part = parts[partIndex];
      if (part?.tokens?.length && !part.hasSemicolon) {
        if (ifBlocks.map.has(partIndex)) continue;
        if (isIfHeaderPart(part)) continue;
        if (isElsePart(part)) continue;
        failConfig(
          `Step ${index + 1} contains an incomplete statement. Each step must be runnable on its own.`,
        );
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
      editable,
      scrollUp: step.scrollUp,
      ifHeaderOnly,
      ifHeaderHasOpenBrace,
    });
    stepByStartLine.set(startLine, stepInfos[stepInfos.length - 1]!);
  });

  if (stepInfos.some((step) => step.endLine < step.startLine)) {
    failConfig("Program steps must each contain at least one line.");
  }

  const total = lineList.length;
  const instructionMap = new Map<number, Part>();
  const scrollUpByBoundary = new Map<number, boolean>();
  const hintMap = new Map<number, ProgramHint>();
  const editableByBoundary = new Map<number, number>();
  const stepByBoundary = new Map<number, StepInfo>();
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

  const state: ProgramTemplateState = {
    boundary: 0,
    passes: {},
    ws: {},
    baseline: {},
    branchPasses: {},
    allocBase: null,
    workspaceEl: null,
    lastInstructionKey: null,
    lastBranchCorrectBoundary: null,
  };

  function allocFactory(): (type?: string) => string {
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

  function statementRangeEndingAt(
    statementMap: StatementMap,
    boundary: number,
  ): StatementRange | null {
    const endLine = boundary - 1;
    if (!Number.isFinite(endLine)) return null;
    return (
      statementMap.parts.find(
        (part) => part.endLine === endLine && part.hasSemicolon,
      ) || null
    );
  }

  function isMultiLineStatement(
    range: { startLine?: number; endLine?: number } | null,
  ) {
    const startLine = range?.startLine;
    const endLine = range?.endLine;
    if (typeof startLine !== "number" || typeof endLine !== "number")
      return false;
    if (!Number.isFinite(startLine) || !Number.isFinite(endLine)) return false;
    return endLine > startLine;
  }

  const statementMap = simulator.buildStatementMap(lineList);
  const parts = statementMap.parts || [];
  const totalLines = total;
  const ifBlocks = simulator.buildIfStatementMap(parts, {
    lastLine: Math.max(0, total - 1),
  });
  let activeBranchTargets: Map<number, number> | null = null;
  let branchSelectionActive = false;
  let branchSelectionTarget: number | null = null;
  let branchSelectionBoundary: number | null = null;
  const groupRanges: NormalizedRunGroup[] = stepInfos.map((step) => ({
    startLine: step.startLine,
    endLine: step.endLine,
  }));
  const editableSet = new Set(editableByBoundary.keys());
  const stepBoundaries = stepInfos.map((step) => step.boundary);
  const allBoundaries = Array.from({ length: totalLines + 1 }, (_, i) => i);
  const allBoundaryTargets = new Map<number, number>();
  allBoundaries.forEach((boundary) => {
    allBoundaryTargets.set(boundary, boundary);
  });
  const hasInitialInstructionsContent =
    typeof initialInstructions === "string" && initialInstructions.length > 0;

  function stopIndexForBoundary(boundary: number): number {
    const target = Math.max(0, Math.min(totalLines, boundary));
    if (!parts.length) return 0;
    const idx = parts.findIndex((part) => {
      const end = part?.endLine;
      return Number.isFinite(end) && end >= target;
    });
    return idx === -1 ? parts.length : idx;
  }

  function headerIndexForLine(lineIndex: number): number | null {
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

  function clampBoundaryLine(line: number): number {
    if (!Number.isFinite(line)) return line;
    return Math.max(0, Math.min(totalLines, line));
  }

  function lineAfterHeader(block: IfBlock): number | null {
    if (Number.isFinite(block.headerEndLine)) {
      return clampBoundaryLine(block.headerEndLine + 1);
    }
    const truePart = parts[block.trueTarget];
    if (Number.isFinite(truePart?.startLine)) {
      return clampBoundaryLine(truePart!.startLine);
    }
    return null;
  }


  function lineAfterClose(block: IfBlock): number | null {
    const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
      ? parts[block.closeIndex]!.endLine
      : block.headerEndLine;
    if (!Number.isFinite(closeLine)) return null;
    return clampBoundaryLine(closeLine + 1);
  }

  function lineForPartIndex(partIndex: number | null | undefined): number | null {
    if (partIndex == null || !Number.isFinite(partIndex)) return null;
    const part = parts[partIndex];
    if (!Number.isFinite(part?.startLine)) return null;
    return clampBoundaryLine(part!.startLine);
  }

  function lineForFalseBranch(block: IfBlock): number | null {
    if (block.elseIndex == null) return lineAfterClose(block);
    const elseEntryIndex =
      block.elseTarget ??
      block.elseOpenIndex ??
      block.elseIndex;
    const elseEntryLine = lineForPartIndex(elseEntryIndex);
    if (Number.isFinite(elseEntryLine)) return elseEntryLine;
    return lineAfterClose(block);
  }

  function branchEntryLine(
    block: IfBlock,
    branch: "true" | "false",
  ): number | null {
    if (branch === "true") return lineAfterHeader(block);
    return lineForFalseBranch(block);
  }

  function enclosingIfBlockForLine(
    lineIndex: number,
  ): {
    block: IfBlock;
    ifCloseLine: number;
    elseCloseLine: number | null;
    blockEndLine: number;
  } | null {
    if (!Number.isFinite(lineIndex)) return null;
    let chosen: {
      block: IfBlock;
      ifCloseLine: number;
      elseCloseLine: number | null;
      blockEndLine: number;
    } | null = null;
    for (const block of ifBlocks.map.values()) {
      const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
        ? parts[block.closeIndex]!.endLine
        : block.headerEndLine;
      if (!Number.isFinite(ifCloseLine)) continue;
      const elseCloseLine =
        block.elseCloseIndex != null &&
        Number.isFinite(parts[block.elseCloseIndex]?.endLine)
          ? parts[block.elseCloseIndex]!.endLine
          : null;
      const blockEndLine =
        typeof elseCloseLine === "number" ? elseCloseLine : ifCloseLine;
      if (lineIndex <= block.headerEndLine) continue;
      if (lineIndex > blockEndLine) continue;
      const span = blockEndLine - block.headerStartLine;
      if (!chosen || span < chosen.blockEndLine - chosen.block.headerStartLine) {
        chosen = { block, ifCloseLine, elseCloseLine, blockEndLine };
      }
    }
    return chosen;
  }

  function clampToBranchClose(
    current: number,
    candidate: number,
    rangeEnd?: number | null,
  ): number {
    if (!Number.isFinite(current) || !Number.isFinite(candidate))
      return candidate;
    const enclosing = enclosingIfBlockForLine(current);
    if (!enclosing) return candidate;
    const { block, ifCloseLine, elseCloseLine } = enclosing;
    const currentState = stateBeforePart(block.headerIndex);
    const condition = simulator.evaluateCondition(block.expr, currentState);
    if ("error" in condition) return candidate;
    const closeLine = condition.value
      ? ifCloseLine
      : block.elseIndex != null
        ? elseCloseLine ?? ifCloseLine
        : ifCloseLine;
    if (
      typeof closeLine === "number" &&
      Number.isFinite(closeLine) &&
      current < closeLine &&
      candidate > closeLine
    ) {
      if (
        Number.isFinite(rangeEnd) &&
        rangeEnd === closeLine &&
        candidate === closeLine + 1
      ) {
        return candidate;
      }
      return Math.min(totalLines, closeLine);
    }
    return candidate;
  }


  function normalizedStatementRange(
    lineIndex: number,
  ): { start: number; end: number } | null {
    const range = simulator.statementRangeForLine(statementMap, lineIndex);
    if (
      range &&
      typeof range.startLine === "number" &&
      typeof range.endLine === "number" &&
      Number.isFinite(range.startLine) &&
      Number.isFinite(range.endLine) &&
      range.endLine >= range.startLine
    ) {
      return { start: range.startLine, end: range.endLine };
    }
    return null;
  }

  function runRangeForBoundary(
    boundary: number,
  ): { start: number; end: number } | null {
    const lineIndex = Math.max(0, boundary);
    const groupRange = groupForLine(lineIndex);
    if (groupRange && groupRange.endLine > groupRange.startLine) {
      return { start: groupRange.startLine, end: groupRange.endLine };
    }
    const range = normalizedStatementRange(lineIndex);
    if (range && range.end > range.start) return range;
    return null;
  }

  function executedRangeForBoundary(
    boundary: number,
  ): { start: number; end: number } | null {
    if (!Number.isFinite(boundary) || boundary <= 0) return null;
    const groupRange = groupRangeEndingAt(boundary);
    if (groupRange && groupRange.endLine > groupRange.startLine) {
      return { start: groupRange.startLine, end: groupRange.endLine };
    }
    const range = normalizedStatementRange(boundary - 1);
    if (range && range.end > range.start) return range;
    return null;
  }

  stepInfos.forEach((step) => {
    const text = lineList.slice(0, step.boundary).join("\n");
    const result = simulator.applyProgram(text);
    const allowHeaderOnly =
      step.ifHeaderOnly && !step.ifHeaderHasOpenBrace;
    if (
      !Array.isArray(result) &&
      !canRunWithAutoClosedBlocks(text) &&
      !allowHeaderOnly
    ) {
      failConfig(
        `Step ${step.index + 1} cannot be run as-is. Fix the code in this step.`,
      );
    }
  });

  editableSet.forEach((step) => {
    if (Number.isFinite(step)) state.passes[step] = false;
  });

  function groupForLine(lineIndex: number): NormalizedRunGroup | null {
    if (!Number.isFinite(lineIndex)) return null;
    return (
      groupRanges.find(
        (group) => lineIndex >= group.startLine && lineIndex <= group.endLine,
      ) || null
    );
  }

  function stepEndingAtBoundary(boundary: number): StepInfo | null {
    return stepByBoundary.get(boundary) || null;
  }

  function branchStepInfo(boundary: number): StepInfo | null {
    const step = stepEndingAtBoundary(boundary);
    if (!step?.ifHeaderOnly) return null;
    return step;
  }

  function isHeaderReachable(headerIndex: number): boolean {
    if (!Number.isFinite(headerIndex)) return false;
    for (const parent of ifBlocks.map.values()) {
      if (parent.headerIndex === headerIndex) continue;
      const parentEnd = parent.elseCloseIndex ?? parent.closeIndex;
      if (headerIndex <= parent.headerIndex || headerIndex > parentEnd) {
        continue;
      }
      const inTrueBranch =
        headerIndex >= parent.openIndex && headerIndex <= parent.closeIndex;
      const inElseBranch =
        parent.elseIndex != null &&
        headerIndex >= parent.elseIndex &&
        headerIndex <= parentEnd;
      if (!inTrueBranch && !inElseBranch) continue;
      const parentState = stateBeforePart(parent.headerIndex);
      const parentCondition = simulator.evaluateCondition(parent.expr, parentState);
      if ("error" in parentCondition) continue;
      if (inTrueBranch && !parentCondition.value) return false;
      if (inElseBranch && parentCondition.value) return false;
    }
    return true;
  }

  function branchInfoForLabelBoundary(boundary: number): {
    rangeStart: number;
    rangeEnd: number;
    targets: number[];
    targetMap: Map<number, number>;
    expected: number | null;
  } | null {
    const step = branchStepInfo(boundary);
    if (!step) return null;
    const headerIndex = headerIndexForLine(step.startLine);
    if (headerIndex == null || !isHeaderReachable(headerIndex)) return null;
    return branchInfoForBoundary(boundary);
  }

  function branchInfoForBoundary(boundary: number): {
    rangeStart: number;
    rangeEnd: number;
    targets: number[];
    targetMap: Map<number, number>;
    expected: number | null;
  } | null {
    const step = stepEndingAtBoundary(boundary);
    if (!step?.ifHeaderOnly) return null;
    const headerIndex = headerIndexForLine(step.startLine);
    if (headerIndex == null) return null;
    const block = ifBlocks.map.get(headerIndex);
    if (!block) return null;
    const trueLine = branchEntryLine(block, "true");
    const falseLine = branchEntryLine(block, "false");
    const maxLine = Math.max(0, totalLines);
    const targets: number[] = [];
    const targetMap = new Map<number, number>();
    const addTarget = (boundaryIndex: number) => {
      if (!Number.isFinite(boundaryIndex)) return;
      if (boundaryIndex < 0 || boundaryIndex > totalLines) return;
      if (!targets.includes(boundaryIndex)) targets.push(boundaryIndex);
      targetMap.set(boundaryIndex, boundaryIndex);
    };
    if (Number.isFinite(trueLine)) {
      const trueTarget = Math.max(0, Math.min(maxLine, trueLine!));
      addTarget(trueTarget);
    }
    if (Number.isFinite(falseLine)) {
      const falseTarget = Math.max(0, Math.min(maxLine, falseLine!));
      addTarget(falseTarget);
    }
    let expected: number | null = null;
    const currentState = stateBeforePart(headerIndex);
    const condition = simulator.evaluateCondition(block.expr, currentState);
    if (!("error" in condition)) {
      const nextLine = condition.value ? trueLine : falseLine;
      if (Number.isFinite(nextLine)) {
        expected = Math.max(0, Math.min(maxLine, nextLine!));
      }
    }
    return {
      rangeStart: step.startLine,
      rangeEnd: step.endLine,
      targets,
      targetMap,
      expected,
    };
  }

  function nextStepBoundary(boundary: number): number {
    for (const b of stepBoundaries) {
      if (b > boundary) return b;
    }
    return totalLines;
  }

  function groupRangeEndingAt(boundary: number): NormalizedRunGroup | null {
    const endLine = boundary - 1;
    if (!Number.isFinite(endLine)) return null;
    return groupRanges.find((group) => group.endLine === endLine) || null;
  }

  function stateBeforePart(partIndex: number): BoxState[] {
    const safeIndex = Math.max(0, Math.min(parts.length, partIndex));
    const alloc = allocFactory();
    const result = simulator.applyProgramParts(parts, {
      alloc,
      stop: safeIndex,
    });
    return Array.isArray(result) ? result : [];
  }

  function branchSkipLineForBoundary(boundary: number): number | null {
    if (!Number.isFinite(boundary)) return null;
    for (const block of ifBlocks.map.values()) {
      if (block.elseIndex == null) continue;
      const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
        ? parts[block.closeIndex]!.endLine
        : block.headerEndLine;
      if (!Number.isFinite(ifCloseLine)) continue;
      const elseCloseLine =
        block.elseCloseIndex != null &&
        Number.isFinite(parts[block.elseCloseIndex]?.endLine)
          ? parts[block.elseCloseIndex]!.endLine
          : null;
      const fallbackClose =
        typeof elseCloseLine === "number" ? elseCloseLine : ifCloseLine;
      const afterPart = parts[block.afterIndex];
      const afterLine = Number.isFinite(afterPart?.startLine)
        ? afterPart!.startLine
        : fallbackClose + 1;
      if (afterLine !== boundary) continue;
      const currentState = stateBeforePart(block.headerIndex);
      const condition = simulator.evaluateCondition(block.expr, currentState);
      if ("error" in condition) continue;
      if (condition.value) return ifCloseLine;
      return null;
    }
    return null;
  }

  function skipElseBoundaryForLine(boundary: number): number | null {
    if (!Number.isFinite(boundary)) return null;
    const lineIndex = boundary;
    for (const block of ifBlocks.map.values()) {
      if (block.elseIndex == null || block.elseCloseIndex == null) continue;
      const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
        ? parts[block.closeIndex]!.endLine
        : block.headerEndLine;
      const elseStartLine = Number.isFinite(parts[block.elseIndex]?.startLine)
        ? parts[block.elseIndex]!.startLine
        : null;
      const elseOpenLine = Number.isFinite(
        parts[block.elseOpenIndex ?? -1]?.startLine,
      )
        ? parts[block.elseOpenIndex ?? -1]!.startLine
        : null;
      const elseCloseLine = Number.isFinite(parts[block.elseCloseIndex]?.endLine)
        ? parts[block.elseCloseIndex]!.endLine
        : null;
      if (elseStartLine == null || elseCloseLine == null) continue;
      let skipStartLine = elseStartLine;
      if (
        typeof ifCloseLine === "number" &&
        Number.isFinite(ifCloseLine) &&
        elseStartLine === ifCloseLine
      ) {
        const base = Number.isFinite(elseOpenLine)
          ? elseOpenLine!
          : elseStartLine;
        skipStartLine = base + 1;
      }
      if (lineIndex < skipStartLine || lineIndex > elseCloseLine) continue;
      const currentState = stateBeforePart(block.headerIndex);
      const condition = simulator.evaluateCondition(block.expr, currentState);
      if ("error" in condition) continue;
      if (!condition.value) return null;
      const afterPart = parts[block.afterIndex];
      const afterLine = Number.isFinite(afterPart?.startLine)
        ? afterPart!.startLine
        : Math.min(totalLines, elseCloseLine + 1);
      return clampBoundaryLine(afterLine);
    }
    return null;
  }

  function getExpectedState(boundary: number): BoxState[] {
    const stopIndex = stopIndexForBoundary(boundary);
    const alloc = allocFactory();
    const result = simulator.applyProgramParts(parts, {
      alloc,
      stop: stopIndex,
    });
    return Array.isArray(result) ? result : [];
  }

  function decorateState(boxes: BoxState[]): BoxState[] {
    return cloneBoxes(boxes || []);
  }

  function normalizeState(
    list: BoxState[],
  ): Array<{ name: string; type: string; value: string; address: string }> {
    if (!Array.isArray(list)) return [];
    return list
      .map((b) => ({
        name: (b.name || "").trim(),
        type: (b.type || "").trim(),
        value: normalizeZeroDisplay((b.value ?? "").trim()),
        address: (b.address ?? "").trim(),
      }))
      .sort((a, b) => {
        if (a.name === b.name) return a.address.localeCompare(b.address);
        return a.name.localeCompare(b.name);
      });
  }

  function statesEqual(a: BoxState[], b: BoxState[]) {
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

  function ensureBaseline(boundary: number, defaults: BoxState[]): BoxState[] {
    if (!state.baseline[boundary]) {
      state.baseline[boundary] = cloneBoxes(defaults || []);
    }
    return state.baseline[boundary];
  }

  function getWorkspaceEl(): HTMLElement | null {
    return (
      state.workspaceEl ||
      (stageEl?.querySelector?.(
        '[data-role="workspace"]',
      ) as HTMLElement | null)
    );
  }

  function updateResetVisibility(boundary: number) {
    if (!resetBtn) return;
    if (!editableSet.has(boundary) || state.passes[boundary]) {
      resetBtn.classList.add("hidden");
      return;
    }
    const baseline = state.baseline[boundary];
    const current = serializeWorkspace(getWorkspaceEl()) || [];
    const changed = Array.isArray(baseline) && !statesEqual(current, baseline);
    resetBtn.classList.toggle("hidden", !changed);
  }

  function attachResetWatcher(wrap: HTMLElement | null, boundary: number) {
    if (!wrap) return;
    const refresh = () => {
      updateResetVisibility(boundary);
    };
    wrap.addEventListener("input", refresh);
    wrap.addEventListener("click", () => {
      setTimeout(refresh, 0);
    });
    refresh();
  }

  function nextWorkspaceAddress(
    wrap: HTMLElement | null,
    type: string = "int",
  ): string {
    if (!wrap) return String(randAddr(type || "int"));
    const used = new Set<string>();
    let maxAddr: number | null = null;
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

  function refreshOtherNames() {
    if (!showOtherNames) return;
    applyOtherNames(stageEl, { onToggle: refreshOtherNames });
  }

  function boxesEqual(actual: BoxState[], expected: BoxState[]) {
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
      if (!boxValueMatchesSpec(simulator, act, exp).ok) return false;
    }
    return true;
  }

  function defaultsForBoundary(boundary: number): BoxState[] {
    if (!editableSet.has(boundary))
      return decorateState(getExpectedState(boundary));
    const groupRange = groupRangeEndingAt(boundary);
    if (groupRange && Number.isFinite(groupRange.startLine)) {
      return decorateState(
        getExpectedState(groupRange.startLine),
      );
    }
    const range = statementRangeEndingAt(statementMap, boundary);
    if (range && Number.isFinite(range.startLine)) {
      return decorateState(
        getExpectedState(range.startLine),
      );
    }
    return decorateState(
      getExpectedState(Math.max(0, boundary - 1)),
    );
  }

  function setStatus(text: string, cls: string = "muted") {
    if (!statusEl) return;
    statusEl.textContent = text;
    statusEl.className = cls;
  }


  function buttonReplacements(runLabel: string) {
    const resolvedLabel = runLabel || "Run line";
    return [
      ["$runLineButton", `$b{${resolvedLabel}}`],
      ["$backButton", "$b{Back ◀}"],
      ["$checkButton", "$b{Check}"],
      ["$resetButton", "$b{Reset}"],
      ["$newVariableButton", "$b{+ New variable}"],
      ["$showAliasesButton", "$b{Show aliases}"],
    ] as const;
  }

  function applyButtonTokens(
    parts: ProgramParts | null,
    runLabel: string,
  ): ProgramParts | null {
    return applyTextTokenReplacements(parts, buttonReplacements(runLabel)) as
      | ProgramParts
      | null;
  }

  function formatRunLabel(
    start: number,
    end: number,
    withArrow: boolean,
    verb: "Run" | "Solve" | "Branch from" = "Run",
  ) {
    if (start === end) {
      return `${verb} line ${start}${withArrow ? " ▶" : ""}`;
    }
    return `${verb} lines ${start}-${end}${withArrow ? " ▶" : ""}`;
  }

  function runLabelForBoundary(boundary: number): string {
    const nextStep = nextStepBoundary(boundary);
    const needsSolve =
      editableSet.has(nextStep) && !state.passes[nextStep];
    const verb: "Run" | "Solve" = needsSolve ? "Solve" : "Run";
    const branchInfo =
      branchInfoForLabelBoundary(boundary) ||
      branchInfoForLabelBoundary(nextStep);
    if (branchInfo) {
      const start = branchInfo.rangeStart + 1;
      const end = branchInfo.rangeEnd + 1;
      return formatRunLabel(start, end, true, "Branch from");
    }
    const afterSkippedElse = branchSkipLineForBoundary(boundary);
    if (Number.isFinite(afterSkippedElse)) {
      const lineNumber = Math.max(1, Math.min(totalLines, boundary + 1));
      return `${verb} line ${lineNumber} ▶`;
    }
    const skipElseBoundary = skipElseBoundaryForLine(boundary);
    if (
      Number.isFinite(skipElseBoundary) &&
      skipElseBoundary !== boundary
    ) {
      const lineNumber = Math.max(
        1,
        Math.min(totalLines, skipElseBoundary! + 1),
      );
      return `${verb} line ${lineNumber} ▶`;
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

  function nextBoundaryForLine(current: number): number {
    if (!Number.isFinite(current)) return current + 1;
    if (current >= totalLines) return totalLines;
    const solvedBranchStep = branchStepInfo(current);
    if (
      solvedBranchStep?.editable &&
      state.branchPasses[solvedBranchStep.startLine]
    ) {
      const solvedBranch = branchInfoForBoundary(current);
      if (
        solvedBranch &&
        Number.isFinite(solvedBranch.expected) &&
        solvedBranch.expected! > current
      ) {
        return clampToBranchClose(
          current,
          Math.max(0, Math.min(totalLines, solvedBranch.expected!)),
        );
      }
    }
    const skipElseBoundary = skipElseBoundaryForLine(current);
    if (
      Number.isFinite(skipElseBoundary) &&
      skipElseBoundary !== current
    ) {
      return skipElseBoundary!;
    }
    const logNext = null;
    const closeBlock = (() => {
      for (const block of ifBlocks.map.values()) {
        const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
          ? parts[block.closeIndex]!.endLine
          : null;
        const elseCloseLine =
          block.elseCloseIndex != null &&
          Number.isFinite(parts[block.elseCloseIndex]?.endLine)
            ? parts[block.elseCloseIndex]!.endLine
            : null;
        if (ifCloseLine === current || elseCloseLine === current) {
          return { block, ifCloseLine, elseCloseLine };
        }
      }
      return null;
    })();
    if (closeBlock) {
      const { block, ifCloseLine, elseCloseLine } = closeBlock;
      const currentState = stateBeforePart(block.headerIndex);
      const condition = simulator.evaluateCondition(block.expr, currentState);
      if (!("error" in condition)) {
        const afterPart = parts[block.afterIndex];
        const fallbackClose =
          block.elseCloseIndex != null && elseCloseLine != null
            ? elseCloseLine
            : ifCloseLine;
        const safeFallbackClose =
          typeof fallbackClose === "number" && Number.isFinite(fallbackClose)
            ? fallbackClose
            : current;
        const afterLine = Number.isFinite(afterPart?.startLine)
          ? afterPart!.startLine
          : Math.min(totalLines, safeFallbackClose + 1);
        if (
          block.elseIndex != null &&
          ifCloseLine === current &&
          condition.value
        ) {
          return clampToBranchClose(
            current,
            Math.min(totalLines, Math.max(0, afterLine)),
          );
        }
        if (elseCloseLine === current && !condition.value) {
          return clampToBranchClose(
            current,
            Math.min(totalLines, Math.max(0, afterLine)),
          );
        }
      }
    }
    const range = simulator.statementRangeForLine(statementMap, current);
    const rangeStart =
      typeof range?.startLine === "number" ? range.startLine : current;
    const rangeEnd =
      typeof range?.endLine === "number" ? range.endLine : current;
    void logNext;
    const groupRange = groupForLine(current);
    const branchStep = !!branchStepInfo(current);
    if (
      groupRange &&
      !branchStep &&
      Number.isFinite(groupRange.endLine) &&
      groupRange.endLine > current
    ) {
      return clampToBranchClose(
        current,
        Math.min(totalLines, groupRange.endLine + 1),
        groupRange.endLine,
      );
    }
    const headerIndex = headerIndexForLine(rangeStart);
    if (headerIndex == null) {
      if (
        groupRange &&
        Number.isFinite(groupRange.endLine) &&
        groupRange.endLine > current
      ) {
        return clampToBranchClose(
          current,
          Math.min(totalLines, groupRange.endLine + 1),
          groupRange.endLine,
        );
      }
      if (Number.isFinite(rangeEnd) && rangeEnd > current)
        return clampToBranchClose(
          current,
          Math.min(totalLines, rangeEnd + 1),
          rangeEnd,
        );
      return clampToBranchClose(
        current,
        Math.min(totalLines, current + 1),
      );
    }
    const block = ifBlocks.map.get(headerIndex);
    if (!block) {
      if (
        groupRange &&
        Number.isFinite(groupRange.endLine) &&
        groupRange.endLine > current
      ) {
        return clampToBranchClose(
          current,
          Math.min(totalLines, groupRange.endLine + 1),
          groupRange.endLine,
        );
      }
      if (Number.isFinite(rangeEnd) && rangeEnd > current)
        return clampToBranchClose(
          current,
          Math.min(totalLines, rangeEnd + 1),
          rangeEnd,
        );
      return clampToBranchClose(
        current,
        Math.min(totalLines, current + 1),
      );
    }
    const currentState = stateBeforePart(headerIndex);
    const condition = simulator.evaluateCondition(block.expr, currentState);
    const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
      ? parts[block.closeIndex]!.endLine
      : block.headerEndLine;
    const elseCloseLine =
      block.elseCloseIndex != null &&
      Number.isFinite(parts[block.elseCloseIndex]?.endLine)
        ? parts[block.elseCloseIndex]!.endLine
        : null;
    if ("error" in condition || condition.value) {
      let effectiveRangeEnd = rangeEnd;
      if (
        Number.isFinite(ifCloseLine) &&
        Number.isFinite(effectiveRangeEnd) &&
        current < ifCloseLine &&
        effectiveRangeEnd > ifCloseLine
      ) {
        effectiveRangeEnd = ifCloseLine;
      }
      if (
        groupRange &&
        Number.isFinite(groupRange.endLine) &&
        groupRange.endLine > current
      ) {
        return clampToBranchClose(
          current,
          Math.min(totalLines, groupRange.endLine + 1),
          groupRange.endLine,
        );
      }
      const trueLine = branchEntryLine(block, "true");
      if (Number.isFinite(trueLine))
        return clampToBranchClose(
          current,
          Math.min(totalLines, Math.max(0, trueLine!)),
        );
      if (Number.isFinite(effectiveRangeEnd) && effectiveRangeEnd > current)
        return clampToBranchClose(
          current,
          Math.min(totalLines, effectiveRangeEnd + 1),
          effectiveRangeEnd,
        );
      return clampToBranchClose(
        current,
        Math.min(totalLines, current + 1),
      );
    }
    if (block.elseIndex != null) {
      const falseLine = branchEntryLine(block, "false");
      if (Number.isFinite(falseLine))
        return clampToBranchClose(
          current,
          Math.min(totalLines, Math.max(0, falseLine!)),
        );
    }
    let effectiveRangeEnd = rangeEnd;
    if (
      typeof elseCloseLine === "number" &&
      Number.isFinite(elseCloseLine) &&
      Number.isFinite(effectiveRangeEnd) &&
      current < elseCloseLine &&
      effectiveRangeEnd > elseCloseLine
    ) {
      effectiveRangeEnd = elseCloseLine;
    }
    if (Number.isFinite(effectiveRangeEnd) && effectiveRangeEnd > current)
      return clampToBranchClose(
        current,
        Math.min(totalLines, effectiveRangeEnd + 1),
        effectiveRangeEnd,
      );
    const closeLine = lineAfterClose(block);
    if (!Number.isFinite(closeLine))
      return clampToBranchClose(
        current,
        Math.min(totalLines, current + 1),
      );
    return clampToBranchClose(
      current,
      Math.min(totalLines, closeLine!),
    );
  }

  function prevBoundaryForLine(current: number): number {
    if (!Number.isFinite(current)) return current - 1;
    if (current <= 0) return current - 1;
    let boundary = 0;
    let prev = 0;
    let guard = 0;
    while (boundary < current && guard < totalLines + 5) {
      prev = boundary;
      const next = nextBoundaryForLine(boundary);
      boundary = next === boundary ? boundary + 1 : next;
      guard += 1;
    }
    if (boundary === current) return prev;
    return current - 1;
  }

  function formatNameList(names: string[]): string {
    const tokens = names.map((name) => `$n{${name}}`);
    if (tokens.length === 1) return tokens[0] || "";
    if (tokens.length === 2) return `${tokens[0]} and ${tokens[1]}`;
    return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
  }

  function baselineForBoundary(boundary: number): BoxState[] {
    const baseline = state.baseline[boundary];
    if (Array.isArray(baseline)) return baseline;
    return defaultsForBoundary(boundary);
  }

  function basicHintForBoxes(
    boxes: BoxState[],
    boundary: number,
  ): {
    message: string;
    kind: "count" | "removed" | "not-removed" | "name" | "type" | "value";
    variable?: string;
  } | null {
    const actual = Array.isArray(boxes) ? boxes : [];
    const expected = getExpectedState(boundary);
    const actualCount = actual.length;
    const expectedCount = expected.length;
    const nameOf = (box: BoxState | null | undefined) =>
      String(box?.name || "").trim();
    const typeOf = (box: BoxState | null | undefined) =>
      String(box?.type || "").trim();
    const expectedNames = expected.map(nameOf).filter(Boolean);
    const expectedNameSet = new Set(expectedNames);
    const actualNames = actual.map(nameOf);
    const actualNameSet = new Set(actualNames.filter(Boolean));
    const missingExpectedNames = expectedNames.filter(
      (name) => !actualNameSet.has(name),
    );
    const baselineAtBoundary = baselineForBoundary(boundary);
    const baselineNames = new Set(
      baselineAtBoundary.map(nameOf).filter(Boolean),
    );

    const removedName = missingExpectedNames.find((name) =>
      baselineNames.has(name),
    );
    if (removedName) {
      return {
        message: `This line shouldn't remove the $n{${removedName}} variable.`,
        kind: "removed",
        variable: removedName,
      };
    }

    const extraBaselineNames = actualNames.filter(
      (name) => name && baselineNames.has(name) && !expectedNameSet.has(name),
    );
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
      const expectedNewNames = expectedNames.filter(
        (name) => !baselineNames.has(name),
      );
      if (expectedNewNames.length > 1) {
        return {
          message: `The new variables should be named ${formatNameList(
            expectedNewNames,
          )}.`,
          kind: "name",
          variable: expectedNewNames[0],
        };
      }
      const expectedName = expectedNewNames[0] || missingExpectedNames[0] || "";
      if (!expectedName) return null;
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
      } else if (statementRange && isMultiLineStatement(statementRange)) {
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

    const baselineByName = new Map<string, BoxState>();
    baselineAtBoundary.forEach((box) => {
      const name = nameOf(box);
      if (name && !baselineByName.has(name)) baselineByName.set(name, box);
    });
    const actualByName = new Map<string, BoxState>();
    actual.forEach((box) => {
      const name = nameOf(box);
      if (name && !actualByName.has(name)) actualByName.set(name, box);
    });

    let deferredBe: {
      message: string;
      kind: "value";
      variable: string;
    } | null = null;
    for (const exp of expected) {
      const name = nameOf(exp);
      if (!name) continue;
      const act = actualByName.get(name);
      if (!act) continue;
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
        const label =
          expVal === "" ? "empty" : `$v{${normalizeZeroDisplay(expVal)}}`;
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

  function partsContext({
    boxes,
  }: {
    boxes?: BoxState[];
  } = {}): ProgramContext {
    const resolvedBoxes = boxes ?? [];
    const normalizedBoxes = resolvedBoxes.map((box) =>
      normalizeBoxValueForContext(simulator, box),
    );
    const topic = basicHintForBoxes(normalizedBoxes, state.boundary);
    return {
      boxes: normalizedBoxes,
      basicHint: topic?.message ?? null,
      basicHintTopicIs: (kind, variable) =>
        !!topic &&
        topic.kind === kind &&
        (variable === undefined || topic.variable === variable),
      _basicHintTopic: topic,
      boxNamed: (name) => boxNamed(resolvedBoxes, name),
      boxesNamed: (...names) => boxesNamed(resolvedBoxes, ...names),
    };
  }

  function resolveParts(
    spec: ((ctx: ProgramContext) => Part | null | undefined) | null,
    ctx: ProgramContext,
  ): ProgramParts | null {
    if (!spec) return null;
    const resolved = spec(ctx);
    if (!resolved) return null;
    return String(resolved);
  }

  function getInstructionParts(boundary: number): ProgramParts | null {
    if (boundary === 0 && hasInitialInstructionsContent) {
      return initialInstructions || "";
    }
    const entry = instructionMap.get(boundary) ?? null;
    return entry ? String(entry) : null;
  }

  function getHintParts(ctx: ProgramContext): ProgramParts | null {
    const entry = hintMap.get(state.boundary) ?? null;
    return resolveParts(entry, ctx);
  }

  function solvedBranchInstructionForBoundary(boundary: number): Part | null {
    for (const step of stepInfos) {
      if (!step.ifHeaderOnly || !step.instructions) continue;
      if (!state.branchPasses[step.startLine]) continue;
      if (boundary === step.boundary) {
        return step.instructions;
      }
      const info = branchInfoForBoundary(step.boundary);
      if (
        info &&
        Number.isFinite(info.expected) &&
        boundary === info.expected
      ) {
        return step.instructions;
      }
    }
    return null;
  }

  function renderCodePaneForBoundary() {
    if (!codeEl) return;
    const key = state.boundary;
    const branchStep = branchStepInfo(key);
    const branchStepEditable =
      !!branchStep?.editable && !state.branchPasses[branchStep.startLine];
    let progress =
      editableSet.has(key) &&
      !state.passes[key] &&
      !branchStepEditable;
    let progressRange: [number, number] | undefined;
    let progressIndex: number | undefined;
    let doneBoundary: number | undefined;
    const branchInfo = branchInfoForBoundary(key);
    const branchSelectable =
      !!branchInfo && !!branchStep?.editable && branchSelectionActive;
    activeBranchTargets = branchSelectable ? allBoundaryTargets : null;
    if (progress) {
      const range = executedRangeForBoundary(key);
      const hasMultiLineRange =
        !!range &&
        Number.isFinite(range.start) &&
        Number.isFinite(range.end) &&
        range.end > range.start;
      if (range) {
        progressRange = [range.start, range.end];
        progressIndex = range.end;
        doneBoundary = range.start;
      }
      const branchSkipLine = branchSkipLineForBoundary(key);
      if (Number.isFinite(branchSkipLine) && !hasMultiLineRange) {
        progressRange = [branchSkipLine!, branchSkipLine!];
        progressIndex = branchSkipLine!;
        doneBoundary = branchSkipLine!;
      }
    } else if (
      branchStepEditable &&
      branchSelectionActive &&
      branchSelectionBoundary === key
    ) {
      const branchInfo = branchInfoForBoundary(key);
      if (branchInfo) {
        progress = true;
        progressRange = [branchInfo.rangeStart, branchInfo.rangeEnd];
        progressIndex = undefined;
      }
    }
    let strikeRanges: Array<[number, number]> = [];
    let strikeFragments: Array<{ line: number; start: number; end: number }> =
      [];
    const elseColumnForLine = (lineIndex: number, block: IfBlock): number => {
      const rawLine = lineList[lineIndex] || "";
      const elseTok = parts[block.elseIndex ?? -1]?.tokens?.[0];
      if (Number.isFinite(elseTok?.col)) {
        const col = Number(elseTok!.col);
        if (col >= 0 && col <= rawLine.length) return col;
      }
      const direct = rawLine.indexOf("else");
      if (direct >= 0) return direct;
      const match = rawLine.search(/\belse\b/);
      if (match >= 0) return match;
      const braceIdx = rawLine.indexOf("}");
      if (braceIdx >= 0) {
        let col = braceIdx + 1;
        while (col < rawLine.length && /\s/.test(rawLine[col])) col += 1;
        return col < rawLine.length ? col : -1;
      }
      return -1;
    };
    const headerStrikeStartForLine = (lineIndex: number, block: IfBlock): number => {
      const rawLine = lineList[lineIndex] || "";
      const headerTok = parts[block.headerIndex]?.tokens?.[0];
      const ifCol = Number.isFinite(headerTok?.col)
        ? Number(headerTok!.col)
        : -1;
      if (ifCol > 0 && ifCol <= rawLine.length) {
        let foundElseCol = -1;
        const re = /\belse\b/g;
        let match: RegExpExecArray | null = null;
        while ((match = re.exec(rawLine)) !== null) {
          if (match.index >= ifCol) break;
          foundElseCol = match.index;
        }
        if (foundElseCol >= 0) return foundElseCol;
      }
      return 0;
    };
    ifBlocks.map.forEach((block) => {
      if (key <= block.headerEndLine) return;
      const headerGroup = groupForLine(block.headerStartLine);
      const headerStep = headerGroup
        ? (stepByStartLine.get(headerGroup.startLine) ?? null)
        : null;
      if (
        headerStep?.editable &&
        !state.passes[headerStep.boundary] &&
        key <= headerStep.boundary
      ) {
        return;
      }
      const allowStrike =
        !headerStep?.editable ||
        !headerStep.ifHeaderOnly ||
        !!state.branchPasses[headerStep.startLine];
      if (!allowStrike) return;
      const currentState = stateBeforePart(block.headerIndex);
      const condition = simulator.evaluateCondition(block.expr, currentState);
      if ("error" in condition) return;
      const headerLine = block.headerStartLine;
      const headerText = lineList[headerLine] || "";
      const blockElseLine =
        block.elseIndex != null &&
        Number.isFinite(parts[block.elseIndex]?.startLine)
          ? parts[block.elseIndex]!.startLine
          : null;
      if (headerText.includes("else") && blockElseLine === headerLine) {
        const elseCol = elseColumnForLine(headerLine, block);
        if (Number.isFinite(elseCol) && elseCol >= 0) {
          strikeFragments.push({
            line: headerLine,
            start: condition.value ? elseCol : 0,
            end: condition.value ? headerText.length : elseCol,
          });
        }
        return;
      }
      const ifCloseLine = Number.isFinite(parts[block.closeIndex]?.endLine)
        ? parts[block.closeIndex]!.endLine
        : block.headerEndLine;
      const ifOpenLine = Number.isFinite(parts[block.openIndex]?.startLine)
        ? parts[block.openIndex]!.startLine
        : block.headerEndLine;
      if (!condition.value) {
        if (
          Number.isFinite(ifOpenLine) &&
          Number.isFinite(headerLine) &&
          ifOpenLine > headerLine
        ) {
          const headerStart = headerStrikeStartForLine(headerLine, block);
          strikeFragments.push({
            line: headerLine,
            start: headerStart,
            end: headerText.length,
          });
        }
        if (
          block.elseIndex != null &&
          block.elseOpenIndex != null &&
          ifCloseLine ===
            (Number.isFinite(parts[block.elseIndex]?.startLine)
              ? parts[block.elseIndex]!.startLine
              : ifCloseLine)
        ) {
          const elseCol = elseColumnForLine(ifCloseLine, block);
          if (Number.isFinite(elseCol) && elseCol >= 0) {
            strikeFragments.push({
              line: ifCloseLine,
              start: 0,
              end: elseCol,
            });
          }
          if (
            Number.isFinite(ifOpenLine) &&
            Number.isFinite(ifCloseLine) &&
            ifCloseLine > ifOpenLine
          ) {
            strikeRanges.push([ifOpenLine, ifCloseLine - 1]);
          }
        } else if (Number.isFinite(ifOpenLine) && Number.isFinite(ifCloseLine)) {
          strikeRanges.push([ifOpenLine, ifCloseLine]);
        }
        return;
      }
      if (block.elseIndex == null || block.elseCloseIndex == null) return;
      const rawElseStart = Number.isFinite(
        parts[block.elseOpenIndex ?? block.elseIndex]?.startLine,
      )
        ? parts[block.elseOpenIndex ?? block.elseIndex]!.startLine
        : ifCloseLine;
      const elseStartLine =
        rawElseStart === ifCloseLine ? rawElseStart + 1 : rawElseStart;
      const elseCloseLine = Number.isFinite(
        parts[block.elseCloseIndex]?.endLine,
      )
        ? parts[block.elseCloseIndex]!.endLine
        : elseStartLine;
      if (
        Number.isFinite(elseStartLine) &&
        Number.isFinite(elseCloseLine) &&
        elseStartLine <= elseCloseLine
      ) {
        const lineHasElse = (lineList[ifCloseLine] || "").includes("else");
        const sameLine = lineHasElse || elseStartLine === ifCloseLine;
        if (sameLine) {
          const elseCol = elseColumnForLine(ifCloseLine, block);
          if (Number.isFinite(elseCol) && elseCol >= 0) {
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
      suppressProgressMid: branchStepEditable && branchSelectionActive,
      boundaryTargets: branchSelectable,
      strikeRanges,
      strikeFragments,
    });
    ensureCodeLineVisible(key);
  }

  function ensureCodeLineVisible(lineIndex: number) {
    if (!codeEl) return;
    if (!Number.isFinite(lineIndex)) return;
    if (lineIndex < 0 || lineIndex >= lineList.length) return;
    const range = runRangeForBoundary(lineIndex);
    const startIndex = range ? range.start : lineIndex;
    const endIndex = range ? range.end : lineIndex;
    const lines = codeEl.querySelectorAll(".line");
    const startEl = lines[startIndex] as HTMLElement | undefined;
    const endEl = lines[endIndex] as HTMLElement | undefined;
    if (!startEl || !endEl) return;
    const container = codeEl as HTMLElement;
    const containerRect = container.getBoundingClientRect();
    const startRect = startEl.getBoundingClientRect();
    const endRect = endEl.getBoundingClientRect();
    const lineTop = startRect.top - containerRect.top + container.scrollTop;
    const lineBottom = endRect.bottom - containerRect.top + container.scrollTop;
    const viewTop = container.scrollTop;
    const viewBottom = viewTop + container.clientHeight;
    if (lineTop < viewTop) {
      container.scrollTop = Math.max(0, Math.floor(lineTop));
      return;
    }
    if (lineBottom > viewBottom) {
      container.scrollTop = Math.max(
        0,
        Math.ceil(lineBottom - container.clientHeight),
      );
    }
  }

  function ensureNewVariableVisible(node: HTMLElement | null) {
    if (!node) return;
    requestAnimationFrame(() => {
      const stateContainer = stageEl as HTMLElement | null;
      const edgePad = 16;
      if (stateContainer && !isMobileViewport()) {
        stateContainer.scrollTo({
          top: stateContainer.scrollHeight,
          behavior: "smooth",
        });
        return;
      }
      const canScrollState =
        !!stateContainer &&
        stateContainer.scrollHeight > stateContainer.clientHeight + 1;
      if (stateContainer && canScrollState) {
        const containerRect = stateContainer.getBoundingClientRect();
        const nodeRect = node.getBoundingClientRect();
        const nodeTop =
          nodeRect.top - containerRect.top + stateContainer.scrollTop;
        const nodeBottom = nodeTop + nodeRect.height;
        const viewTop = stateContainer.scrollTop + edgePad;
        const viewBottom = viewTop + stateContainer.clientHeight - edgePad * 2;
        if (nodeTop < viewTop) {
          stateContainer.scrollTop = Math.max(0, Math.floor(nodeTop - edgePad));
          return;
        }
        if (nodeBottom > viewBottom) {
          stateContainer.scrollTop = Math.max(
            0,
            Math.ceil(nodeBottom - stateContainer.clientHeight + edgePad),
          );
          return;
        }
      }

      const rect = node.getBoundingClientRect();
      const viewTop = edgePad;
      const viewBottom = window.innerHeight - edgePad;
      if (rect.top < viewTop || rect.bottom > viewBottom) {
        node.scrollIntoView({ block: "nearest", behavior: "smooth" });
      }
    });
  }

  function renderStage() {
    if (!stageEl) return;
    stageEl.innerHTML = "";

    const key = state.boundary;
    const editStep = stepEndingAtBoundary(key);
    const editable =
      !!editStep?.editable &&
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
      } else {
        expected.forEach((b) => {
          const node = vbox({
            address: b.address ?? "—",
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
      const target = event?.target as HTMLElement | null;
      if (target?.closest?.(".delete")) {
        requestAnimationFrame(refreshOtherNames);
      }
    });
  }

  function scrollInstructionsUpIfNeeded(
    scrollUp: boolean,
    instructionKey: string | null,
  ) {
    if (!scrollUp || !instructionKey || instructionKey === state.lastInstructionKey)
      return;
    requestAnimationFrame(() => {
      const rect = instructionsEl?.getBoundingClientRect();
      if (!rect) return;
      const offset = 24;
      const top = Math.max(0, rect.top + window.scrollY - offset);
      if (top < window.scrollY) {
        window.scrollTo({ top, behavior: "smooth" });
      }
    });
  }

  function updateInstructions() {
    const key = state.boundary;
    const runLabel = runLabelForBoundary(state.boundary);
    const scrollUp =
      scrollUpByBoundary.get(key) !== false && !!instructionsEl;
    let instructionKey: string | null = null;
    if (key === total && state.passes[key]) {
      setPartsContent(instructionsEl, "Program solved!");
      instructionKey = "__solved__";
      scrollInstructionsUpIfNeeded(scrollUp, instructionKey);
      state.lastInstructionKey = instructionKey;
      return;
    }
    let parts = getInstructionParts(key);
    if (Array.isArray(parts) && parts.length === 0) parts = null;
    if (!parts) {
      const step = stepByStartLine.get(key);
      if (step?.instructions) {
        if (step.ifHeaderOnly) {
          if (branchSelectionActive && branchSelectionBoundary === key) {
            parts = String(step.instructions);
          }
        } else {
          const nextBoundary = nextBoundaryForLine(key);
          if (nextBoundary > step.boundary) {
            parts = String(step.instructions);
          }
        }
      }
    }
    if (!parts) {
      const solvedBranchInstructions = solvedBranchInstructionForBoundary(key);
      if (solvedBranchInstructions) {
        parts = String(solvedBranchInstructions);
      }
    }
    if (key === 0) {
      instructionKey = "__initial__";
    } else {
      const spec = instructionMap.get(key) ?? null;
      instructionKey = spec ? String(spec) : null;
    }
    parts = applyButtonTokens(parts || null, runLabel);
    setPartsContent(instructionsEl, parts);
    scrollInstructionsUpIfNeeded(scrollUp, instructionKey);
    state.lastInstructionKey = instructionKey;
  }

  function hideHint() {
    if (!hintPanel) return;
    hintPanel.textContent = "";
    hintPanel.classList.add("hidden");
  }

  function readWorkspaceBoxes(): BoxState[] {
    const ws = getWorkspaceEl();
    if (!ws) return [];
    return [...ws.querySelectorAll(".vbox")]
      .map((v) => readBoxState(v))
      .filter(Boolean) as BoxState[];
  }

  function evaluateWorkspace(boxes: BoxState[]) {
    const expected = getExpectedState(state.boundary);
    return { ok: boxesEqual(boxes, expected), expected };
  }

  function showHint(parts: ProgramParts | null, runLabel: string) {
    if (!hintPanel) return;
    if (!parts || (Array.isArray(parts) && parts.length === 0)) return;
    const rendered = applyButtonTokens(parts, runLabel);
    renderParts(hintPanel, rendered || "");
    hintPanel.classList.remove("hidden");
    flashStatus(hintPanel);
  }

  function isLooksGoodParts(parts: ProgramParts | null) {
    if (typeof parts === "string") {
      return parts.trim() === "Looks good. Press $checkButton.";
    }
    return false;
  }

  function render() {
    if (
      branchSelectionActive &&
      branchSelectionBoundary !== state.boundary
    ) {
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
    const branchSelectionHere =
      branchSelectionActive && branchSelectionBoundary === key;
    const hasSolvedEditable = !!editStep?.editable && editPass;
    const normalEditable =
      !!editStep?.editable && !editPass && !editStep?.ifHeaderOnly;
    const branchInfo = branchInfoForBoundary(key);
    const branchEditable =
      branchSelectionActive &&
      branchSelectionBoundary === key &&
      !!branchInfo &&
      !!branchStep?.editable &&
      !branchSolved;
    if (statusEl) {
      if (hasSolvedEditable && !branchSelectionHere) {
        setStatus("correct", "ok");
      } else if (normalEditable) {
        setStatus("", "muted");
      } else if (branchStep) {
        setStatus(branchSolved ? "correct" : "", branchSolved ? "ok" : "muted");
      } else if (editableSet.has(key)) {
        setStatus("correct", "ok");
      } else {
        setStatus("", "muted");
      }
    }
    if (
      state.lastBranchCorrectBoundary !== null &&
      state.lastBranchCorrectBoundary === key &&
      statusEl
    ) {
      setStatus("correct", "ok");
      state.lastBranchCorrectBoundary = null;
    }
    if (checkBtn)
      checkBtn.classList.toggle(
        "hidden",
        !normalEditable && !branchEditable,
      );
    const branchStepActive =
      branchSelectionActive && branchSelectionBoundary === key;
    if (hintBtn)
      hintBtn.classList.toggle("hidden", !normalEditable || branchStepActive);
    if (addBtn)
      addBtn.classList.toggle(
        "hidden",
        !normalEditable ||
          !workspace.allowVariableCreation ||
          branchStepActive,
      );
    if (resetBtn) resetBtn.classList.add("hidden");
    updateMobileActionsVisibility();
    updateInstructions();
    if (normalEditable && resetBtn) updateResetVisibility(key);
  }

  function save() {
    const key = state.boundary;
    if (!editableSet.has(key)) return;
    const snapshot = serializeWorkspace(getWorkspaceEl());
    if (Array.isArray(snapshot)) {
      state.ws[key] = snapshot;
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
      ensureNewVariableVisible(node);
      updateResetVisibility(state.boundary);
      refreshOtherNames();
    });
  }

  if (resetBtn) {
    resetBtn.addEventListener("click", () => {
      const key = state.boundary;
      if (!editableSet.has(key)) return;
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
      if (
        branchSelectionActive &&
        branchSelectionBoundary !== key
      ) {
        branchSelectionActive = false;
        branchSelectionBoundary = null;
        branchSelectionTarget = null;
      }
      const branchInfo = branchInfoForBoundary(key);
      const branchStep = branchStepInfo(key);
      const branchSolved = branchStep
        ? !!state.branchPasses[branchStep.startLine]
        : false;
      const branchEditable =
        branchSelectionActive &&
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
        if (branchInfo.expected != null) {
          const expectedBoundary = branchInfo.expected;
          const expectedLine = expectedBoundary - 1;
          const expectedStep = stepEndingAtBoundary(expectedBoundary);
          const expectedPart = parts.find(
            (part) =>
              part.startLine === expectedLine &&
              part.endLine === expectedLine,
          );
          const isElseHeader =
            expectedPart?.tokens?.length === 1 &&
            expectedPart.tokens[0]?.type === "kw" &&
            expectedPart.tokens[0]?.value === "else";
          if (isElseHeader && expectedStep && !expectedStep.editable) {
            state.passes[expectedBoundary] = true;
            ensureBaseline(
              expectedBoundary,
              defaultsForBoundary(expectedBoundary),
            );
          }
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
        state.lastBranchCorrectBoundary = Math.max(
          0,
          Math.min(totalLines, branchInfo.expected),
        );
        state.boundary = Math.max(
          0,
          Math.min(totalLines, branchInfo.expected),
        );
        render();
        pager.update();
        pager.pulseNext();
        return;
      }
      if (!editableSet.has(key)) return;
      const boxes = readWorkspaceBoxes();
      const result = evaluateWorkspace(boxes);
      const ok = result.ok;
      setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
      flashStatus(statusEl);
      if (!ok) return;
      state.passes[key] = true;
      state.ws[key] = boxes;
      const ws = getWorkspaceEl();
      if (ws) {
        ws.querySelectorAll(".vbox").forEach((v) => disableBoxEditing(v));
        removeBoxDeleteButtons(ws);
      }
      if (checkBtn) checkBtn.classList.add("hidden");
      if (hintBtn) hintBtn.classList.add("hidden");
      if (addBtn) addBtn.classList.add("hidden");
      if (resetBtn) resetBtn.classList.add("hidden");
      const skipBoundary = skipElseBoundaryForLine(key);
      if (
        Number.isFinite(skipBoundary) &&
        skipBoundary !== key
      ) {
        state.boundary = skipBoundary!;
      }
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
      const runLabel = runLabelForBoundary(state.boundary);
      if (result.ok) {
        showHint("Looks good. Press $checkButton.", runLabel);
        return;
      }
      const parts = getHintParts(ctx);
      if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.",
          runLabel,
        );
        return;
      }
      if (isLooksGoodParts(parts)) {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.",
          runLabel,
        );
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
        if (solved) return false;
        return branchSelectionActive && branchSelectionBoundary === boundary;
      }
      if (editStep?.editable) {
        return !state.passes[boundary];
      }
      return false;
    },
    getStepBadge: () => {
      const nextBoundary = nextStepBoundary(state.boundary);
      if (!editableSet.has(nextBoundary)) return "";
      return state.passes[nextBoundary] ? "check" : "note";
    },
    getNextLabel: (boundary) => {
      const atEnd = boundary >= totalLines;
      if (atEnd) return endLabel;
      if (
        branchSelectionActive &&
        branchSelectionBoundary === boundary
      ) {
        return "???";
      }
      const nextBoundary = nextStepBoundary(boundary);
      const needsSolve =
        editableSet.has(nextBoundary) && !state.passes[nextBoundary];
      const verb: "Run" | "Solve" = needsSolve ? "Solve" : "Run";
      const branchInfo = branchInfoForLabelBoundary(nextBoundary);
      if (branchInfo) {
        const start = branchInfo.rangeStart + 1;
        const end = branchInfo.rangeEnd + 1;
        return formatRunLabel(start, end, false, "Branch from");
      }
      const afterSkippedElse = branchSkipLineForBoundary(boundary);
      if (Number.isFinite(afterSkippedElse)) {
        const lineNumber = Math.max(1, Math.min(totalLines, boundary + 1));
        return `${verb} line ${lineNumber}`;
      }
      const skipElseBoundary = skipElseBoundaryForLine(boundary);
      if (
        Number.isFinite(skipElseBoundary) &&
        skipElseBoundary !== boundary
      ) {
        const lineNumber = Math.max(
          1,
          Math.min(totalLines, skipElseBoundary! + 1),
        );
        return `${verb} line ${lineNumber}`;
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
      if (
        stepNext > current &&
        stepNext <= next &&
        editableSet.has(stepNext) &&
        !state.passes[stepNext]
      ) {
        return stepNext;
      }
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
      if (
        branchSelectionActive &&
        branchSelectionBoundary === current
      ) {
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
      if (!activeBranchTargets) return;
      const target = event?.target as HTMLElement | null;
      const boundaryEl = target?.closest?.(".boundary.selectable") as
        | HTMLElement
        | null;
      if (!boundaryEl) return;
      const boundaryStr = boundaryEl.dataset.boundary;
      if (!boundaryStr) return;
      const boundaryIndex = Number(boundaryStr);
      if (!Number.isFinite(boundaryIndex)) return;
      if (!activeBranchTargets.has(boundaryIndex)) return;
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

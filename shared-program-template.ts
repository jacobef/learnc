import {
  applyTextTokenReplacements,
  applyOtherNames,
  appendStateObjects,
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
  removeBoxDeleteButtons,
  renderCodePane,
  renderParts,
  resolveActiveNavItem,
  restoreWorkspace,
  serializeWorkspace,
  setPartsContent,
  typeInfo,
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
type ProgramInstructionSpec = Part | Part[];
type ProgramHintSpec = ProgramHint | ProgramHint[];
type ProgramEditableSpec = boolean | boolean[];

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
  instructions?: ProgramInstructionSpec;
  hints?: ProgramHintSpec;
  editable?: ProgramEditableSpec;
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
  executionSteps: number;
  allocBase: number | null;
  workspaceEl: HTMLElement | null;
  lastInstructionKey: string | null;
  lastRenderedStateCount: number | null;
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
  const simulator = createSimpleSimulator();

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
    [resetBtn, addBtn, hintBtn, checkBtn, statusEl].forEach((node) => {
      if (node && node.parentElement !== target) target.appendChild(node);
    });
    if (!hintPanel) return;
    if (mobileMode && mobileActionsEl) {
      if (hintPanel.parentElement !== mobileActionsEl) {
        mobileActionsEl.appendChild(hintPanel);
      }
      return;
    }
    const desktopHintParent = instructionsEl?.parentElement ?? null;
    if (
      desktopHintParent &&
      (hintPanel.parentElement !== desktopHintParent ||
        hintPanel.nextElementSibling !== instructionsEl)
    ) {
      desktopHintParent.insertBefore(hintPanel, instructionsEl);
    }
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
    instructions?: ProgramInstructionSpec;
    hints?: ProgramHintSpec;
    editable: ProgramEditableSpec;
    canBeEditable: boolean;
    ifHeaderOnly?: boolean;
    ifHeaderHasOpenBrace?: boolean;
    whileHeaderOnly?: boolean;
    whileHeaderHasOpenBrace?: boolean;
  };

  const stepInfos: StepInfo[] = [];
  const lineList: string[] = [];
  const canRunWithStepBudget = (text: string): boolean => {
    const tokens = simulator.tokenizeProgram(text);
    const localParts = simulator.splitStatements(tokens);
    const stepBudget = Math.max(1, localParts.length * 8);
    const trace = simulator.traceProgramParts(localParts, {
      stopSteps: stepBudget,
    });
    return !!trace;
  };
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
    return canRunWithStepBudget(patched);
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
  const isWhileHeaderPart = (part: StatementPart | null): boolean => {
    const tokens = part?.tokens;
    if (!tokens || tokens.length < 2) return false;
    const first = tokens[0];
    const second = tokens[1];
    if (first.type !== "kw" || first.value !== "while") return false;
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
  const isInstructionSpec = (
    value: ProgramInstructionSpec | undefined,
  ): value is ProgramInstructionSpec =>
    typeof value === "string" ||
    (Array.isArray(value) && value.every((item) => typeof item === "string"));
  const isHintSpec = (
    value: ProgramHintSpec | undefined,
  ): value is ProgramHintSpec =>
    typeof value === "function" ||
    (Array.isArray(value) && value.every((item) => typeof item === "function"));
  const isEditableSpec = (
    value: ProgramEditableSpec | undefined,
  ): value is ProgramEditableSpec =>
    typeof value === "boolean" ||
    (Array.isArray(value) && value.every((item) => typeof item === "boolean"));
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
    if (
      step.instructions !== undefined &&
      !isInstructionSpec(step.instructions)
    ) {
      failConfig(
        `Step ${index + 1} instructions must be a string or an array of strings.`,
      );
    }
    if (step.hints !== undefined && !isHintSpec(step.hints)) {
      failConfig(
        `Step ${index + 1} hints must be a function or an array of functions.`,
      );
    }
    if (step.editable !== undefined && !isEditableSpec(step.editable)) {
      failConfig(
        `Step ${index + 1} editable must be true/false or an array of true/false values.`,
      );
    }
    const editable: ProgramEditableSpec = step.editable ?? false;
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
    const whileBlocks = simulator.buildWhileStatementMap(parts, {
      lastLine: Math.max(0, rawLines.length - 1),
    });
    const headerIndices: number[] = [];
    const whileHeaderIndices: number[] = [];
    parts.forEach((part, partIndex) => {
      if (part.tokens.length && isIfHeaderPart(part)) {
        headerIndices.push(partIndex);
      }
      if (part.tokens.length && isWhileHeaderPart(part)) {
        whileHeaderIndices.push(partIndex);
      }
    });
    const nonEmptyParts = parts.filter((part) => part.tokens.length > 0);
    let ifHeaderOnly = false;
    let ifHeaderHasOpenBrace = false;
    let whileHeaderOnly = false;
    let whileHeaderHasOpenBrace = false;
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
    if (whileHeaderIndices.length > 0) {
      const headerOnlyCandidate =
        whileHeaderIndices.length === 1 &&
        nonEmptyParts.every(
          (part) =>
            isWhileHeaderPart(part) ||
            isOpenBracePart(part) ||
            isCloseBracePart(part),
        );
      if (headerOnlyCandidate) {
        whileHeaderOnly = true;
        whileHeaderHasOpenBrace = nonEmptyParts.some((part) =>
          isOpenBracePart(part),
        );
      } else {
        const incompleteHeader = whileHeaderIndices.some((idx) => {
          const block = whileBlocks.map.get(idx);
          if (!block) return true;
          const closeLine = Number.isFinite(parts[block.closeIndex]?.endLine)
            ? parts[block.closeIndex]!.endLine
            : block.headerEndLine;
          if (!Number.isFinite(closeLine)) return true;
          return stepStartLine + closeLine > stepEndLine;
        });
        if (incompleteHeader) {
          failConfig(
            `Step ${index + 1} contains a while statement that doesn't include its full block. Either include the full while statement in the step, or make the step only the while header (optionally with "{").`,
          );
        }
      }
    }
    for (let partIndex = 0; partIndex < parts.length; partIndex++) {
      const part = parts[partIndex];
      if (part?.tokens?.length && !part.hasSemicolon) {
        if (ifBlocks.map.has(partIndex)) continue;
        if (whileBlocks.map.has(partIndex)) continue;
        if (isIfHeaderPart(part)) continue;
        if (isWhileHeaderPart(part)) continue;
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
      canBeEditable: Array.isArray(editable)
        ? editable.some((value) => value)
        : editable === true,
      ifHeaderOnly,
      ifHeaderHasOpenBrace,
      whileHeaderOnly,
      whileHeaderHasOpenBrace,
    });
  });

  if (stepInfos.some((step) => step.endLine < step.startLine)) {
    failConfig("Program steps must each contain at least one line.");
  }

  const total = lineList.length;
  const stepByLine: Array<StepInfo | null> = new Array(total).fill(null);
  stepInfos.forEach((step) => {
    for (let line = step.startLine; line <= step.endLine && line < total; line++) {
      if (line >= 0) stepByLine[line] = step;
    }
  });

  function stepForLine(line: number): StepInfo | null {
    if (!Number.isFinite(line)) return null;
    const safeLine = Math.max(0, Math.min(total - 1, Math.floor(line)));
    return stepByLine[safeLine] ?? null;
  }

  const state: ProgramTemplateState = {
    boundary: 0,
    executionSteps: -1,
    allocBase: null,
    workspaceEl: null,
    lastInstructionKey: null,
    lastRenderedStateCount: null,
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
  const whileBlocks = simulator.buildWhileStatementMap(parts, {
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
  const allBoundaries = Array.from({ length: totalLines + 1 }, (_, i) => i);
  const allBoundaryTargets = new Map<number, number>();
  allBoundaries.forEach((boundary) => {
    allBoundaryTargets.set(boundary, boundary);
  });
  const hasInitialInstructionsContent =
    typeof initialInstructions === "string" && initialInstructions.length > 0;
  const MAX_RUNTIME_TRACE_STEPS = 2000;
  type RuntimeTrace = NonNullable<ReturnType<typeof simulator.traceProgramParts>>;
  type RuntimeStage = {
    index: number;
    traceIndex: number;
    partIndex: number;
    runLine: number;
    runEndLine: number;
    beforeBoundary: number;
    afterBoundary: number;
    stateAfter: BoxState[];
    step: StepInfo | null;
    stepVisitIndex: number | null;
    stepEditable: boolean;
    instructions?: Part;
    hints?: ProgramHint;
    editableMode: "none" | "state" | "boundary";
    interactionBoundary: number | null;
    expectedBoundary: number | null;
    branchTargets: number[];
    expectedState: BoxState[] | null;
    baselineState: BoxState[] | null;
  };
  const runtimeTraceByStep: RuntimeTrace[] = [];
  const runtimeStages: RuntimeStage[] = [];
  let runtimeLatestSolvedStage = -1;
  const runtimeWorkspaceByStage = new Map<number, BoxState[] | null>();
  const stepStartLines = stepInfos
    .map((step) => step.startLine)
    .sort((a, b) => a - b);
  const stepReachCounts = new Array(stepInfos.length).fill(0);
  let lastRuntimeStageStepIndex: number | null = null;
  const resolveEditableForVisit = (
    stepInfo: StepInfo,
    visitIndex: number,
  ): boolean => {
    const { editable } = stepInfo;
    if (Array.isArray(editable)) {
      return editable[visitIndex] === true;
    }
    return editable === true;
  };
  const resolveInstructionsForVisit = (
    stepInfo: StepInfo,
    visitIndex: number,
  ): Part | undefined => {
    const { instructions } = stepInfo;
    if (Array.isArray(instructions)) {
      return instructions[visitIndex];
    }
    return instructions;
  };
  const resolveHintsForEditableVisit = (
    stepInfo: StepInfo,
    editableVisitIndex: number | null,
  ): ProgramHint | undefined => {
    const { hints } = stepInfo;
    if (Array.isArray(hints)) {
      if (editableVisitIndex == null) return undefined;
      return hints[editableVisitIndex];
    }
    return hints;
  };
  const resolveStepVisitForStage = (
    stepInfo: StepInfo | null,
  ): {
    stepVisitIndex: number | null;
    editableVisitIndex: number | null;
    stepEditable: boolean;
    instructions: Part | undefined;
    hints: ProgramHint | undefined;
  } => {
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
    let editableVisitIndex: number | null = null;
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
  const executablePartStartLines = new Set<number>();
  parts.forEach((part) => {
    if (!Number.isFinite(part?.startLine)) return;
    executablePartStartLines.add(clampBoundaryLine(part.startLine));
  });

  function pushRuntimeStage(stage: Omit<RuntimeStage, "index">) {
    runtimeStages.push({
      index: runtimeStages.length,
      ...stage,
    });
  }

  function boundaryForPartIndex(partIndex: number): number {
    if (!Number.isFinite(partIndex) || partIndex >= parts.length) return totalLines;
    const part = parts[Math.max(0, Math.floor(partIndex))];
    if (!part || !Number.isFinite(part.startLine)) return totalLines;
    return clampBoundaryLine(part.startLine);
  }

  for (let step = 0; step <= MAX_RUNTIME_TRACE_STEPS; step++) {
    const alloc = allocFactory();
    const trace = simulator.traceProgramParts(parts, {
      alloc,
      stopSteps: step,
    });
    if (!trace) {
      failConfig(
        `Program trace failed at step ${step + 1}. Check the flow in this level.`,
      );
    }
    const runtimeTrace = trace as RuntimeTrace;
    runtimeTraceByStep.push(runtimeTrace);
    if (runtimeTrace.nextIndex >= parts.length) break;
  }
  const finalTrace = runtimeTraceByStep[runtimeTraceByStep.length - 1] || null;
  if (!finalTrace || finalTrace.nextIndex < parts.length) {
    failConfig(
      `Program exceeds ${MAX_RUNTIME_TRACE_STEPS} execution steps. Reduce loop iterations in this level.`,
    );
  }
  const buildRuntimeStage = ({
    traceIndex,
    partIndex,
    runLine,
    stepInfo,
    stepVisitIndex,
    stepEditable,
    instructions,
    hints,
    beforeBoundary,
    afterBoundary,
    stateBefore,
    stateAfter,
    forceStateEditable = false,
  }: {
    traceIndex: number;
    partIndex: number;
    runLine: number;
    stepInfo: StepInfo | null;
    stepVisitIndex: number | null;
    stepEditable: boolean;
    instructions?: Part;
    hints?: ProgramHint;
    beforeBoundary: number;
    afterBoundary: number;
    stateBefore: BoxState[];
    stateAfter: BoxState[];
    forceStateEditable?: boolean;
  }): Omit<RuntimeStage, "index"> => {
    let resolvedAfterBoundary = afterBoundary;
    const ifBlock = ifBlocks.map.get(partIndex);
    if (ifBlock) {
      const condition = simulator.evaluateCondition(ifBlock.expr, stateBefore || []);
      if (!("error" in condition)) {
        const branchBoundary = condition.value
          ? branchEntryLine(ifBlock, "true")
          : branchEntryLine(ifBlock, "false");
        if (Number.isFinite(branchBoundary)) {
          resolvedAfterBoundary = clampBoundaryLine(Number(branchBoundary));
        }
      }
    }
    const rawRunEnd =
      partIndex >= 0 && Number.isFinite(parts[partIndex]?.endLine)
        ? Number(parts[partIndex]!.endLine)
        : Number.isFinite(stepInfo?.endLine)
          ? Number(stepInfo!.endLine)
          : runLine;
    const maxLineIndex = Math.max(0, totalLines - 1);
    const runEndLine = Math.max(
      Math.max(0, Math.min(maxLineIndex, runLine)),
      Math.max(0, Math.min(maxLineIndex, rawRunEnd)),
    );
    let editableMode: RuntimeStage["editableMode"] = "none";
    let interactionBoundary: number | null = null;
    let expectedBoundary: number | null = null;
    let branchTargets: number[] = [];
    let expectedState: BoxState[] | null = null;
    let baselineState: BoxState[] | null = null;
    const stageExitsStep =
      !!stepInfo &&
      Number.isFinite(stepInfo.boundary) &&
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
            if (Number.isFinite(trueLine)) {
              branchTargets.push(Math.max(0, Math.min(totalLines, Number(trueLine))));
            }
            if (Number.isFinite(falseLine)) {
              branchTargets.push(Math.max(0, Math.min(totalLines, Number(falseLine))));
            }
          }
        } else if (stepInfo.ifHeaderOnly) {
          const block = ifBlocks.map.get(partIndex);
          if (block) {
            const trueLine = branchEntryLine(block, "true");
            const falseLine = branchEntryLine(block, "false");
            if (Number.isFinite(trueLine)) {
              branchTargets.push(Math.max(0, Math.min(totalLines, Number(trueLine))));
            }
            if (Number.isFinite(falseLine)) {
              branchTargets.push(Math.max(0, Math.min(totalLines, Number(falseLine))));
            }
          }
        }
        branchTargets = [...new Set(branchTargets)];
        if (
          Number.isFinite(expectedBoundary) &&
          !branchTargets.includes(expectedBoundary!)
        ) {
          branchTargets.push(expectedBoundary!);
        }
      } else if (forceStateEditable || (partIndex >= 0 && stageExitsStep)) {
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
  const appendNoOpStagesInRange = ({
    fromBoundary,
    toBoundary,
    traceIndex,
    stateSnapshot,
  }: {
    fromBoundary: number;
    toBoundary: number;
    traceIndex: number;
    stateSnapshot: BoxState[];
  }) => {
    if (!Number.isFinite(fromBoundary) || !Number.isFinite(toBoundary)) return;
    if (toBoundary <= fromBoundary) return;
    const noOpStarts = stepStartLines
      .filter((startLine) => startLine >= fromBoundary && startLine < toBoundary)
      .filter((startLine) => !executablePartStartLines.has(startLine))
      .sort((a, b) => a - b);
    for (let insertIndex = 0; insertIndex < noOpStarts.length; insertIndex++) {
      const startLine = noOpStarts[insertIndex]!;
      const nextBoundary =
        insertIndex + 1 < noOpStarts.length
          ? noOpStarts[insertIndex + 1]!
          : toBoundary;
      const stepInfo = stepForLine(startLine);
      const resolvedVisit = resolveStepVisitForStage(stepInfo);
      pushRuntimeStage(
        buildRuntimeStage({
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
        }),
      );
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
  let pendingGroupedEditable:
    | {
        step: StepInfo;
        entryBoundary: number;
        entryState: BoxState[];
      }
    | null = null;
  for (let index = 0; index < runtimeTraceByStep.length - 1; index++) {
    const before = runtimeTraceByStep[index];
    const after = runtimeTraceByStep[index + 1];
    const partIndex = before.nextIndex;
    const part = parts[partIndex];
    if (!part || !Number.isFinite(part.startLine)) continue;
    const runLine = clampBoundaryLine(part.startLine);
    const beforeBoundary = boundaryForPartIndex(before.nextIndex);
    const rawAfterBoundary = boundaryForPartIndex(after.nextIndex);
    const isSequentialAdvance = after.nextIndex === partIndex + 1;
    const immediateAfterBoundary = clampBoundaryLine(
      (Number.isFinite(part.endLine) ? Number(part.endLine) : runLine) + 1,
    );
    const stageAfterBoundary = isSequentialAdvance
      ? immediateAfterBoundary
      : rawAfterBoundary;
    const stepInfo = stepForLine(runLine);
    const groupedEditableStep =
      !!stepInfo && stepInfo.canBeEditable && !isHeaderOnlyStep(stepInfo);
    if (groupedEditableStep && stepInfo) {
      if (!pendingGroupedEditable || pendingGroupedEditable.step.index !== stepInfo.index) {
        pendingGroupedEditable = {
          step: stepInfo,
          entryBoundary: beforeBoundary,
          entryState: cloneBoxes(before.state || []),
        };
      }
      const nextPart = parts[after.nextIndex];
      const nextRunLine = Number.isFinite(nextPart?.startLine)
        ? clampBoundaryLine(Number(nextPart!.startLine))
        : null;
      const nextStepInfo = Number.isFinite(nextRunLine)
        ? stepForLine(Number(nextRunLine))
        : null;
      const groupedStepCompleted =
        !nextStepInfo || nextStepInfo.index !== stepInfo.index;
      if (groupedStepCompleted) {
        const grouped = pendingGroupedEditable;
        const resolvedVisit = resolveStepVisitForStage(grouped.step);
        pushRuntimeStage(
          buildRuntimeStage({
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
          }),
        );
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
    pushRuntimeStage(
      buildRuntimeStage({
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
      }),
    );
    if (!isSequentialAdvance) continue;
    const noOpStartBoundary = stepInfo ? stepInfo.boundary : runLine + 1;
    appendNoOpStagesInRange({
      fromBoundary: noOpStartBoundary,
      toBoundary: rawAfterBoundary,
      traceIndex: index + 1,
      stateSnapshot: cloneBoxes(after.state || []),
    });
  }
  stepInfos.forEach((stepInfo) => {
    const reachedCount = stepReachCounts[stepInfo.index] || 0;
    if (
      Array.isArray(stepInfo.instructions) &&
      stepInfo.instructions.length !== reachedCount
    ) {
      failConfig(
        `Step ${stepInfo.index + 1} instructions array length (${stepInfo.instructions.length}) must match the number of times the step is reached (${reachedCount}).`,
      );
    }
    if (Array.isArray(stepInfo.editable) && stepInfo.editable.length !== reachedCount) {
      failConfig(
        `Step ${stepInfo.index + 1} editable array length (${stepInfo.editable.length}) must match the number of times the step is reached (${reachedCount}).`,
      );
    }
    const editableReachedCount = Array.isArray(stepInfo.editable)
      ? stepInfo.editable.slice(0, reachedCount).filter((value) => value).length
      : stepInfo.editable
        ? reachedCount
        : 0;
    if (
      Array.isArray(stepInfo.hints) &&
      stepInfo.hints.length !== editableReachedCount
    ) {
      failConfig(
        `Step ${stepInfo.index + 1} hints array length (${stepInfo.hints.length}) must match the number of times the step is reached as editable (${editableReachedCount}).`,
      );
    }
  });
  console.log("[cBoxes] precomputed stages", runtimeStages);

  function runtimeMaxStep(): number {
    return runtimeStages.length - 1;
  }

  function runtimeStepClamp(stepCount: number): number {
    const safeStep = Math.floor(stepCount);
    return Math.max(-1, Math.min(runtimeMaxStep(), safeStep));
  }

  function runtimeTraceForStage(stepCount: number): RuntimeTrace | null {
    if (!runtimeTraceByStep.length) return null;
    const safeStep = runtimeStepClamp(stepCount);
    if (safeStep < 0) return runtimeTraceByStep[0] || null;
    const stage = runtimeStageAt(safeStep);
    if (!stage) return runtimeTraceByStep[runtimeTraceByStep.length - 1] || null;
    const traceIndex = Math.max(
      0,
      Math.min(runtimeTraceByStep.length - 1, stage.traceIndex + 1),
    );
    return runtimeTraceByStep[traceIndex] || null;
  }

  function runtimeStageAt(stepCount: number): RuntimeStage | null {
    const exactStep = Math.floor(stepCount);
    if (!Number.isFinite(exactStep)) return null;
    if (exactStep < 0) return null;
    if (exactStep > runtimeMaxStep()) return null;
    return runtimeStages[exactStep] || null;
  }

  function runtimeStageNeedsSolve(stage: RuntimeStage | null): boolean {
    if (!stage || stage.editableMode === "none") return false;
    return stage.index > runtimeLatestSolvedStage;
  }

  function runtimeStageSolved(stage: RuntimeStage | null): boolean {
    if (!stage) return false;
    if (stage.editableMode === "none") return true;
    return !runtimeStageNeedsSolve(stage);
  }

  function runtimeBoundaryForSteps(stepCount: number): number {
    const stage = runtimeStageAt(stepCount);
    if (!stage) return 0;
    if (stage.editableMode === "boundary" && runtimeStageNeedsSolve(stage)) {
      return stage.interactionBoundary ?? stage.afterBoundary;
    }
    return stage.afterBoundary;
  }

  function syncBoundaryFromStage() {
    state.boundary = runtimeBoundaryForSteps(state.executionSteps);
  }

  function runtimeCurrentStage(): RuntimeStage | null {
    return runtimeStageAt(state.executionSteps);
  }

  function runtimePendingStage(): RuntimeStage | null {
    const stage = runtimeCurrentStage();
    if (!runtimeStageNeedsSolve(stage)) return null;
    return stage;
  }

  function runtimeStateEditStageForBoundary(
    boundary: number,
    opts: { includeSolved?: boolean } = {},
  ): RuntimeStage | null {
    const stage = runtimeCurrentStage();
    if (!stage || stage.editableMode !== "state") return null;
    const interactionBoundary = stage.interactionBoundary ?? stage.afterBoundary;
    if (interactionBoundary !== boundary) return null;
    if (!opts.includeSolved && runtimeStageSolved(stage)) return null;
    return stage;
  }

  function runtimeBoundaryEditStageForBoundary(
    boundary: number,
    opts: { includeSolved?: boolean } = {},
  ): RuntimeStage | null {
    const stage = runtimeCurrentStage();
    if (!stage || stage.editableMode !== "boundary") return null;
    if (runtimeStageSolved(stage)) {
      if (stage.afterBoundary !== boundary) return null;
    } else {
      const interactionBoundary = stage.interactionBoundary ?? stage.afterBoundary;
      if (interactionBoundary !== boundary) return null;
    }
    if (!opts.includeSolved && runtimeStageSolved(stage)) return null;
    return stage;
  }

  function runtimeEditableStorageKey(boundary: number): number | null {
    const stage = runtimeStateEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (!stage) return null;
    if (runtimeStageSolved(stage)) return null;
    return stage.index;
  }

  function clearBranchSelection() {
    branchSelectionActive = false;
    branchSelectionBoundary = null;
    branchSelectionTarget = null;
  }

  function runtimeCurrentTrace(): RuntimeTrace | null {
    return runtimeTraceForStage(state.executionSteps);
  }

  function runtimeIsComplete(): boolean {
    if (!runtimeStages.length) return true;
    if (state.executionSteps < runtimeMaxStep()) return false;
    const current = runtimeCurrentStage();
    return !!current && !runtimeStageNeedsSolve(current);
  }

  function runtimeStageLineRange(
    stage: RuntimeStage | null,
  ): { start: number; end: number } | null {
    if (!stage) return null;
    const maxLineIndex = Math.max(0, totalLines - 1);
    const start = Math.max(0, Math.min(maxLineIndex, stage.runLine));
    const end = Math.max(start, Math.min(maxLineIndex, stage.runEndLine));
    return { start, end };
  }

  function runtimeRunLabel(withArrow: boolean): string {
    const labelForStage = (stage: RuntimeStage | null) => {
      if (!stage) return endLabel;
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

  function runtimeStepBadge(): "" | "note" | "check" {
    const nextStage = runtimeStageAt(state.executionSteps + 1);
    return runtimeStageNeedsSolve(nextStage) ? "note" : "";
  }

  function runtimeMarkCurrentStageSolved(): boolean {
    const stage = runtimePendingStage();
    if (!stage) return false;
    runtimeLatestSolvedStage = Math.max(runtimeLatestSolvedStage, stage.index);
    clearBranchSelection();
    syncBoundaryFromStage();
    return true;
  }

  function runtimePendingEditableExpectedState(boundary: number): BoxState[] | null {
    const stage = runtimeStateEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (!stage) return null;
    if (!runtimeStageNeedsSolve(stage)) return null;
    if (!stage.expectedState) return null;
    return cloneBoxes(stage.expectedState);
  }

  function runtimePendingEditableBaselineState(boundary: number): BoxState[] | null {
    const stage = runtimeStateEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (!stage) return null;
    if (!runtimeStageNeedsSolve(stage)) return null;
    if (!stage.baselineState) return null;
    return cloneBoxes(stage.baselineState);
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

  function lineAfterWhileHeader(block: {
    headerEndLine: number;
    trueTarget: number;
  }): number | null {
    const trueLine = lineForPartIndex(block.trueTarget);
    if (Number.isFinite(trueLine)) return trueLine;
    if (Number.isFinite(block.headerEndLine)) {
      return clampBoundaryLine(block.headerEndLine + 1);
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

  function lineAfterWhileClose(block: {
    closeIndex: number;
    headerEndLine: number;
    afterIndex: number;
  }): number | null {
    const afterLine = lineForPartIndex(block.afterIndex);
    if (Number.isFinite(afterLine)) return afterLine;
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

  stepInfos.forEach((step) => {
    const text = lineList.slice(0, step.boundary).join("\n");
    const canRun = canRunWithStepBudget(text);
    const allowHeaderOnly =
      (step.ifHeaderOnly && !step.ifHeaderHasOpenBrace) ||
      !!step.whileHeaderOnly;
    if (
      !canRun &&
      !canRunWithAutoClosedBlocks(text) &&
      !allowHeaderOnly
    ) {
      failConfig(
        `Step ${step.index + 1} cannot be run as-is. Fix the code in this step.`,
      );
    }
  });

  function isHeaderOnlyStep(
    step: StepInfo | null | undefined,
  ): step is StepInfo & ({ ifHeaderOnly: true } | { whileHeaderOnly: true }) {
    return !!(step && (step.ifHeaderOnly || step.whileHeaderOnly));
  }

  function branchInfoForBoundary(boundary: number): {
    rangeStart: number;
    rangeEnd: number;
    targets: number[];
    targetMap: Map<number, number>;
    expected: number | null;
  } | null {
    const stage = runtimeBoundaryEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (!stage) return null;
    const targets: number[] = [];
    const targetMap = new Map<number, number>();
    stage.branchTargets.forEach((target) => {
      if (!Number.isFinite(target)) return;
      const normalized = Math.max(0, Math.min(totalLines, Number(target)));
      if (!targets.includes(normalized)) targets.push(normalized);
      targetMap.set(normalized, normalized);
    });
    const expected = Number.isFinite(stage.expectedBoundary)
      ? Math.max(0, Math.min(totalLines, Number(stage.expectedBoundary)))
      : null;
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

  function getExpectedState(boundary: number): BoxState[] {
    const pendingExpected = runtimePendingEditableExpectedState(boundary);
    if (pendingExpected) return pendingExpected;
    const currentStage = runtimeCurrentStage();
    if (currentStage) return cloneBoxes(currentStage.stateAfter || []);
    const trace = runtimeTraceByStep[0] || null;
    if (trace) return cloneBoxes(trace.state || []);
    return [];
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
    void boundary;
    return cloneBoxes(defaults || []);
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
    const stage = runtimeStateEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (!stage || runtimeStageSolved(stage)) {
      resetBtn.classList.add("hidden");
      return;
    }
    const baseline = stage.baselineState || null;
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
    const snapshot = serializeWorkspace(wrap) || [];
    snapshot.forEach((box) => {
      const raw = box.address ?? "";
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
    const pendingBaseline = runtimePendingEditableBaselineState(boundary);
    if (pendingBaseline) return decorateState(pendingBaseline);
    return decorateState(getExpectedState(boundary));
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
    void boundary;
    return runtimeRunLabel(true);
  }

  function formatNameList(names: string[]): string {
    const tokens = names.map((name) => `$n{${name}}`);
    if (tokens.length === 1) return tokens[0] || "";
    if (tokens.length === 2) return `${tokens[0]} and ${tokens[1]}`;
    return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
  }

  function baselineForBoundary(boundary: number): BoxState[] {
    const stage = runtimeStateEditStageForBoundary(boundary, {
      includeSolved: true,
    });
    if (stage?.baselineState) return cloneBoxes(stage.baselineState);
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

  function getHintParts(ctx: ProgramContext): ProgramParts | null {
    const stage = runtimeStateEditStageForBoundary(state.boundary);
    const hintSpec = stage?.hints ?? null;
    return resolveParts(hintSpec, ctx);
  }

  function renderCodePaneForBoundary() {
    if (!codeEl) return;
    const key = state.boundary;
    const currentStage = runtimeCurrentStage();
    const strikeBoundary =
      currentStage &&
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
    const runtimeStateEditable =
      !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
    const runtimeBranchEditable =
      !!runtimeBoundaryStage &&
      runtimeStageNeedsSolve(runtimeBoundaryStage) &&
      (runtimeBoundaryStage.interactionBoundary ?? runtimeBoundaryStage.afterBoundary) ===
        key;
    if (runtimeBranchEditable) {
      branchSelectionActive = true;
      branchSelectionBoundary = key;
    } else {
      clearBranchSelection();
    }
    let progress = runtimeStateEditable;
    let progressRange: [number, number] | undefined;
    let progressIndex: number | undefined;
    let doneBoundary: number | undefined;
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
    } else if (runtimeBranchEditable && branchInfo) {
      progress = true;
      progressRange = [branchInfo.rangeStart, branchInfo.rangeEnd];
      progressIndex = undefined;
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
      if (strikeBoundary <= block.headerEndLine) return;
      const pendingBoundaryStage = runtimePendingStage();
      if (
        pendingBoundaryStage &&
        pendingBoundaryStage.editableMode === "boundary" &&
        !runtimeStageSolved(pendingBoundaryStage) &&
        pendingBoundaryStage.runLine === block.headerStartLine
      ) {
        return;
      }
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
      suppressProgressMid: runtimeBranchEditable,
      boundaryTargets: branchSelectable,
      strikeRanges,
      strikeFragments,
    });
  }

  function ensureCodeLineVisible() {
    if (!codeEl) return;
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
    if (!Number.isFinite(range.start) || !Number.isFinite(range.end)) return;
    if (range.start < 0 || range.start >= lineList.length) return;
    if (range.end < 0 || range.end >= lineList.length) return;
    const lines = codeEl.querySelectorAll(".line");
    const startEl = lines[range.start] as HTMLElement | undefined;
    const endEl = lines[range.end] as HTMLElement | undefined;
    if (!startEl || !endEl) return;
    const container = codeEl as HTMLElement;
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
      container.scrollTop = Math.max(
        0,
        Math.ceil(lineBottom - container.clientHeight + bottomPadding),
      );
    }
  }

  function ensureNewVariableVisible(node: HTMLElement | null) {
    if (!node) return;
    requestAnimationFrame(() => {
      const stateContainer = stageEl as HTMLElement | null;
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

  function maybeScrollStateOnGrowth(nextCount: number) {
    const previousCount = state.lastRenderedStateCount;
    state.lastRenderedStateCount = nextCount;
    if (previousCount == null || nextCount <= previousCount) return;
    requestAnimationFrame(() => {
      const stateContainer = stageEl as HTMLElement | null;
      if (!stateContainer) return;
      stateContainer.scrollTo({
        top: stateContainer.scrollHeight,
        behavior: "smooth",
      });
    });
  }

  function renderStage() {
    if (!stageEl) return;
    stageEl.innerHTML = "";

    const key = state.boundary;
    const runtimeEditableStage = runtimeStateEditStageForBoundary(key, {
      includeSolved: true,
    });
    const editable =
      !!runtimeEditableStage && !runtimeStageSolved(runtimeEditableStage);
    const defaults = defaultsForBoundary(key);
    const traceCount = runtimeCurrentTrace()?.state?.length ?? 0;
    state.workspaceEl = null;

    if (key <= 0 && !editable) {
      maybeScrollStateOnGrowth(traceCount);
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

    const runtimeSnapshot =
      runtimeEditableStage != null
        ? (runtimeWorkspaceByStage.get(runtimeEditableStage.index) ?? null)
        : null;
    const wrap = restoreWorkspace(
      runtimeSnapshot,
      defaults,
      {
        editable,
        deletable: allowVariableDeletion,
        allowNameEdit: null,
        allowTypeEdit: null,
      },
    );
    stageEl.appendChild(wrap);
    state.workspaceEl = wrap;
    attachResetWatcher(wrap, key);
    ensureBaseline(key, defaults);
    maybeScrollStateOnGrowth(defaults.length);
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

  function scrollInstructionsUpIfNeeded(instructionKey: string | null) {
    if (!instructionsEl || !instructionKey || instructionKey === state.lastInstructionKey)
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
    const runLabel = runLabelForBoundary(state.boundary);
    const instructionStage = runtimeCurrentStage();
    let instructionKey: string | null = null;
    let parts: ProgramParts | null = null;
    if (!instructionStage && hasInitialInstructionsContent) {
      parts = initialInstructions || "";
      instructionKey = "__initial__";
    } else if (instructionStage?.instructions) {
      parts = String(instructionStage.instructions);
      instructionKey = `runtime-stage-${instructionStage.index}`;
    }
    parts = applyButtonTokens(parts || null, runLabel);
    setPartsContent(instructionsEl, parts);
    scrollInstructionsUpIfNeeded(instructionKey);
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
    return serializeWorkspace(ws) || [];
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
    const branchSolved =
      !!runtimeBoundaryStage && runtimeStageSolved(runtimeBoundaryStage);
    const branchSelectionHere =
      branchSelectionActive && branchSelectionBoundary === key;
    const hasSolvedEditable =
      (!!runtimeStateStage && runtimeStageSolved(runtimeStateStage)) ||
      (!!runtimeBoundaryStage && runtimeStageSolved(runtimeBoundaryStage));
    const normalEditable =
      !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
    const branchInfo = branchInfoForBoundary(key);
    const branchEditable =
      branchSelectionActive &&
      branchSelectionBoundary === key &&
      !!branchInfo &&
      !!runtimeBoundaryStage &&
      !runtimeStageSolved(runtimeBoundaryStage);
    if (statusEl) {
      if (hasSolvedEditable && !branchSelectionHere) {
        setStatus("correct", "ok");
      } else if (normalEditable) {
        setStatus("", "muted");
      } else if (runtimeBoundaryStage) {
        setStatus(branchSolved ? "correct" : "", branchSolved ? "ok" : "muted");
      } else {
        setStatus("", "muted");
      }
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
    ensureCodeLineVisible();
  }

  function save() {
    const key = state.boundary;
    const runtimeKey = runtimeEditableStorageKey(key);
    if (runtimeKey == null) return;
    const snapshot = serializeWorkspace(getWorkspaceEl());
    if (Array.isArray(snapshot)) {
      runtimeWorkspaceByStage.set(runtimeKey, snapshot);
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
      const runtimeKey = runtimeEditableStorageKey(key);
      if (runtimeKey == null) return;
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
      if (
        branchSelectionActive &&
        branchSelectionBoundary !== key
      ) {
        clearBranchSelection();
      }
      const branchInfo = branchInfoForBoundary(key);
      const runtimeBranchStage = runtimeBoundaryEditStageForBoundary(key, {
        includeSolved: true,
      });
      const runtimeBranchSolved =
        !!runtimeBranchStage && runtimeStageSolved(runtimeBranchStage);
      const runtimeBranchEditable =
        branchSelectionActive &&
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
      const runtimeStateEditable =
        !!runtimeStateStage && !runtimeStageSolved(runtimeStateStage);
      if (!runtimeStateEditable) return;
      const boxes = readWorkspaceBoxes();
      const result = evaluateWorkspace(boxes);
      const ok = result.ok;
      setStatus(ok ? "correct" : "incorrect", ok ? "ok" : "err");
      flashStatus(statusEl);
      if (!ok) return;
      if (!runtimeStateStage) return;
      runtimeWorkspaceByStage.delete(runtimeStateStage.index);
      runtimeMarkCurrentStageSolved();
      const ws = getWorkspaceEl();
      if (ws) {
        ws
          .querySelectorAll(".vbox, .arraybox")
          .forEach((v) => disableBoxEditing(v));
        removeBoxDeleteButtons(ws);
      }
      if (checkBtn) checkBtn.classList.add("hidden");
      if (hintBtn) hintBtn.classList.add("hidden");
      if (addBtn) addBtn.classList.add("hidden");
      if (resetBtn) resetBtn.classList.add("hidden");
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
    onBeforeChange: save,
    onAfterChange: render,
    isStepLocked: () => !!runtimePendingStage(),
    getStepBadge: () => runtimeStepBadge(),
    getNextLabel: () => {
      if (runtimeIsComplete()) return endLabel;
      const pending = runtimePendingStage();
      if (pending && pending.editableMode === "boundary") return "???";
      return runtimeRunLabel(false);
    },
    isAtEnd: () => runtimeIsComplete(),
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

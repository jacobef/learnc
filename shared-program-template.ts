import {
  applyTextTokenReplacements,
  applyOtherNames,
  appendStateObjects,
  bindBtnRefPulse,
  boxValueMatchesSpec,
  clearNode,
  cloneBoxes,
  isMobileViewport,
  ensurePanelizedMain,
  flashStatus,
  getNavLabelForHref,
  makeAnswerBox,
  normalizeZeroDisplay,
  queryRole,
  randAddr,
  renderCodePane,
  renderParts,
  restoreWorkspace,
  serializeWorkspace,
  setPartsContent,
  syncDocumentTitleFromNav,
  typeInfo,
} from "./shared-core.js";
import type { BoxState, Part, Parts } from "./shared-core.js";
import {
  clearLevelProgress,
  currentLevelId,
  maybeRestoreLevelProgress,
  writeLevelProgress,
} from "./shared-progress.js";
import { parseCValueLiteral, runCProgram } from "./shared-c-interpreter.js";

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

interface ProgramTemplateProgress {
  executionSteps: number;
  solvedStage: number;
  workspaceByStage: Array<{ stageIndex: number; boxes: BoxState[] | null }>;
  selectedBoundaryByStage: Array<{ stageIndex: number; boundary: number | null }>;
  otherNamesShown?: string[];
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
  levelResetBtn: HTMLButtonElement | null;
  addBtn: HTMLButtonElement | null;
  resetBtn: HTMLButtonElement | null;
  prevBtn: HTMLButtonElement | null;
  nextBtn: HTMLButtonElement | null;
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
};

type RuntimeStage = {
  index: number;
  runLine: number;
  runEndLine: number;
  beforeBoundary: number;
  afterBoundary: number;
  stateBefore: BoxState[];
  stateAfter: BoxState[];
  step: StepInfo | null;
  traceKind?: string;
  stepVisitIndex: number;
  editableMode: "none" | "state" | "boundary";
  instructions?: Part;
  hints?: ProgramHint;
  branchTargets: number[];
};

function collectProgramElements(root: ParentNode = document): ProgramElements {
  const role = <T extends Element>(name: string) => queryRole<T>(name, root);
  return {
    instructionsEl: role<HTMLElement>("program-instructions"),
    codeEl: role<HTMLElement>("program-code"),
    codeRoot: role<HTMLElement>("program-root"),
    stageEl: role<HTMLElement>("program-stage"),
    controlsActionsEl: role<HTMLElement>("program-controls"),
    mobileActionsEl: role<HTMLElement>("program-mobile-actions"),
    statusEl: role<HTMLElement>("program-status"),
    hintPanel: role<HTMLElement>("program-hint"),
    hintBtn: role<HTMLButtonElement>("program-hint-btn"),
    checkBtn: role<HTMLButtonElement>("program-check"),
    levelResetBtn: role<HTMLButtonElement>("program-reset-level"),
    addBtn: role<HTMLButtonElement>("program-add"),
    resetBtn: role<HTMLButtonElement>("program-reset"),
    prevBtn: queryRole<HTMLButtonElement>("program-prev", root),
    nextBtn: queryRole<HTMLButtonElement>("program-next", root),
  };
}

function ensureProgramLayout(): ProgramElements {
  const resolvedTitle = syncDocumentTitleFromNav();
  const existing = queryRole<HTMLElement>("program-code");
  if (existing) return collectProgramElements();

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
  const codeTitle = document.createElement("div");
  codeTitle.className = "panel-title code-title";
  codeTitle.textContent = "Code";
  const codeEl = document.createElement("div");
  codeEl.dataset.role = "program-code";
  codeEl.className = "codepane panel-body";
  codePanel.append(codeTitle, codeEl);

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
  statePanel.append(stateTitle, stageEl);

  const prevBtn = document.createElement("button");
  prevBtn.dataset.role = "program-prev";
  prevBtn.textContent = "Back ◀";
  const nextBtn = document.createElement("button");
  nextBtn.dataset.role = "program-next";
  nextBtn.textContent = "Run line 1 ▶";
  const levelResetBtn = document.createElement("button");
  levelResetBtn.dataset.role = "program-reset-level";
  levelResetBtn.textContent = "Reset level";
  const spacer = document.createElement("span");
  spacer.className = "controls-spacer";
  spacer.setAttribute("aria-hidden", "true");
  const resetBtn = document.createElement("button");
  resetBtn.dataset.role = "program-reset";
  resetBtn.className = "hidden reserved-button-slot";
  resetBtn.textContent = "Reset";
  const addBtn = document.createElement("button");
  addBtn.dataset.role = "program-add";
  addBtn.className = "hidden gap-wide";
  addBtn.textContent = "+ New variable";
  const hintBtn = document.createElement("button");
  hintBtn.dataset.role = "program-hint-btn";
  hintBtn.type = "button";
  hintBtn.className = "hint-link hidden";
  hintBtn.textContent = "Hint";
  const checkBtn = document.createElement("button");
  checkBtn.dataset.role = "program-check";
  checkBtn.className = "hidden";
  checkBtn.textContent = "Check";
  const statusEl = document.createElement("span");
  statusEl.dataset.role = "program-status";
  statusEl.className = "muted";
  controlsRow.append(prevBtn, nextBtn, spacer, levelResetBtn, resetBtn, addBtn, hintBtn, checkBtn, statusEl);

  const hintPanel = document.createElement("div");
  hintPanel.dataset.role = "program-hint";
  hintPanel.className = "hint-inline hidden";
  actionBar.insertBefore(hintPanel, instructionsEl);

  row.append(codePanel, statePanel);
  row.insertBefore(mobileActions, statePanel);
  return collectProgramElements();
}

function boxNamed(boxes: BoxState[], name: string): BoxState | undefined {
  return boxes.find((box) => box.name === name);
}

function boxesNamed(boxes: BoxState[], ...names: string[]): Array<BoxState | undefined> {
  return names.map((name) => boxNamed(boxes, name));
}

function isHeaderLine(line: string): boolean {
  return /^\s*(?:}\s*else\s+)?(?:if|while)\s*\(/.test(line || "");
}

function resolveIndexed<T>(spec: T | T[] | undefined, index: number): T | undefined {
  if (Array.isArray(spec)) return spec[index];
  return spec;
}

function editableForVisit(step: StepInfo | null, visitIndex: number): boolean {
  if (!step) return false;
  const editable = step.editable;
  if (Array.isArray(editable)) return editable[visitIndex] === true;
  return editable === true;
}

function stateMatches(actual: BoxState[], expected: BoxState[]): boolean {
  const actualByName = new Map(actual.filter((box) => !box.arrayRoot).map((box) => [box.name, box]));
  const expectedByName = new Map(expected.filter((box) => !box.arrayRoot).map((box) => [box.name, box]));
  if (actualByName.size !== expectedByName.size) return false;
  for (const [name, expectedBox] of expectedByName.entries()) {
    const actualBox = actualByName.get(name);
    if (!actualBox) return false;
    if ((actualBox.type || "").trim() !== (expectedBox.type || "").trim()) return false;
    if (!boxValueMatchesSpec(parseCValueLiteral, actualBox, expectedBox).ok) return false;
  }
  return true;
}

function formatNameList(names: string[]): string {
  const tokens = names.map((name) => `$n{${name}}`);
  if (tokens.length === 1) return tokens[0] || "";
  if (tokens.length === 2) return `${tokens[0]} and ${tokens[1]}`;
  return `${tokens.slice(0, -1).join(", ")}, and ${tokens[tokens.length - 1]}`;
}

function basicHintForBoxes(
  actual: BoxState[],
  expected: BoxState[],
  baseline: BoxState[],
  stage: RuntimeStage,
): ProgramContext["_basicHintTopic"] & { message: string } | null {
  const visibleActual = actual.filter((box) => !box.arrayRoot);
  const visibleExpected = expected.filter((box) => !box.arrayRoot);
  const visibleBaseline = baseline.filter((box) => !box.arrayRoot);
  const actualCount = visibleActual.length;
  const expectedCount = visibleExpected.length;
  const nameOf = (box: BoxState | null | undefined) =>
    String(box?.name || "").trim();
  const typeOf = (box: BoxState | null | undefined) =>
    String(box?.type || "").trim();
  const expectedNames = visibleExpected.map(nameOf).filter(Boolean);
  const expectedNameSet = new Set(expectedNames);
  const actualNames = visibleActual.map(nameOf);
  const actualNameSet = new Set(actualNames.filter(Boolean));
  const missingExpectedNames = expectedNames.filter(
    (name) => !actualNameSet.has(name),
  );
  const baselineNames = new Set(visibleBaseline.map(nameOf).filter(Boolean));

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
    if (!expectedName) {
      return { message: "You need to add a new variable.", kind: "count" };
    }
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
    const baselineCount = visibleBaseline.length;
    const expectedNew = Math.max(0, expectedCount - baselineCount);
    const actualNew = Math.max(0, actualCount - baselineCount);
    const extraCount = Math.max(0, actualNew - expectedNew);
    const start = Math.max(1, stage.runLine + 1);
    const end = Math.max(start, stage.runEndLine + 1);
    const label = start === end ? `Line ${start}` : `Lines ${start}-${end}`;
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
  visibleBaseline.forEach((box) => {
    const name = nameOf(box);
    if (name && !baselineByName.has(name)) baselineByName.set(name, box);
  });
  const actualByName = new Map<string, BoxState>();
  visibleActual.forEach((box) => {
    const name = nameOf(box);
    if (name && !actualByName.has(name)) actualByName.set(name, box);
  });

  let deferredBe: (ProgramContext["_basicHintTopic"] & { message: string }) | null = null;
  for (const expectedBox of visibleExpected) {
    const name = nameOf(expectedBox);
    if (!name) continue;
    const actualBox = actualByName.get(name);
    if (!actualBox) continue;
    const expectedType = typeOf(expectedBox);
    const actualType = typeOf(actualBox);
    if (actualType !== expectedType) {
      return {
        message: `$n{${name}}'s type should be $t{${expectedType}}.`,
        kind: "type",
        variable: name,
      };
    }
    const mismatch = !boxValueMatchesSpec(parseCValueLiteral, actualBox, expectedBox).ok;
    if (!mismatch) continue;
    const expectedValue = (expectedBox.value ?? "").trim();
    const label =
      expectedValue === "" ? "empty" : `$v{${normalizeZeroDisplay(expectedValue)}}`;
    const baselineBox = baselineByName.get(name);
    const shouldRemain = baselineBox
      ? boxValueMatchesSpec(parseCValueLiteral, baselineBox, expectedBox).ok
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

  return deferredBe;
}

function formatRunLabel(
  stage: RuntimeStage | null,
  totalLines: number,
  endLabel: string,
  {
    withArrow = true,
    badge = "",
  }: { withArrow?: boolean; badge?: "" | "note" | "check" } = {},
): string {
  if (!stage) return endLabel;
  const start = Math.max(1, Math.min(totalLines, stage.runLine + 1));
  const end = Math.max(start, Math.min(totalLines, stage.runEndLine + 1));
  const headerOnly =
    stage.traceKind !== "block-close" &&
    stage.step?.lines.length === 1 &&
    isHeaderLine(stage.step.lines[0] || "");
  const verb =
    stage.editableMode === "boundary" || (stage.editableMode === "none" && headerOnly)
      ? "Branch from"
      : stage.editableMode === "state" && badge === "note"
        ? "Solve"
        : "Run";
  const suffix = withArrow ? " ▶" : "";
  const prefix = badge === "note" ? "🔧 " : badge === "check" ? "✅ " : "";
  return start === end
    ? `${prefix}${verb} line ${start}${suffix}`
    : `${prefix}${verb} lines ${start}-${end}${suffix}`;
}

function nextWorkspaceAddress(boxes: BoxState[], type: string): string {
  const { size, align } = typeInfo(type || "int");
  let maxAddr = 0;
  for (const box of boxes) {
    const addr = Number(box.address);
    if (Number.isFinite(addr)) maxAddr = Math.max(maxAddr, addr);
  }
  if (!maxAddr) return String(randAddr(type || "int"));
  let next = maxAddr + (size || 4);
  if (align > 1 && next % align !== 0) next = Math.ceil(next / align) * align;
  return String(next);
}

function createProgramTemplate(config: ProgramTemplateConfig): void {
  const {
    steps,
    initialInstructions = "",
    next = null,
    workspace = {},
    isLast = false,
  } = config;
  if (!Array.isArray(steps) || !steps.length) {
    throw new Error("Program steps must be a non-empty array.");
  }

  const {
    instructionsEl,
    codeEl,
    codeRoot,
    stageEl,
    statusEl,
    controlsActionsEl,
    mobileActionsEl,
    hintPanel,
    hintBtn,
    checkBtn,
    levelResetBtn,
    addBtn,
    resetBtn,
    prevBtn,
    nextBtn,
  } = ensureProgramLayout();

  function placeActionButtonsForViewport() {
    const mobileMode = isMobileViewport() && !!mobileActionsEl;
    const target = mobileMode ? mobileActionsEl : controlsActionsEl;
    if (!target) return;
    [levelResetBtn, resetBtn, addBtn, hintBtn, checkBtn, statusEl].forEach((node) => {
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
    const hasVisibleAction = [levelResetBtn, checkBtn, hintBtn, addBtn, resetBtn].some(
      (btn) => !!btn && !btn.classList.contains("hidden"),
    );
    mobileActionsEl.classList.toggle("hidden", !hasVisibleAction);
  }

  placeActionButtonsForViewport();
  updateMobileActionsVisibility();
  window.addEventListener("resize", placeActionButtonsForViewport);
  bindBtnRefPulse(codeRoot || document);

  const endLabel = (() => {
    if (isLast) return "Finish";
    const label = getNavLabelForHref(next);
    return label ? `Next: ${label}` : "Next Program";
  })();

  const stepInfos: StepInfo[] = [];
  const lineList: string[] = [];
  steps.forEach((step, index) => {
    const normalizedCode = step.code.endsWith("\n") ? step.code : `${step.code}\n`;
    const rawLines = normalizedCode.split(/\r?\n/);
    if (rawLines[rawLines.length - 1] === "") rawLines.pop();
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
      editable: step.editable ?? false,
    });
  });
  const totalLines = lineList.length;
  const fullCode = steps.map((step) => step.code).join("");
  const run = runCProgram(fullCode);
  if (run.kind !== "ok") {
    throw new Error(run.diagnostic.message || "Program template code does not compile.");
  }

  const stepForLine = (line: number) =>
    stepInfos.find((step) => line >= step.startLine && line <= step.endLine) || null;
  const visitCounts = new Map<number, number>();
  const runtimeStages: RuntimeStage[] = [];
  let previousBoundary = 0;
  let previousState: BoxState[] = [];
  const trace = run.trace || [];

  function matchingCloseBoundaryForHeader(headerLine: number): number | null {
    const header = lineList[headerLine] || "";
    if (!isHeaderLine(header)) return null;
    let depth = 0;
    let sawOpen = false;
    for (let line = headerLine; line < totalLines; line += 1) {
      const text = lineList[line] || "";
      for (const ch of text) {
        if (ch === "{") {
          depth += 1;
          sawOpen = true;
        } else if (ch === "}" && sawOpen) {
          depth -= 1;
          if (depth <= 0) return line + 1;
        }
      }
    }
    return null;
  }

  function noEventStateAfter(step: StepInfo, stateAfterGap: BoxState[]): BoxState[] {
    if (!step.lines.some((line) => line.includes("}"))) {
      return cloneBoxes(previousState);
    }
    const liveKeys = new Set(stateAfterGap.map(stateBoxKey));
    return previousState.filter((box) => liveKeys.has(stateBoxKey(box)));
  }

  function isElseLine(step: StepInfo): boolean {
    return step.lines.some((line) => /\belse\b/.test(line));
  }

  function pushNoEventStages(
    untilLine: number,
    stateAfterGap: BoxState[],
    finalAfterBoundary: number | null = null,
  ) {
    const gapSteps = stepInfos.filter(
      (step) =>
        step.startLine >= previousBoundary &&
        step.endLine < untilLine &&
        step.startLine < totalLines,
    );
    for (const step of gapSteps) {
      const visitIndex = visitCounts.get(step.index) ?? 0;
      const editable = editableForVisit(step, visitIndex);
      const headerEditable = step.lines.length === 1 && isHeaderLine(step.lines[0] || "");
      const stateAfterStep = noEventStateAfter(step, stateAfterGap);
      runtimeStages.push({
        index: runtimeStages.length,
        runLine: step.startLine,
        runEndLine: step.endLine,
        beforeBoundary: previousBoundary,
        afterBoundary: step.boundary,
        stateBefore: cloneBoxes(previousState),
        stateAfter: cloneBoxes(stateAfterStep),
        step,
        traceKind: "synthetic",
        stepVisitIndex: visitIndex,
        editableMode: editable ? (headerEditable ? "boundary" : "state") : "none",
        instructions: resolveIndexed(step.instructions, visitIndex),
        hints: resolveIndexed(step.hints, visitIndex),
        branchTargets: editable && headerEditable
          ? Array.from({ length: totalLines + 1 }, (_, index) => index)
          : [],
      });
      visitCounts.set(step.index, visitIndex + 1);
      const isLastGapStep = step === gapSteps[gapSteps.length - 1];
      const afterBoundary =
        finalAfterBoundary != null && isLastGapStep
          ? finalAfterBoundary
          : isElseLine(step)
            ? untilLine
            : step.boundary;
      runtimeStages[runtimeStages.length - 1]!.afterBoundary = afterBoundary;
      previousBoundary = afterBoundary;
      previousState = cloneBoxes(stateAfterStep);
      if (isElseLine(step)) break;
    }
  }

  function firstNoEventStep(fromLine: number, untilLine: number): StepInfo | null {
    const searchEnd = untilLine > fromLine ? untilLine : totalLines + 1;
    return stepInfos.find((step) => {
      if (step.startLine < fromLine || step.endLine >= searchEnd) return false;
      return true;
    }) || null;
  }

  function stateBoxKey(box: BoxState): string {
    const address = String(box.address ?? "").trim();
    if (address) return `addr:${address}`;
    return `name:${box.name ?? ""}:${box.type ?? ""}:${box.arrayRoot ?? ""}:${(box.arrayIndices ?? []).join(",")}`;
  }

  for (let i = 0; i < trace.length; i += 1) {
    const event = trace[i]!;
    if (event.startLine < previousBoundary) {
      const loopClose = matchingCloseBoundaryForHeader(event.startLine);
      pushNoEventStages(loopClose ?? previousBoundary, event.state, event.startLine);
    } else {
      pushNoEventStages(event.startLine, event.state);
    }
    const step = stepForLine(event.startLine);
    if (!step) continue;
    const visitIndex = visitCounts.get(step.index) ?? 0;
    const editable = editableForVisit(step, visitIndex);
    const headerOnly =
      event.kind !== "block-close" &&
      step.lines.length === 1 &&
      isHeaderLine(step.lines[0] || "");
    const headerEditable = editable && headerOnly;
    let lastEvent = event;
    let j = i;
    if (!headerOnly) {
      while (j + 1 < trace.length) {
        const nextEvent = trace[j + 1]!;
        if (nextEvent.startLine < step.startLine || nextEvent.startLine > step.endLine) break;
        lastEvent = nextEvent;
        j += 1;
      }
    }
    const nextEvent = trace[j + 1] || null;
    const afterExecutedLine = headerOnly ? lastEvent.endLine + 1 : step.boundary;
    const nextTraceLine = nextEvent?.startLine ?? totalLines;
    const headerCloseBoundary = headerOnly
      ? matchingCloseBoundaryForHeader(step.startLine)
      : null;
    const headerAfterBoundary =
      headerOnly &&
      headerCloseBoundary != null &&
      headerCloseBoundary < nextTraceLine &&
      nextTraceLine > afterExecutedLine
        ? headerCloseBoundary
        : nextTraceLine;
    const pendingNoEventStep = headerOnly
      ? null
      : firstNoEventStep(afterExecutedLine, nextTraceLine);
    const skipElseTail =
      !headerOnly && isElseLine(step) && nextTraceLine > afterExecutedLine;
    const loopBackAfterClose =
      !headerOnly &&
      step.lines.some((line) => line.includes("}")) &&
      !!nextEvent &&
      nextTraceLine < afterExecutedLine;
    const naturalAfter = headerOnly
      ? headerAfterBoundary
      : skipElseTail || loopBackAfterClose
        ? nextTraceLine
      : pendingNoEventStep
        ? afterExecutedLine
        : Math.max(afterExecutedLine, nextTraceLine);
    const afterBoundary = Math.max(0, Math.min(totalLines, naturalAfter));
    const stateAfterStage = cloneBoxes(lastEvent.state);
    const branchTargets = headerEditable
      ? Array.from({ length: totalLines + 1 }, (_, index) => index)
      : [];
    runtimeStages.push({
      index: runtimeStages.length,
      runLine: step.startLine,
      runEndLine: !headerOnly ? step.endLine : event.endLine,
      beforeBoundary: previousBoundary,
      afterBoundary,
      stateBefore: cloneBoxes(previousState),
      stateAfter: cloneBoxes(stateAfterStage),
      step,
      traceKind: event.kind,
      stepVisitIndex: visitIndex,
      editableMode: editable ? (headerEditable ? "boundary" : "state") : "none",
      instructions: resolveIndexed(step.instructions, visitIndex),
      hints: resolveIndexed(step.hints, visitIndex),
      branchTargets,
    });
    visitCounts.set(step.index, visitIndex + 1);
    previousBoundary = afterBoundary;
    previousState = cloneBoxes(stateAfterStage);
    i = j;
  }
  pushNoEventStages(totalLines + 1, run.state || previousState);

  const levelId = currentLevelId();
  const restored = maybeRestoreLevelProgress<ProgramTemplateProgress>(levelId);
  let executionSteps = Math.max(-1, Math.min(runtimeStages.length - 1, restored?.executionSteps ?? -1));
  let solvedStage = Math.max(-1, restored?.solvedStage ?? -1);
  const workspaceByStage = new Map<number, BoxState[] | null>(
    (restored?.workspaceByStage || []).map((entry) => [entry.stageIndex, entry.boxes ? cloneBoxes(entry.boxes) : null]),
  );
  const selectedBoundaryByStage = new Map<number, number | null>(
    (restored?.selectedBoundaryByStage || []).map((entry) => [entry.stageIndex, entry.boundary]),
  );
  const otherNamesShown = new Set(
    Array.isArray(restored?.otherNamesShown)
      ? restored.otherNamesShown.filter((addr): addr is string => typeof addr === "string")
      : [],
  );

  const currentStage = () => (executionSteps >= 0 ? runtimeStages[executionSteps] || null : null);
  const stageNeedsSolve = (stage: RuntimeStage | null) =>
    !!stage && stage.editableMode !== "none" && stage.index > solvedStage;
  const stageBadge = (stage: RuntimeStage | null): "" | "note" | "check" => {
    if (stageNeedsSolve(stage)) return "note";
    return "";
  };
  const currentBoundary = () => {
    const stage = currentStage();
    if (!stage) return 0;
    if (stageNeedsSolve(stage) && stage.editableMode === "boundary") {
      return stage.step?.boundary ?? stage.afterBoundary;
    }
    return stage.afterBoundary;
  };
  const nextLabelStage = () => {
    return runtimeStages[executionSteps + 1] || null;
  };
  const nextProgramLabel = () => `${endLabel} ▶▶`;
  const nextButtonLabel = () => {
    const stage = currentStage();
    if (stageNeedsSolve(stage) && stage?.editableMode === "boundary") {
      return "🔧 ??? ▶";
    }
    if (executionSteps >= runtimeStages.length - 1) {
      return nextProgramLabel();
    }
    const labelStage = nextLabelStage();
    return formatRunLabel(labelStage, totalLines, endLabel, {
      badge: stageBadge(labelStage),
    });
  };

  function progressSnapshot(): ProgramTemplateProgress {
    return {
      executionSteps,
      solvedStage,
      workspaceByStage: Array.from(workspaceByStage.entries()).map(([stageIndex, boxes]) => ({
        stageIndex,
        boxes: boxes ? cloneBoxes(boxes) : null,
      })),
      selectedBoundaryByStage: Array.from(selectedBoundaryByStage.entries()).map(([stageIndex, boundary]) => ({
        stageIndex,
        boundary,
      })),
      otherNamesShown: Array.from(otherNamesShown),
    };
  }

  function isDefaultProgress(snapshot: ProgramTemplateProgress): boolean {
    return (
      snapshot.executionSteps === -1 &&
      snapshot.solvedStage === -1 &&
      snapshot.workspaceByStage.length === 0 &&
      snapshot.selectedBoundaryByStage.length === 0 &&
      (snapshot.otherNamesShown?.length ?? 0) === 0
    );
  }

  function persistProgress() {
    const snapshot = progressSnapshot();
    if (isDefaultProgress(snapshot)) {
      clearLevelProgress(levelId);
    } else {
      writeLevelProgress(snapshot, levelId);
    }
  }

  function withSidebarParam(url: string | null): string | null {
    if (!url) return url;
    const [base, hash = ""] = url.split("#");
    const [path, query = ""] = base.split("?");
    const params = new URLSearchParams(query);
    params.set(
      "sidebar",
      document.body.classList.contains("sidebar-collapsed") ? "0" : "1",
    );
    const nextQuery = params.toString();
    return `${path}${nextQuery ? `?${nextQuery}` : ""}${hash ? `#${hash}` : ""}`;
  }

  function setStatus(text: string, cls = "muted") {
    if (!statusEl) return;
    statusEl.textContent = text;
    statusEl.className = cls;
  }

  function clearNextPulse() {
    nextBtn?.classList.remove("pulse-success");
  }

  function pulseNext() {
    nextBtn?.classList.add("pulse-success");
  }

  const buttonReplacements = () =>
    [
      ["$runLineButton", `$b{${nextButtonLabel()}}`],
      ["$backButton", "$b{Back ◀}"],
      ["$checkButton", "$b{Check}"],
      ["$resetButton", "$b{Reset}"],
      ["$newVariableButton", "$b{+ New variable}"],
      ["$showAliasesButton", "$b{Show aliases}"],
    ] as const;

  function applyButtonTokens(parts: ProgramParts | null): ProgramParts | null {
    return applyTextTokenReplacements(parts, buttonReplacements()) as ProgramParts | null;
  }

  function renderInstructions() {
    const stage = currentStage();
    const text =
      stage?.instructions ??
      (executionSteps < 0 ? initialInstructions : null);
    setPartsContent(instructionsEl, applyButtonTokens(text as ProgramParts | null));
  }

  function refreshOtherNames() {
    applyOtherNames(stageEl, {
      shownAddrs: otherNamesShown,
      onToggle: () => {
        persistProgress();
        refreshOtherNames();
      },
    });
  }

  function renderWorkspace(stage: RuntimeStage | null) {
    if (!stageEl) return;
    clearNode(stageEl);
    const editable = !!stage && stageNeedsSolve(stage) && stage.editableMode === "state";
    if (editable && stage) {
      const snapshot = workspaceByStage.get(stage.index) ?? null;
      const wrap = restoreWorkspace(snapshot, stage.stateBefore, {
        editable: true,
        deletable: !!workspace.allowVariableDeletion,
        allowNameEdit: null,
        allowTypeEdit: null,
      });
      stageEl.appendChild(wrap);
      wrap.addEventListener("input", () => {
        workspaceByStage.set(stage.index, serializeWorkspace(wrap) || []);
        updateResetVisibility();
        persistProgress();
      });
      wrap.addEventListener("click", () => {
        window.setTimeout(() => {
          workspaceByStage.set(stage.index, serializeWorkspace(wrap) || []);
          updateResetVisibility();
          persistProgress();
        }, 0);
      });
      if (workspace.showOtherNames) {
        refreshOtherNames();
      }
      return;
    }

    const wrap = document.createElement("div");
    wrap.dataset.role = "workspace";
    wrap.className = "grid";
    const boxes = cloneBoxes(stage?.stateAfter || []);
    if (!boxes.length) {
      const msg = document.createElement("div");
      msg.className = "muted";
      msg.style.padding = "8px";
      msg.textContent = "(no variables yet)";
      wrap.appendChild(msg);
    } else {
      appendStateObjects(wrap, boxes, {
        editable,
        deletable: editable && !!workspace.allowVariableDeletion,
      });
    }
    stageEl.appendChild(wrap);
    if (workspace.showOtherNames) {
      refreshOtherNames();
    }
  }

  function getWorkspaceEl(): HTMLElement | null {
    return stageEl?.querySelector<HTMLElement>('[data-role="workspace"]') || null;
  }

  function readWorkspace(): BoxState[] {
    return serializeWorkspace(getWorkspaceEl()) || [];
  }

  function updateResetVisibility() {
    const stage = currentStage();
    if (!resetBtn) return;
    if (!stage || !stageNeedsSolve(stage) || stage.editableMode !== "state") {
      resetBtn.classList.add("hidden");
      return;
    }
    const current = readWorkspace();
    const baseline = stage.stateBefore;
    resetBtn.classList.toggle("hidden", stateMatches(current, baseline));
  }

  function renderCode() {
    if (!codeEl) return;
    const stage = currentStage();
    const solving = stageNeedsSolve(stage);
    const rawBoundary = currentBoundary();
    const headerOnly =
      stage?.traceKind !== "block-close" &&
      stage?.step?.lines.length === 1 &&
      isHeaderLine(stage.step.lines[0] || "");
    const displayBoundary =
      stage && !headerOnly && stage.step && !isElseLine(stage.step)
        ? Math.min(rawBoundary, stage.step.boundary)
        : rawBoundary;
    const selectable =
      stage && solving && stage.editableMode === "boundary"
        ? stage.branchTargets
        : [];
    renderCodePane(codeEl, lineList, displayBoundary, {
      progress: solving,
      progressRange: solving && stage ? { start: stage.runLine, end: stage.runEndLine } : undefined,
      progressIndex: solving && stage?.editableMode === "state" ? stage.runEndLine : undefined,
      doneBoundary: solving && stage
        ? stage.editableMode === "boundary"
          ? rawBoundary
          : stage.runLine
        : displayBoundary,
      suppressProgressMid: solving && stage?.editableMode === "boundary",
      hideBoundary: selectable.length > 0,
      boundaryTargets: selectable.length > 0,
      selectableBoundaries: selectable,
      selectedBoundary: stage ? selectedBoundaryByStage.get(stage.index) ?? null : null,
    });
    codeEl.querySelectorAll<HTMLElement>(".boundary.selectable").forEach((node) => {
      node.addEventListener("click", () => {
        const boundary = Number(node.dataset.boundary);
        if (!Number.isFinite(boundary) || !stage) return;
        selectedBoundaryByStage.set(stage.index, boundary);
        setStatus("", "muted");
        hideHint();
        render();
      });
    });
  }

  function hideHint() {
    if (!hintPanel) return;
    hintPanel.classList.add("hidden");
  }

  function showHint(parts: ProgramParts | null) {
    if (!hintPanel) return;
    if (!parts || (Array.isArray(parts) && parts.length === 0)) return;
    renderParts(hintPanel, applyButtonTokens(parts) || "");
    hintPanel.classList.remove("hidden");
    flashStatus(hintPanel);
  }

  function hintContext(stage: RuntimeStage): ProgramContext {
    const boxes = stage.editableMode === "state" && stageNeedsSolve(stage)
      ? readWorkspace()
      : cloneBoxes(stage.stateAfter);
    const basic = basicHintForBoxes(boxes, stage.stateAfter, stage.stateBefore, stage);
    return {
      boxes,
      basicHint: basic?.message ?? null,
      _basicHintTopic: basic ? { kind: basic.kind, variable: basic.variable } : null,
      basicHintTopicIs(kind, variable) {
        const topic = this._basicHintTopic;
        if (!topic || topic.kind !== kind) return false;
        return variable == null || topic.variable === variable;
      },
      boxNamed: (name) => boxNamed(boxes, name),
      boxesNamed: (...names) => boxesNamed(boxes, ...names),
    };
  }

  function currentHint(): ProgramParts | null {
    const stage = currentStage();
    if (!stage) return null;
    const custom = stage.hints;
    if (typeof custom === "function") {
      const resolved = custom(hintContext(stage));
      if (resolved) return resolved as ProgramParts;
    } else if (custom) {
      return custom as ProgramParts;
    }
    const ctx = hintContext(stage);
    return ctx.basicHint;
  }

  function checkCurrentStage(): boolean {
    const stage = currentStage();
    if (!stage || !stageNeedsSolve(stage)) return true;
    if (stage.editableMode === "boundary") {
      const selected = selectedBoundaryByStage.get(stage.index);
      if (selected == null) {
        setStatus("Select a line boundary.", "err");
        flashStatus(statusEl);
        return false;
      }
      if (selected !== stage.afterBoundary) {
        setStatus("incorrect", "err");
        flashStatus(statusEl);
        return false;
      }
    } else {
      const actual = readWorkspace();
      workspaceByStage.set(stage.index, cloneBoxes(actual));
      if (!stateMatches(actual, stage.stateAfter)) {
        setStatus("incorrect", "err");
        flashStatus(statusEl);
        return false;
      }
    }
    solvedStage = Math.max(solvedStage, stage.index);
    setStatus("correct", "ok");
    flashStatus(statusEl);
    hideHint();
    workspaceByStage.delete(stage.index);
    persistProgress();
    render();
    pulseNext();
    return true;
  }

  function renderControls() {
    const stage = currentStage();
    const locked = stageNeedsSolve(stage);
    if (prevBtn) prevBtn.disabled = executionSteps < 0;
    if (nextBtn) {
      const complete = executionSteps >= runtimeStages.length - 1 && !locked;
      nextBtn.disabled = locked;
      const label = complete ? nextProgramLabel() : nextButtonLabel();
      nextBtn.textContent = locked ? `${label} 🔒` : label;
    }
    if (checkBtn) checkBtn.classList.toggle("hidden", !locked);
    if (hintBtn) hintBtn.classList.toggle("hidden", !(locked && stage?.editableMode === "state"));
    if (addBtn) {
      addBtn.classList.toggle(
        "hidden",
        !(locked && stage?.editableMode === "state" && workspace.allowVariableCreation),
      );
    }
    if (levelResetBtn) {
      levelResetBtn.disabled = isDefaultProgress(progressSnapshot());
    }
    updateResetVisibility();
    updateMobileActionsVisibility();
  }

  function render() {
    renderInstructions();
    renderCode();
    renderWorkspace(currentStage());
    renderControls();
    persistProgress();
  }

  function visibleObjectKeys(boxes: BoxState[]): Set<string> {
    return new Set(
      boxes
        .filter((box) => !box.arrayRoot)
        .map((box) => stateBoxKey(box)),
    );
  }

  function introducesVisibleObject(
    before: BoxState[],
    after: BoxState[],
  ): boolean {
    const beforeKeys = visibleObjectKeys(before);
    return [...visibleObjectKeys(after)].some((key) => !beforeKeys.has(key));
  }

  function changedValueBoxes(
    before: BoxState[],
    after: BoxState[],
  ): BoxState[] {
    const beforeByKey = new Map(
      before.map((box) => [stateBoxKey(box), box] as const),
    );
    return after.filter((box) => {
      const previous = beforeByKey.get(stateBoxKey(box));
      return previous != null && String(previous.value ?? "") !== String(box.value ?? "");
    });
  }

  function scrollProgramStateToBottom() {
    window.requestAnimationFrame(() => {
      stageEl?.scrollTo({
        top: stageEl.scrollHeight,
        behavior: "smooth",
      });
    });
  }

  function renderedNodeForBox(box: BoxState): HTMLElement | null {
    if (!stageEl) return null;
    const address = String(box.address ?? "").trim();
    if (box.arrayRoot) {
      const values = stageEl.querySelectorAll<HTMLElement>(".array-col-value");
      for (const value of values) {
        if (
          (address && value.dataset.arrayAddress === address) ||
          (!address && value.dataset.arrayName === box.name)
        ) {
          return value;
        }
      }
      return null;
    }

    const boxes = stageEl.querySelectorAll<HTMLElement>(".vbox");
    for (const node of boxes) {
      const nodeAddress =
        node.querySelector(".address")?.textContent?.trim() ?? "";
      const nodeName =
        node.querySelector(".name-text")?.textContent?.trim() ?? "";
      if (
        (address && nodeAddress === address) ||
        (!address && nodeName === box.name)
      ) {
        return node;
      }
    }
    return null;
  }

  function minimallyRevealChangedValue(boxes: BoxState[]) {
    if (!stageEl || !boxes.length) return;
    window.requestAnimationFrame(() => {
      if (!stageEl) return;
      const hostRect = stageEl.getBoundingClientRect();
      const padding = 8;
      const visibleTop = hostRect.top + padding;
      const visibleBottom = hostRect.bottom - padding;
      let target: HTMLElement | null = null;

      for (const box of boxes) {
        const node = renderedNodeForBox(box);
        if (!node) continue;
        const rect = node.getBoundingClientRect();
        if (rect.top < visibleTop || rect.bottom > visibleBottom) {
          target = node;
          break;
        }
      }
      if (!target) return;

      const targetRect = target.getBoundingClientRect();
      let nextScrollTop = stageEl.scrollTop;
      if (targetRect.top < visibleTop) {
        nextScrollTop -= visibleTop - targetRect.top;
      } else if (targetRect.bottom > visibleBottom) {
        nextScrollTop += targetRect.bottom - visibleBottom;
      }
      const maxScroll = Math.max(0, stageEl.scrollHeight - stageEl.clientHeight);
      stageEl.scrollTo({
        top: Math.max(0, Math.min(maxScroll, nextScrollTop)),
        behavior: "smooth",
      });
    });
  }

  function scrollCodeStatementIntoView(stage: RuntimeStage | null) {
    if (!codeEl || !stage) return;
    window.requestAnimationFrame(() => {
      const lines = codeEl.querySelectorAll<HTMLElement>(".line");
      const firstLine = lines[stage.runLine] ?? null;
      const lastLine = lines[stage.runEndLine] ?? firstLine;
      if (!firstLine || !lastLine) return;

      const hostRect = codeEl.getBoundingClientRect();
      const firstRect = firstLine.getBoundingClientRect();
      const lastRect = lastLine.getBoundingClientRect();
      const padding = 8;
      const statementTop =
        codeEl.scrollTop + firstRect.top - hostRect.top;
      const statementBottom =
        codeEl.scrollTop + lastRect.bottom - hostRect.top;
      const statementHeight = statementBottom - statementTop;
      const availableHeight = Math.max(0, codeEl.clientHeight - padding * 2);
      let target = codeEl.scrollTop;

      if (statementHeight > availableHeight) {
        target = statementBottom - codeEl.clientHeight + padding;
      } else {
        const minimum = statementBottom - codeEl.clientHeight + padding;
        const maximum = statementTop - padding;
        target = Math.max(minimum, Math.min(maximum, target));
      }

      const maxScroll = Math.max(0, codeEl.scrollHeight - codeEl.clientHeight);
      codeEl.scrollTo({
        top: Math.max(0, Math.min(maxScroll, target)),
        behavior: "smooth",
      });
    });
  }

  prevBtn?.addEventListener("click", () => {
    if (executionSteps < 0) return;
    clearNextPulse();
    executionSteps -= 1;
    setStatus("", "muted");
    hideHint();
    render();
    scrollCodeStatementIntoView(currentStage());
  });

  nextBtn?.addEventListener("click", () => {
    const stage = currentStage();
    if (stageNeedsSolve(stage)) return;
    clearNextPulse();
    if (executionSteps >= runtimeStages.length - 1) {
      const nextUrl = withSidebarParam(next);
      if (nextUrl) window.location.href = nextUrl;
      return;
    }
    executionSteps += 1;
    const nextStage = currentStage();
    const nextDisplayedState =
      nextStage?.editableMode === "state" && stageNeedsSolve(nextStage)
        ? nextStage.stateBefore
        : nextStage?.stateAfter || [];
    const shouldScrollState = introducesVisibleObject(
      stage?.stateAfter || [],
      nextDisplayedState,
    );
    const changedBoxes = changedValueBoxes(
      stage?.stateAfter || [],
      nextDisplayedState,
    );
    if (nextStage?.editableMode === "state" && !workspaceByStage.has(nextStage.index)) {
      workspaceByStage.set(nextStage.index, cloneBoxes(nextStage.stateBefore));
    }
    setStatus("", "muted");
    hideHint();
    render();
    scrollCodeStatementIntoView(nextStage);
    if (shouldScrollState) scrollProgramStateToBottom();
    else minimallyRevealChangedValue(changedBoxes);
  });

  checkBtn?.addEventListener("click", () => {
    checkCurrentStage();
  });

  hintBtn?.addEventListener("click", () => {
    const stage = currentStage();
    if (stage?.editableMode === "state" && stageNeedsSolve(stage)) {
      const boxes = readWorkspace();
      if (stateMatches(boxes, stage.stateAfter)) {
        showHint("Looks good. Press $checkButton.");
        return;
      }
      const parts = currentHint();
      if (!parts || (Array.isArray(parts) && parts.length === 0)) {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.",
        );
        return;
      }
      if (typeof parts === "string" && parts.trim() === "Looks good. Press $checkButton.") {
        showHint(
          "Your program has a problem that isn't covered by a hint, sorry. You can click $resetButton to undo all of your changes for this step.",
        );
        return;
      }
      showHint(parts);
      return;
    }
    showHint(currentHint());
  });

  resetBtn?.addEventListener("click", () => {
    const stage = currentStage();
    if (!stage || stage.editableMode !== "state") return;
    workspaceByStage.set(stage.index, cloneBoxes(stage.stateBefore));
    setStatus("", "muted");
    hideHint();
    render();
  });

  addBtn?.addEventListener("click", () => {
    const stage = currentStage();
    if (!stage || stage.editableMode !== "state") return;
    const ws = getWorkspaceEl();
    if (!ws) return;
    const node = makeAnswerBox({
      address: nextWorkspaceAddress(readWorkspace(), "int"),
      allowNameEdit: true,
      deletable: true,
    });
    node.dataset.allowDelete = "true";
    ws.appendChild(node);
    scrollProgramStateToBottom();
    workspaceByStage.set(stage.index, serializeWorkspace(ws) || []);
    updateResetVisibility();
    persistProgress();
  });

  levelResetBtn?.addEventListener("click", () => {
    const confirmed = window.confirm(
      "Reset your saved progress for this level and start over?",
    );
    if (!confirmed) return;
    executionSteps = -1;
    solvedStage = -1;
    workspaceByStage.clear();
    selectedBoundaryByStage.clear();
    otherNamesShown.clear();
    setStatus("", "muted");
    hideHint();
    clearLevelProgress(levelId);
    render();
  });

  window.addEventListener("keydown", (event) => {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
    if (event.key === "ArrowRight") nextBtn?.click();
    if (event.key === "ArrowLeft") prevBtn?.click();
  });

  render();
}

export { createProgramTemplate };
